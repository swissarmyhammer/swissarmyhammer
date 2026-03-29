//! Clipboard command implementations: copy, cut, paste.
//!
//! These commands bridge the operation layer (CopyTask, CutTask, PasteTask)
//! with the system clipboard via `ClipboardProviderExt` and update the
//! `has_clipboard` flag on `UIState` for availability tracking.

use super::run_op;
use crate::clipboard::ClipboardProviderExt;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Copy a task to the system clipboard.
///
/// Requires `task` in the scope chain. Writes the clipboard payload to the
/// system clipboard and sets `has_clipboard` on UIState.
pub struct CopyTaskCmd;

#[async_trait]
impl Command for CopyTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        let op = crate::task::CopyTask::new(task_id);
        let result = run_op(&op, &kanban).await?;

        // Write clipboard JSON to system clipboard
        let clipboard_json = result["clipboard_json"]
            .as_str()
            .ok_or_else(|| CommandError::ExecutionFailed("missing clipboard_json".into()))?;

        if let Ok(clipboard) = ctx.require_extension::<ClipboardProviderExt>() {
            clipboard
                .0
                .write_text(clipboard_json)
                .await
                .map_err(|e| CommandError::ExecutionFailed(format!("clipboard write failed: {e}")))?;
        }

        // Set has_clipboard flag
        if let Some(ref ui) = ctx.ui_state {
            ui.set_has_clipboard(true);
        }

        Ok(result)
    }
}

/// Cut a task: copy to clipboard and delete.
///
/// Requires `task` in the scope chain. Writes the clipboard payload to the
/// system clipboard, deletes the task, and sets `has_clipboard` on UIState.
pub struct CutTaskCmd;

#[async_trait]
impl Command for CutTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        let op = crate::task::CutTask::new(task_id);
        let result = run_op(&op, &kanban).await?;

        // Write clipboard JSON to system clipboard
        let clipboard_json = result["clipboard_json"]
            .as_str()
            .ok_or_else(|| CommandError::ExecutionFailed("missing clipboard_json".into()))?;

        if let Ok(clipboard) = ctx.require_extension::<ClipboardProviderExt>() {
            clipboard
                .0
                .write_text(clipboard_json)
                .await
                .map_err(|e| CommandError::ExecutionFailed(format!("clipboard write failed: {e}")))?;
        }

        // Set has_clipboard flag
        if let Some(ref ui) = ctx.ui_state {
            ui.set_has_clipboard(true);
        }

        Ok(result)
    }
}

/// Paste a task from the system clipboard.
///
/// Requires `has_clipboard` on UIState and either `column` or `board` in the
/// scope chain. Reads the clipboard, validates the payload, and creates a
/// new task via PasteTask.
pub struct PasteTaskCmd;

