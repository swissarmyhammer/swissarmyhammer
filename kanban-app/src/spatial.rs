//! Tauri commands for spatial focus management.
//!
//! These are transient UI plumbing commands — they manage the spatial entry
//! registry and focused key state. They do NOT flow through `dispatch_command`
//! (no undo/redo, no persistence, no command logging).

use std::collections::HashMap;

use crate::state::AppState;
use serde::Deserialize;
use swissarmyhammer_spatial_nav::{BatchEntry, Direction, Rect};
use tauri::{Emitter, Runtime, State, WebviewWindow};

/// Wire format for `spatial_register`.
///
/// Wrapping the full arg list in a `Deserialize` struct lets us attach
/// serde aliases per field so callers may use either camelCase or
/// snake_case — Tauri's built-in `rename_all` picks exactly one naming
/// convention at the command level and silently drops fields in the other,
/// which is a footgun for a command this central to keyboard focus. A
/// struct wrapper deserialized by serde applies `rename_all = "camelCase"`
/// as the canonical wire form and `#[serde(alias = "...")]` to accept the
/// snake_case form too.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialRegisterArgs {
    /// Unique spatial key (ULID generated client-side).
    pub key: String,
    /// Entity moniker (e.g. `"task:01ABC"`).
    pub moniker: String,
    /// Left edge x-coordinate.
    pub x: f64,
    /// Top edge y-coordinate.
    pub y: f64,
    /// Width in logical pixels.
    pub w: f64,
    /// Height in logical pixels.
    pub h: f64,
    /// Spatial key of the FocusLayer this scope lives in. Accepted as
    /// either `layerKey` (default) or `layer_key` (serde alias) on the
    /// wire — see the struct-level doc comment for the rationale.
    #[serde(alias = "layer_key")]
    pub layer_key: String,
    /// Optional parent scope key for container-first navigation.
    /// Accepts both `parentScope` and `parent_scope`.
    #[serde(alias = "parent_scope", default)]
    pub parent_scope: Option<String>,
    /// Directional navigation overrides.
    #[serde(default)]
    pub overrides: Option<HashMap<String, Option<String>>>,
}

/// Emit a `focus-changed` event scoped to a single window.
///
/// Uses `emit_to` with the window label (which matches `EventTarget::AnyLabel`)
/// so only listeners registered on this specific window fire. Using the
/// app-wide `app.emit` would broadcast to every open window, causing window
/// B's focus-claim handlers to run in response to focus moving inside
/// window A.
fn emit_focus_changed<R: Runtime>(
    window: &WebviewWindow<R>,
    event: &swissarmyhammer_spatial_nav::FocusChanged,
) {
    let _ = window.emit_to(window.label(), "focus-changed", event);
}

/// Register a spatial entry (FocusScope mount or ResizeObserver update).
///
/// Called by React when a FocusScope mounts or its rect changes. The
/// spatial key is a ULID generated client-side, stable across re-renders,
/// unique per mount. The optional `overrides` map allows per-entry
/// navigation redirection or blocking by direction string.
///
/// The entry is written into the `SpatialState` owned by the invoking
/// window — resolved via the `WebviewWindow` parameter's label. Entries
/// registered from window A are invisible to navigation in window B.
///
/// Accepts both camelCase (`layerKey`, `parentScope`) and snake_case
/// (`layer_key`, `parent_scope`) field names on the wire — see
/// [`SpatialRegisterArgs`] for why.
#[tauri::command]
pub async fn spatial_register<R: Runtime>(
    args: SpatialRegisterArgs,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    spatial_state.register(
        args.key,
        args.moniker,
        Rect {
            x: args.x,
            y: args.y,
            width: args.w,
            height: args.h,
        },
        args.layer_key,
        args.parent_scope,
        args.overrides.unwrap_or_default(),
    );
    Ok(())
}

