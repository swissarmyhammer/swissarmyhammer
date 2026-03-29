//! Polymorphic clipboard command implementations: copy, cut, paste.
//!
//! These commands dispatch to the appropriate entity-specific operation
//! based on what's in the scope chain (copy/cut) or what's on the
//! clipboard (paste). Currently supports tasks and tags.

use super::run_op;
use crate::clipboard::{self, ClipboardProviderExt};
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Helper: write clipboard JSON to system clipboard and set has_clipboard flag.
async fn write_to_clipboard(
    ctx: &CommandContext,
    clipboard_json: &str,
) -> swissarmyhammer_commands::Result<()> {
    if let Ok(clipboard) = ctx.require_extension::<ClipboardProviderExt>() {
        clipboard
            .0
            .write_text(clipboard_json)
            .await
            .map_err(|e| CommandError::ExecutionFailed(format!("clipboard write failed: {e}")))?;
    }
    if let Some(ref ui) = ctx.ui_state {
        ui.set_has_clipboard(true);
    }
    Ok(())
}

/// Copy the focused entity to the system clipboard.
///
/// Dispatches by innermost scope: tag > task.
/// Tag in scope → CopyTag. Task in scope → CopyTask.
pub struct CopyCmd;

#[async_trait]
impl Command for CopyCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("tag") || ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let result = if let Some(tag_id) = ctx.resolve_entity_id("tag") {
            // Tag is innermost — copy the tag
            let op = crate::tag::CopyTag::new(tag_id);
            run_op(&op, &kanban).await?
        } else if let Some(task_id) = ctx.resolve_entity_id("task") {
            // Task copy
            let op = crate::task::CopyTask::new(task_id);
            run_op(&op, &kanban).await?
        } else {
            return Err(CommandError::MissingScope("tag or task".into()));
        };

        // Write to system clipboard
        if let Some(clipboard_json) = result["clipboard_json"].as_str() {
            write_to_clipboard(ctx, clipboard_json).await?;
        }

        Ok(result)
    }
}

/// Cut the focused entity: copy to clipboard and remove/delete.
///
/// Dispatches by innermost scope: tag > task.
/// Tag in scope (+ task for untag) → CutTag. Task only → CutTask.
pub struct CutCmd;

#[async_trait]
impl Command for CutCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("tag") || ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let result = if ctx.has_in_scope("tag") {
            // Tag cut: needs both tag and task in scope
            let tag_id = ctx
                .resolve_entity_id("tag")
                .ok_or_else(|| CommandError::MissingScope("tag".into()))?;
            let task_id = ctx
                .resolve_entity_id("task")
                .ok_or_else(|| CommandError::MissingScope("task".into()))?;

            // Resolve tag name from the tag entity
            let ectx = kanban
                .entity_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let tag_entity = ectx
                .read("tag", tag_id)
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let tag_name = tag_entity
                .get_str("tag_name")
                .unwrap_or(tag_id)
                .to_string();

            let op = crate::tag::CutTag::new(task_id, tag_name);
            run_op(&op, &kanban).await?
        } else if let Some(task_id) = ctx.resolve_entity_id("task") {
            // Task cut
            let op = crate::task::CutTask::new(task_id);
            run_op(&op, &kanban).await?
        } else {
            return Err(CommandError::MissingScope("tag or task".into()));
        };

        // Write to system clipboard
        if let Some(clipboard_json) = result["clipboard_json"].as_str() {
            write_to_clipboard(ctx, clipboard_json).await?;
        }

        Ok(result)
    }
}

/// Paste from the system clipboard.
///
/// Dispatches by clipboard entity_type:
/// - "tag" + task in scope → PasteTag (tags the task)
/// - "task" + column/board in scope → PasteTask (creates new task)
pub struct PasteCmd;

#[async_trait]
impl Command for PasteCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        let has_clipboard = ctx
            .ui_state
            .as_ref()
            .map(|ui| ui.has_clipboard())
            .unwrap_or(false);

        // Paste is available if clipboard has content AND we have a valid target:
        // - task in scope (for pasting a tag onto it)
        // - column/board in scope (for pasting a task into it)
        has_clipboard
            && (ctx.has_in_scope("task")
                || ctx.has_in_scope("column")
                || ctx.has_in_scope("board"))
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

        // Peek at entity_type to decide which paste operation to run
        let entity_type = clipboard::deserialize_from_clipboard(&clipboard_text)
            .map(|p| p.swissarmyhammer_clipboard.entity_type.clone())
            .unwrap_or_default();

        match entity_type.as_str() {
            "tag" => {
                // Paste tag onto focused task
                let task_id = ctx
                    .resolve_entity_id("task")
                    .ok_or_else(|| {
                        CommandError::ExecutionFailed(
                            "paste tag requires a task in scope".into(),
                        )
                    })?;
                let op = crate::tag::PasteTag::new(task_id, clipboard_text);
                run_op(&op, &kanban).await
            }
            "task" => {
                // Paste task into column
                let column = ctx
                    .resolve_entity_id("column")
                    .or_else(|| ctx.arg("column").and_then(|v| v.as_str()))
                    .ok_or_else(|| CommandError::MissingScope("column".into()))?;
                let after_id = ctx
                    .resolve_entity_id("task")
                    .map(crate::types::TaskId::from_string);
                let op = crate::task::PasteTask::new(column, after_id, clipboard_text);
                run_op(&op, &kanban).await
            }
            other => Err(CommandError::ExecutionFailed(format!(
                "unknown clipboard entity type: '{other}'"
            ))),
        }
    }
}

