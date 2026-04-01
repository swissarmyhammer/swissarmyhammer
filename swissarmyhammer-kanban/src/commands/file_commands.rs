//! File (board management) command implementations.
//!
//! These commands update UIState tracking for open boards and active board.
//! The actual BoardHandle lifecycle (opening/closing KanbanContext) is managed
//! by the Tauri layer, which hooks into dispatch_command results to perform
//! side effects.

use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Switch the current window to a different board.
///
/// Updates UIState: sets the per-window board assignment via `windows[label].board_path`.
/// Required arg: `path` (canonical path to the .kanban directory).
/// Optional arg: `window_label` (defaults to "main").
///
/// The Tauri dispatch_command handler also opens the BoardHandle as a side
/// effect when this command returns a `BoardSwitch` result.
pub struct SwitchBoardCmd;

#[async_trait]
impl Command for SwitchBoardCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        let path = ctx
            .args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| CommandError::MissingArg("path".into()))?;

        let window_label = ctx
            .args
            .get("windowLabel")
            .or_else(|| ctx.args.get("window_label"))
            .and_then(|v| v.as_str())
            .unwrap_or("main");

        // Update per-window board assignment (stored in windows[label].board_path).
        ui.set_window_board(window_label, path);

        // Track the most recently active board for quick capture and default commands.
        ui.set_most_recent_board(path);

        Ok(json!({
            "BoardSwitch": {
                "path": path,
                "window_label": window_label,
            }
        }))
    }
}

/// Open the "New Board" dialog.
///
/// Returns a `NewBoardDialog` marker so the Tauri layer can trigger the
/// native folder picker dialog.
pub struct NewBoardCmd;

#[async_trait]
impl Command for NewBoardCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "NewBoardDialog": true }))
    }
}

/// Open the "Open Board" dialog.
///
/// Returns an `OpenBoardDialog` marker so the Tauri layer can trigger the
/// native folder picker dialog.
pub struct OpenBoardCmd;

#[async_trait]
impl Command for OpenBoardCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "OpenBoardDialog": true }))
    }
}

/// Create a new window.
///
/// Returns a `CreateWindow` marker so the Tauri layer can create a new
/// webview window.
pub struct NewWindowCmd;

#[async_trait]
impl Command for NewWindowCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, _ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        Ok(json!({ "CreateWindow": true }))
    }
}

/// Close a board, removing it from the open boards list in UIState.
///
/// Optional arg: `path`. If omitted, closes the currently active board.
///
/// The Tauri dispatch_command handler also drops the BoardHandle as a side
/// effect when this command returns a `BoardClose` result.
pub struct CloseBoardCmd;

#[async_trait]
impl Command for CloseBoardCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("UIState not available".into()))?;

        // `path` can be explicitly provided, or resolved from the window's board_path.
        let window_label = ctx.window_label_from_scope().unwrap_or("main");
        let raw_path = ctx
            .args
            .get("path")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .or_else(|| ui.window_board(window_label))
            .ok_or_else(|| CommandError::MissingArg("path".into()))?;

        // Canonicalize for consistent matching with how boards are stored.
        let canonical = std::path::PathBuf::from(&raw_path)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(&raw_path));
        let path = canonical.display().to_string();

        ui.remove_open_board(&path);
        // Also remove the raw form in case the stored path wasn't canonical
        if path != raw_path {
            ui.remove_open_board(&raw_path);
        }

        Ok(json!({
            "BoardClose": {
                "path": path,
            }
        }))
    }
}
