//! The `#[operation]` structs that make up the `window` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the same
//! `WINDOW_OPERATIONS` slice via the `operation_tool!` macro, so the two cannot
//! drift.
//!
//! Operations divide into two groups:
//!
//! - **window** — window-manager actions: `OpenNewWindow` (ports the original
//!   `create_window` Tauri command), plus the net-new `ActivateWindow`,
//!   `SetWindowPosition`, `GetWindowPosition`, `GetMonitors`, and `CloseWindow`.
//! - **OS file actions** — `OpenPath` (open a file in the OS default app, backs
//!   `attachment.open`) and `RevealPath` (reveal a file in the OS file manager,
//!   backs `attachment.reveal`). These relocate the direct OS calls that lived
//!   inside the kanban attachment command path.
//!
//! The board-file lifecycle operations (`SwitchBoard` / `CloseBoard` /
//! `NewBoard` / `OpenBoard`) are a separate follow-up task on this same crate
//! and are deliberately absent here.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

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
    ]
});

/// Get the canonical slice of all window operations.
pub fn operations() -> &'static [&'static dyn Operation] {
    &WINDOW_OPERATIONS
}
