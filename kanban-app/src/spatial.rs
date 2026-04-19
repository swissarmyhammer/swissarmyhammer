//! Tauri commands for spatial focus management.
//!
//! These are transient UI plumbing commands — they manage the spatial entry
//! registry and focused key state. They do NOT flow through `dispatch_command`
//! (no undo/redo, no persistence, no command logging).

use std::collections::HashMap;

use crate::state::AppState;
use serde::Deserialize;
use swissarmyhammer_spatial_nav::{BatchEntry, Direction, Rect};
use tauri::{AppHandle, Emitter, Runtime, State};

/// Register a spatial entry (FocusScope mount or ResizeObserver update).
///
/// Called by React when a FocusScope mounts or its rect changes. The
/// spatial key is a ULID generated client-side, stable across re-renders,
/// unique per mount. The optional `overrides` map allows per-entry
/// navigation redirection or blocking by direction string.
// Tauri command signatures are the frontend API contract — each JS-side
// arg is one parameter here. Refactoring to a struct would require changes
// on the React side that aren't in scope.
#[allow(clippy::too_many_arguments)]
#[tauri::command]
pub async fn spatial_register(
    key: String,
    moniker: String,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    layer_key: String,
    parent_scope: Option<String>,
    overrides: Option<HashMap<String, Option<String>>>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.spatial_state.register(
        key,
        moniker,
        Rect {
            x,
            y,
            width: w,
            height: h,
        },
        layer_key,
        parent_scope,
        overrides.unwrap_or_default(),
    );
    Ok(())
}

