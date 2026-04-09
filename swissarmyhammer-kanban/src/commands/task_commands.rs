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
    use crate::task::AddTask;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::CommandContext;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Initialize a board and return a (TempDir, KanbanContext) pair.
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

    /// Build a CommandContext with scope, target, args, and a KanbanContext extension.
    fn make_ctx(
        kanban: Arc<KanbanContext>,
        scope: Vec<String>,
        target: Option<String>,
        args: HashMap<String, serde_json::Value>,
    ) -> CommandContext {
        let mut ctx = CommandContext::new("test", scope, target, args);
        ctx.set_extension(kanban);
        ctx
    }

    // =========================================================================
    // AddTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn add_task_cmd_execute_with_column_in_scope() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = AddTaskCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec!["column:todo".into()],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "todo");
        // Default title
        assert!(result["title"].as_str().is_some());
    }

    #[tokio::test]
    async fn add_task_cmd_execute_with_title_arg() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = AddTaskCmd;

        let mut args = HashMap::new();
        args.insert("title".into(), serde_json::json!("Custom title"));
        let ctx = make_ctx(Arc::clone(&kanban), vec!["column:doing".into()], None, args);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["title"], "Custom title");
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn add_task_cmd_execute_with_column_arg() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = AddTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("done"));
        args.insert("title".into(), serde_json::json!("From arg"));
        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, args);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["title"], "From arg");
        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn add_task_cmd_fails_without_column() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = AddTaskCmd;

        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, HashMap::new());
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without column");
    }

    #[tokio::test]
    async fn add_task_cmd_fails_without_kanban_context() {
        let cmd = AddTaskCmd;
        let ctx = CommandContext::new("task.add", vec!["column:todo".into()], None, HashMap::new());
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without KanbanContext");
    }

    // =========================================================================
    // AddTaskCmd availability
    // =========================================================================

    #[test]
    fn add_task_available_with_column_scope() {
        let ctx = CommandContext::new("task.add", vec!["column:todo".into()], None, HashMap::new());
        assert!(AddTaskCmd.available(&ctx));
    }

    #[test]
    fn add_task_available_with_column_arg() {
        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        let ctx = CommandContext::new("task.add", vec![], None, args);
        assert!(AddTaskCmd.available(&ctx));
    }

    #[test]
    fn add_task_not_available_without_column() {
        let ctx = CommandContext::new("task.add", vec![], None, HashMap::new());
        assert!(!AddTaskCmd.available(&ctx));
    }

    // =========================================================================
    // MoveTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn move_task_cmd_execute_basic() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Movable")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{task_id}")],
            None,
            args,
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_cmd_with_target_moniker() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Moniker move")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        // Target moniker provides column
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{task_id}")],
            Some("column:done".into()),
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn move_task_cmd_with_explicit_ordinal() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Ordinal move")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        args.insert("ordinal".into(), serde_json::json!("m5"));
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{task_id}")],
            None,
            args,
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
        // The ordinal is passed through to the MoveTask operation
        assert!(result["position"]["ordinal"].as_str().is_some());
    }

    #[tokio::test]
    async fn move_task_cmd_with_before_id() {
        let (_temp, kctx) = setup().await;
        // Add two tasks in doing
        let mut doing_op = AddTask::new("First");
        doing_op.column = Some("doing".into());
        let r1 = doing_op.execute(&kctx).await.into_result().unwrap();
        let first_id = r1["id"].as_str().unwrap().to_string();

        let task_to_move = AddTask::new("Mover")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let mover_id = task_to_move["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        args.insert("before_id".into(), serde_json::json!(first_id));
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{mover_id}")],
            None,
            args,
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_cmd_with_after_id() {
        let (_temp, kctx) = setup().await;
        let mut doing_op = AddTask::new("Anchor");
        doing_op.column = Some("doing".into());
        let r1 = doing_op.execute(&kctx).await.into_result().unwrap();
        let anchor_id = r1["id"].as_str().unwrap().to_string();

        let mover = AddTask::new("After mover")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let mover_id = mover["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        args.insert("after_id".into(), serde_json::json!(anchor_id));
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{mover_id}")],
            None,
            args,
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_cmd_with_drop_index() {
        let (_temp, kctx) = setup().await;
        // Add two tasks in doing
        let mut doing_op1 = AddTask::new("D1");
        doing_op1.column = Some("doing".into());
        doing_op1.execute(&kctx).await.into_result().unwrap();

        let mut doing_op2 = AddTask::new("D2");
        doing_op2.column = Some("doing".into());
        doing_op2.execute(&kctx).await.into_result().unwrap();

        let mover = AddTask::new("Drop mover")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let mover_id = mover["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        args.insert("drop_index".into(), serde_json::json!(1));
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{mover_id}")],
            None,
            args,
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_cmd_with_id_arg() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Id arg move")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("id".into(), serde_json::json!(task_id));
        args.insert("column".into(), serde_json::json!("doing"));
        // No task in scope — using id arg
        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, args);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["position"]["column"], "doing");
    }

    #[tokio::test]
    async fn move_task_cmd_fails_without_task() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let mut args = HashMap::new();
        args.insert("column".into(), serde_json::json!("doing"));
        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, args);
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without task id");
    }

    #[tokio::test]
    async fn move_task_cmd_fails_without_column() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("No col")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = MoveTaskCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{task_id}")],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without column");
    }

    // =========================================================================
    // MoveTaskCmd availability
    // =========================================================================

    #[test]
    fn move_task_available_with_task_scope() {
        let ctx = CommandContext::new("task.move", vec!["task:01X".into()], None, HashMap::new());
        assert!(MoveTaskCmd.available(&ctx));
    }

    #[test]
    fn move_task_available_with_id_arg() {
        let mut args = HashMap::new();
        args.insert("id".into(), serde_json::json!("task-1"));
        let ctx = CommandContext::new("task.move", vec![], None, args);
        assert!(MoveTaskCmd.available(&ctx));
    }

    #[test]
    fn move_task_not_available_without_task_or_id() {
        let ctx = CommandContext::new("task.move", vec![], None, HashMap::new());
        assert!(!MoveTaskCmd.available(&ctx));
    }

    // =========================================================================
    // UntagTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn untag_task_cmd_execute() {
        let (_temp, kctx) = setup().await;

        // Add task with tag
        let add_result = AddTask::new("Tagged")
            .with_description("Has #bug")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(kctx);
        let cmd = UntagTaskCmd;

        // The scope chain uses the tag name (slug) as the ID part of the moniker,
        // because UntagTask::new expects the tag name, not the ULID.
        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec!["tag:bug".into(), format!("task:{task_id}")],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["untagged"], true);
    }

    #[tokio::test]
    async fn untag_task_cmd_fails_without_task() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = UntagTaskCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec!["tag:01X".into()],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without task in scope");
    }

    #[tokio::test]
    async fn untag_task_cmd_fails_without_tag() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = UntagTaskCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec!["task:01X".into()],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without tag in scope");
    }

    // =========================================================================
    // UntagTaskCmd availability
    // =========================================================================

    #[test]
    fn untag_available_with_both() {
        let ctx = CommandContext::new(
            "task.untag",
            vec!["tag:01X".into(), "task:01Y".into()],
            None,
            HashMap::new(),
        );
        assert!(UntagTaskCmd.available(&ctx));
    }

    #[test]
    fn untag_not_available_without_tag() {
        let ctx = CommandContext::new("task.untag", vec!["task:01X".into()], None, HashMap::new());
        assert!(!UntagTaskCmd.available(&ctx));
    }

    #[test]
    fn untag_not_available_without_task() {
        let ctx = CommandContext::new("task.untag", vec!["tag:01X".into()], None, HashMap::new());
        assert!(!UntagTaskCmd.available(&ctx));
    }

    // =========================================================================
    // DeleteTaskCmd
    // =========================================================================

    #[tokio::test]
    async fn delete_task_cmd_execute_via_scope() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Deletable")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = DeleteTaskCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{task_id}")],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn delete_task_cmd_execute_via_id_arg() {
        let (_temp, kctx) = setup().await;
        let add_result = AddTask::new("Deletable via arg")
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap().to_string();
        let kanban = Arc::new(kctx);
        let cmd = DeleteTaskCmd;

        let mut args = HashMap::new();
        args.insert("id".into(), serde_json::json!(task_id));
        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, args);
        let result = cmd.execute(&ctx).await.unwrap();
        assert_eq!(result["deleted"], true);
    }

    #[tokio::test]
    async fn delete_task_cmd_fails_without_task() {
        let (_temp, kctx) = setup().await;
        let kanban = Arc::new(kctx);
        let cmd = DeleteTaskCmd;

        let ctx = make_ctx(Arc::clone(&kanban), vec![], None, HashMap::new());
        let result = cmd.execute(&ctx).await;
        assert!(result.is_err(), "should fail without task id");
    }

    // =========================================================================
    // DeleteTaskCmd availability
    // =========================================================================

    #[test]
    fn delete_task_available_with_task_scope() {
        let ctx = CommandContext::new("task.delete", vec!["task:01X".into()], None, HashMap::new());
        assert!(DeleteTaskCmd.available(&ctx));
    }

    #[test]
    fn delete_task_available_with_id_arg() {
        let mut args = HashMap::new();
        args.insert("id".into(), serde_json::json!("task-1"));
        let ctx = CommandContext::new("task.delete", vec![], None, args);
        assert!(DeleteTaskCmd.available(&ctx));
    }

    #[test]
    fn delete_task_not_available_without_task_or_id() {
        let ctx = CommandContext::new("task.delete", vec![], None, HashMap::new());
        assert!(!DeleteTaskCmd.available(&ctx));
    }
}
