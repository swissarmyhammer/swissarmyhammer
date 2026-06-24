//! The `#[operation]` structs that make up the `window` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the same
//! `WINDOW_OPERATIONS` slice via the `operation_tool!` macro, so the two cannot
//! drift.
//!
//! Operations divide into three groups:
//!
//! - **window** — window-manager actions: `OpenNewWindow` (ports the original
//!   `create_window` Tauri command), plus the net-new `ActivateWindow`,
//!   `SetWindowPosition`, `GetWindowPosition`, `GetMonitors`, and `CloseWindow`.
//! - **OS file actions** — `OpenPath` (open a file in the OS default app, backs
//!   `attachment.open`) and `RevealPath` (reveal a file in the OS file manager,
//!   backs `attachment.reveal`). These relocate the direct OS calls that lived
//!   inside the kanban attachment command path.
//!
//! - **board lifecycle** — `SwitchBoard` (backs `file.switchBoard`, wraps
//!   `AppState::open_board`), `CloseBoard` (backs `file.closeBoard`, wraps
//!   `AppState::close_board`), `NewBoard` (backs `file.newBoard`, ports the
//!   `new_board_dialog` folder-picker path), and `OpenBoard` (backs
//!   `file.openBoard`, ports the `open_board_dialog` folder-picker path).
//!
//! - **board-management reads** — `ListOpenBoards` (ports `list_open_boards`,
//!   enumerates the open-board set marking the active board) and `GetBoardData`
//!   (ports `get_board_data`, projects one board's aggregate summary). These
//!   ride the app-wide `window` server alongside the board-lifecycle writes: the
//!   server already owns the full open/close/new/switch board lifecycle and is
//!   AppHandle-backed, so the read counterparts (`list` / `get`) belong here too.
//!   The per-board `entity` server cannot host them — they span the whole open
//!   set / resolve a handle across it.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{notification, operation, Notification, Operation};

use crate::shell::ContextMenuItem;

// Window operations ─────────────────────────────────────────────────────

/// Open a new application window.
///
/// Ports the existing `create_window` Tauri command (`apps/kanban-app/src/
/// commands.rs`): it resolves the board to display, builds and shows a new
/// webview window, and returns its label. Routing it through the `WindowShell`
/// seam keeps the create-window behavior while making the action testable
/// without a live GUI.
///
/// Returns `{ ok: true, label: <string>, board_path: <string|null> }`.
#[operation(
    verb = "new",
    noun = "window",
    description = "Open a new application window, optionally pointed at a board path"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpenNewWindow {
    /// The board path the new window should display. When omitted, the shell
    /// falls back to the most-recently-focused / first open board.
    #[serde(default)]
    pub board_path: Option<String>,
}

/// Bring an existing window to the front and focus it.
///
/// Net-new behavior implemented against the tauri window API — there was no
/// `activate_window` Tauri command to port.
///
/// Returns `{ ok: true, label: <string> }`.
#[operation(
    verb = "activate",
    noun = "window",
    description = "Bring the window with the given label to the front and focus it"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ActivateWindow {
    /// The label of the window to activate (e.g. `"main"`, `"board-01jxyz"`).
    #[serde(default)]
    pub label: String,
}

/// Move a window to a logical-pixel position.
///
/// Net-new behavior implemented against the tauri window API. Coordinates are
/// in logical pixels relative to the primary display's top-left origin.
///
/// Returns `{ ok: true, label: <string>, x: <int>, y: <int> }`.
#[operation(
    verb = "set",
    noun = "position",
    description = "Move the window with the given label to a logical-pixel position"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SetWindowPosition {
    /// The label of the window to move.
    #[serde(default)]
    pub label: String,
    /// Target logical-pixel x coordinate of the window's top-left corner.
    #[serde(default)]
    pub x: i32,
    /// Target logical-pixel y coordinate of the window's top-left corner.
    #[serde(default)]
    pub y: i32,
}

/// Read a window's current logical-pixel position.
///
/// Net-new behavior implemented against the tauri window API.
///
/// Returns `{ ok: true, label: <string>, x: <int>, y: <int> }`.
#[operation(
    verb = "get",
    noun = "position",
    description = "Read the current logical-pixel position of the window with the given label"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetWindowPosition {
    /// The label of the window to query.
    #[serde(default)]
    pub label: String,
}

