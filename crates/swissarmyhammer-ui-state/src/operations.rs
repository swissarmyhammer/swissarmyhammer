//! The `#[operation]` structs that make up the `ui_state` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same `UI_STATE_OPERATIONS` slice via the `operation_tool!` macro, so the
//! two cannot drift.
//!
//! Every operation is a 1:1 port of a mutating method on
//! [`crate::state::UiState`]. They divide into five groups:
//!
//! - **inspector** — [`Inspect`], [`InspectorClose`], [`InspectorCloseAll`],
//!   [`InspectorSetWidth`].
//! - **palette** — [`PaletteOpen`], [`PaletteClose`].
//! - **keymap** — [`SetKeymapMode`] (covers `settings.keymap.vim` / `cua` /
//!   `emacs` via the `mode` param).
//! - **rename** — [`StartRename`] (backend no-op; exists so the palette can
//!   discover the command).
//! - **drag** — [`DragStart`], [`DragCancel`], [`DragComplete`].
//! - **app-ui toggles** — [`ShowCommand`], [`ShowPalette`], [`ShowSearch`],
//!   [`Dismiss`].
//!
//! There is deliberately **no** focus / `set_focus` operation here. Spatial
//! focus is owned by the separate `focus` MCP server; `UiState`'s
//! `set_scope_chain` setter is intentionally left unwrapped.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{notification, operation, Notification, Operation};

// Inspector operations ──────────────────────────────────────────────────

/// Open the inspector for a moniker in a window.
///
/// Ports [`crate::state::UiState::inspect`]. Pushes the moniker onto the
/// window's inspector stack (moving it to the top if already present).
///
/// Returns the new inspector stack: `{ ok: true, inspector_stack: [...] }`.
#[operation(
    verb = "inspect",
    noun = "inspector",
    description = "Open the inspector for a moniker in a window (push onto the inspector stack)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Inspect {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
    /// The `type:id` moniker to inspect (e.g. `"task:01XYZ"`).
    #[serde(default)]
    pub moniker: String,
}

/// Close the topmost inspector entry for a window.
///
/// Ports [`crate::state::UiState::inspector_close`]. No-op when the stack is
/// already empty.
///
/// Returns the new inspector stack (or `null` when nothing changed):
/// `{ ok: true, inspector_stack: [...] | null }`.
#[operation(
    verb = "close",
    noun = "inspector",
    description = "Close the topmost inspector entry for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorClose {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// Close all inspector entries for a window.
///
/// Ports [`crate::state::UiState::inspector_close_all`]. No-op when the stack
/// is already empty.
///
/// Returns the (now empty) inspector stack, or `null` when nothing changed.
#[operation(
    verb = "close_all",
    noun = "inspector",
    description = "Close all inspector entries for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorCloseAll {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// Persist the user-chosen inspector panel width for a window.
///
/// Ports [`crate::state::UiState::set_inspector_width`]. No-op when the width
/// is unchanged.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "set_width",
    noun = "inspector",
    description = "Persist the user-chosen inspector panel width for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct InspectorSetWidth {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
    /// The new inspector width in CSS pixels.
    #[serde(default)]
    pub width: u32,
}

// Palette operations ────────────────────────────────────────────────────

/// Open the command palette for a window in a given mode.
///
/// Ports [`crate::state::UiState::set_palette_open_with_mode`]. `mode` is
/// `"command"` or `"search"`. No-op when both flag and mode are unchanged.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "open",
    noun = "palette",
    description = "Open the command palette for a window in the given mode (command|search)"
)]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteOpen {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
    /// The palette mode: `"command"` (default) or `"search"`.
    #[serde(default = "default_palette_mode")]
    pub mode: String,
}

impl Default for PaletteOpen {
    fn default() -> Self {
        Self {
            scope_chain: Vec::new(),
            mode: default_palette_mode(),
        }
    }
}

/// Default palette mode used when the `mode` param is omitted.
fn default_palette_mode() -> String {
    "command".to_string()
}

/// Close the command palette for a window.
///
/// Ports [`crate::state::UiState::set_palette_open`] with `open = false`.
/// No-op when the palette is already closed.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "close",
    noun = "palette",
    description = "Close the command palette for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct PaletteClose {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

// Keymap operation ──────────────────────────────────────────────────────

/// Set the active keymap mode.
///
/// Ports [`crate::state::UiState::set_keymap_mode`]. The `mode` param covers
/// `settings.keymap.vim` / `cua` / `emacs`. No-op when unchanged. Persisted
/// to the config file.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "set",
    noun = "keymap",
    description = "Set the active keymap mode (cua|vim|emacs)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetKeymapMode {
    /// The keymap mode: `"cua"`, `"vim"`, or `"emacs"`.
    #[serde(default)]
    pub mode: String,
}

// Focus operation ─────────────────────────────────────────────────────────

