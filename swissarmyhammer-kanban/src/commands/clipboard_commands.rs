//! Clipboard command implementations: copy and cut entities.

use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{
    ClipboardMode, ClipboardState, Command, CommandContext, CommandError,
};

/// Copy a task's fields into the UIState clipboard with mode=Copy.
///
/// Available when `task` is in the scope chain.
/// Loads the task entity, snapshots all fields as JSON, and stores a
/// `ClipboardState` with `mode: Copy` in UIState.
pub struct CopyCmd;

#[async_trait]
impl Command for CopyCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("no UIState available".into()))?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Load the task entity and snapshot its fields
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let entity = ectx
            .read("task", task_id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let fields = entity.to_json();

        ui.set_clipboard(ClipboardState {
            mode: ClipboardMode::Copy,
            entity_type: "task".into(),
            entity_id: task_id.to_string(),
            fields,
        });

        Ok(serde_json::json!({ "copied": task_id }))
    }
}

/// Cut a task: snapshot its fields into the UIState clipboard with mode=Cut,
/// then delete the task.
///
/// Available when `task` is in the scope chain.
/// Marked `undoable: true` in YAML so the delete is wrapped in a transaction.
pub struct CutCmd;

#[async_trait]
impl Command for CutCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;
        let ui = ctx
            .ui_state
            .as_ref()
            .ok_or_else(|| CommandError::ExecutionFailed("no UIState available".into()))?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Load the task entity and snapshot its fields before deletion
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        let entity = ectx
            .read("task", task_id)
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let fields = entity.to_json();

        ui.set_clipboard(ClipboardState {
            mode: ClipboardMode::Cut,
            entity_type: "task".into(),
            entity_id: task_id.to_string(),
            fields,
        });

        // Delete the task via the existing operation
        let op = crate::task::DeleteTask::new(task_id);
        super::run_op(&op, &kanban).await?;

        Ok(serde_json::json!({ "cut": task_id }))
    }
}