/// Enumerate the connected monitors.
///
/// Net-new behavior implemented against the tauri window API. Returns each
/// monitor's name, position, size, and scale factor.
///
/// Returns `{ ok: true, monitors: [<MonitorInfo>, ...] }`.
#[operation(
    verb = "get",
    noun = "monitors",
    description = "Enumerate the connected monitors with position, size, and scale factor"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetMonitors {}

/// Close a window.
///
/// Net-new behavior implemented against the tauri window API.
///
/// Returns `{ ok: true, label: <string> }`.
#[operation(
    verb = "close",
    noun = "window",
    description = "Close the window with the given label"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CloseWindow {
    /// The label of the window to close.
    #[serde(default)]
    pub label: String,
}

// OS file actions ───────────────────────────────────────────────────────

/// Open a file in the OS default application.
///
/// Backs `attachment.open`. Relocates the `open::that` call that lived inside
/// the kanban `AttachmentOpenCmd` into the `window` server, behind the
/// `WindowShell` seam.
///
/// Returns `{ ok: true, opened: <string> }`.
#[operation(
    verb = "open",
    noun = "path",
    description = "Open a file in the OS default application"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpenPath {
    /// The filesystem path to open.
    #[serde(default)]
    pub path: String,
}

/// Reveal a file in the OS file manager (Finder / Explorer / file browser).
///
/// Backs `attachment.reveal`. Relocates the platform-specific reveal command
/// that lived inside the kanban `AttachmentRevealCmd` into the `window` server,
/// behind the `WindowShell` seam.
///
/// Returns `{ ok: true, revealed: <string> }`.
#[operation(
    verb = "reveal",
    noun = "path",
    description = "Reveal a file in the OS file manager"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RevealPath {
    /// The filesystem path to reveal.
    #[serde(default)]
    pub path: String,
}

// Board-file lifecycle ──────────────────────────────────────────────────

/// Switch the active board to the one at the given path.
///
/// Backs `file.switchBoard`. Wraps `AppState::open_board`
/// (`apps/kanban-app/src/state.rs`) behind the `WindowShell` seam without
/// behavior change: resolve the `.kanban` directory, open / touch the board,
/// update MRU tracking.
///
/// Returns `{ ok: true, path: <string> }`.
#[operation(
    verb = "switch",
    noun = "board",
    description = "Switch the active board to the one at the given path"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SwitchBoard {
    /// The board path to switch to (a folder, a `.kanban` directory, or a path
    /// inside one).
    #[serde(default)]
    pub path: String,
}

/// Close the board at the given path.
///
/// Backs `file.closeBoard`. Wraps `AppState::close_board`
/// (`apps/kanban-app/src/state.rs`) behind the `WindowShell` seam without
/// behavior change: remove the board from the open set, re-point MRU if needed,
/// stop any running agent.
///
/// Returns `{ ok: true, path: <string> }`.
#[operation(
    verb = "close",
    noun = "board",
    description = "Close the board at the given path"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct CloseBoard {
    /// The board path to close.
    #[serde(default)]
    pub path: String,
}

/// Create a new board via the OS folder picker.
///
/// Backs `file.newBoard`. Ports the `new_board_dialog` handler
/// (`apps/kanban-app/src/commands.rs`): show the folder picker, derive the board
/// name from the chosen folder, initialize a board at its `.kanban` directory,
/// then open it. The picker is the injectable shim so the path is testable
/// without a native dialog.
///
/// Returns `{ ok: true, path: <string>, name: <string> }`.
#[operation(
    verb = "new",
    noun = "board",
    description = "Create a new board via the OS folder picker"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct NewBoard {}

/// Open an existing board via the OS file-open dialog.
///
/// Backs `file.openBoard`. Ports the `open_board_dialog` handler
/// (`apps/kanban-app/src/commands.rs`): show the folder picker and open the
/// chosen board. The picker is the injectable shim so the path is testable
/// without a native dialog.
///
/// Returns `{ ok: true, opened: <bool>, path: <string|null> }` — `opened` is
/// `false` with a null `path` when the user cancelled the dialog.
#[operation(
    verb = "open",
    noun = "board",
    description = "Open an existing board via the OS file-open dialog"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct OpenBoard {}

