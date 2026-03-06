//! Task-related command implementations: add, move, untag, delete.

use super::run_op;
use crate::context::KanbanContext;
use crate::types::{Ordinal, Position};
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Add a new task to the board.
///
/// Requires `column` in the scope chain to determine placement.
/// Optional args: `title` (defaults to "New task").
pub struct AddTaskCmd;

#[async_trait]
impl Command for AddTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("column")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let column_id = ctx
            .resolve_entity_id("column")
            .ok_or_else(|| CommandError::MissingScope("column".into()))?;

        let title = ctx
            .arg("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::task_helpers::default_task_title().to_string());

        let position = Position::new(column_id.into(), None, Ordinal::first());
        let op = crate::task::AddTask::new(title).with_position(position);

        run_op(&op, &kanban).await
    }
}

/// Move a task to a different column/position.
///
/// Requires `task` in the scope chain. Target column comes from the `target`
/// moniker or the `column` arg.
///
/// Position can be specified via:
/// - `ordinal` arg (explicit ordinal string), or
/// - `drop_index` arg (integer index in the target column; ordinal is computed
///   server-side from neighbor ordinals via `compute_ordinal_for_drop`)
///
/// Optional: `swimlane` arg.
pub struct MoveTaskCmd;

#[async_trait]
impl Command for MoveTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Column from target moniker or from args
        let column = ctx
            .target_moniker()
            .filter(|(t, _)| *t == "column")
            .map(|(_, id)| id.to_string())
            .or_else(|| {
                ctx.arg("column")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .ok_or_else(|| CommandError::MissingArg("column".into()))?;

        let mut op = crate::task::MoveTask::to_column(task_id, column.clone());

        // Determine ordinal: explicit string, or computed from drop_index
        if let Some(ordinal) = ctx.arg("ordinal").and_then(|v| v.as_str()) {
            op = op.with_ordinal(ordinal);
        } else if let Some(drop_index) = ctx.arg("drop_index").and_then(|v| v.as_u64()) {
            // Load tasks in the target column, sorted by ordinal, to compute
            // the new ordinal from the drop position.
            let all_tasks = kanban
                .list_entities_generic("task")
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

            let mut column_tasks: Vec<_> = all_tasks
                .into_iter()
                .filter(|t| {
                    t.get_str("position_column") == Some(&column) && t.id != task_id
                })
                .collect();
            column_tasks
                .sort_by(|a, b| {
                    let oa = a.get_str("position_ordinal").unwrap_or("a0");
                    let ob = b.get_str("position_ordinal").unwrap_or("a0");
                    oa.cmp(ob)
                });

            let ordinal = crate::task_helpers::compute_ordinal_for_drop(
                &column_tasks,
                drop_index as usize,
            );
            op = op.with_ordinal(ordinal.as_str());
        }

        if let Some(swimlane) = ctx.arg("swimlane").and_then(|v| v.as_str()) {
            op.swimlane = Some(swimlane.into());
        }

        run_op(&op, &kanban).await
    }
}

/// Remove a tag from a task.
///
/// Requires both `tag` and `task` in the scope chain.
pub struct UntagTaskCmd;

#[async_trait]
impl Command for UntagTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("tag") && ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;
        let tag_name = ctx
            .resolve_entity_id("tag")
            .ok_or_else(|| CommandError::MissingScope("tag".into()))?;

        let op = crate::task::UntagTask::new(task_id, tag_name);

        run_op(&op, &kanban).await
    }
}

/// Delete a task.
///
/// Requires `task` in the scope chain.
pub struct DeleteTaskCmd;

#[async_trait]
impl Command for DeleteTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        let op = crate::task::DeleteTask::new(task_id);

        run_op(&op, &kanban).await
    }
}