/// Set the focus scope chain (the routing target for `app.setFocus`).
///
/// Ports [`crate::state::UiState::set_scope_chain`]. The frontend computes the
/// chain by walking the focus registry from the focused scope to the root
/// (leaf-first) and sends it on every focus change; recording it here is what
/// drives command gating's scope fallback and the `scope_chain` UI-state echo.
///
/// This is the UI-state scope chain — distinct from the spatial `focus` kernel,
/// which the separate `focus` MCP server owns. `app.setFocus` consumes the
/// `scope_chain` the frontend already sends; there is no separate `fq` to
/// supply (the focus target is the chain's leaf).
///
/// Returns the change payload: `{ ok: true, change: <UiStateChange> }`.
#[operation(
    verb = "set",
    noun = "scope_chain",
    description = "Set the focus scope chain (routing target for app.setFocus)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetScopeChain {
    /// The focus scope chain, leaf-first (the leaf is the focus target).
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

// Active-view operation ───────────────────────────────────────────────────

/// Set the active view for a window (the `view.set` command).
///
/// Ports [`crate::state::UiState::set_active_view`]: records the per-window
/// active view id AND rewrites every `view:*` moniker in the recorded focus
/// scope chain to point at the new view, so the palette / context menu keep
/// offering the right view-scoped commands until the next `app.setFocus`. The
/// target window is resolved from the scope chain's `window:` moniker — there
/// is no separate `window_label`.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "set",
    noun = "active_view",
    description = "Set the active view for a window (resolved from the scope chain)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetActiveView {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker, and its `view:*` monikers are rewritten to `view_id`.
    #[serde(default)]
    pub scope_chain: Vec<String>,
    /// The id of the view to make active.
    #[serde(default)]
    pub view_id: String,
}

// Rename operation ──────────────────────────────────────────────────────

/// Enter inline rename mode for the active perspective tab.
///
/// Backend no-op — the frontend intercepts this command before it reaches
/// the backend. Exists on the server so the command palette can discover it.
/// Mirrors the original `StartRenamePerspectiveCmd::execute`, which returns
/// `null`.
///
/// Returns `{ ok: true }`.
#[operation(
    verb = "start",
    noun = "rename",
    description = "Enter inline rename mode for the active perspective tab (frontend-handled no-op)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StartRename {
    /// The active scope chain (carried for parity with the frontend command;
    /// the backend does not read it).
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

// Drag operations ───────────────────────────────────────────────────────

/// Start a cross-window drag session.
///
/// Ports [`crate::state::UiState::start_drag`] (cancelling any existing
/// session first, as the original `DragStartCmd` does). Stores a focus-chain
/// drag session keyed on the source entity. Transient — not persisted.
///
/// Returns the stored session: `{ ok: true, session_id, ... }`.
#[operation(
    verb = "start",
    noun = "drag",
    description = "Start a cross-window drag session (focus-chain source)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DragStart {
    /// Unique session id (ULID) for the drag.
    #[serde(default)]
    pub session_id: String,
    /// The entity type being dragged (e.g. `"task"`).
    #[serde(default)]
    pub entity_type: String,
    /// The entity id (ULID) being dragged.
    #[serde(default)]
    pub entity_id: String,
    /// Filesystem path of the board the source entity belongs to.
    #[serde(default)]
    pub source_board_path: String,
    /// Window label of the source window.
    #[serde(default)]
    pub source_window_label: String,
    /// Whether Alt/Option was held (copy mode).
    #[serde(default)]
    pub copy_mode: bool,
    /// When the session started (epoch millis).
    #[serde(default)]
    pub started_at_ms: u64,
}

/// Cancel the active drag session.
///
/// Ports [`crate::state::UiState::cancel_drag`]. Clears the session without
/// returning it. Transient — not persisted.
///
/// Returns `{ ok: true }`.
#[operation(
    verb = "cancel",
    noun = "drag",
    description = "Cancel the active drag session"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DragCancel {}

/// Complete (take) the active drag session.
///
/// Ports [`crate::state::UiState::take_drag`]. Returns and clears the active
/// session. Transient — not persisted.
///
/// Returns `{ ok: true, session: <DragSession> | null }`.
#[operation(
    verb = "complete",
    noun = "drag",
    description = "Complete the active drag session, returning and clearing it"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct DragComplete {}

// App-UI toggle operations ──────────────────────────────────────────────

/// Open the command palette in `"command"` mode (the `app.command` toggle).
///
/// Ports the original `CommandPaletteCmd`:
/// `set_palette_open_with_mode(window, true, "command")`.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "show",
    noun = "command",
    description = "Open the command palette in command mode for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowCommand {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// Open the command palette for a window (the `app.palette` toggle).
///
/// Ports `set_palette_open(window, true)` — opens the palette without forcing
/// a mode change.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "show",
    noun = "palette",
    description = "Open the command palette for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowPalette {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// Open the command palette in `"search"` mode (the `app.search` toggle).
///
/// Ports the original `SearchPaletteCmd`:
/// `set_palette_open_with_mode(window, true, "search")`.
///
/// Returns the change payload, or `null` when nothing changed.
#[operation(
    verb = "show",
    noun = "search",
    description = "Open the command palette in search mode for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowSearch {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// Dismiss the topmost UI surface for a window (the `app.dismiss` toggle).