// App-wide window affordances ────────────────────────────────────────────

/// Show a native context menu for the given items at the current pointer.
///
/// Ports the original `show_context_menu` Tauri command. Context menus are an
/// app-wide window affordance (the right-click target is the calling window,
/// identified by `window_label`), so this rides the app-wide `window` server
/// rather than any per-board wiring.
/// Each item carries its own dispatch info (`cmd`, `target`, `scope_chain`); the
/// shell encodes that into the native menu so the app's menu-event handler can
/// emit `context-menu-command` on selection. Selection delivery is therefore
/// unchanged — the op returns once the menu is shown and does not carry the
/// chosen item back over the wire.
///
/// Returns `{ ok: true, count: <int> }` — the number of items handed to the
/// shell.
#[operation(
    verb = "show",
    noun = "context menu",
    description = "Show a native context menu for the given items at the current pointer"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ShowContextMenu {
    /// The menu items to render, in display order. Each non-separator item
    /// carries the dispatch info delivered back on selection.
    #[serde(default)]
    pub items: Vec<ContextMenuItem>,
    /// Label of the webview window the right-click originated in.
    ///
    /// The frontend passes its own window label (`getCurrentWindow().label`)
    /// so the shell can pop the menu on the *calling* window — deterministic
    /// targeting that matches the original native command, which popped on its
    /// calling `tauri::Window`. Optional for back-compat: when absent (or the
    /// label no longer resolves), the shell falls back to focused-then-any.
    #[serde(default)]
    pub window_label: Option<String>,
}

// Board-management reads ─────────────────────────────────────────────────

/// List every currently open board.
///
/// Ports the `list_open_boards` Tauri command (`apps/kanban-app/src/
/// commands.rs`): enumerate the open-board set, mark which one is active
/// (most-recently-focused), and return each board's path / name / active flag.
/// This is a multi-board read with no per-board `entity` server home, so it
/// rides the app-wide `window` server alongside the board-lifecycle writes,
/// behind an injected shell callback.
///
/// Returns `{ ok: true, boards: [{ path, name, is_active }, ...] }`.
#[operation(
    verb = "list",
    noun = "open boards",
    description = "List every currently open board, marking the active one"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct ListOpenBoards {}

/// Project one board's aggregate summary.
///
/// Ports the `get_board_data` Tauri command (`apps/kanban-app/src/commands.rs`):
/// resolve the board handle (the given path, or the active board when omitted)
/// and return the board entity, its columns with injected task / ready counts,
/// its tags, the virtual-tag metadata, and a summary of aggregate totals. Tasks
/// themselves are NOT included (callers use the entity listing for those).
///
/// Returns `{ ok: true, board, columns, tags, virtual_tag_meta, summary }`.
#[operation(
    verb = "get",
    noun = "board data",
    description = "Project one board's aggregate summary (columns w/ counts, tags, totals)"
)]
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct GetBoardData {
    /// The board path to summarize. When omitted, the shell resolves the active
    /// board — matching the original command's `resolve_handle(None)` fallback.
    #[serde(default)]
    pub board_path: Option<String>,
}

/// All window operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the discovery
/// `_meta` generator (`generate_operations_meta`) are driven from this single
/// slice via the `operation_tool!` macro, so there is one source of truth for
/// what the `window` tool exposes.
static WINDOW_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<OpenNewWindow>::default()) as &dyn Operation,
        Box::leak(Box::<ActivateWindow>::default()) as &dyn Operation,
        Box::leak(Box::<SetWindowPosition>::default()) as &dyn Operation,
        Box::leak(Box::<GetWindowPosition>::default()) as &dyn Operation,
        Box::leak(Box::<GetMonitors>::default()) as &dyn Operation,
        Box::leak(Box::<CloseWindow>::default()) as &dyn Operation,
        Box::leak(Box::<OpenPath>::default()) as &dyn Operation,
        Box::leak(Box::<RevealPath>::default()) as &dyn Operation,
        Box::leak(Box::<SwitchBoard>::default()) as &dyn Operation,
        Box::leak(Box::<CloseBoard>::default()) as &dyn Operation,
        Box::leak(Box::<NewBoard>::default()) as &dyn Operation,
        Box::leak(Box::<OpenBoard>::default()) as &dyn Operation,
        Box::leak(Box::<ShowContextMenu>::default()) as &dyn Operation,
        Box::leak(Box::<ListOpenBoards>::default()) as &dyn Operation,
        Box::leak(Box::<GetBoardData>::default()) as &dyn Operation,
    ]
});