/// Unregister a spatial entry (FocusScope unmount).
///
/// If the unregistered entry was the focused key, the spatial state picks a
/// successor (layer memory → sibling in the same parent scope → top-left
/// entry in the active layer) and a `focus-changed` event is emitted scoped
/// to the invoking window. Focus clears to `None` only when the removed
/// entry was the last registered candidate on its layer. This preserves the
/// "something is always focused" invariant so nav keys in the successor
/// view have a valid target.
#[tauri::command]
pub async fn spatial_unregister<R: Runtime>(
    key: String,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.unregister(&key) {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

/// Set focus to a spatial key (click or programmatic).
///
/// Updates the focused key for the invoking window and emits a
/// `focus-changed` event scoped to that window if the focus actually
/// changed. No-op if the key is already focused.
#[tauri::command]
pub async fn spatial_focus<R: Runtime>(
    key: String,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.focus(&key) {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

/// Clear focus without removing any entry.
///
/// Called when React clears focus (e.g. `setFocus(null)`). Emits a
/// `focus-changed` event scoped to the invoking window if something was
/// previously focused.
#[tauri::command]
pub async fn spatial_clear_focus<R: Runtime>(
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.clear_focus() {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

/// Navigate from a key in a direction using beam test + scoring.
///
/// Filters to the active layer within this window's `SpatialState`, applies
/// container-first search, and emits a `focus-changed` event scoped to this
/// window if focus moves. Returns the moniker of the newly focused entry,
/// or `None` if no target was found. Candidates from other windows are
/// invisible because each window has its own registry.
///
/// `key` is optional. React passes `null` when no moniker is focused or
/// when it has a moniker but no spatial key mapping for it. In that case
/// Rust's [`SpatialState::navigate`] falls through to the top-left entry
/// of the active layer — the safety net that makes the "something is
/// always focused" invariant recoverable from a null/stale JS state.
#[tauri::command]
pub async fn spatial_navigate<R: Runtime>(
    key: Option<String>,
    direction: String,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let dir: Direction = direction
        .parse()
        .map_err(|e: swissarmyhammer_spatial_nav::ParseDirectionError| e.to_string())?;
    let spatial_state = state.spatial_state_for(window.label()).await;
    match spatial_state.navigate(key.as_deref(), dir)? {
        Some(event) => {
            let next = event.next_key.clone();
            emit_focus_changed(&window, &event);
            Ok(next.and_then(|k| spatial_state.get(&k).map(|e| e.moniker)))
        }
        None => Ok(None),
    }
}

/// Push a focus layer onto the layer stack (FocusLayer mount).
///
/// The active (topmost) layer determines which entries are visible to
/// `spatial_navigate`. Pushes onto the invoking window's layer stack —
/// another window's stack is not touched.
#[tauri::command]
pub async fn spatial_push_layer<R: Runtime>(
    key: String,
    name: String,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    spatial_state.push_layer(key, name);
    Ok(())
}

/// Remove a focus layer from the layer stack by key (FocusLayer unmount).
///
/// Removal is by key, not pop — supports out-of-order unmount. Operates on
/// the invoking window's layer stack. If the layer below has a
/// `last_focused` key, focus is restored and a `focus-changed` event is
/// emitted scoped to that window.
#[tauri::command]
pub async fn spatial_remove_layer<R: Runtime>(
    key: String,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.remove_layer(&key) {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

/// Wire format for `spatial_focus_first_in_layer`.
///
/// A struct wrapper with `rename_all = "camelCase"` + `#[serde(alias)]`
/// so the command accepts both `layerKey` (canonical camelCase) and
/// `layer_key` (snake_case) on the wire — same convention as
/// [`SpatialRegisterArgs`].
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpatialFocusFirstInLayerArgs {
    /// Spatial key of the layer whose first (upper-left) entry should
    /// claim focus. Accepted as either `layerKey` (default) or
    /// `layer_key` (serde alias).
    #[serde(alias = "layer_key")]
    pub layer_key: String,
}

/// Focus the upper-left (first) registered entry in the given layer.
///
/// Called by `FocusLayer` on a `requestAnimationFrame` after
/// `spatial_push_layer`, so descendant `FocusScope`s have had a tick to
/// register their rects. A no-op when the layer is empty or the focused
/// key already belongs to the given layer (see
/// [`SpatialState::focus_first_in_layer`] for the full rationale).
///
/// Emits `focus-changed` scoped to the invoking window if focus moved.
///
/// Accepts either `layerKey` (default) or `layer_key` (serde alias) on
/// the wire — matches the forgiving arg-name convention used by
/// `spatial_register`.
#[tauri::command]
pub async fn spatial_focus_first_in_layer<R: Runtime>(
    args: SpatialFocusFirstInLayerArgs,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.focus_first_in_layer(&args.layer_key) {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

/// Wire format for a single entry in a batch registration call.
///
/// Mirrors the fields of `spatial_register` but packed into a struct so the
/// frontend can send an array in one invoke.
///
/// Like [`SpatialRegisterArgs`], this struct accepts both camelCase
/// (`layerKey`, `parentScope`) and snake_case (`layer_key`, `parent_scope`)
/// field names on the wire via `rename_all = "camelCase"` plus
/// `#[serde(alias)]`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BatchEntryPayload {
    /// Unique spatial key (ULID generated client-side).
    pub key: String,
    /// Entity moniker (e.g. `"task:01ABC"`).
    pub moniker: String,
    /// Left edge x-coordinate.
    pub x: f64,
    /// Top edge y-coordinate.
    pub y: f64,
    /// Width in logical pixels.
    pub w: f64,
    /// Height in logical pixels.
    pub h: f64,
    /// Spatial key of the FocusLayer this scope lives in. Accepts either
    /// `layerKey` (default) or `layer_key` (serde alias).
    #[serde(alias = "layer_key")]
    pub layer_key: String,
    /// Optional parent scope key for container-first navigation. Accepts
    /// either `parentScope` or `parent_scope`.
    #[serde(alias = "parent_scope", default)]
    pub parent_scope: Option<String>,
    /// Directional navigation overrides.
    #[serde(default)]
    pub overrides: Option<HashMap<String, Option<String>>>,
}

/// Register multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer to register estimated rects for off-screen items.
/// Each entry is an upsert — overwrites any existing entry with the same
/// key in the invoking window's `SpatialState`.
#[tauri::command]
pub async fn spatial_register_batch<R: Runtime>(
    entries: Vec<BatchEntryPayload>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let batch: Vec<BatchEntry> = entries
        .into_iter()
        .map(|e| BatchEntry {
            key: e.key,
            moniker: e.moniker,
            rect: Rect {
                x: e.x,
                y: e.y,
                width: e.w,
                height: e.h,
            },
            layer_key: e.layer_key,
            parent_scope: e.parent_scope,
            overrides: e.overrides.unwrap_or_default(),
        })
        .collect();
    let spatial_state = state.spatial_state_for(window.label()).await;
    spatial_state.register_batch(batch);
    Ok(())
}

/// Unregister multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer on unmount to clean up placeholder entries in
/// the invoking window's `SpatialState`. If the focused key was among those
/// removed, a `focus-changed` event is emitted scoped to the window.
#[tauri::command]
pub async fn spatial_unregister_batch<R: Runtime>(
    keys: Vec<String>,
    window: WebviewWindow<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    let spatial_state = state.spatial_state_for(window.label()).await;
    if let Some(event) = spatial_state.unregister_batch(&keys) {
        emit_focus_changed(&window, &event);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Debug-only: __spatial_dump
// ---------------------------------------------------------------------------
//
// SINGLE SOURCE OF TRUTH for the `__spatial_dump` debug command gate.
//
// Everything that distinguishes debug from release builds for this command
// lives inside the `debug_commands` submodule below: the command itself, its
// serde payload types, and the macro (`kanban_invoke_handler!`) that wires
// it into `tauri::generate_handler!` in `main.rs`. The module is gated by
// exactly one `#[cfg(debug_assertions)]` — if the gate is wrong, nothing
// downstream compiles, and there is no way for a future edit to leak
// `__spatial_dump` into a release binary while the registration quietly
// drops it (or vice-versa).
//
// Do NOT add another `#[cfg(debug_assertions)]` anywhere else for this
// command. The `main.rs` registration calls `kanban_invoke_handler!`
// unconditionally; the macro handles debug vs. release internally.
//
// Tests can check `typeof (window as any).__TAURI__ === 'object'` and
// then `invoke('__spatial_dump')` without any further setup. Release
// builds omit the symbol entirely so there is no way for a user-level
// caller to reach it.

#[cfg(debug_assertions)]
pub mod debug_commands {
    use super::*;
    use serde::Serialize;

    /// Serializable snapshot of the spatial focus state.
    ///
    /// Returned by `__spatial_dump` for test assertions against the Rust-side
    /// state — tests that rely only on DOM inspection can't tell whether
    /// `SpatialState` and the React tree agree, so this command closes that
    /// gap. Release builds exclude this struct with the command itself.
    #[derive(Debug, Serialize)]
    pub struct SpatialDump {
        /// The currently-focused spatial key, or `None` if nothing is focused.
        pub focused_key: Option<String>,
        /// The entity moniker of the focused entry, resolved via the entry
        /// registry. `None` when either nothing is focused or the focused key
        /// is no longer in the registry (a stale focus the caller should
        /// investigate).
        pub focused_moniker: Option<String>,
        /// Total number of registered spatial entries across every layer.
        pub entry_count: usize,
        /// Layer stack snapshot, bottom-first. The last element is the active
        /// layer. Each entry carries its own per-layer entry count and
        /// `last_focused` memory so tests can verify focus-restoration logic.
        pub layer_stack: Vec<LayerDumpEntry>,
    }

    /// Serializable snapshot of a single layer in the spatial layer stack.
    #[derive(Debug, Serialize)]
    pub struct LayerDumpEntry {
        /// Layer key (ULID minted client-side).
        pub key: String,
        /// Human-readable layer name — e.g. `"window"`, `"inspector"`.
        pub name: String,
        /// The key of the entry most recently focused in this layer, if any.
        pub last_focused: Option<String>,
        /// Number of registered spatial entries tagged with this layer's key.
        pub entry_count_in_layer: usize,
    }

    /// Dump the spatial state for the invoking window, for test assertions.
    ///
    /// Per-window: returns only the entries, layers, and focused key tracked
    /// by the `SpatialState` owned by this window, so a test can assert that
    /// window A's registry never contains window B's entries. Only compiled
    /// into debug builds (this entire module is gated by
    /// `#[cfg(debug_assertions)]`). Registered via `kanban_invoke_handler!`
    /// in `main.rs`, which drops the identifier from the handler list in
    /// release builds — so there is no way to invoke this command from a
    /// production binary.
    #[tauri::command]
    pub async fn __spatial_dump<R: Runtime>(
        window: WebviewWindow<R>,
        state: State<'_, AppState>,
    ) -> Result<SpatialDump, String> {
        let spatial_state = state.spatial_state_for(window.label()).await;
        let entries = spatial_state.entries_snapshot();
        let layers = spatial_state.layers_snapshot();
        let focused_key = spatial_state.focused_key();

        // Resolve focused moniker by walking the entries snapshot rather than
        // round-tripping back through `SpatialState::get` — cheaper, and keeps
        // the two halves of the snapshot internally consistent even if another
        // thread mutates state between the two calls.
        let focused_moniker = focused_key.as_deref().and_then(|fk| {
            entries
                .iter()
                .find(|e| e.key == fk)
                .map(|e| e.moniker.clone())
        });

        // Per-layer counts: one pass over the entries vector keyed by layer_key.
        let mut counts: HashMap<String, usize> = HashMap::new();
        for entry in &entries {
            *counts.entry(entry.layer_key.clone()).or_insert(0) += 1;
        }

        let layer_stack = layers
            .into_iter()
            .map(|layer| LayerDumpEntry {
                entry_count_in_layer: counts.get(&layer.key).copied().unwrap_or(0),
                key: layer.key,
                name: layer.name,
                last_focused: layer.last_focused,
            })
            .collect();

        Ok(SpatialDump {
            focused_key,
            focused_moniker,
            entry_count: entries.len(),
            layer_stack,
        })
    }
}

/// Per-window [`SpatialNavigator`] implementation for the Tauri binary.
///
/// Resolves the `SpatialState` for the named window via
/// [`AppState::spatial_state_for`], reads its current focused key as the
/// navigation source, applies `SpatialState::navigate`, and emits a
/// `focus-changed` event scoped to that window when focus moves. This is
/// the production impl of the `nav.*` command path:
///
/// ```text
/// keypress → dispatch_command(nav.down) → NavigateCmd(Direction::Down)
///     → TauriSpatialNavigator::navigate("main", Down)
///     → spatial_state.navigate(focused_key, Down)
///     → emit_focus_changed → React store → FocusScope re-render
/// ```
///
/// Unlike the standalone `spatial_navigate` Tauri command (kept for
/// direct JS-driven invokes via `syncSpatialFocus`), the navigator reads
/// the focused key from `SpatialState` itself — React never has to pass
/// it through. Rust is the authoritative owner of focus and can always
/// find its own source without a round-trip.
pub(crate) struct TauriSpatialNavigator {
    app: tauri::AppHandle,
}

impl TauriSpatialNavigator {
    /// Build a navigator bound to the given Tauri `AppHandle`.
    ///
    /// The handle resolves both the per-window `SpatialState` (via
    /// `app.state::<AppState>()`) and the per-window event emitter
    /// (`app.get_webview_window(label)`), so one handle is enough for the
    /// entire navigator surface.
    pub(crate) fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

#[async_trait::async_trait]
impl swissarmyhammer_kanban::spatial::SpatialNavigator for TauriSpatialNavigator {
    async fn navigate(
        &self,
        window_label: &str,
        direction: swissarmyhammer_spatial_nav::Direction,
    ) -> Result<Option<String>, String> {
        use tauri::Manager;
        let state = self.app.state::<AppState>();
        let spatial_state = state.spatial_state_for(window_label).await;
        // Rust owns focus — read the source key from SpatialState itself.
        // This removes the last place React had to pass the focused key
        // back to Rust (`spatial_navigate` still takes it for the
        // `syncSpatialFocus` click path).
        let from_key = spatial_state.focused_key();
        match spatial_state.navigate(from_key.as_deref(), direction)? {
            Some(event) => {
                // Emit scoped to the invoking window — matches how the
                // standalone `spatial_navigate` Tauri command emits, so
                // the React listener on `getCurrentWebviewWindow()` fires
                // for this window only.
                if let Some(window) = self.app.get_webview_window(window_label) {
                    use tauri::Emitter;
                    let _ = window.emit_to(window.label(), "focus-changed", &event);
                }
                let next_moniker = event
                    .next_key
                    .and_then(|k| spatial_state.get(&k).map(|e| e.moniker));
                Ok(next_moniker)
            }
            None => Ok(None),
        }
    }
}

/// Build the kanban-app Tauri `invoke_handler` from a comma-separated list of
/// command idents, automatically appending the debug-only `__spatial_dump`
/// command in debug builds and omitting it entirely in release builds.
///
/// This is the single wiring point for debug commands. `main.rs` calls it
/// unconditionally, without any `#[cfg]` of its own. Adding a new debug-only
/// command means appending it inside the `#[cfg(debug_assertions)]` branch of
/// this macro — nothing else changes.
///
/// Why a macro instead of a helper function? `tauri::generate_handler!` is a
/// proc-macro that must see the full comma-separated list of command idents
/// as literal tokens at call time; it cannot accept a pre-built handler
/// value. `macro_rules!` runs before the proc-macro expands, so by the time
/// `generate_handler!` sees its input, the debug command is either present
/// or absent — no runtime dispatch, no `Fn` chaining, no Tauri-builder
/// limitation (`invoke_handler` replaces, it does not append).
#[macro_export]
macro_rules! kanban_invoke_handler {
    ($($cmd:path),* $(,)?) => {{
        #[cfg(debug_assertions)]
        {
            ::tauri::generate_handler![
                $($cmd,)*
                $crate::spatial::debug_commands::__spatial_dump,
            ]
        }
        #[cfg(not(debug_assertions))]
        {
            ::tauri::generate_handler![$($cmd,)*]
        }
    }};
}

#[cfg(all(test, debug_assertions))]
mod debug_dump_tests {
    use super::debug_commands::{LayerDumpEntry, SpatialDump};
    use std::collections::HashMap;
    use swissarmyhammer_spatial_nav::{Rect, SpatialState};

    /// Build a `SpatialState` with a layer, three entries, and a focused key;
    /// then render it to `SpatialDump` the same way the Tauri command does
    /// and check every field of the payload shape.
    ///
    /// The Tauri command is a thin wrapper over the logic below, so
    /// covering the wrapper's body here is enough — no full Tauri app
    /// needed for a pure state snapshot.
    fn rect(x: f64, y: f64, w: f64, h: f64) -> Rect {
        Rect {
            x,
            y,
            width: w,
            height: h,
        }
    }

    /// Render a dump from the given `SpatialState`. Mirrors the body of
    /// `__spatial_dump` so we can assert on `SpatialDump` without a real
    /// Tauri `State<'_, AppState>`.
    fn render_dump(spatial: &SpatialState) -> SpatialDump {
        let entries = spatial.entries_snapshot();
        let layers = spatial.layers_snapshot();
        let focused_key = spatial.focused_key();
        let focused_moniker = focused_key.as_deref().and_then(|fk| {
            entries
                .iter()
                .find(|e| e.key == fk)
                .map(|e| e.moniker.clone())
        });
        let mut counts: HashMap<String, usize> = HashMap::new();
        for entry in &entries {
            *counts.entry(entry.layer_key.clone()).or_insert(0) += 1;
        }
        let layer_stack = layers
            .into_iter()
            .map(|layer| LayerDumpEntry {
                entry_count_in_layer: counts.get(&layer.key).copied().unwrap_or(0),
                key: layer.key,
                name: layer.name,
                last_focused: layer.last_focused,
            })
            .collect();
        SpatialDump {
            focused_key,
            focused_moniker,
            entry_count: entries.len(),
            layer_stack,
        }
    }

    #[test]
    fn spatial_dump_three_entries_focus_and_layer() {
        let spatial = SpatialState::new();
        spatial.push_layer("layer-window".into(), "window".into());

        spatial.register(
            "k1".into(),
            "task:01".into(),
            rect(0.0, 0.0, 100.0, 50.0),
            "layer-window".into(),
            None,
            HashMap::new(),
        );
        spatial.register(
            "k2".into(),
            "task:02".into(),
            rect(200.0, 0.0, 100.0, 50.0),
            "layer-window".into(),
            None,
            HashMap::new(),
        );
        spatial.register(
            "k3".into(),
            "task:03".into(),
            rect(400.0, 0.0, 100.0, 50.0),
            "layer-window".into(),
            None,
            HashMap::new(),
        );
        spatial.focus("k2");

        let dump = render_dump(&spatial);

        assert_eq!(dump.focused_key.as_deref(), Some("k2"));
        assert_eq!(dump.focused_moniker.as_deref(), Some("task:02"));
        assert_eq!(dump.entry_count, 3);
        assert_eq!(dump.layer_stack.len(), 1);

        let layer = &dump.layer_stack[0];
        assert_eq!(layer.key, "layer-window");
        assert_eq!(layer.name, "window");
        // last_focused is written only on focus *changes* — since k2 was
        // the first focus, the active layer's slot is still empty.
        assert_eq!(layer.last_focused, None);
        assert_eq!(layer.entry_count_in_layer, 3);

        // Serialize to JSON and check the wire format matches the task
        // spec's `#[derive(Serialize)]` contract — all fields present,
        // snake_case names unchanged.
        let json = serde_json::to_value(&dump).expect("SpatialDump is Serialize");
        assert_eq!(json["focused_key"], "k2");
        assert_eq!(json["focused_moniker"], "task:02");
        assert_eq!(json["entry_count"], 3);
        assert!(json["layer_stack"].is_array());
        assert_eq!(json["layer_stack"][0]["key"], "layer-window");
        assert_eq!(json["layer_stack"][0]["name"], "window");
        assert_eq!(json["layer_stack"][0]["entry_count_in_layer"], 3);
    }

    #[test]
    fn spatial_dump_after_layer_push_reports_two_layers_with_correct_counts() {
        let spatial = SpatialState::new();
        spatial.push_layer("layer-A".into(), "window".into());
        spatial.push_layer("layer-B".into(), "inspector".into());

        spatial.register(
            "win".into(),
            "task:01".into(),
            rect(0.0, 0.0, 100.0, 50.0),
            "layer-A".into(),
            None,
            HashMap::new(),
        );
        spatial.register(
            "field-1".into(),
            "field:title".into(),
            rect(300.0, 0.0, 100.0, 30.0),
            "layer-B".into(),
            None,
            HashMap::new(),
        );
        spatial.register(
            "field-2".into(),
            "field:desc".into(),
            rect(300.0, 40.0, 100.0, 30.0),
            "layer-B".into(),
            None,
            HashMap::new(),
        );
        spatial.focus("field-1");

        let dump = render_dump(&spatial);

        assert_eq!(dump.entry_count, 3);
        assert_eq!(dump.layer_stack.len(), 2);

        // layers_snapshot is bottom-first — index 0 is layer-A (window),
        // index 1 is the active layer (layer-B).
        assert_eq!(dump.layer_stack[0].key, "layer-A");
        assert_eq!(dump.layer_stack[0].entry_count_in_layer, 1);
        assert_eq!(dump.layer_stack[1].key, "layer-B");
        assert_eq!(dump.layer_stack[1].entry_count_in_layer, 2);
    }

    #[test]
    fn spatial_dump_with_nothing_focused_returns_none_fields() {
        let spatial = SpatialState::new();
        let dump = render_dump(&spatial);
        assert!(dump.focused_key.is_none());
        assert!(dump.focused_moniker.is_none());
        assert_eq!(dump.entry_count, 0);
        assert!(dump.layer_stack.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Tauri integration tests
// ---------------------------------------------------------------------------
//
// These tests exercise the `#[tauri::command]` handlers *in process* against a
// real `AppState` and a `tauri::test::mock_app()`. They close the gap between:
//
//   - React tests that mock `invoke` from `@tauri-apps/api/core` — never
//     exercise the Rust side.
//   - `SpatialState` unit tests — never exercise the Tauri wrapper layer.
//
// The wrapper layer *is* the wire format. It destructures positional args,
// maps `None` overrides to `unwrap_or_default()`, turns `BatchEntryPayload`
// into `BatchEntry`, emits `focus-changed` events, and parses direction
// strings. A typo on any of those paths would silently break the React ↔ Rust
// contract with no test coverage today — that's what these tests lock down.
//
// We register handler functions as commands via `mock_builder()` and drive
// them through `get_ipc_response`, which takes the same JSON-encoded path
// that real IPC invocations take. Any future change to the serde wire format
// (field rename, type change, new enum variant) breaks at least one of these
// tests.
#[cfg(test)]
mod tauri_integration_tests {
    use super::*;
    use crate::state::AppState;
    use serde_json::json;
    use std::sync::{Arc, Mutex};
    use tauri::test::{mock_builder, noop_assets, MockRuntime};
    use tauri::{Listener, Manager, WebviewWindowBuilder};

    /// Build a `App<MockRuntime>` with every spatial command registered and
    /// the shared `AppState` managed.
    ///
    /// The resulting app exposes a `"main"` webview window so
    /// `get_ipc_response` has a target to dispatch into. Event listening works
    /// through the webview's `Listener` impl (per-window scope — matches the
    /// production emission path) and does not require a running event loop —
    /// emissions land synchronously in the listener's closure before
    /// `emit()` returns.
    ///
    /// Uses [`kanban_invoke_handler!`] so the debug-only `__spatial_dump`
    /// command is available to tests just as it is in development binaries.
    fn build_test_app() -> tauri::App<MockRuntime> {
        build_test_app_with_windows(&["main"])
    }

    /// Build a mock app with the given webview windows all registered and
    /// ready to dispatch IPC.
    ///
    /// Used by the multi-window tests to exercise the invariant that each
    /// window owns a distinct `SpatialState` and that `focus-changed` events
    /// never cross window boundaries.
    fn build_test_app_with_windows(labels: &[&str]) -> tauri::App<MockRuntime> {
        assert!(!labels.is_empty(), "at least one window label required");
        let app = mock_builder()
            .invoke_handler(crate::kanban_invoke_handler![
                spatial_register,
                spatial_register_batch,
                spatial_unregister,
                spatial_unregister_batch,
                spatial_focus,
                spatial_clear_focus,
                spatial_navigate,
                spatial_push_layer,
                spatial_remove_layer,
                spatial_focus_first_in_layer,
            ])
            .build(tauri::test::mock_context(noop_assets()))
            .expect("failed to build mock Tauri app");

        app.manage(AppState::new_for_test());

        for label in labels {
            WebviewWindowBuilder::new(&app, *label, tauri::WebviewUrl::default())
                .build()
                .unwrap_or_else(|e| panic!("failed to build mock webview {label}: {e}"));
        }

        app
    }

    /// Invoke a `#[tauri::command]` through the `"main"` webview and return
    /// the parsed JSON response.
    ///
    /// Panics with a loud message on IPC failure, because a failing command
    /// in one of these tests means the serde wire format broke — and we want
    /// that to be the first failure the engineer sees, not a silent `None`.
    fn invoke(
        app: &tauri::App<MockRuntime>,
        cmd: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        invoke_in_window(app, "main", cmd, payload)
    }

    /// Invoke a `#[tauri::command]` through the webview with the given label.
    ///
    /// The command sees that webview's `WebviewWindow<R>` as its `window`
    /// parameter — which is how per-window state routing (the whole point of
    /// this refactor) gets exercised end-to-end.
    fn invoke_in_window(
        app: &tauri::App<MockRuntime>,
        label: &str,
        cmd: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        let webview = app
            .get_webview_window(label)
            .unwrap_or_else(|| panic!("webview {label} not present"));
        let res = tauri::test::get_ipc_response(
            &webview,
            tauri::webview::InvokeRequest {
                cmd: cmd.into(),
                callback: tauri::ipc::CallbackFn(0),
                error: tauri::ipc::CallbackFn(1),
                url: "http://tauri.localhost".parse().unwrap(),
                body: tauri::ipc::InvokeBody::Json(payload),
                headers: Default::default(),
                invoke_key: tauri::test::INVOKE_KEY.to_string(),
            },
        );
        match res {
            Ok(body) => body
                .deserialize::<serde_json::Value>()
                .expect("response body is valid JSON"),
            Err(e) => panic!("IPC call to {cmd} via window {label} failed: {e}"),
        }
    }

    /// Subscribe to `focus-changed` events on a specific window before any
    /// command is invoked.
    ///
    /// `focus-changed` emissions are scoped per-window via `emit_to(label, ...)`
    /// so each window's listener must be registered on its own
    /// `WebviewWindow`, not the `AppHandle`. This mirrors the frontend, which
    /// uses `getCurrentWebviewWindow().listen(...)` for the same reason.
    ///
    /// Returns an `Arc<Mutex<Vec<FocusChanged>>>` that accumulates one entry
    /// per emission, in order. The listener decodes each event payload from
    /// JSON — that is the core assertion target for these tests, since a
    /// field rename in the `FocusChanged` struct would break deserialisation
    /// and surface as a panic here.
    fn capture_focus_events_on_window(
        app: &tauri::App<MockRuntime>,
        label: &str,
    ) -> Arc<Mutex<Vec<swissarmyhammer_spatial_nav::FocusChanged>>> {
        let webview = app
            .get_webview_window(label)
            .unwrap_or_else(|| panic!("webview window {label} not present"));
        let events: Arc<Mutex<Vec<swissarmyhammer_spatial_nav::FocusChanged>>> =
            Arc::new(Mutex::new(Vec::new()));
        let events_cl = Arc::clone(&events);
        webview.listen("focus-changed", move |ev| {
            let parsed: swissarmyhammer_spatial_nav::FocusChanged =
                serde_json::from_str(ev.payload())
                    .expect("focus-changed payload deserializes as FocusChanged");
            events_cl.lock().unwrap().push(parsed);
        });
        events
    }

    /// Shorthand for `capture_focus_events_on_window(app, "main")` — the
    /// single-window tests all register the "main" webview and listen on it.
    fn capture_focus_events(
        app: &tauri::App<MockRuntime>,
    ) -> Arc<Mutex<Vec<swissarmyhammer_spatial_nav::FocusChanged>>> {
        capture_focus_events_on_window(app, "main")
    }

    /// Register → focus → navigate, end-to-end: the happy path that every
    /// keyboard-driven focus shift on the board takes.
    ///
    /// Verifies:
    /// - `spatial_register` accepts the full positional arg list (including
    ///   explicit `null` for `parent_scope` and `overrides`, which the React
    ///   side sends when the `FocusScope` has no ancestor or override config).
    /// - `spatial_focus` emits exactly one `focus-changed` event and the
    ///   payload round-trips through the JSON wire format intact.
    /// - `spatial_navigate` returns the moniker of the new focus and emits a
    ///   second `focus-changed` event with the expected `prev_key`/`next_key`.
    #[test]
    fn register_focus_navigate_emits_events_and_returns_moniker() {
        let app = build_test_app();
        let events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k2",
                    "moniker": "task:B",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        invoke(&app, "spatial_focus", json!({"key": "k1"}));
        let nav_result = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "k1", "direction": "Right"}),
        );
        assert_eq!(nav_result, json!("task:B"));

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2, "expected focus + navigate events");

        // First emission: `spatial_focus` setting k1 for the first time.
        assert_eq!(events[0].prev_key, None);
        assert_eq!(events[0].next_key.as_deref(), Some("k1"));

        // Second: `spatial_navigate` transitioning k1 → k2.
        assert_eq!(events[1].prev_key.as_deref(), Some("k1"));
        assert_eq!(events[1].next_key.as_deref(), Some("k2"));
    }

    /// Override map round-trip: React's `Record<string, string | null>` must
    /// land in the Rust `HashMap<String, Option<String>>` field on the entry
    /// and actually affect navigation.
    ///
    /// Exercises both the `Some(target)` redirect path (maps "Right" →
    /// "task:C" even though k2 is the geometric right neighbour) and the
    /// `None` block path (maps "Down" to blocked, so navigate returns the
    /// moniker of the already-focused entry rather than silently walking to
    /// a geometric sibling).
    #[test]
    fn navigate_honors_override_redirect_and_block() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );

        // k1 lives at origin. k2 is its geometric right neighbour. k3 lives
        // below — but the override on k1 redirects "Right" to "task:C" (k3)
        // and blocks "Down" entirely. Both geometric defaults should be
        // overridden.
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": {
                        "Right": "task:C",
                        "Down": null,
                    },
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k2",
                    "moniker": "task:B",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k3",
                    "moniker": "task:C",
                    "x": 0.0, "y": 200.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "k1"}));

        // Right would naturally hit k2 (task:B); the override redirects to
        // task:C (k3).
        let right = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "k1", "direction": "Right"}),
        );
        assert_eq!(
            right,
            json!("task:C"),
            "override should redirect Right to task:C"
        );

        // Re-focus k1 before testing Down (k1 → k3 took focus to k3).
        invoke(&app, "spatial_focus", json!({"key": "k1"}));

        // Down is blocked by the override — navigate returns Null and focus
        // stays on k1.
        let down = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "k1", "direction": "Down"}),
        );
        assert_eq!(
            down,
            serde_json::Value::Null,
            "blocked override should return null"
        );
    }

    /// `spatial_register_batch` must map its `BatchEntryPayload` to the
    /// underlying `BatchEntry` correctly. This is the virtualizer's hot
    /// path — one invoke registers N placeholder rects so off-screen
    /// items are still navigation targets.
    ///
    /// The test registers three entries in one batch, focuses the leftmost,
    /// and verifies navigate-right reaches the right-most batch entry with
    /// the correct moniker.
    #[test]
    fn register_batch_then_navigate_reaches_batch_entry() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );

        // The batch entry struct uses snake_case field names on the wire —
        // this mirrors the React call site in `column-view.tsx` which builds
        // each entry with `layer_key`, `parent_scope`, and `overrides`. The
        // top-level `entries` argument to `spatial_register_batch` itself is
        // camelCase per Tauri's default command arg convention, which would
        // still be `entries` either way.
        invoke(
            &app,
            "spatial_register_batch",
            json!({
                "entries": [
                    {
                        "key": "b1",
                        "moniker": "task:A",
                        "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layer_key": "L1",
                        "parent_scope": null,
                        "overrides": null,
                    },
                    {
                        "key": "b2",
                        "moniker": "task:B",
                        "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layer_key": "L1",
                        "parent_scope": null,
                        "overrides": null,
                    },
                    {
                        "key": "b3",
                        "moniker": "task:C",
                        "x": 400.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layer_key": "L1",
                        "parent_scope": null,
                        "overrides": null,
                    },
                ],
            }),
        );

        invoke(&app, "spatial_focus", json!({"key": "b1"}));
        let right1 = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "b1", "direction": "Right"}),
        );
        assert_eq!(right1, json!("task:B"));

        let right2 = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "b2", "direction": "Right"}),
        );
        assert_eq!(right2, json!("task:C"));
    }

    /// Layer stack semantics end-to-end: push a second layer, register
    /// entries on it, verify navigate on layer 2 cannot reach layer 1, then
    /// remove layer 2 and verify focus restores to layer 1's `last_focused`.
    ///
    /// This is the inspector-overlay interaction in miniature — the
    /// inspector mounts a new layer, user navigates inside it, then closes
    /// it and focus must come back to where they were on the board.
    #[test]
    fn push_layer_isolates_navigation_and_remove_restores_focus() {
        let app = build_test_app();
        let events = capture_focus_events(&app);

        // Layer 1: two task cards. Focus the first, which saves it as the
        // layer's last_focused before layer 2 takes over.
        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "window"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "b1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "b2",
                    "moniker": "task:B",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "b1"}));
        // Triggering a second focus_change stores b1 as last_focused on L1.
        invoke(&app, "spatial_focus", json!({"key": "b2"}));

        // Layer 2: inspector fields. The inspector's nav must not reach b1
        // or b2 on layer 1.
        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L2", "name": "inspector"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "f1",
                    "moniker": "field:title",
                    "x": 500.0, "y": 0.0, "w": 100.0, "h": 30.0,
                    "layerKey": "L2",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "f2",
                    "moniker": "field:desc",
                    "x": 500.0, "y": 40.0, "w": 100.0, "h": 30.0,
                    "layerKey": "L2",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "f1"}));

        // Navigate Down from f1 — must land on f2 (layer-2 sibling), not on
        // anything from layer 1 even though geometrically a layer-1 entry
        // might be "below" in some layouts.
        let down = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "f1", "direction": "Down"}),
        );
        assert_eq!(
            down,
            json!("field:desc"),
            "Down from f1 must stay inside layer 2"
        );

        // Navigate Left from f1 — no entries to the left inside layer 2,
        // and navigation cannot cross to layer 1. Expect null.
        // Re-focus f1 first since navigation may have moved focus.
        invoke(&app, "spatial_focus", json!({"key": "f1"}));
        let left = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "f1", "direction": "Left"}),
        );
        assert_eq!(
            left,
            serde_json::Value::Null,
            "Left from f1 must not cross layers, expected null"
        );

        // Close layer 2 — focus must restore to L1.last_focused (b2).
        let events_before_remove = events.lock().unwrap().len();
        invoke(&app, "spatial_remove_layer", json!({"key": "L2"}));

        // One more `focus-changed` event — restoring focus to b2 — should
        // have fired on the original listener.
        let events_after = events.lock().unwrap();
        assert!(
            events_after.len() > events_before_remove,
            "spatial_remove_layer should emit a focus-restore event"
        );
        let restored = events_after.last().unwrap();
        assert_eq!(
            restored.next_key.as_deref(),
            Some("b2"),
            "focus should restore to layer 1's last_focused (b2)"
        );

        let state = app.state::<AppState>();
        let spatial_state = tauri::async_runtime::block_on(state.spatial_state_for("main"));
        assert_eq!(
            spatial_state.focused_key().as_deref(),
            Some("b2"),
            "focus should restore to layer 1's last_focused after remove"
        );
    }

    /// Unregistering the focused key must emit a `focus-changed` event whose
    /// `next_key` is `null`. React listens for exactly this shape to clear
    /// its local `focusedKey` ref — a field rename on either side breaks
    /// keyboard focus across a re-mount.
    #[test]
    fn unregister_focused_key_emits_focus_changed_with_null_next_key() {
        let app = build_test_app();
        let events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "k1"}));

        // Baseline: one focus event from the `spatial_focus` above.
        assert_eq!(events.lock().unwrap().len(), 1);

        invoke(&app, "spatial_unregister", json!({"key": "k1"}));

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2, "unregister of focused key emits event");
        let last = events.last().unwrap();
        assert_eq!(last.prev_key.as_deref(), Some("k1"));
        assert_eq!(
            last.next_key, None,
            "next_key must be null after focus clear"
        );
    }

    /// `spatial_clear_focus` must emit a `focus-changed` event with both
    /// fields pointing at the transition: the previously focused key in
    /// `prev_key`, `null` in `next_key`. This is the path the app takes
    /// when it wants to defocus without unmounting anything (e.g. clicking
    /// empty board area).
    #[test]
    fn clear_focus_emits_event_with_prev_key_and_null_next() {
        let app = build_test_app();
        let events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "k1"}));
        invoke(&app, "spatial_clear_focus", json!({}));

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 2);
        let last = events.last().unwrap();
        assert_eq!(last.prev_key.as_deref(), Some("k1"));
        assert_eq!(last.next_key, None);
    }

    /// Serialized `FocusChanged` payload must use snake_case field names
    /// (`prev_key`, `next_key`) — React's `listen("focus-changed")` handler
    /// destructures those names directly. If the struct's serde tags ever
    /// drift (e.g. a rename, or a global `rename_all = "camelCase"`), this
    /// test catches it.
    #[test]
    fn focus_changed_payload_uses_snake_case_field_names() {
        let app = build_test_app();

        // Listen on the webview window, not the AppHandle — focus-changed is
        // emitted via `window.emit_to(label, …)` and app-wide listeners don't
        // see those targeted emissions.
        let webview = app
            .get_webview_window("main")
            .expect("main webview present");
        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_cl = Arc::clone(&captured);
        webview.listen("focus-changed", move |ev| {
            captured_cl.lock().unwrap().push(ev.payload().to_string());
        });

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(&app, "spatial_focus", json!({"key": "k1"}));

        let payloads = captured.lock().unwrap();
        assert_eq!(payloads.len(), 1);
        let parsed: serde_json::Value =
            serde_json::from_str(&payloads[0]).expect("focus-changed payload is valid JSON");
        assert!(
            parsed.get("prev_key").is_some(),
            "payload missing prev_key: {parsed}"
        );
        assert!(
            parsed.get("next_key").is_some(),
            "payload missing next_key: {parsed}"
        );
        assert_eq!(parsed["next_key"], json!("k1"));
    }

    // ---- Forgiving arg-name deserialization -------------------------------
    //
    // The four tests below lock down the invariant that every spatial command
    // whose payload carries multi-word field names (`layer_key`, `parent_scope`)
    // must accept both snake_case AND camelCase forms on the wire. Tauri v2
    // defaults to camelCase arg names, which silently drops snake_case fields
    // sent by naive callers — serde aliases on the argument struct close that
    // gap without forcing every caller to pick one naming convention.

    /// After `spatial_register` fires, the entry is present in the registry
    /// with the supplied `layer_key` attached — if the field was silently
    /// dropped during deserialization, this readback would show a stale or
    /// missing layer_key and the subsequent focus + navigate assertions would
    /// break. The assertion chain here is the cheapest way to verify the
    /// full round-trip (register -> focus -> navigate) rather than just that
    /// the command returned Ok.
    #[test]
    fn spatial_register_accepts_snake_case_arg_names() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layer_key": "L1",
                    "parent_scope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k2",
                    "moniker": "task:B",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layer_key": "L1",
                    "parent_scope": null,
                    "overrides": null,
                }
            }),
        );

        // Both entries registered with the same layer_key? Navigate proves it —
        // if one entry silently dropped its layer, navigate Right from k1 would
        // never find k2 (navigation is layer-scoped).
        invoke(&app, "spatial_focus", json!({"key": "k1"}));
        let moved = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "k1", "direction": "Right"}),
        );
        assert_eq!(
            moved,
            json!("task:B"),
            "snake_case layer_key must be accepted so navigation can find \
             the right-neighbour on the same layer"
        );
    }

    /// Mirror of the snake_case test, using camelCase arg names. This also
    /// passes today with Tauri's default camelCase wire convention — keeping
    /// it here documents that the forgiving layer does not break the existing
    /// contract.
    #[test]
    fn spatial_register_accepts_camel_case_arg_names() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k1",
                    "moniker": "task:A",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "args": {
                    "key": "k2",
                    "moniker": "task:B",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "L1",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        invoke(&app, "spatial_focus", json!({"key": "k1"}));
        let moved = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "k1", "direction": "Right"}),
        );
        assert_eq!(moved, json!("task:B"));
    }

    /// Batch entries must accept snake_case for `layer_key` / `parent_scope`.
    /// The entries array lives inside the payload, so each element is a
    /// `BatchEntryPayload` — the serde aliases live on that struct.
    #[test]
    fn spatial_register_batch_accepts_snake_case_entry_field_names() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register_batch",
            json!({
                "entries": [
                    {
                        "key": "b1",
                        "moniker": "task:A",
                        "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layer_key": "L1",
                        "parent_scope": null,
                        "overrides": null,
                    },
                    {
                        "key": "b2",
                        "moniker": "task:B",
                        "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layer_key": "L1",
                        "parent_scope": null,
                        "overrides": null,
                    },
                ],
            }),
        );

        invoke(&app, "spatial_focus", json!({"key": "b1"}));
        let right = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "b1", "direction": "Right"}),
        );
        assert_eq!(
            right,
            json!("task:B"),
            "snake_case layer_key on a batch entry must be honoured so the \
             two entries share a layer and are each other's neighbours"
        );
    }

    /// Batch entries must also accept camelCase for `layerKey` / `parentScope`.
    /// Today those field names are snake_case on the wire because serde's
    /// default is to match Rust struct field names verbatim — adding
    /// `#[serde(rename_all = "camelCase")]` + snake_case aliases is what
    /// makes both forms work.
    #[test]
    fn spatial_register_batch_accepts_camel_case_entry_field_names() {
        let app = build_test_app();
        let _events = capture_focus_events(&app);

        invoke(
            &app,
            "spatial_push_layer",
            json!({"key": "L1", "name": "root"}),
        );
        invoke(
            &app,
            "spatial_register_batch",
            json!({
                "entries": [
                    {
                        "key": "b1",
                        "moniker": "task:A",
                        "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layerKey": "L1",
                        "parentScope": null,
                        "overrides": null,
                    },
                    {
                        "key": "b2",
                        "moniker": "task:B",
                        "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                        "layerKey": "L1",
                        "parentScope": null,
                        "overrides": null,
                    },
                ],
            }),
        );

        invoke(&app, "spatial_focus", json!({"key": "b1"}));
        let right = invoke(
            &app,
            "spatial_navigate",
            json!({"key": "b1", "direction": "Right"}),
        );
        assert_eq!(right, json!("task:B"));
    }

    // ---- Multi-window isolation -----------------------------------------
    //
    // These tests exercise the invariant introduced by this refactor: every
    // webview window owns a distinct `SpatialState` so entry registrations,
    // the layer stack, focus tracking, and `focus-changed` emissions never
    // leak across windows. Before the fix, `AppState` held a single
    // `SpatialState` shared by every window — `h/j/k/l` in window A could
    // jump focus to an entry registered in window B, because the beam-test
    // pool was one global set of rects.

    /// Each window owns an independent `SpatialState`: entries registered in
    /// window A are invisible to window B's `__spatial_dump`, and vice versa.
    ///
    /// Against HEAD (pre-fix), both entries would land in the single shared
    /// registry and both dumps would see `entry_count = 2`.
    #[cfg(debug_assertions)]
    #[test]
    fn two_windows_register_independently() {
        let app = build_test_app_with_windows(&["A", "B"]);

        // Window A: push a layer + register one entry at the origin.
        invoke_in_window(
            &app,
            "A",
            "spatial_push_layer",
            json!({"key": "LA", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a1",
                    "moniker": "task:a",
                    "x": 0.0, "y": 0.0, "w": 10.0, "h": 10.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        // Window B: push its own layer + register an entry 100px below.
        invoke_in_window(
            &app,
            "B",
            "spatial_push_layer",
            json!({"key": "LB", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "B",
            "spatial_register",
            json!({
                "args": {
                    "key": "b1",
                    "moniker": "task:b",
                    "x": 0.0, "y": 100.0, "w": 10.0, "h": 10.0,
                    "layerKey": "LB",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        let dump_a = invoke_in_window(&app, "A", "__spatial_dump", json!({}));
        assert_eq!(
            dump_a["entry_count"], 1,
            "window A sees only its own entry: {dump_a}"
        );
        assert_eq!(dump_a["layer_stack"].as_array().unwrap().len(), 1);
        assert_eq!(dump_a["layer_stack"][0]["key"], "LA");
        assert_eq!(dump_a["layer_stack"][0]["entry_count_in_layer"], 1);

        let dump_b = invoke_in_window(&app, "B", "__spatial_dump", json!({}));
        assert_eq!(
            dump_b["entry_count"], 1,
            "window B sees only its own entry: {dump_b}"
        );
        assert_eq!(dump_b["layer_stack"].as_array().unwrap().len(), 1);
        assert_eq!(dump_b["layer_stack"][0]["key"], "LB");
        assert_eq!(dump_b["layer_stack"][0]["entry_count_in_layer"], 1);
    }

    /// Navigating in window A only fires `focus-changed` for listeners
    /// attached to window A. Window B's listener receives nothing, even
    /// though both were subscribed before the navigate ran.
    ///
    /// Against HEAD, `app.emit("focus-changed", …)` was app-wide, so both
    /// listeners would fire. This test fails pre-fix.
    #[test]
    fn navigate_in_one_window_does_not_emit_events_to_the_other() {
        let app = build_test_app_with_windows(&["A", "B"]);
        let events_a = capture_focus_events_on_window(&app, "A");
        let events_b = capture_focus_events_on_window(&app, "B");

        // Window A: layer + two cards so there's something to navigate to.
        invoke_in_window(
            &app,
            "A",
            "spatial_push_layer",
            json!({"key": "LA", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a1",
                    "moniker": "task:a1",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a2",
                    "moniker": "task:a2",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        // Window B: register an entry too, so it has something in its
        // registry. It should see zero events for navigation in window A.
        invoke_in_window(
            &app,
            "B",
            "spatial_push_layer",
            json!({"key": "LB", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "B",
            "spatial_register",
            json!({
                "args": {
                    "key": "b1",
                    "moniker": "task:b1",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LB",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        // Focus + navigate in window A. Two focus-changed events fire for
        // window A's listener — one from `spatial_focus`, one from
        // `spatial_navigate`.
        invoke_in_window(&app, "A", "spatial_focus", json!({"key": "a1"}));
        let moved = invoke_in_window(
            &app,
            "A",
            "spatial_navigate",
            json!({"key": "a1", "direction": "Right"}),
        );
        assert_eq!(moved, json!("task:a2"), "navigate should reach a2");

        let events_a = events_a.lock().unwrap();
        assert_eq!(
            events_a.len(),
            2,
            "window A's listener should see focus + navigate emissions"
        );

        let events_b = events_b.lock().unwrap();
        assert_eq!(
            events_b.len(),
            0,
            "window B's listener must not see window A's focus-changed events; got {events_b:?}"
        );
    }

    /// Beam-test candidates are scoped to the window that originated the
    /// navigate call: entries registered in window B are invisible to window
    /// A's `spatial_navigate`, even if they would otherwise be the
    /// geometrically-best target.
    ///
    /// Window A has two entries at (0,0) and (200,0). Window B has one at
    /// (0,50) — which, if pooled with A's entries, would be the closest
    /// "Down" candidate from A's focused (0,0) entry. A properly-scoped
    /// navigate returns None (no A-local entry below) rather than window
    /// B's (0,50).
    #[test]
    fn spatial_navigate_from_window_a_cannot_return_window_b_candidates() {
        let app = build_test_app_with_windows(&["A", "B"]);

        invoke_in_window(
            &app,
            "A",
            "spatial_push_layer",
            json!({"key": "LA", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a1",
                    "moniker": "task:a1",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a2",
                    "moniker": "task:a2",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        // Window B: an entry geometrically below window A's focused entry.
        // If the registry were shared, navigate Down from a1 would pick b1.
        invoke_in_window(
            &app,
            "B",
            "spatial_push_layer",
            json!({"key": "LB", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "B",
            "spatial_register",
            json!({
                "args": {
                    "key": "b1",
                    "moniker": "task:b1",
                    "x": 0.0, "y": 50.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LB",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        invoke_in_window(&app, "A", "spatial_focus", json!({"key": "a1"}));

        // navigate Down from a1: window A has no entry below a1, window B
        // has b1 below — but b1 must not be a candidate. Expect null.
        let down = invoke_in_window(
            &app,
            "A",
            "spatial_navigate",
            json!({"key": "a1", "direction": "Down"}),
        );
        assert_eq!(
            down,
            serde_json::Value::Null,
            "spatial_navigate in window A must not reach window B's b1"
        );

        // And navigating Right from a1 should still work (a2 is in the same
        // window) — verifies the test isn't accidentally breaking
        // intra-window nav.
        let right = invoke_in_window(
            &app,
            "A",
            "spatial_navigate",
            json!({"key": "a1", "direction": "Right"}),
        );
        assert_eq!(
            right,
            json!("task:a2"),
            "intra-window right navigation must still work"
        );
    }

    /// `spatial_focus_first_in_layer` scopes its `focus-changed` emission
    /// to the invoking window — window A's listener sees the event,
    /// window B's does not. Exercises the full wire contract:
    ///
    /// - camelCase (`layerKey`) is honoured on the wire.
    /// - The command emits `focus-changed` with `prev_key: null`,
    ///   `next_key: <first entry's key>` when focus moves into the layer.
    /// - The emission goes to the invoking window only (`emit_to(label)`).
    #[test]
    fn spatial_focus_first_in_layer_emits_focus_changed_scoped_to_window() {
        let app = build_test_app_with_windows(&["A", "B"]);
        let events_a = capture_focus_events_on_window(&app, "A");
        let events_b = capture_focus_events_on_window(&app, "B");

        // Window A: push a layer and register two entries — left at (0,0)
        // and right at (200,0). The First sort order (y then x) picks the
        // left entry.
        invoke_in_window(
            &app,
            "A",
            "spatial_push_layer",
            json!({"key": "LA", "name": "window"}),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a-left",
                    "moniker": "task:left",
                    "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );
        invoke_in_window(
            &app,
            "A",
            "spatial_register",
            json!({
                "args": {
                    "key": "a-right",
                    "moniker": "task:right",
                    "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                    "layerKey": "LA",
                    "parentScope": null,
                    "overrides": null,
                }
            }),
        );

        // Window B: also push a layer so it has a listener target but is
        // otherwise untouched by window A's focus_first_in_layer call.
        invoke_in_window(
            &app,
            "B",
            "spatial_push_layer",
            json!({"key": "LB", "name": "window"}),
        );

        // Invoke the new command on window A with the camelCase wire form.
        invoke_in_window(
            &app,
            "A",
            "spatial_focus_first_in_layer",
            json!({"args": {"layerKey": "LA"}}),
        );

        let events_a = events_a.lock().unwrap();
        assert_eq!(
            events_a.len(),
            1,
            "window A should see exactly one focus-changed emission",
        );
        assert_eq!(events_a[0].prev_key, None);
        assert_eq!(
            events_a[0].next_key.as_deref(),
            Some("a-left"),
            "first entry by (y, x) should win",
        );

        let events_b = events_b.lock().unwrap();
        assert_eq!(
            events_b.len(),
            0,
            "window B must not see window A's focus-changed: {events_b:?}",
        );
    }
}