///
/// Ports the original `DismissCmd`: a layered close — close the palette if it
/// is open, otherwise pop the topmost inspector entry, otherwise no-op.
///
/// Returns the change payload, or `null` when nothing was dismissed.
#[operation(
    verb = "dismiss",
    noun = "ui",
    description = "Dismiss the topmost UI surface (palette first, then inspector) for a window"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Dismiss {
    /// The active scope chain; the target window is resolved from its
    /// `window:` moniker.
    #[serde(default)]
    pub scope_chain: Vec<String>,
}

/// All `ui_state` operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the discovery
/// `_meta` generator (`generate_operations_meta`) are driven from this single
/// slice via the `operation_tool!` macro, so there is one source of truth for
/// what the `ui_state` tool exposes. The `set scope_chain` op records the
/// UI-state focus scope chain (the `app.setFocus` routing target); it is NOT a
/// spatial focus op — the separate `focus` MCP server owns the focus kernel.
static UI_STATE_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<Inspect>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorClose>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorCloseAll>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorSetWidth>::default()) as &dyn Operation,
        Box::leak(Box::<PaletteOpen>::default()) as &dyn Operation,
        Box::leak(Box::<PaletteClose>::default()) as &dyn Operation,
        Box::leak(Box::<SetKeymapMode>::default()) as &dyn Operation,
        Box::leak(Box::<SetScopeChain>::default()) as &dyn Operation,
        Box::leak(Box::<SetActiveView>::default()) as &dyn Operation,
        Box::leak(Box::<StartRename>::default()) as &dyn Operation,
        Box::leak(Box::<DragStart>::default()) as &dyn Operation,
        Box::leak(Box::<DragCancel>::default()) as &dyn Operation,
        Box::leak(Box::<DragComplete>::default()) as &dyn Operation,
        Box::leak(Box::<ShowCommand>::default()) as &dyn Operation,
        Box::leak(Box::<ShowPalette>::default()) as &dyn Operation,
        Box::leak(Box::<ShowSearch>::default()) as &dyn Operation,
        Box::leak(Box::<Dismiss>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all `ui_state` operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &UI_STATE_OPERATIONS
}

// Notifications ──────────────────────────────────────────────────────────

/// The `notifications/ui_state/ai_streaming` event payload.
///
/// Reports whether the AI panel's conversation is currently streaming a turn,
/// so a subscriber can gate streaming-only behaviour without a synchronous
/// handle to the webview-owned conversation. The single declared subscriber is
/// the `ai-commands` builtin plugin, which caches `streaming` and returns it
/// from `ai.cancel`'s synchronous `available` callback (a generation can only
/// be stopped while one is in flight).
///
/// This struct is the single source of truth for the event: it IS the published
/// payload (it serializes to the notification's `params` via
/// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
/// AND the declaration the SDK reads (its fields drive the
/// `io.swissarmyhammer/notifications` `_meta`, resolved by
/// `this.ui_state.on("aiStreaming", …)`). The two cannot drift.
///
/// The webview is the source of truth for the live flag (`aiStreaming()` in
/// `apps/kanban-app/ui/src/ai/commands.ts`); the production publish point is the
/// `ai_set_streaming` Tauri command, which builds this payload and publishes it
/// onto the host's `NotificationBridge`. Provenance (`txn`/`origin`) is
/// universal cross-cutting metadata stamped at publish time; it is intentionally
/// NOT a field here.
///
/// The short event name is `"aiStreaming"` (an explicit override of the
/// method's last segment `"ai_streaming"`) so a plugin subscribes with
/// `this.ui_state.on("aiStreaming", …)` — the camelCase form matching the
/// webview's `aiStreaming()` reader.
#[notification(
    method = "notifications/ui_state/ai_streaming",
    event = "aiStreaming",
    description = "The AI panel conversation's streaming turn-status changed."
)]
#[derive(Debug, Default, Serialize)]
pub struct AiStreamingChanged {
    /// Whether the AI conversation is currently streaming a turn.
    pub streaming: bool,
}