/// Get the canonical slice of all window operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &WINDOW_OPERATIONS
}

// Board-lifecycle notifications ─────────────────────────────────────────
//
// Each board a window displays is, from a plugin's point of view, the "window
// change" it cares about: a board is opened into a window, switched within a
// window, or closed. These three notifications make that lifecycle observable
// on the MCP bridge — a plugin subscribes with `this.window.on("board.opened")`
// / `"board.switched"` / `"board.closed"`.
//
// The methods live under the `board/*` path segment AND each declares an
// explicit two-segment short `event` (`board.opened` / `board.switched` /
// `board.closed`) so they share this `window` tool with the sibling raw
// OS-window lifecycle (`notifications/window/created|focused|closed`, short
// events `window.created` / `window.focused` / `window.closed`) without
// colliding. The notifications `_meta` tree is keyed by the SHORT event name
// (a duplicate key silently overwrites), so the default last-path-segment short
// event would make `board/closed` and a future `window/closed` BOTH key as
// `"closed"` and clash; the explicit `board.*` / `window.*` short events keep
// the two families distinct. A plugin subscribes with
// `this.window.on("board.opened", …)`. Both families hang off the single
// window-service `#[notification] struct == payload` declaration pattern.
//
// All three carry only the board's filesystem path — a thin event (the
// publisher has the path in hand at the open/switch/close site; no enrichment
// re-fetch). The payload struct IS the published `params` (serialized via
// [`McpNotification::from_declared`](swissarmyhammer_plugin::McpNotification::from_declared))
// AND the declaration the SDK reads for `_meta`, so the two cannot drift.

/// The `notifications/board/opened` event payload — a board was opened into a
/// window.
///
/// `path` is the opened board's canonical filesystem path. Published from the
/// app's board-open site once the board is registered and its window is
/// switched to it.
#[notification(
    method = "notifications/board/opened",
    event = "board.opened",
    description = "A board was opened into a window."
)]
#[derive(Debug, Default, Serialize)]
pub struct BoardOpened {
    /// Canonical filesystem path of the opened board.
    pub path: String,
}

/// The `notifications/board/switched` event payload — a window's active board
/// changed to another already-open board.
///
/// `path` is the now-active board's canonical filesystem path. Published from
/// the app's board-switch site once the window is rebound to the board.
#[notification(
    method = "notifications/board/switched",
    event = "board.switched",
    description = "A window's active board switched to another open board."
)]
#[derive(Debug, Default, Serialize)]
pub struct BoardSwitched {
    /// Canonical filesystem path of the now-active board.
    pub path: String,
}

/// The `notifications/board/closed` event payload — a board was closed.
///
/// `path` is the closed board's canonical filesystem path. Published from the
/// app's board-close site once the board is removed from the open set.
#[notification(
    method = "notifications/board/closed",
    event = "board.closed",
    description = "A board was closed."
)]
#[derive(Debug, Default, Serialize)]
pub struct BoardClosed {
    /// Canonical filesystem path of the closed board.
    pub path: String,
}

/// The canonical slice of notifications the `window` tool emits.
///
/// Mirrors [`operations`]: a leaked `Default` instance per notification, used
/// only for its static metadata. Fed to `operation_tool!`'s `notifications:`
/// field so the tool advertises its events in `_meta` and `.on()` can resolve
/// them.
static WINDOW_NOTIFICATIONS: LazyLock<Vec<&'static dyn Notification>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<BoardOpened>::default()) as &dyn Notification,
        Box::leak(Box::<BoardSwitched>::default()) as &dyn Notification,
        Box::leak(Box::<BoardClosed>::default()) as &dyn Notification,
    ]
});

