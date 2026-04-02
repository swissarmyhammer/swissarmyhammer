//! Task-related command implementations: add, move, tag, untag, delete.

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

/// Move a task to the top of the todo (first) column.
///
/// Requires `task` in the scope chain. Finds the first column on the board,
/// loads the first task in that column, and dispatches a `MoveTask` with
/// `before_id` set to that first task so the target lands at position zero.
pub struct DoThisNextCmd;

#[async_trait]
impl Command for DoThisNextCmd {
    fn available(&self, ctx: &CommandContext) -> bool {
        ctx.has_in_scope("task")
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let task_id = ctx
            .resolve_entity_id("task")
            .ok_or_else(|| CommandError::MissingScope("task".into()))?;

        // Find the first column (todo) on the board.
        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let columns = ectx
            .list("column")
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let mut sorted_columns = columns;
        sorted_columns.sort_by(|a, b| {
            let oa = a.get_str("order").unwrap_or("0");
            let ob = b.get_str("order").unwrap_or("0");
            oa.cmp(ob)
        });

        let todo_column = sorted_columns
            .first()
            .ok_or_else(|| CommandError::ExecutionFailed("no columns on board".into()))?;
        let todo_col_id = todo_column.id.as_str();

        // Find the first task in the todo column (by ordinal).
        let all_tasks = ectx
            .list("task")
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        let mut todo_tasks: Vec<_> = all_tasks
            .into_iter()
            .filter(|t| {
                t.get_str("position_column") == Some(todo_col_id) && t.id.as_str() != task_id
            })
            .collect();
        todo_tasks.sort_by(|a, b| {
            let oa = a
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            let ob = b
                .get_str("position_ordinal")
                .unwrap_or(Ordinal::DEFAULT_STR);
            oa.cmp(ob)
        });

        let mut op = crate::task::MoveTask::to_column(task_id, todo_col_id.to_string());

        // Place before the first task in todo, if any.
        if let Some(first_task) = todo_tasks.first() {
            let first_ord = Ordinal::from_string(
                first_task
                    .get_str("position_ordinal")
                    .unwrap_or(Ordinal::DEFAULT_STR),
            );
            let ordinal =
                crate::task_helpers::compute_ordinal_for_neighbors(None, Some(&first_ord));
            op = op.with_ordinal(ordinal.as_str());
        }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::swimlane::AddSwimlane;
    use crate::task::{AddTask, TagTask};
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::CommandContext;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Create a fresh board with the default todo/doing/done columns.
    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    /// Build a CommandContext with args, kanban extension, and optional scope/target.
    fn make_ctx(
        kanban: Arc<KanbanContext>,
        args: HashMap<String, Value>,
        scope: Vec<String>,
        target: Option<String>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope, target, args);
        ctx.set_extension(kanban);
        ctx
    }

    /// Helper to create a task in the given column, returning its ID.
    async fn create_task(ctx: &KanbanContext, title: &str, column: &str) -> String {
        let mut op = AddTask::new(title);
        op.column = Some(column.to_string());
        let result = op.execute(ctx).await.into_result().unwrap();
        result["id"].as_str().unwrap().to_string()
    }