/// The `notifications/ui_state/changed` event payload.
///
/// The single observable change-stream for ephemeral UI state — inspector
/// stack, palette open/mode, keymap mode, active view/perspective, app mode,
/// inspector width, and the atomic perspective+filter switch. A `kind`
/// discriminator names which slice changed; `state` carries the full
/// per-window-keyed UI-state snapshot after the change so a consumer self-selects
/// the slice it cares about (the webview reads only its own
/// `windows[<label>]`).
///
/// This struct is the single source of truth for the event: it IS the published
/// payload (it serializes to the notification's `params` via
/// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
/// AND the declaration the SDK reads (its fields drive the
/// `io.swissarmyhammer/notifications` `_meta`, resolved by
/// `this.ui_state.on("changed", …)`). The two cannot drift.
///
/// Carrying the already-computed snapshot is NOT an enrichment re-fetch: the
/// publisher has the snapshot in hand at publish time (the UI-state mutation
/// just produced it). Provenance (`txn`/`origin`) is universal cross-cutting
/// metadata stamped at publish time; it is intentionally NOT a field here —
/// ephemeral UI state is not undoable and carries no transaction.
///
/// # `kind` value-space
///
/// One discriminator per [`crate::state::UiStateChange`] variant the
/// production publish path classifies:
/// `scope_chain`, `palette_open`, `keymap_mode`, `inspector_stack`,
/// `active_view`, `active_perspective`, `app_mode`, `inspector_width`,
/// `perspective_switch`. The classification lives at the publish point (the
/// kanban app's UI-state side-effect) so this struct stays a thin, transport-
/// agnostic declaration.
#[notification(
    method = "notifications/ui_state/changed",
    description = "An ephemeral UI-state slice changed (palette, inspector, keymap, active view/perspective, app mode)."
)]
#[derive(Debug, Default, Serialize)]
pub struct UiStateChanged {
    /// Which UI-state slice changed — one discriminator per
    /// [`crate::state::UiStateChange`] variant the publisher classifies
    /// (e.g. `"palette_open"`, `"inspector_stack"`, `"perspective_switch"`).
    pub kind: String,
    /// The full per-window-keyed UI-state snapshot after the change.
    ///
    /// A consumer self-selects the slice it cares about (the webview reads only
    /// its own `windows[<label>]`); carrying the whole snapshot avoids a
    /// follow-up read.
    pub state: serde_json::Value,
}

/// The `notifications/ui_state/drag_started` event payload.
///
/// Announces that the cross-window drag state machine started a session — the
/// observable form of [`crate::state::UiState::start_drag`]. The drag machine
/// lives in this `ui_state` service, so the lifecycle is declared here beside
/// it (the same way `ai_streaming` is, distinct from the per-slice `changed`
/// stream).
///
/// The payload IS the full session wire shape the frontend's `DragSession`
/// consumes — the exact fields the legacy direct `drag-session-active` Tauri
/// event carried, so the only change for an existing focus-chain task drag is
/// the transport. `from` is the discriminated-union source envelope mirroring
/// [`crate::state::DragSource`]; the flat `task_*` fields stay for focus-chain
/// listeners that read them.
///
/// This struct is the single source of truth: it IS the published payload (via
/// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
/// AND the declaration the SDK reads (its fields drive the
/// `io.swissarmyhammer/notifications` `_meta`, resolved by
/// `this.ui_state.on("drag_started", …)`). The two cannot drift. Provenance
/// (`txn`/`origin`) is stamped at publish time and is intentionally NOT a field
/// — a transient drag carries no transaction.
#[notification(
    method = "notifications/ui_state/drag_started",
    description = "A cross-window drag session started (the drag state machine stored a source session)."
)]
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct DragStarted {
    /// Unique session id (ULID) for the drag.
    pub session_id: String,
    /// Filesystem path of the source board (empty for file drags).
    pub source_board_path: String,
    /// Tauri window label of the source window.
    pub source_window_label: String,
    /// The dragged task id (empty for file drags).
    pub task_id: String,
    /// Serialized source entity field snapshot for ghost preview.
    pub task_fields: serde_json::Value,
    /// Whether Alt/Option was held (copy mode).
    pub copy_mode: bool,
    /// When the session started (epoch millis).
    pub started_at_ms: u64,
    /// Discriminated-union drag source mirroring [`crate::state::DragSource`].
    pub from: serde_json::Value,
}

/// The `notifications/ui_state/drag_cancelled` event payload.
///
/// Announces that the active drag session was cancelled — the observable form
/// of [`crate::state::UiState::cancel_drag`] / `take_drag` on the cancel path.
/// Carries only the `session_id`, exactly as the legacy direct
/// `drag-session-cancelled` Tauri event did, so a subscriber can clear the
/// matching local session.
///
/// Single source of truth for the event (see [`DragStarted`]); resolved by
/// `this.ui_state.on("drag_cancelled", …)`.
#[notification(
    method = "notifications/ui_state/drag_cancelled",
    description = "The active cross-window drag session was cancelled."
)]
#[derive(Debug, Default, Serialize)]
pub struct DragCancelled {
    /// The cancelled session's id (ULID).
    pub session_id: String,
}

/// The `notifications/ui_state/drag_completed` event payload.
///
/// Announces that the active drag session was completed (dropped) — the
/// observable form of [`crate::state::UiState::take_drag`] on the complete
/// path, after the drop's side-effects (same-board flush or cross-board
/// transfer) ran. Carries the `session_id` and a `success` flag, exactly as the
/// legacy direct `drag-session-completed` Tauri event did.
///
/// Single source of truth for the event (see [`DragStarted`]); resolved by
/// `this.ui_state.on("drag_completed", …)`.
#[notification(
    method = "notifications/ui_state/drag_completed",
    description = "A cross-window drag session completed (the drop's side-effects ran)."
)]
#[derive(Debug, Default, Serialize)]
pub struct DragCompleted {
    /// The completed session's id (ULID).
    pub session_id: String,
    /// Whether the drop's side-effects (transfer / flush) succeeded.
    pub success: bool,
}