/// Get the canonical slice of all `window` notifications.
pub fn notifications() -> &'static [&'static dyn Notification] {
    &WINDOW_NOTIFICATIONS
}

/// Build the `notifications/board/opened` notification carrying the opened
/// board's path.
///
/// The single production publish helper for the board-opened event: it
/// serializes the declared [`BoardOpened`] payload (so the `_meta` schema and
/// the wire payload share one source) and stamps `user` provenance. Lives here,
/// in the crate that DECLARES the notification, so the wire method comes from
/// the `#[notification]` attribute rather than a string literal at the call
/// site — the app's board-open sink calls this so the declared schema and the
/// published payload cannot drift.
pub fn board_opened_notification(
    path: impl Into<String>,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = BoardOpened { path: path.into() };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/board/switched` notification carrying the
/// now-active board's path. See [`board_opened_notification`] for the
/// struct == payload contract.
pub fn board_switched_notification(
    path: impl Into<String>,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = BoardSwitched { path: path.into() };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

/// Build the `notifications/board/closed` notification carrying the closed
/// board's path. See [`board_opened_notification`] for the struct == payload
/// contract.
pub fn board_closed_notification(
    path: impl Into<String>,
) -> swissarmyhammer_plugin::McpNotification {
    let payload = BoardClosed { path: path.into() };
    swissarmyhammer_plugin::McpNotification::from_declared(
        payload.method(),
        &payload,
        swissarmyhammer_plugin::Provenance::user(),
    )
}

#[cfg(test)]
mod notification_tests {
    use super::*;
    use swissarmyhammer_operations::generate_notifications_meta;

    /// Each board-lifecycle notification declares its wire method and an
    /// explicit two-segment short event (`board.opened` / `board.switched` /
    /// `board.closed`) a plugin subscribes to with
    /// `this.window.on("board.opened", …)`. The explicit `event` override (NOT
    /// the default last-path-segment) keeps the `closed` event from colliding
    /// with the sibling raw-window lifecycle's `window.closed` in the shared
    /// `window` tool's flat-keyed notifications `_meta`.
    #[test]
    fn board_notifications_declare_method_and_event() {
        assert_eq!(
            BoardOpened::default().method(),
            "notifications/board/opened"
        );
        assert_eq!(BoardOpened::default().event(), "board.opened");
        assert_eq!(
            BoardSwitched::default().method(),
            "notifications/board/switched"
        );
        assert_eq!(BoardSwitched::default().event(), "board.switched");
        assert_eq!(
            BoardClosed::default().method(),
            "notifications/board/closed"
        );
        assert_eq!(BoardClosed::default().event(), "board.closed");
    }

    /// Each publish helper builds the single-field `{ path }` payload under the
    /// declared method — the struct == payload publish path.
    #[test]
    fn board_notifications_build_path_payload() {
        let opened = board_opened_notification("/boards/alpha");
        assert_eq!(opened.method, "notifications/board/opened");
        assert_eq!(
            opened.params.as_object().expect("params is an object")["path"],
            "/boards/alpha"
        );

        let switched = board_switched_notification("/boards/beta");
        assert_eq!(switched.method, "notifications/board/switched");
        assert_eq!(
            switched.params.as_object().expect("params is an object")["path"],
            "/boards/beta"
        );

        let closed = board_closed_notification("/boards/gamma");
        assert_eq!(closed.method, "notifications/board/closed");
        assert_eq!(
            closed.params.as_object().expect("params is an object")["path"],
            "/boards/gamma"
        );
    }

    /// Declared ⟺ raised coverage guard: every method a publish helper actually
    /// emits MUST be one the `window` service declares in its notification
    /// `_meta`. Pins the declared==published contract so a new helper or a
    /// renamed method can never publish an undeclared event.
    #[test]
    fn published_board_methods_are_declared() {
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
            board_opened_notification("/x"),
            board_switched_notification("/x"),
            board_closed_notification("/x"),
        ] {
            assert!(
                declared.contains(&note.method),
                "emitted method {:?} is not declared in _meta ({:?})",
                note.method,
                declared,
            );
        }
    }
}