#[async_trait]
impl Command for PasteTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        let has_clipboard = ctx
            .ui_state
            .as_ref()
            .map(|ui| ui.has_clipboard())
            .unwrap_or(false);

        has_clipboard && (ctx.has_in_scope("column") || ctx.has_in_scope("board"))
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Read clipboard text
        let clipboard = ctx.require_extension::<ClipboardProviderExt>()?;
        let clipboard_text = clipboard
            .0
            .read_text()
            .await
            .map_err(|e| CommandError::ExecutionFailed(format!("clipboard read failed: {e}")))?
            .ok_or_else(|| CommandError::ExecutionFailed("clipboard is empty".into()))?;

        // Resolve target column
        let column = ctx
            .resolve_entity_id("column")
            .or_else(|| ctx.arg("column").and_then(|v| v.as_str()))
            .ok_or_else(|| CommandError::MissingScope("column".into()))?;

        // Optional: place after the focused task
        let after_id = ctx
            .resolve_entity_id("task")
            .map(crate::types::TaskId::from_string);

        let op = crate::task::PasteTask::new(column, after_id, clipboard_text);
        run_op(&op, &kanban).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::clipboard::{ClipboardProviderExt, InMemoryClipboard};
    use crate::task::AddTask;
    use crate::Execute;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::UIState;

    /// Build a test CommandContext with kanban, clipboard, and UI state.
    async fn setup() -> (
        tempfile::TempDir,
        Arc<KanbanContext>,
        Arc<ClipboardProviderExt>,
        Arc<UIState>,
    ) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let kanban = Arc::new(KanbanContext::new(&kanban_dir));

        InitBoard::new("Test")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();

        let clipboard = Arc::new(ClipboardProviderExt(Arc::new(InMemoryClipboard::new())));
        let ui = Arc::new(UIState::new());

        (temp, kanban, clipboard, ui)
    }

    fn make_ctx(
        command_id: &str,
        scope: &[&str],
        kanban: &Arc<KanbanContext>,
        clipboard: &Arc<ClipboardProviderExt>,
        ui: &Arc<UIState>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new(
            command_id,
            scope.iter().map(|s| s.to_string()).collect(),
            None,
            HashMap::new(),
        );
        ctx.set_extension(Arc::clone(kanban));
        ctx.set_extension(Arc::clone(clipboard));
        ctx.ui_state = Some(Arc::clone(ui));
        ctx
    }

    #[tokio::test]
    async fn copy_cmd_writes_to_clipboard_and_sets_flag() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        // Add a task
        let add_result = AddTask::new("Copy me")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let ctx = make_ctx(
            "task.copy",
            &[&format!("task:{task_id}"), "column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );

        assert!(CopyTaskCmd.available(&ctx));
        let result = CopyTaskCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);

        // Clipboard should have content
        assert!(ui.has_clipboard());

        // System clipboard should contain valid JSON
        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = crate::clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
    }

    #[tokio::test]
    async fn cut_cmd_writes_to_clipboard_and_deletes() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        let add_result = AddTask::new("Cut me")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let ctx = make_ctx(
            "task.cut",
            &[&format!("task:{task_id}"), "column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );

        assert!(CutTaskCmd.available(&ctx));
        let result = CutTaskCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["cut"], true);

        // Clipboard should have content
        assert!(ui.has_clipboard());

        // Task should be deleted
        let ectx = kanban.entity_context().await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert!(tasks.is_empty());
    }

    #[tokio::test]
    async fn paste_cmd_creates_new_task_from_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        // Copy a task first
        let add_result = AddTask::new("Source task")
            .execute(kanban.as_ref())
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Execute copy
        let copy_ctx = make_ctx(
            "task.copy",
            &[&format!("task:{task_id}"), "column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );
        CopyTaskCmd.execute(&copy_ctx).await.unwrap();

        // Now paste
        let paste_ctx = make_ctx(
            "task.paste",
            &["column:doing"],
            &kanban,
            &clipboard,
            &ui,
        );
        assert!(PasteTaskCmd.available(&paste_ctx));
        let result = PasteTaskCmd.execute(&paste_ctx).await.unwrap();

        assert_eq!(result["title"], "Source task");
        assert_eq!(result["position"]["column"], "doing");

        // Should have 2 tasks now
        let ectx = kanban.entity_context().await.unwrap();
        let tasks = ectx.list("task").await.unwrap();
        assert_eq!(tasks.len(), 2);
    }

    #[tokio::test]
    async fn paste_not_available_without_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        let ctx = make_ctx(
            "task.paste",
            &["column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );

        // has_clipboard is false — paste should not be available
        assert!(!PasteTaskCmd.available(&ctx));
    }

    #[tokio::test]
    async fn paste_not_available_without_column_or_board() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_has_clipboard(true);

        let ctx = make_ctx(
            "task.paste",
            &["task:01ABC"],
            &kanban,
            &clipboard,
            &ui,
        );

        assert!(!PasteTaskCmd.available(&ctx));
    }

    #[tokio::test]
    async fn copy_not_available_without_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        let ctx = make_ctx(
            "task.copy",
            &["column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );

        assert!(!CopyTaskCmd.available(&ctx));
    }

    #[tokio::test]
    async fn cut_not_available_without_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        let ctx = make_ctx(
            "task.cut",
            &["column:todo"],
            &kanban,
            &clipboard,
            &ui,
        );

        assert!(!CutTaskCmd.available(&ctx));
    }
}
