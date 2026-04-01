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
    use swissarmyhammer_commands::ui_state::UIState;
    use swissarmyhammer_commands::CommandContext;

    /// Build a CommandContext with UIState and the given args/scope chain.
    fn make_ctx(
        ui: Arc<UIState>,
        args: HashMap<String, Value>,
        scope_chain: Vec<String>,
    ) -> CommandContext {
        CommandContext::new("test", scope_chain, None, args).with_ui_state(ui)
    }

    // ── SwitchBoardCmd ──────────────────────────────────────────────

    #[tokio::test]
    async fn switch_board_updates_window_board_and_most_recent() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/my-board/.kanban"));
        let ctx = make_ctx(Arc::clone(&ui), args, vec![]);

        let result = SwitchBoardCmd.execute(&ctx).await.unwrap();

        // Returns BoardSwitch payload
        assert_eq!(result["BoardSwitch"]["path"], "/tmp/my-board/.kanban");
        assert_eq!(result["BoardSwitch"]["window_label"], "main");

        // Side effects on UIState
        assert_eq!(
            ui.window_board("main"),
            Some("/tmp/my-board/.kanban".to_string())
        );
        assert_eq!(
            ui.most_recent_board(),
            Some("/tmp/my-board/.kanban".to_string())
        );
    }

    #[tokio::test]
    async fn switch_board_uses_explicit_window_label() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board2/.kanban"));
        args.insert("windowLabel".into(), json!("secondary"));
        let ctx = make_ctx(Arc::clone(&ui), args, vec![]);

        let result = SwitchBoardCmd.execute(&ctx).await.unwrap();

        assert_eq!(result["BoardSwitch"]["window_label"], "secondary");
        assert_eq!(
            ui.window_board("secondary"),
            Some("/tmp/board2/.kanban".to_string())
        );
        // "main" window should be unaffected
        assert_eq!(ui.window_board("main"), None);
    }

    #[tokio::test]
    async fn switch_board_also_accepts_snake_case_window_label() {
        let ui = Arc::new(UIState::new());
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        args.insert("window_label".into(), json!("tertiary"));
        let ctx = make_ctx(Arc::clone(&ui), args, vec![]);

        let result = SwitchBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["BoardSwitch"]["window_label"], "tertiary");
        assert_eq!(
            ui.window_board("tertiary"),
            Some("/tmp/board/.kanban".to_string())
        );
    }

    #[tokio::test]
    async fn switch_board_fails_without_path() {
        let ui = Arc::new(UIState::new());
        let ctx = make_ctx(ui, HashMap::new(), vec![]);

        let err = SwitchBoardCmd.execute(&ctx).await.unwrap_err();
        assert!(
            matches!(err, CommandError::MissingArg(ref a) if a == "path"),
            "expected MissingArg(\"path\"), got: {err:?}"
        );
    }

    #[tokio::test]
    async fn switch_board_fails_without_ui_state() {
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        // No UIState attached
        let ctx = CommandContext::new("test", vec![], None, args);

        let err = SwitchBoardCmd.execute(&ctx).await.unwrap_err();
        assert!(matches!(err, CommandError::ExecutionFailed(_)));
    }

    // ── NewBoardCmd ─────────────────────────────────────────────────

    #[tokio::test]
    async fn new_board_returns_dialog_marker() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = NewBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(result, json!({ "NewBoardDialog": true }));
    }

    // ── OpenBoardCmd ────────────────────────────────────────────────

    #[tokio::test]
    async fn open_board_returns_dialog_marker() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = OpenBoardCmd.execute(&ctx).await.unwrap();
        assert_eq!(result, json!({ "OpenBoardDialog": true }));
    }

    // ── NewWindowCmd ────────────────────────────────────────────────

    #[tokio::test]
    async fn new_window_returns_create_window_marker() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        let result = NewWindowCmd.execute(&ctx).await.unwrap();
        assert_eq!(result, json!({ "CreateWindow": true }));
    }

    // ── CloseBoardCmd ───────────────────────────────────────────────

    #[tokio::test]
    async fn close_board_with_explicit_path() {
        let ui = Arc::new(UIState::new());
        // Pre-populate: add a board to the open list and assign it to "main"
        ui.add_open_board("/tmp/board/.kanban");
        ui.set_window_board("main", "/tmp/board/.kanban");

        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        let ctx = make_ctx(Arc::clone(&ui), args, vec!["window:main".into()]);

        let result = CloseBoardCmd.execute(&ctx).await.unwrap();

        // Returns BoardClose with the path
        assert!(result["BoardClose"]["path"].as_str().is_some());

        // Board should be removed from open list
        assert!(
            !ui.open_boards().contains(&"/tmp/board/.kanban".to_string()),
            "board should have been removed from open_boards"
        );
    }

    #[tokio::test]
    async fn close_board_resolves_path_from_window() {
        let ui = Arc::new(UIState::new());
        ui.add_open_board("/tmp/board/.kanban");
        ui.set_window_board("main", "/tmp/board/.kanban");

        // No explicit path arg — should resolve from window's board_path
        let ctx = make_ctx(Arc::clone(&ui), HashMap::new(), vec!["window:main".into()]);

        let result = CloseBoardCmd.execute(&ctx).await.unwrap();
        assert!(result["BoardClose"]["path"].as_str().is_some());
    }

    #[tokio::test]
    async fn close_board_fails_without_path_or_window_board() {
        let ui = Arc::new(UIState::new());
        // No path arg, and no board assigned to window
        let ctx = make_ctx(Arc::clone(&ui), HashMap::new(), vec!["window:main".into()]);

        let err = CloseBoardCmd.execute(&ctx).await.unwrap_err();
        assert!(
            matches!(err, CommandError::MissingArg(ref a) if a == "path"),
            "expected MissingArg(\"path\"), got: {err:?}"
        );
    }

    #[tokio::test]
    async fn close_board_fails_without_ui_state() {
        let mut args = HashMap::new();
        args.insert("path".into(), json!("/tmp/board/.kanban"));
        let ctx = CommandContext::new("test", vec![], None, args);

        let err = CloseBoardCmd.execute(&ctx).await.unwrap_err();
        assert!(matches!(err, CommandError::ExecutionFailed(_)));
    }

    // ── Availability ────────────────────────────────────────────────

    #[test]
    fn all_file_commands_are_always_available() {
        let ctx = CommandContext::new("test", vec![], None, HashMap::new());
        assert!(SwitchBoardCmd.available(&ctx));
        assert!(NewBoardCmd.available(&ctx));
        assert!(OpenBoardCmd.available(&ctx));
        assert!(NewWindowCmd.available(&ctx));
        assert!(CloseBoardCmd.available(&ctx));
    }
}