/// The canonical slice of notifications the `ui_state` tool emits.
///
/// Mirrors [`operations`]: a leaked `Default` instance per notification, used
/// only for its static metadata. Fed to `operation_tool!`'s `notifications:`
/// field so the tool advertises its events in `_meta` and `.on()` can resolve
/// them.
static UI_STATE_NOTIFICATIONS: LazyLock<Vec<&'static dyn Notification>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<AiStreamingChanged>::default()) as &dyn Notification,
        Box::leak(Box::<UiStateChanged>::default()) as &dyn Notification,
        Box::leak(Box::<DragStarted>::default()) as &dyn Notification,
        Box::leak(Box::<DragCancelled>::default()) as &dyn Notification,
        Box::leak(Box::<DragCompleted>::default()) as &dyn Notification,
    ]
});

/// Get the canonical slice of all `ui_state` notifications.
pub fn notifications() -> &'static [&'static dyn Notification] {
    &UI_STATE_NOTIFICATIONS
}

/// Build the `notifications/ui_state/ai_streaming` notification for `streaming`.
///
/// The single production publish helper: serializes the declared
/// [`AiStreamingChanged`] payload and stamps `user` provenance. Lives here, in
/// the crate that DECLARES the notification, so the wire method comes from the
/// `#[notification]` attribute (via the [`Notification`] trait) rather than
/// being repeated as a string literal at the call site — the kanban app's
/// `ai_set_streaming` Tauri command calls this so the declared schema and the
/// published payload cannot drift.
pub fn ai_streaming_notification(streaming: bool) -> swissarmyhammer_plugin::McpNotification {
    let payload = AiStreamingChanged { streaming };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/ui_state/changed` notification for `kind` + `state`.
///
/// The single production publish helper: serializes the declared
/// [`UiStateChanged`] payload (so the `_meta` schema and the wire payload share
/// one source) and stamps `user` provenance. `kind` is the slice discriminator
/// classified at the publish point from a [`crate::state::UiStateChange`]; `state`
/// is the full UI-state snapshot after the change (`UiState::to_json()`).
///
/// Lives here, in the crate that DECLARES the notification, so the wire method
/// comes from the `#[notification]` attribute (via the [`Notification`] trait)
/// rather than being repeated as a string literal at the call site — the kanban
/// app's UI-state side-effect calls this so the declared schema and the
/// published payload cannot drift.
pub fn ui_state_changed_notification(
    kind: impl Into<String>,
    state: serde_json::Value,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = UiStateChanged {
        kind: kind.into(),
        state,
    };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/ui_state/drag_started` notification from the drag
/// session wire `Value` (the full [`DragStarted`] shape).
///
/// The single production publish helper for the drag-start lifecycle event: it
/// reshapes the already-built session wire payload (the same `Value` the legacy
/// `drag-session-active` Tauri event carried) into the declared [`DragStarted`]
/// struct and stamps `user` provenance. Lives here, in the crate that DECLARES
/// the notification, so the wire method comes from the `#[notification]`
/// attribute rather than a string literal at the call site — the kanban app's
/// drag side-effect calls this so the declared schema and the published payload
/// cannot drift.
pub fn drag_started_notification(
    session: serde_json::Value,
) -> swissarmyhammer_plugin::McpNotification {
    let payload: DragStarted = serde_json::from_value(session).unwrap_or_else(|error| {
        // The session wire shape is built by the app's drag side-effect from the
        // live `DragStart` envelope and round-trips cleanly in production; a
        // deserialization failure means a malformed envelope reached the publish
        // path, so log it rather than silently publishing an empty-session event.
        tracing::warn!(%error, "drag_started: malformed session payload, publishing empty drag_started");
        DragStarted::default()
    });
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/ui_state/drag_cancelled` notification for a session.
///
/// The single production publish helper for the drag-cancel lifecycle event:
/// serializes the declared [`DragCancelled`] payload and stamps `user`
/// provenance, so the `_meta` schema and the wire payload share one source.
pub fn drag_cancelled_notification(
    session_id: impl Into<String>,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = DragCancelled {
        session_id: session_id.into(),
    };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/ui_state/drag_completed` notification for a session.
///
/// The single production publish helper for the drag-complete lifecycle event:
/// serializes the declared [`DragCompleted`] payload (session id + `success`
/// flag) and stamps `user` provenance, so the `_meta` schema and the wire
/// payload share one source.
pub fn drag_completed_notification(
    session_id: impl Into<String>,
    success: bool,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = DragCompleted {
        session_id: session_id.into(),
        success,
    };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_operations::generate_notifications_meta;

    /// The `aiStreaming` notification declares the wire method and the
    /// camelCase short event a plugin subscribes to with
    /// `this.ui_state.on("aiStreaming", …)`.
    #[test]
    fn ai_streaming_notification_declares_method_and_event() {
        let note = AiStreamingChanged::default();
        assert_eq!(note.method(), "notifications/ui_state/ai_streaming");
        assert_eq!(
            note.event(),
            "aiStreaming",
            "the short event must be the camelCase override, not the method's \
             last segment, so the plugin subscribes with `.on(\"aiStreaming\")`"
        );
    }

    /// The notification serializes to its declared `params` shape — a single
    /// `streaming` boolean — so `from_declared` produces the right payload.
    #[test]
    fn ai_streaming_payload_serializes_to_streaming_flag() {
        let value = serde_json::to_value(AiStreamingChanged { streaming: true })
            .expect("AiStreamingChanged serializes");
        assert_eq!(value, serde_json::json!({ "streaming": true }));
    }

    /// The notification appears in the generated `io.swissarmyhammer/notifications`
    /// `_meta` tree under its short event name with its wire method — the
    /// discovery surface `this.ui_state.on(...)` resolves against.
    #[test]
    fn ui_state_notifications_meta_advertises_ai_streaming() {
        let meta = generate_notifications_meta(notifications());
        let obj = meta.as_object().expect("notifications meta is an object");
        let leaf = obj
            .get("aiStreaming")
            .expect("aiStreaming event must be declared in the _meta tree");
        assert_eq!(leaf["method"], "notifications/ui_state/ai_streaming");
    }

    /// The `changed` notification declares the wire method and the short event
    /// a plugin subscribes to with `this.ui_state.on("changed", …)` — the
    /// method's last segment, so no `event` override is needed.
    #[test]
    fn ui_state_changed_notification_declares_method_and_event() {
        let note = UiStateChanged::default();
        assert_eq!(note.method(), "notifications/ui_state/changed");
        assert_eq!(
            note.event(),
            "changed",
            "the short event must be the method's last segment so the plugin \
             subscribes with `.on(\"changed\")`"
        );
    }

    /// The notification serializes to its declared `params` shape — a `kind`
    /// discriminator plus the full UI-state `state` snapshot — so
    /// `from_declared` produces the right payload.
    #[test]
    fn ui_state_changed_payload_serializes_to_kind_and_state() {
        let payload = UiStateChanged {
            kind: "palette_open".to_string(),
            state: serde_json::json!({ "keymap_mode": "vim" }),
        };
        let value = serde_json::to_value(&payload).expect("UiStateChanged serializes");
        assert_eq!(
            value,
            serde_json::json!({
                "kind": "palette_open",
                "state": { "keymap_mode": "vim" },
            })
        );
    }

    /// The `changed` notification appears in the generated `_meta` tree under
    /// its short event name with its wire method.
    #[test]
    fn ui_state_notifications_meta_advertises_changed() {
        let meta = generate_notifications_meta(notifications());
        let obj = meta.as_object().expect("notifications meta is an object");
        let leaf = obj
            .get("changed")
            .expect("changed event must be declared in the _meta tree");
        assert_eq!(leaf["method"], "notifications/ui_state/changed");
    }

    /// Coverage guard (declared ⟺ raised). The method the production helper
    /// actually publishes MUST be one the `ui_state` service declares — so
    /// `ui_state/changed` can never be raised without appearing in `_meta`.
    #[test]
    fn ui_state_changed_emitted_method_is_declared() {
        let note = ui_state_changed_notification("palette_open", serde_json::json!({ "a": 1 }));
        let declared: std::collections::BTreeSet<String> =
            generate_notifications_meta(notifications())
                .as_object()
                .expect("notifications meta is an object")
                .values()
                .map(|leaf| {
                    leaf["method"]
                        .as_str()
                        .expect("each notification leaf carries a method")
                        .to_string()
                })
                .collect();
        assert!(
            declared.contains(&note.method),
            "emitted method {:?} is not declared in _meta ({:?})",
            note.method,
            declared,
        );
    }

    /// The production helper builds the `{ kind, state }` payload under the
    /// declared method — the struct=payload publish path.
    #[test]
    fn ui_state_changed_notification_builds_kind_and_state_payload() {
        let note =
            ui_state_changed_notification("active_view", serde_json::json!({ "windows": {} }));
        assert_eq!(note.method, "notifications/ui_state/changed");
        let params = note.params.as_object().expect("params is an object");
        assert_eq!(params["kind"], "active_view");
        assert_eq!(params["state"], serde_json::json!({ "windows": {} }));
    }

    /// Real-pipeline loop: a genuine `UiState` mutation → its `UiStateChange`
    /// classification + full snapshot → the declared notification → a real
    /// `NotificationBridge` → a live subscriber. No mock boundary: the same
    /// path the app's UI-state side-effect drives, minus the Tauri shell.
    ///
    /// Proves a plugin doing `this.ui_state.on("changed", cb)` receives the
    /// real `{ kind, state }` carrying the mutated slice.
    #[tokio::test]
    async fn mutation_publishes_ui_state_changed_on_the_bridge() {
        use crate::state::UiState;
        use swissarmyhammer_plugin::notify::NotificationBridge;

        let ui_state = UiState::new();
        let bridge = NotificationBridge::new();
        let mut sub = bridge.subscribe();

        // A real mutation produces a typed change…
        let change = ui_state
            .set_palette_open("main", true)
            .expect("opening the palette is a real change");

        // …which the publish path classifies and snapshots, then publishes the
        // declared notification onto the bridge (exactly as the app side-effect
        // does).
        let note = ui_state_changed_notification(change.kind(), ui_state.to_json());
        let reached = bridge.publish(note);
        assert_eq!(reached, 1, "the live subscriber must receive the publish");

        // The subscriber (a plugin's `.on(\"changed\")`) sees the real payload.
        let received = sub
            .recv()
            .await
            .expect("subscriber receives the notification");
        assert_eq!(received.method, "notifications/ui_state/changed");
        assert_eq!(received.params["kind"], "palette_open");
        assert_eq!(
            received.params["state"]["windows"]["main"]["palette_open"], true,
            "the published snapshot carries the mutated slice"
        );
        // Ephemeral UI state is not undoable → no transaction.
        assert_eq!(received.params["txn"], serde_json::Value::Null);
        assert_eq!(received.params["origin"], "user");
    }

    // Drag lifecycle notifications ──────────────────────────────────────

    /// Each drag-lifecycle notification declares its wire method and the short
    /// event a plugin subscribes to with `this.ui_state.on("drag_started", …)`
    /// — the method's last segment, so no `event` override is needed.
    #[test]
    fn drag_notifications_declare_method_and_event() {
        assert_eq!(
            DragStarted::default().method(),
            "notifications/ui_state/drag_started"
        );
        assert_eq!(DragStarted::default().event(), "drag_started");
        assert_eq!(
            DragCancelled::default().method(),
            "notifications/ui_state/drag_cancelled"
        );
        assert_eq!(DragCancelled::default().event(), "drag_cancelled");
        assert_eq!(
            DragCompleted::default().method(),
            "notifications/ui_state/drag_completed"
        );
        assert_eq!(DragCompleted::default().event(), "drag_completed");
    }

    /// The `drag_started` payload serializes to the full session wire shape the
    /// frontend's `DragSession` consumes — the same fields the direct
    /// `drag-session-active` event carried.
    #[test]
    fn drag_started_payload_serializes_to_session_wire_shape() {
        let payload = DragStarted {
            session_id: "sess-1".to_string(),
            source_board_path: "/board/a".to_string(),
            source_window_label: "main".to_string(),
            task_id: "task-1".to_string(),
            task_fields: serde_json::json!({ "title": "x" }),
            copy_mode: true,
            started_at_ms: 42,
            from: serde_json::json!({ "kind": "focus_chain", "entity_id": "task-1" }),
        };
        let value = serde_json::to_value(&payload).expect("DragStarted serializes");
        assert_eq!(
            value,
            serde_json::json!({
                "session_id": "sess-1",
                "source_board_path": "/board/a",
                "source_window_label": "main",
                "task_id": "task-1",
                "task_fields": { "title": "x" },
                "copy_mode": true,
                "started_at_ms": 42,
                "from": { "kind": "focus_chain", "entity_id": "task-1" },
            })
        );
    }

    /// The `drag_cancelled` payload serializes to `{ session_id }` — the same
    /// shape the direct `drag-session-cancelled` event carried.
    #[test]
    fn drag_cancelled_payload_serializes_to_session_id() {
        let value = serde_json::to_value(DragCancelled {
            session_id: "sess-1".to_string(),
        })
        .expect("DragCancelled serializes");
        assert_eq!(value, serde_json::json!({ "session_id": "sess-1" }));
    }

    /// The `drag_completed` payload serializes to `{ session_id, success }` —
    /// the same shape the direct `drag-session-completed` event carried.
    #[test]
    fn drag_completed_payload_serializes_to_session_id_and_success() {
        let value = serde_json::to_value(DragCompleted {
            session_id: "sess-1".to_string(),
            success: true,
        })
        .expect("DragCompleted serializes");
        assert_eq!(
            value,
            serde_json::json!({ "session_id": "sess-1", "success": true })
        );
    }

    /// Each drag-lifecycle notification appears in the generated
    /// `io.swissarmyhammer/notifications` `_meta` tree under its short event
    /// name with its wire method — the surface `this.ui_state.on(...)` resolves.
    #[test]
    fn ui_state_notifications_meta_advertises_drag_lifecycle() {
        let meta = generate_notifications_meta(notifications());
        let obj = meta.as_object().expect("notifications meta is an object");
        for (event, method) in [
            ("drag_started", "notifications/ui_state/drag_started"),
            ("drag_cancelled", "notifications/ui_state/drag_cancelled"),
            ("drag_completed", "notifications/ui_state/drag_completed"),
        ] {
            let leaf = obj
                .get(event)
                .unwrap_or_else(|| panic!("{event} must be declared in the _meta tree"));
            assert_eq!(leaf["method"], method);
        }
    }

    /// Coverage guard (declared ⟺ raised). Every method the drag publish helpers
    /// actually publish MUST be one the `ui_state` service declares — so no drag
    /// notification can be raised without appearing in `_meta`.
    #[test]
    fn drag_emitted_methods_are_declared() {
        let declared: std::collections::BTreeSet<String> =
            generate_notifications_meta(notifications())
                .as_object()
                .expect("notifications meta is an object")
                .values()
                .map(|leaf| {
                    leaf["method"]
                        .as_str()
                        .expect("each notification leaf carries a method")
                        .to_string()
                })
                .collect();
        for note in [
            drag_started_notification(serde_json::json!({ "session_id": "s" })),
            drag_cancelled_notification("s"),
            drag_completed_notification("s", true),
        ] {
            assert!(
                declared.contains(&note.method),
                "emitted method {:?} is not declared in _meta ({:?})",
                note.method,
                declared,
            );
        }
    }

    /// The drag publish helpers build the declared payloads under the declared
    /// methods — the struct=payload publish path.
    #[test]
    fn drag_notification_helpers_build_declared_payloads() {
        let started = drag_started_notification(serde_json::json!({
            "session_id": "sess-1",
            "source_window_label": "main",
        }));
        assert_eq!(started.method, "notifications/ui_state/drag_started");
        assert_eq!(started.params["session_id"], "sess-1");
        assert_eq!(started.params["source_window_label"], "main");

        let cancelled = drag_cancelled_notification("sess-1");
        assert_eq!(cancelled.method, "notifications/ui_state/drag_cancelled");
        assert_eq!(cancelled.params["session_id"], "sess-1");

        let completed = drag_completed_notification("sess-1", false);
        assert_eq!(completed.method, "notifications/ui_state/drag_completed");
        assert_eq!(completed.params["session_id"], "sess-1");
        assert_eq!(completed.params["success"], false);
    }

    /// Real-pipeline loop: a genuine `UiState` drag mutation → the declared
    /// notification built from the live session → a real `NotificationBridge` →
    /// a live subscriber. No mock boundary: the same path the app's drag
    /// side-effect drives, minus the Tauri shell.
    ///
    /// Proves a plugin doing `this.ui_state.on("drag_started", cb)` receives the
    /// real session payload, and that cancelling then publishes `drag_cancelled`.
    #[tokio::test]
    async fn drag_transitions_publish_on_the_bridge() {
        use crate::state::{DragSession, DragSource, UiState};
        use swissarmyhammer_plugin::notify::NotificationBridge;

        let ui_state = UiState::new();
        let bridge = NotificationBridge::new();
        let mut sub = bridge.subscribe();

        // A real start_drag mutation stores a session…
        let session = DragSession {
            session_id: "sess-1".to_string(),
            from: DragSource::FocusChain {
                entity_type: "task".to_string(),
                entity_id: "task-1".to_string(),
                fields: serde_json::json!({}),
                source_board_path: "/board/a".to_string(),
                source_window_label: "main".to_string(),
            },
            copy_mode: false,
            started_at_ms: 7,
        };
        ui_state.start_drag(session.clone());

        // The publish path reads the session back OUT of the mutated `UiState`
        // (never off the local `session` variable) and reshapes it into the
        // `DragStarted` wire payload — exactly as the app's drag side-effect
        // builds the notification from the live `DragStart` envelope. Reading
        // back through `drag_session()` is what couples this loop to the real
        // mutation: if `start_drag` were a no-op the read-back would be `None`
        // and the test would fail here rather than silently restating its input.
        let stored = ui_state
            .drag_session()
            .expect("start_drag stored the session");
        let started = drag_started_notification(serde_json::json!({
            "session_id": stored.session_id,
            "source_board_path": stored.source_board_path(),
            "source_window_label": stored.source_window_label(),
            "task_id": stored.entity_id(),
            "copy_mode": stored.copy_mode,
            "started_at_ms": stored.started_at_ms,
            "from": stored.from,
        }));
        assert_eq!(
            bridge.publish(started),
            1,
            "subscriber receives the publish"
        );
        let received = sub.recv().await.expect("subscriber receives the start");
        assert_eq!(received.method, "notifications/ui_state/drag_started");
        assert_eq!(received.params["session_id"], "sess-1");
        assert_eq!(received.params["source_board_path"], "/board/a");
        assert_eq!(received.params["source_window_label"], "main");
        assert_eq!(received.params["task_id"], "task-1");
        assert_eq!(received.params["started_at_ms"], 7);
        assert_eq!(received.params["origin"], "user");

        // A real cancel_drag clears the session and publishes drag_cancelled —
        // the cancelled payload's `session_id` is read back from the session the
        // mutation actually stored, so a no-op `cancel_drag` would leave the
        // session present and fail the `is_none` assertion.
        ui_state.cancel_drag();
        assert!(ui_state.drag_session().is_none());
        let cancelled = drag_cancelled_notification(stored.session_id);
        assert_eq!(bridge.publish(cancelled), 1);
        let received = sub.recv().await.expect("subscriber receives the cancel");
        assert_eq!(received.method, "notifications/ui_state/drag_cancelled");
        assert_eq!(received.params["session_id"], "sess-1");
    }
}
