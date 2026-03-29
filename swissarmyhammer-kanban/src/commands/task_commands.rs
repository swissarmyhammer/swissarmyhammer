//! Task-related command implementations: add, move, untag, delete.

use super::run_op;
use crate::context::KanbanContext;
use crate::types::Ordinal;
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
        ctx.has_in_scope("column") || ctx.arg("column").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Column from scope chain, or fallback to args.column
        let column_id = ctx
            .resolve_entity_id("column")
            .or_else(|| ctx.arg("column").and_then(|v| v.as_str()))
            .ok_or_else(|| CommandError::MissingScope("column".into()))?;

        let title = ctx
            .arg("title")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| crate::task_helpers::default_task_title().to_string());

        let mut op = crate::task::AddTask::new(title);
        op.column = Some(column_id.to_string());

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
///   server-side from neighbor ordinals via `compute_ordinal_for_drop`), or
/// - `before_id` and/or `after_id` args (task IDs of the neighbors; ordinal is
///   computed server-side from their ordinals).
///
/// Optional: `swimlane` arg.
pub struct MoveTaskCmd;

#[async_trait]
impl Command for MoveTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task") || ctx.arg("id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        // Task ID from scope chain, or fallback to args.id
        let task_id = ctx
            .resolve_entity_id("task")
            .or_else(|| ctx.arg("id").and_then(|v| v.as_str()))
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

        // Determine ordinal: explicit > before_id/after_id placement > drop_index > append
        if let Some(ordinal) = ctx.arg("ordinal").and_then(|v| v.as_str()) {
            op = op.with_ordinal(ordinal);
        } else if ctx.arg("before_id").is_some() || ctx.arg("after_id").is_some() {
            // Placement-based ordering:
            //   before_id = "place me before this task"
            //   after_id  = "place me after this task"
            // We load all tasks in the target column (sorted by ordinal),
            // find the reference task, and compute an ordinal between it
            // and its neighbor. Only ONE entity (the moved task) is updated.
            let before_id = ctx.arg("before_id").and_then(|v| v.as_str());
            let after_id = ctx.arg("after_id").and_then(|v| v.as_str());

            let ectx = kanban
                .entity_context()
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

            // Load and sort all tasks in the target column (excluding the moved task)
            let all_tasks = ectx
                .list("task")
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
            let mut col_tasks: Vec<_> = all_tasks
                .into_iter()
                .filter(|t| {
                    t.get_str("position_column") == Some(&column) && t.id.as_str() != task_id
                })
                .collect();
            col_tasks.sort_by(|a, b| {
                let oa = a
                    .get_str("position_ordinal")
                    .unwrap_or(Ordinal::DEFAULT_STR);
                let ob = b
                    .get_str("position_ordinal")
                    .unwrap_or(Ordinal::DEFAULT_STR);
                oa.cmp(ob)
            });

            let ordinal = if let Some(ref_id) = before_id {
                // "Place me before ref_id" — find ref in sorted list,
                // compute ordinal between predecessor and ref.
                let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
                match ref_idx {
                    Some(0) => {
                        // Placing before the first task
                        let ref_ord = Ordinal::from_string(
                            col_tasks[0]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        crate::task_helpers::compute_ordinal_for_neighbors(None, Some(&ref_ord))
                    }
                    Some(idx) => {
                        // Between predecessor and ref
                        let pred_ord = Ordinal::from_string(
                            col_tasks[idx - 1]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        let ref_ord = Ordinal::from_string(
                            col_tasks[idx]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        crate::task_helpers::compute_ordinal_for_neighbors(
                            Some(&pred_ord),
                            Some(&ref_ord),
                        )
                    }
                    None => {
                        // ref not found — append at end
                        crate::task_helpers::compute_ordinal_for_neighbors(
                            col_tasks
                                .last()
                                .map(|t| {
                                    Ordinal::from_string(
                                        t.get_str("position_ordinal")
                                            .unwrap_or(Ordinal::DEFAULT_STR),
                                    )
                                })
                                .as_ref(),
                            None,
                        )
                    }
                }
            } else if let Some(ref_id) = after_id {
                // "Place me after ref_id" — find ref in sorted list,
                // compute ordinal between ref and successor.
                let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
                match ref_idx {
                    Some(idx) if idx == col_tasks.len() - 1 => {
                        // Placing after the last task
                        let ref_ord = Ordinal::from_string(
                            col_tasks[idx]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), None)
                    }
                    Some(idx) => {
                        // Between ref and successor
                        let ref_ord = Ordinal::from_string(
                            col_tasks[idx]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        let succ_ord = Ordinal::from_string(
                            col_tasks[idx + 1]
                                .get_str("position_ordinal")
                                .unwrap_or(Ordinal::DEFAULT_STR),
                        );
                        crate::task_helpers::compute_ordinal_for_neighbors(
                            Some(&ref_ord),
                            Some(&succ_ord),
                        )
                    }
                    None => {
                        // ref not found — append at end
                        crate::task_helpers::compute_ordinal_for_neighbors(
                            col_tasks
                                .last()
                                .map(|t| {
                                    Ordinal::from_string(
                                        t.get_str("position_ordinal")
                                            .unwrap_or(Ordinal::DEFAULT_STR),
                                    )
                                })
                                .as_ref(),
                            None,
                        )
                    }
                }
            } else {
                // Neither — shouldn't happen, append at end
                crate::task_helpers::compute_ordinal_for_neighbors(None, None)
            };

            op = op.with_ordinal(ordinal.as_str());
        } else if let Some(drop_index) = ctx.arg("drop_index").and_then(|v| v.as_u64()) {
            // Load tasks in the target column, sorted by ordinal, to compute
            // the new ordinal from the drop position.
            let all_tasks = kanban
                .list_entities_generic("task")
                .await
                .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

            let mut column_tasks: Vec<_> = all_tasks
                .into_iter()
                .filter(|t| t.get_str("position_column") == Some(&column) && t.id != task_id)
                .collect();
            column_tasks.sort_by(|a, b| {
                let oa = a
                    .get_str("position_ordinal")
                    .unwrap_or(Ordinal::DEFAULT_STR);
                let ob = b
                    .get_str("position_ordinal")
                    .unwrap_or(Ordinal::DEFAULT_STR);
                oa.cmp(ob)
            });

            let ordinal =
                crate::task_helpers::compute_ordinal_for_drop(&column_tasks, drop_index as usize);
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
/// Requires `task` in the scope chain or `id` in args.
pub struct DeleteTaskCmd;

#[async_trait]
impl Command for DeleteTaskCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task") || ctx.arg("id").and_then(|v| v.as_str()).is_some()
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .or_else(|| ctx.arg("id").and_then(|v| v.as_str()))
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        let op = crate::task::DeleteTask::new(task_id);

        run_op(&op, &kanban).await
    }
}