    // =========================================================================
    // AddTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn add_task_cmd_creates_task_in_column() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("title".into(), Value::String("My task".into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec!["column:todo".into()], None);
        let cmd = AddTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert!(result["id"].is_string());
        assert_eq!(result["title"], "My task");
    }

    #[tokio::test]
    async fn add_task_cmd_uses_column_from_args() {
        let (_temp, ctx) = setup().await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("title".into(), Value::String("Arg task".into()));
        args.insert("column".into(), Value::String("doing".into()));

        // No column in scope — should use args.column
        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = AddTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
    }

    // =========================================================================
    // MoveTaskCmd — basic column move
    // =========================================================================

    #[tokio::test]
    async fn move_task_to_different_column() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Movable", "todo").await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_id.clone()));
        args.insert("column".into(), Value::String("doing".into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
    }

    // =========================================================================
    // MoveTaskCmd — explicit ordinal
    // =========================================================================

    #[tokio::test]
    async fn move_task_with_explicit_ordinal() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Ordinal task", "todo").await;
        let kanban = Arc::new(ctx);

        // Use a valid FractionalIndex ordinal (Ordinal::after default "80")
        let target_ordinal = Ordinal::after(&Ordinal::first());
        let ord_str = target_ordinal.as_str().to_string();

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_id.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("ordinal".into(), Value::String(ord_str.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        assert_eq!(result["position"]["ordinal"], ord_str);
    }

    // =========================================================================
    // MoveTaskCmd — before_id positioning
    // =========================================================================

    #[tokio::test]
    async fn move_task_before_first_task() {
        let (_temp, ctx) = setup().await;
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "todo").await;
        let kanban = Arc::new(ctx);

        // Move B before A in "doing"
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_b.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("before_id".into(), Value::String(task_a.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        // B's ordinal should be less than A's
        let b_ord = result["position"]["ordinal"].as_str().unwrap();

        // Verify A's ordinal for comparison
        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        assert!(b_ord < a_ord, "B ({b_ord}) should sort before A ({a_ord})");
    }

    #[tokio::test]
    async fn move_task_before_middle_task() {
        let (_temp, ctx) = setup().await;
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "doing").await;
        let task_c = create_task(&ctx, "Task C", "todo").await;
        let kanban = Arc::new(ctx);

        // Move C before B — should land between A and B
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_c.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("before_id".into(), Value::String(task_b.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        let c_ord = result["position"]["ordinal"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let b_entity = ectx.read("task", &task_b).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        let b_ord = b_entity.get_str("position_ordinal").unwrap();

        assert!(a_ord < c_ord, "A ({a_ord}) should sort before C ({c_ord})");
        assert!(c_ord < b_ord, "C ({c_ord}) should sort before B ({b_ord})");
    }

    #[tokio::test]
    async fn move_task_before_nonexistent_appends() {
        let (_temp, ctx) = setup().await;
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "todo").await;
        let kanban = Arc::new(ctx);

        // before_id references a task not in the target column — should append
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_b.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("before_id".into(), Value::String("nonexistent-id".into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        // Should have gotten an ordinal (appended after A)
        let b_ord = result["position"]["ordinal"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        assert!(a_ord < b_ord, "A ({a_ord}) should sort before B ({b_ord})");
    }

    // =========================================================================
    // MoveTaskCmd — after_id positioning
    // =========================================================================

    #[tokio::test]
    async fn move_task_after_last_task() {
        let (_temp, ctx) = setup().await;
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "todo").await;
        let kanban = Arc::new(ctx);

        // Move B after A (A is the only/last task in doing)
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_b.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("after_id".into(), Value::String(task_a.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        let b_ord = result["position"]["ordinal"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        assert!(a_ord < b_ord, "A ({a_ord}) should sort before B ({b_ord})");
    }

    #[tokio::test]
    async fn move_task_after_middle_task() {
        let (_temp, ctx) = setup().await;
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "doing").await;
        let task_c = create_task(&ctx, "Task C", "todo").await;
        let kanban = Arc::new(ctx);

        // Move C after A — should land between A and B
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_c.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("after_id".into(), Value::String(task_a.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        let c_ord = result["position"]["ordinal"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let b_entity = ectx.read("task", &task_b).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        let b_ord = b_entity.get_str("position_ordinal").unwrap();

        assert!(a_ord < c_ord, "A ({a_ord}) should sort before C ({c_ord})");
        assert!(c_ord < b_ord, "C ({c_ord}) should sort before B ({b_ord})");
    }

    #[tokio::test]
    async fn move_task_after_nonexistent_appends() {
        let (_temp, ctx) = setup().await;
        let _task_a = create_task(&ctx, "Task A", "doing").await;
        let task_b = create_task(&ctx, "Task B", "todo").await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_b.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("after_id".into(), Value::String("nonexistent-id".into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        assert!(result["position"]["ordinal"].as_str().is_some());
    }

    // =========================================================================
    // MoveTaskCmd — drop_index positioning
    // =========================================================================

    #[tokio::test]
    async fn move_task_with_drop_index() {
        let (_temp, ctx) = setup().await;
        let _task_a = create_task(&ctx, "Task A", "doing").await;
        let _task_b = create_task(&ctx, "Task B", "doing").await;
        let task_c = create_task(&ctx, "Task C", "todo").await;
        let kanban = Arc::new(ctx);

        // Drop C at index 0 in doing (before A)
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_c.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("drop_index".into(), Value::Number(0.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        assert!(result["position"]["ordinal"].as_str().is_some());
    }

    // =========================================================================
    // MoveTaskCmd — swimlane
    // =========================================================================

    #[tokio::test]
    async fn move_task_with_swimlane() {
        let (_temp, ctx) = setup().await;
        // Create a swimlane
        AddSwimlane::new("urgent", "Urgent")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let task_id = create_task(&ctx, "Swim task", "todo").await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_id.clone()));
        args.insert("column".into(), Value::String("doing".into()));
        args.insert("swimlane".into(), Value::String("urgent".into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "doing");
        assert_eq!(result["position"]["swimlane"], "urgent");
    }

    // =========================================================================
    // MoveTaskCmd — scope chain and target moniker
    // =========================================================================

    #[tokio::test]
    async fn move_task_uses_scope_chain_for_task_id() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Scoped task", "todo").await;
        let kanban = Arc::new(ctx);

        // task in scope, column in args
        let mut args = HashMap::new();
        args.insert("column".into(), Value::String("doing".into()));

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            vec![format!("task:{task_id}")],
            None,
        );
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_uses_target_moniker_for_column() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Target task", "todo").await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_id.clone()));

        // column via target moniker instead of args
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            args,
            vec![],
            Some("column:doing".into()),
        );
        let cmd = MoveTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    // =========================================================================
    // UntagTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn untag_task_cmd_removes_tag() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Tagged task", "todo").await;

        // Tag it first
        TagTask::new(task_id.as_str(), "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let kanban = Arc::new(ctx);

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec![format!("tag:bug"), format!("task:{task_id}")],
            None,
        );
        let cmd = UntagTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        // Verify the tag was removed
        let tags = result["tags"].as_array().cloned().unwrap_or_default();
        assert!(
            !tags.iter().any(|t| t.as_str() == Some("bug")),
            "tag 'bug' should have been removed"
        );
    }

    // =========================================================================
    // DeleteTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn delete_task_cmd_removes_task() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Doomed task", "todo").await;
        let kanban = Arc::new(ctx);

        let mut args = HashMap::new();
        args.insert("id".into(), Value::String(task_id.clone()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args, vec![], None);
        let cmd = DeleteTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["deleted"], true);

        // Verify it's really gone
        let ectx = kanban.entity_context().await.unwrap();
        let read_result = ectx.read("task", &task_id).await;
        assert!(read_result.is_err(), "task should no longer exist");
    }

    #[tokio::test]
    async fn delete_task_cmd_uses_scope_chain() {
        let (_temp, ctx) = setup().await;
        let task_id = create_task(&ctx, "Scoped doomed", "todo").await;
        let kanban = Arc::new(ctx);

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec![format!("task:{task_id}")],
            None,
        );
        let cmd = DeleteTaskCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    // =========================================================================
    // DoThisNextCmd
    // =========================================================================

    #[tokio::test]
    async fn do_this_next_moves_task_to_top_of_todo() {
        let (_temp, ctx) = setup().await;

        // Create 3 tasks in todo — they get sequential ordinals.
        let task_a = create_task(&ctx, "Task A", "todo").await;
        let task_b = create_task(&ctx, "Task B", "todo").await;
        let task_c = create_task(&ctx, "Task C", "todo").await;
        let kanban = Arc::new(ctx);

        // Execute DoThisNext on task C — it should move to the top.
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec![format!("task:{task_c}")],
            None,
        );
        let cmd = DoThisNextCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        // C should now be in todo column
        assert_eq!(result["position"]["column"], "todo");

        // C's ordinal should be less than A's (top of the list)
        let c_ord = result["position"]["ordinal"].as_str().unwrap();

        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        assert!(c_ord < a_ord, "C ({c_ord}) should sort before A ({a_ord})");

        // B should still be after A
        let b_entity = ectx.read("task", &task_b).await.unwrap();
        let b_ord = b_entity.get_str("position_ordinal").unwrap();
        assert!(a_ord < b_ord, "A ({a_ord}) should sort before B ({b_ord})");
    }

    #[tokio::test]
    async fn do_this_next_from_different_column() {
        let (_temp, ctx) = setup().await;

        // Task in todo and task in doing
        let task_a = create_task(&ctx, "Task A", "todo").await;
        let task_b = create_task(&ctx, "Task B", "doing").await;
        let kanban = Arc::new(ctx);

        // DoThisNext on B (in doing) should move it to top of todo
        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec![format!("task:{task_b}")],
            None,
        );
        let cmd = DoThisNextCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        assert_eq!(result["position"]["column"], "todo");

        let b_ord = result["position"]["ordinal"].as_str().unwrap();
        let ectx = kanban.entity_context().await.unwrap();
        let a_entity = ectx.read("task", &task_a).await.unwrap();
        let a_ord = a_entity.get_str("position_ordinal").unwrap();
        assert!(b_ord < a_ord, "B ({b_ord}) should sort before A ({a_ord})");
    }

    #[tokio::test]
    async fn do_this_next_on_empty_todo() {
        let (_temp, ctx) = setup().await;

        // Only task is in doing — todo is empty
        let task_a = create_task(&ctx, "Task A", "doing").await;
        let kanban = Arc::new(ctx);

        let cmd_ctx = make_ctx(
            Arc::clone(&kanban),
            HashMap::new(),
            vec![format!("task:{task_a}")],
            None,
        );
        let cmd = DoThisNextCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();

        // Should succeed and place in todo
        assert_eq!(result["position"]["column"], "todo");
    }
}
