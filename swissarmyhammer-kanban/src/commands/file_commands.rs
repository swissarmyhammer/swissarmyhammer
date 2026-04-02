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

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::{CommandContext, UIState};

    /// Build a minimal CommandContext with the given UIState, scope, and args.
    fn make_ctx(
        ui: Arc<UIState>,
        scope: Vec<String>,
        args: HashMap<String, serde_json::Value>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope, None, args);
        ctx.ui_state = Some(ui);
        ctx
    }

    // =========================================================================
    // NewBoardCmd
    // =========================================================================

    #[tokio::test]
    async fn new_board_cmd_returns_dialog_marker() {
        let ctx = CommandContext::new("file.newBoard", vec![], None, HashMap::new());
        let result = NewBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(
            result["NewBoardDialog"].as_bool(),
            Some(true),
            "NewBoardCmd should return {{\"NewBoardDialog\": true}}"
        );
    }

    #[tokio::test]
    async fn new_board_cmd_is_always_available() {
        let ctx = CommandContext::new("file.newBoard", vec![], None, HashMap::new());
        assert!(NewBoardCmd.available(&ctx));
    }

    // =========================================================================
    // OpenBoardCmd
    // =========================================================================

    #[tokio::test]
    async fn open_board_cmd_returns_dialog_marker() {
        let ctx = CommandContext::new("file.openBoard", vec![], None, HashMap::new());
        let result = OpenBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(
            result["OpenBoardDialog"].as_bool(),
            Some(true),
            "OpenBoardCmd should return {{\"OpenBoardDialog\": true}}"
        );
    }

    #[tokio::test]
    async fn open_board_cmd_is_always_available() {
        let ctx = CommandContext::new("file.openBoard", vec![], None, HashMap::new());
        assert!(OpenBoardCmd.available(&ctx));
    }

    // =========================================================================
    // NewWindowCmd
    // =========================================================================

    #[tokio::test]
    async fn new_window_cmd_returns_create_window_marker() {
        let ctx = CommandContext::new("window.new", vec![], None, HashMap::new());
        let result = NewWindowCmd.execute(&ctx).await.unwrap();
        assert_eq!(
            result["CreateWindow"].as_bool(),
            Some(true),
            "NewWindowCmd should return {{\"CreateWindow\": true}}"
        );
    }

    #[tokio::test]
    async fn new_window_cmd_is_always_available() {
        let ctx = CommandContext::new("window.new", vec![], None, HashMap::new());
        assert!(NewWindowCmd.available(&ctx));
    }

    // =========================================================================
    // SwitchBoardCmd
    // =========================================================================

    #[tokio::test]
    async fn switch_board_cmd_updates_ui_state() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/myboard/.kanban"));
        let ctx = make_ctx(Arc::clone(&ui), vec![], args);

        let result = SwitchBoardCmd.execute(&ctx).await.unwrap();

        // Result should have BoardSwitch with correct path
        let switch = &result["BoardSwitch"];
        assert_eq!(switch["path"].as_str(), Some("/tmp/myboard/.kanban"));
        assert_eq!(
            switch["window_label"].as_str(),
            Some("main"),
            "defaults to 'main' window label"
        );
    }

    #[tokio::test]
    async fn switch_board_cmd_uses_explicit_window_label() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        args.insert("windowLabel".into(), json!("secondary"));
        let ctx = make_ctx(Arc::clone(&ui), vec![], args);

        let result = SwitchBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(
            result["BoardSwitch"]["window_label"].as_str(),
            Some("secondary")
        );
    }

    #[tokio::test]
    async fn switch_board_cmd_missing_path_returns_error() {
        let ui = Arc::new(UIState::new());
        let ctx = make_ctx(Arc::clone(&ui), vec![], HashMap::new());
        let result = SwitchBoardCmd.execute(&ctx).await;
        assert!(result.is_err(), "SwitchBoardCmd with no path should fail");
    }

    #[tokio::test]
    async fn switch_board_cmd_without_ui_state_returns_error() {
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        let ctx = CommandContext::new("file.switchBoard", vec![], None, args);
        // No ui_state set
        let result = SwitchBoardCmd.execute(&ctx).await;
        assert!(
            result.is_err(),
            "SwitchBoardCmd without UIState should return an error"
        );
    }

    #[tokio::test]
    async fn switch_board_cmd_sets_most_recent_board() {
        let ui = Arc::new(UIState::new());
        let path = "/tmp/recent/.kanban";
        let mut args = HashMap::new();
        args.insert("path".into(), json!(path));
        let ctx = make_ctx(Arc::clone(&ui), vec![], args);

        SwitchBoardCmd.execute(&ctx).await.unwrap();

        assert_eq!(
            ui.most_recent_board().as_deref(),
            Some(path),
            "UIState most_recent_board should be updated after switch"
        );
    }

    // =========================================================================
    // CloseBoardCmd
    // =========================================================================

    #[tokio::test]
    async fn close_board_cmd_with_explicit_path() {
        let ui = Arc::new(UIState::new());
        // First open/register the board so there's something to close
        let path = "/tmp/closeable/.kanban";
        ui.add_open_board(path);

        let mut args = HashMap::new();
        args.insert("path".into(), json!(path));
        let ctx = make_ctx(Arc::clone(&ui), vec![], args);

        let result = CloseBoardCmd.execute(&ctx).await.unwrap();
        // Result should identify the closed board
        assert!(result["BoardClose"]["path"].as_str().is_some());
    }

    #[tokio::test]
    async fn close_board_cmd_uses_window_board_when_no_path_arg() {
        let ui = Arc::new(UIState::new());
        let path = "/tmp/window-board/.kanban";
        ui.set_window_board("main", path);

        // No path arg — should resolve from window board
        let ctx = make_ctx(Arc::clone(&ui), vec!["window:main".into()], HashMap::new());
        let result = CloseBoardCmd.execute(&ctx).await.unwrap();
        assert!(result["BoardClose"]["path"].as_str().is_some());
    }

    #[tokio::test]
    async fn close_board_cmd_without_ui_state_returns_error() {
        let ctx = CommandContext::new("file.closeBoard", vec![], None, HashMap::new());
        let result = CloseBoardCmd.execute(&ctx).await;
        assert!(
            result.is_err(),
            "CloseBoardCmd without UIState should return an error"
        );
    }

    #[tokio::test]
    async fn close_board_cmd_no_path_and_no_window_board_returns_error() {
        let ui = Arc::new(UIState::new());
        // No path arg, no window board configured
        let ctx = make_ctx(Arc::clone(&ui), vec![], HashMap::new());
        let result = CloseBoardCmd.execute(&ctx).await;
        assert!(
            result.is_err(),
            "CloseBoardCmd with no path info should return an error"
        );
    }
}
