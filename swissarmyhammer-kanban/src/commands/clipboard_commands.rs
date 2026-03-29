//! Clipboard command implementations: copy, cut, paste.
//!
//! These commands operate on entities (currently tasks) via the scope chain.
//! They use `ClipboardProviderExt` for clipboard I/O and `UIState` for the
//! `has_clipboard` availability flag.

use super::run_op;
use crate::clipboard::ClipboardProviderExt;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Copy the focused entity's fields to the clipboard as JSON.
///
/// Available when a `task` is in the scope chain. Non-undoable — the source
/// entity is not modified. Sets `has_clipboard` on UIState.
pub struct CopyTaskCmd;

#[async_trait]
impl Command for CopyTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let clipboard_ext = ctx.require_extension::<ClipboardProviderExt>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Read the entity and serialize its fields to JSON
        let entity = kanban
            .read_entity_generic("task", task_id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let json_str = serde_json::to_string(&entity.to_json())
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        clipboard_ext.0.write_text(&json_str);

        // Set has_clipboard flag on UIState
        if let Some(ui) = ctx.ui_state.as_ref() {
            ui.set_has_clipboard(true);
        }

        Ok(json!({ "copied": task_id }))
    }
}

/// Cut the focused entity: copy its fields to the clipboard, then delete it.
///
/// Available when a `task` is in the scope chain. Undoable — the delete
/// participates in the undo stack. Sets `has_clipboard` on UIState.
pub struct CutTaskCmd;

#[async_trait]
impl Command for CutTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let clipboard_ext = ctx.require_extension::<ClipboardProviderExt>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Read the entity and copy to clipboard first
        let entity = kanban
            .read_entity_generic("task", task_id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let json_str = serde_json::to_string(&entity.to_json())
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        clipboard_ext.0.write_text(&json_str);

        // Set has_clipboard flag
        if let Some(ui) = ctx.ui_state.as_ref() {
            ui.set_has_clipboard(true);
        }

        // Now delete the task (this goes through the operation processor for undo)
        let delete_op = crate::task::DeleteTask::new(task_id);
        run_op(&delete_op, &kanban).await
    }
}

/// Paste a previously copied/cut entity into the target column.
///
/// Available when a `column` is in the scope chain. Reads JSON from the
/// clipboard provider, creates a new task with a fresh ID and the copied fields.
/// Undoable — the created task can be removed via undo.
pub struct PasteTaskCmd;

#[async_trait]
impl Command for PasteTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("column")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let clipboard_ext = ctx.require_extension::<ClipboardProviderExt>()?;

        let column_id = ctx
            .resolve_entity_id("column")
            .ok_or_else(|| CommandError::MissingScope("column".into()))?;

        // Read clipboard JSON
        let json_str = clipboard_ext.0.read_text().ok_or_else(|| {
            CommandError::ExecutionFailed("clipboard is empty".into())
        })?;

        let clipboard_data: Value = serde_json::from_str(&json_str)
            .map_err(|e| CommandError::ExecutionFailed(format!("invalid clipboard JSON: {}", e)))?;

        // Extract the title from the clipboard data, default to "Pasted task"
        let title = clipboard_data
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Pasted task");

        // Create a new task with the copied title in the target column
        let mut add_op = crate::task::AddTask::new(title);
        add_op.column = Some(column_id.to_string());

        // Copy over description if present
        if let Some(desc) = clipboard_data.get("description").and_then(|v| v.as_str()) {
            add_op = add_op.with_description(desc);
        }

        run_op(&add_op, &kanban).await
    }
}