/// Unregister a spatial entry (FocusScope unmount).
///
/// If the unregistered entry was the focused key, focus is cleared and a
/// `focus-changed` event is emitted.
#[tauri::command]
pub async fn spatial_unregister<R: Runtime>(
    key: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.unregister(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Set focus to a spatial key (click or programmatic).
///
/// Updates the focused key and emits a `focus-changed` event if the focus
/// actually changed. No-op if the key is already focused.
#[tauri::command]
pub async fn spatial_focus<R: Runtime>(
    key: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.focus(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Clear focus without removing any entry.
///
/// Called when React clears focus (e.g. `setFocus(null)`). Emits a
/// `focus-changed` event if something was previously focused.
#[tauri::command]
pub async fn spatial_clear_focus<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.clear_focus() {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Navigate from a key in a direction using beam test + scoring.
///
/// Filters to the active layer, applies container-first search, and
/// emits a `focus-changed` event if focus moves. Returns the moniker
/// of the newly focused entry, or `None` if no target was found.
#[tauri::command]
pub async fn spatial_navigate<R: Runtime>(
    key: String,
    direction: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<Option<String>, String> {
    let dir: Direction = direction
        .parse()
        .map_err(|e: swissarmyhammer_spatial_nav::ParseDirectionError| e.to_string())?;
    match state.spatial_state.navigate(&key, dir)? {
        Some(event) => {
            let next = event.next_key.clone();
            let _ = app.emit("focus-changed", &event);
            Ok(next.and_then(|k| state.spatial_state.get(&k).map(|e| e.moniker)))
        }
        None => Ok(None),
    }
}

/// Push a focus layer onto the layer stack (FocusLayer mount).
///
/// The active (topmost) layer determines which entries are visible to
/// `spatial_navigate`.
#[tauri::command]
pub async fn spatial_push_layer(
    key: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<(), String> {
    state.spatial_state.push_layer(key, name);
    Ok(())
}

/// Remove a focus layer from the layer stack by key (FocusLayer unmount).
///
/// Removal is by key, not pop — supports out-of-order unmount. If the
/// layer below has a `last_focused` key, focus is restored and a
/// `focus-changed` event is emitted.
#[tauri::command]
pub async fn spatial_remove_layer<R: Runtime>(
    key: String,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.remove_layer(&key) {
        let _ = app.emit("focus-changed", &event);
    }
    Ok(())
}

/// Wire format for a single entry in a batch registration call.
///
/// Mirrors the fields of `spatial_register` but packed into a struct so the
/// frontend can send an array in one invoke.
#[derive(Debug, Deserialize)]
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
    /// Spatial key of the FocusLayer this scope lives in.
    pub layer_key: String,
    /// Optional parent scope key for container-first navigation.
    pub parent_scope: Option<String>,
    /// Directional navigation overrides.
    pub overrides: Option<HashMap<String, Option<String>>>,
}

/// Register multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer to register estimated rects for off-screen items.
/// Each entry is an upsert — overwrites any existing entry with the same key.
#[tauri::command]
pub async fn spatial_register_batch(
    entries: Vec<BatchEntryPayload>,
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
    state.spatial_state.register_batch(batch);
    Ok(())
}

/// Unregister multiple spatial entries in a single Tauri invoke.
///
/// Used by the virtualizer on unmount to clean up placeholder entries.
/// If the focused key was among those removed, a `focus-changed` event
/// is emitted.
#[tauri::command]
pub async fn spatial_unregister_batch<R: Runtime>(
    keys: Vec<String>,
    app: AppHandle<R>,
    state: State<'_, AppState>,
) -> Result<(), String> {
    if let Some(event) = state.spatial_state.unregister_batch(&keys) {
        let _ = app.emit("focus-changed", &event);
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

    /// Dump the full spatial state for test assertions.
    ///
    /// Only compiled into debug builds (this entire module is gated by
    /// `#[cfg(debug_assertions)]`). Registered via `kanban_invoke_handler!`
    /// in `main.rs`, which drops the identifier from the handler list in
    /// release builds — so there is no way to invoke this command from a
    /// production binary.
    #[tauri::command]
    pub async fn __spatial_dump(state: State<'_, AppState>) -> Result<SpatialDump, String> {
        let entries = state.spatial_state.entries_snapshot();
        let layers = state.spatial_state.layers_snapshot();
        let focused_key = state.spatial_state.focused_key();

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
    /// through the `AppHandle`'s `Listener` impl and does not require a
    /// running event loop — emissions land synchronously in the listener's
    /// closure before `emit()` returns.
    fn build_test_app() -> tauri::App<MockRuntime> {
        let app = mock_builder()
            .invoke_handler(tauri::generate_handler![
                spatial_register,
                spatial_register_batch,
                spatial_unregister,
                spatial_unregister_batch,
                spatial_focus,
                spatial_clear_focus,
                spatial_navigate,
                spatial_push_layer,
                spatial_remove_layer,
            ])
            .build(tauri::test::mock_context(noop_assets()))
            .expect("failed to build mock Tauri app");

        app.manage(AppState::new_for_test());

        // Ensure a webview exists — `get_ipc_response` dispatches through it.
        WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .expect("failed to build mock webview");

        app
    }

    /// Invoke a `#[tauri::command]` by name and return the parsed JSON
    /// response.
    ///
    /// Panics with a loud message on IPC failure, because a failing command
    /// in one of these tests means the serde wire format broke — and we want
    /// that to be the first failure the engineer sees, not a silent `None`.
    fn invoke(
        app: &tauri::App<MockRuntime>,
        cmd: &str,
        payload: serde_json::Value,
    ) -> serde_json::Value {
        let webview = app
            .get_webview_window("main")
            .expect("main webview present");
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
            Err(e) => panic!("IPC call to {cmd} failed: {e}"),
        }
    }

    /// Subscribe to `focus-changed` events before any command is invoked.
    ///
    /// Returns an `Arc<Mutex<Vec<FocusChanged>>>` that accumulates one entry
    /// per emission, in order. The listener decodes each event payload from
    /// JSON — that is the core assertion target for these tests, since a
    /// field rename in the `FocusChanged` struct would break deserialisation
    /// and surface as a panic here.
    fn capture_focus_events(
        app: &tauri::App<MockRuntime>,
    ) -> Arc<Mutex<Vec<swissarmyhammer_spatial_nav::FocusChanged>>> {
        let events: Arc<Mutex<Vec<swissarmyhammer_spatial_nav::FocusChanged>>> =
            Arc::new(Mutex::new(Vec::new()));
        let events_cl = Arc::clone(&events);
        app.listen("focus-changed", move |ev| {
            let parsed: swissarmyhammer_spatial_nav::FocusChanged =
                serde_json::from_str(ev.payload())
                    .expect("focus-changed payload deserializes as FocusChanged");
            events_cl.lock().unwrap().push(parsed);
        });
        events
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
                "key": "k1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "key": "k2",
                "moniker": "task:B",
                "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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
                "key": "k1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": {
                    "Right": "task:C",
                    "Down": null,
                },
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "key": "k2",
                "moniker": "task:B",
                "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "key": "k3",
                "moniker": "task:C",
                "x": 0.0, "y": 200.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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
                "key": "b1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "key": "b2",
                "moniker": "task:B",
                "x": 200.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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
                "key": "f1",
                "moniker": "field:title",
                "x": 500.0, "y": 0.0, "w": 100.0, "h": 30.0,
                "layerKey": "L2",
                "parentScope": null,
                "overrides": null,
            }),
        );
        invoke(
            &app,
            "spatial_register",
            json!({
                "key": "f2",
                "moniker": "field:desc",
                "x": 500.0, "y": 40.0, "w": 100.0, "h": 30.0,
                "layerKey": "L2",
                "parentScope": null,
                "overrides": null,
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
        assert_eq!(
            state.spatial_state.focused_key().as_deref(),
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
                "key": "k1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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
                "key": "k1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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

        let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
        let captured_cl = Arc::clone(&captured);
        app.listen("focus-changed", move |ev| {
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
                "key": "k1",
                "moniker": "task:A",
                "x": 0.0, "y": 0.0, "w": 100.0, "h": 50.0,
                "layerKey": "L1",
                "parentScope": null,
                "overrides": null,
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
}
