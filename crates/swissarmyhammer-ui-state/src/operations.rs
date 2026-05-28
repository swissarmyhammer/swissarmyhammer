//! The `#[operation]` structs that make up the `ui_state` operation tool.
//!
//! These structs are the source of truth for the tool's verb / noun /
//! description / parameters surface. Both the wire-level `inputSchema`
//! generator and the discovery `_meta` tree generator are driven from the
//! same `UI_STATE_OPERATIONS` slice via the `operation_tool!` macro, so the
//! two cannot drift.
//!
//! Every operation is a 1:1 port of a mutating method on
//! [`crate::state::UIState`]. They divide into five groups:
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
//! focus is owned by the separate `focus` MCP server; `UIState`'s
//! `set_scope_chain` setter is intentionally left unwrapped.

use rmcp::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;
use swissarmyhammer_operations::{operation, Operation};

// Inspector operations ──────────────────────────────────────────────────

/// Open the inspector for a moniker in a window.
///
/// Ports [`crate::state::UIState::inspect`]. Pushes the moniker onto the
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
    /// The window whose inspector stack is mutated (e.g. `"main"`).
    #[serde(default)]
    pub window_label: String,
    /// The `type:id` moniker to inspect (e.g. `"task:01XYZ"`).
    #[serde(default)]
    pub moniker: String,
}

/// Close the topmost inspector entry for a window.
///
/// Ports [`crate::state::UIState::inspector_close`]. No-op when the stack is
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
    /// The window whose inspector stack is popped.
    #[serde(default)]
    pub window_label: String,
}

/// Close all inspector entries for a window.
///
/// Ports [`crate::state::UIState::inspector_close_all`]. No-op when the stack
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
    /// The window whose inspector stack is cleared.
    #[serde(default)]
    pub window_label: String,
}

/// Persist the user-chosen inspector panel width for a window.
///
/// Ports [`crate::state::UIState::set_inspector_width`]. No-op when the width
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
    /// The window whose inspector width is set.
    #[serde(default)]
    pub window_label: String,
    /// The new inspector width in CSS pixels.
    #[serde(default)]
    pub width: u32,
}

// Palette operations ────────────────────────────────────────────────────

/// Open the command palette for a window in a given mode.
///
/// Ports [`crate::state::UIState::set_palette_open_with_mode`]. `mode` is
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
    /// The window whose palette is opened.
    #[serde(default)]
    pub window_label: String,
    /// The palette mode: `"command"` (default) or `"search"`.
    #[serde(default = "default_palette_mode")]
    pub mode: String,
}

impl Default for PaletteOpen {
    fn default() -> Self {
        Self {
            window_label: String::new(),
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
/// Ports [`crate::state::UIState::set_palette_open`] with `open = false`.
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
    /// The window whose palette is closed.
    #[serde(default)]
    pub window_label: String,
}

// Keymap operation ──────────────────────────────────────────────────────

/// Set the active keymap mode.
///
/// Ports [`crate::state::UIState::set_keymap_mode`]. The `mode` param covers
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
    /// The window the rename applies to (carried for parity with the
    /// frontend command; the backend does not read it).
    #[serde(default)]
    pub window_label: String,
}

// Drag operations ───────────────────────────────────────────────────────

/// Start a cross-window drag session.
///
/// Ports [`crate::state::UIState::start_drag`] (cancelling any existing
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
/// Ports [`crate::state::UIState::cancel_drag`]. Clears the session without
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
/// Ports [`crate::state::UIState::take_drag`]. Returns and clears the active
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
    /// The window whose command palette is opened.
    #[serde(default)]
    pub window_label: String,
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
    /// The window whose palette is opened.
    #[serde(default)]
    pub window_label: String,
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
    /// The window whose search palette is opened.
    #[serde(default)]
    pub window_label: String,
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
    /// The window whose UI surface is dismissed.
    #[serde(default)]
    pub window_label: String,
}

/// All `ui_state` operations — the canonical list used for schema generation.
///
/// Both the wire-schema generator (`generate_mcp_schema`) and the discovery
/// `_meta` generator (`generate_operations_meta`) are driven from this single
/// slice via the `operation_tool!` macro, so there is one source of truth for
/// what the `ui_state` tool exposes. There are exactly 15 operations and none
/// of them is a focus / `set_focus` op.
static UI_STATE_OPERATIONS: LazyLock<Vec<&'static dyn Operation>> = LazyLock::new(|| {
    vec![
        Box::leak(Box::<Inspect>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorClose>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorCloseAll>::default()) as &dyn Operation,
        Box::leak(Box::<InspectorSetWidth>::default()) as &dyn Operation,
        Box::leak(Box::<PaletteOpen>::default()) as &dyn Operation,
        Box::leak(Box::<PaletteClose>::default()) as &dyn Operation,
        Box::leak(Box::<SetKeymapMode>::default()) as &dyn Operation,
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