// Re-export old names for backward compatibility with mod.rs registration
pub use CopyCmd as CopyTaskCmd;
pub use CutCmd as CutTaskCmd;
pub use PasteCmd as PasteTaskCmd;

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

    async fn setup() -> (
        tempfile::TempDir,
        Arc<KanbanContext>,
        Arc<ClipboardProviderExt>,
        Arc<UIState>,
    ) {
        let temp = tempfile::TempDir::new().unwrap();
        let kanban = Arc::new(KanbanContext::new(temp.path().join(".kanban")));
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

    // --- Copy tests ---

    #[tokio::test]
    async fn copy_task_when_task_in_scope() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Copy me").execute(kanban.as_ref()).await.into_result().unwrap();
        let task_id = add["id"].as_str().unwrap();

        let ctx = make_ctx("entity.copy", &[&format!("task:{task_id}"), "column:todo"], &kanban, &clipboard, &ui);
        assert!(CopyCmd.available(&ctx));
        let result = CopyCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["copied"], true);
        assert!(ui.has_clipboard());

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "task");
    }

    #[tokio::test]
    async fn copy_tag_when_tag_in_scope() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = crate::tag::AddTag::new("bug").execute(kanban.as_ref()).await.into_result().unwrap();
        let tag_id = add["id"].as_str().unwrap();

        let ctx = make_ctx("entity.copy", &[&format!("tag:{tag_id}"), "task:01TASK", "column:todo"], &kanban, &clipboard, &ui);
        assert!(CopyCmd.available(&ctx));
        let result = CopyCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["entity_type"], "tag");
        assert!(ui.has_clipboard());

        let text = clipboard.0.read_text().await.unwrap().unwrap();
        let payload = clipboard::deserialize_from_clipboard(&text).unwrap();
        assert_eq!(payload.swissarmyhammer_clipboard.entity_type, "tag");
    }

    #[tokio::test]
    async fn copy_not_available_without_tag_or_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx("entity.copy", &["column:todo"], &kanban, &clipboard, &ui);
        assert!(!CopyCmd.available(&ctx));
    }

    // --- Cut tests ---

    #[tokio::test]
    async fn cut_tag_untags_from_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Tagged").with_description("Fix #bug").execute(kanban.as_ref()).await.into_result().unwrap();
        let task_id = add["id"].as_str().unwrap();

        // Find the tag entity ID
        let ectx = kanban.entity_context().await.unwrap();
        let tag = crate::tag::find_tag_entity_by_name(&ectx, "bug").await.unwrap();
        let tag_id = tag.id.to_string();

        let ctx = make_ctx("entity.cut", &[&format!("tag:{tag_id}"), &format!("task:{task_id}"), "column:todo"], &kanban, &clipboard, &ui);
        assert!(CutCmd.available(&ctx));
        let result = CutCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["cut"], true);
        assert_eq!(result["tag"], "bug");

        // Task should no longer have the tag
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(!task.get_str("body").unwrap_or("").contains("#bug"));
    }

    // --- Paste tests ---

    #[tokio::test]
    async fn paste_tag_onto_task() {
        let (_temp, kanban, clipboard, ui) = setup().await;

        // Create a task
        let add = AddTask::new("Target").execute(kanban.as_ref()).await.into_result().unwrap();
        let task_id = add["id"].as_str().unwrap();

        // Put a tag on clipboard
        let tag_clip = clipboard::serialize_to_clipboard("tag", "01FAKE", "copy", serde_json::json!({"tag_name": "urgent", "color": "ff0000"}));
        clipboard.0.write_text(&tag_clip).await.unwrap();
        ui.set_has_clipboard(true);

        let ctx = make_ctx("entity.paste", &[&format!("task:{task_id}"), "column:todo"], &kanban, &clipboard, &ui);
        assert!(PasteCmd.available(&ctx));
        let result = PasteCmd.execute(&ctx).await.unwrap();
        assert_eq!(result["pasted"], true);
        assert_eq!(result["tag"], "urgent");

        // Task should have the tag
        let ectx = kanban.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(task.get_str("body").unwrap_or("").contains("#urgent"));
    }

    #[tokio::test]
    async fn paste_task_into_column() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let add = AddTask::new("Source").execute(kanban.as_ref()).await.into_result().unwrap();
        let task_id = add["id"].as_str().unwrap();

        // Copy the task
        let copy_ctx = make_ctx("entity.copy", &[&format!("task:{task_id}"), "column:todo"], &kanban, &clipboard, &ui);
        CopyCmd.execute(&copy_ctx).await.unwrap();

        // Paste into doing
        let paste_ctx = make_ctx("entity.paste", &["column:doing"], &kanban, &clipboard, &ui);
        assert!(PasteCmd.available(&paste_ctx));
        let result = PasteCmd.execute(&paste_ctx).await.unwrap();
        assert_eq!(result["title"], "Source");

        let ectx = kanban.entity_context().await.unwrap();
        assert_eq!(ectx.list("task").await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn paste_not_available_without_clipboard() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        let ctx = make_ctx("entity.paste", &["column:todo"], &kanban, &clipboard, &ui);
        assert!(!PasteCmd.available(&ctx));
    }

    #[tokio::test]
    async fn paste_not_available_without_target() {
        let (_temp, kanban, clipboard, ui) = setup().await;
        ui.set_has_clipboard(true);
        let ctx = make_ctx("entity.paste", &[], &kanban, &clipboard, &ui);
        assert!(!PasteCmd.available(&ctx));
    }
}
