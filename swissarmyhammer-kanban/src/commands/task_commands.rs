//! Task-related command implementations: add, move, tag, untag, delete.

use super::run_op;
use crate::context::KanbanContext;
use crate::types::Ordinal;
use async_trait::async_trait;
use serde_json::Value;
use swissarmyhammer_commands::{Command, CommandContext, CommandError};
use swissarmyhammer_entity::Entity;

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
        let (task_id, column) = resolve_move_task_args(ctx)?;

        let mut op = crate::task::MoveTask::to_column(task_id.clone(), column.clone());

        // Determine ordinal: explicit > before_id/after_id placement > drop_index > append
        if let Some(ordinal) = ctx.arg("ordinal").and_then(|v| v.as_str()) {
            op = op.with_ordinal(ordinal);
        } else if ctx.arg("before_id").is_some() || ctx.arg("after_id").is_some() {
            let before_id = ctx.arg("before_id").and_then(|v| v.as_str());
            let after_id = ctx.arg("after_id").and_then(|v| v.as_str());
            let ordinal =
                compute_placement_ordinal(&kanban, &column, &task_id, before_id, after_id).await?;
            op = op.with_ordinal(ordinal.as_str());
        } else if let Some(drop_index) = ctx.arg("drop_index").and_then(|v| v.as_u64()) {
            let ordinal =
                compute_drop_ordinal(&kanban, &column, &task_id, drop_index as usize).await?;
            op = op.with_ordinal(ordinal.as_str());
        }

        run_op(&op, &kanban).await
    }
}

/// Resolve the `(task_id, column)` pair for a move-task command.
///
/// Task ID comes from the `task` scope chain entry, falling back to the `id`
/// arg. Column comes from the `column` target moniker, falling back to the
/// `column` arg. Returns `MissingScope` / `MissingArg` errors when neither
/// source is provided.
fn resolve_move_task_args(ctx: &CommandContext) -> Result<(String, String), CommandError> {
    let task_id = ctx
        .resolve_entity_id("task")
        .or_else(|| ctx.arg("id").and_then(|v| v.as_str()))
        .ok_or_else(|| CommandError::MissingScope("task".into()))?
        .to_string();

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

    Ok((task_id, column))
}

/// Load all tasks currently in `column`, excluding `exclude_task_id`, sorted
/// by `position_ordinal` ascending.
///
/// Used by both the before/after placement path and the drop-index path to
/// materialize the neighbor list needed for ordinal computation.
async fn load_sorted_column_tasks(
    kanban: &KanbanContext,
    column: &str,
    exclude_task_id: &str,
) -> Result<Vec<Entity>, CommandError> {
    let all_tasks = kanban
        .list_entities_generic("task")
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

    let mut col_tasks: Vec<Entity> = all_tasks
        .into_iter()
        .filter(|t| {
            t.get_str("position_column") == Some(column) && t.id.as_str() != exclude_task_id
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
    Ok(col_tasks)
}

/// Return an entity's `position_ordinal` parsed into an [`Ordinal`], falling
/// back to [`Ordinal::DEFAULT_STR`] when the field is missing.
fn task_ordinal(task: &Entity) -> Ordinal {
    Ordinal::from_string(
        task.get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR),
    )
}

/// Compute an ordinal that places the moved task *before* `ref_id`.
///
/// Finds `ref_id` in the sorted column list and returns an ordinal between
/// the predecessor (or nothing, at index 0) and `ref_id`. When `ref_id` is
/// not found the moved task is appended to the end.
fn ordinal_for_before(col_tasks: &[Entity], ref_id: &str) -> Ordinal {
    let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
    match ref_idx {
        Some(0) => {
            // Placing before the first task
            let ref_ord = task_ordinal(&col_tasks[0]);
            crate::task_helpers::compute_ordinal_for_neighbors(None, Some(&ref_ord))
        }
        Some(idx) => {
            // Between predecessor and ref
            let pred_ord = task_ordinal(&col_tasks[idx - 1]);
            let ref_ord = task_ordinal(&col_tasks[idx]);
            crate::task_helpers::compute_ordinal_for_neighbors(Some(&pred_ord), Some(&ref_ord))
        }
        None => {
            // ref not found — append at end
            crate::task_helpers::compute_ordinal_for_neighbors(
                col_tasks.last().map(task_ordinal).as_ref(),
                None,
            )
        }
    }
}

/// Compute an ordinal that places the moved task *after* `ref_id`.
///
/// Finds `ref_id` in the sorted column list and returns an ordinal between
/// `ref_id` and its successor (or nothing, at the end). When `ref_id` is
/// not found the moved task is appended to the end.
fn ordinal_for_after(col_tasks: &[Entity], ref_id: &str) -> Ordinal {
    let ref_idx = col_tasks.iter().position(|t| t.id.as_str() == ref_id);
    match ref_idx {
        Some(idx) if idx == col_tasks.len() - 1 => {
            // Placing after the last task
            let ref_ord = task_ordinal(&col_tasks[idx]);
            crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), None)
        }
        Some(idx) => {
            // Between ref and successor
            let ref_ord = task_ordinal(&col_tasks[idx]);
            let succ_ord = task_ordinal(&col_tasks[idx + 1]);
            crate::task_helpers::compute_ordinal_for_neighbors(Some(&ref_ord), Some(&succ_ord))
        }
        None => {
            // ref not found — append at end
            crate::task_helpers::compute_ordinal_for_neighbors(
                col_tasks.last().map(task_ordinal).as_ref(),
                None,
            )
        }
    }
}

/// Compute the ordinal for a placement-based move (`before_id` / `after_id`).
///
/// Loads and sorts the target column, then dispatches to [`ordinal_for_before`]
/// or [`ordinal_for_after`]. `before_id` wins when both are supplied; if
/// neither is set the moved task is appended to the end (this is a defensive
/// fallback — callers only invoke this path when at least one is set).
async fn compute_placement_ordinal(
    kanban: &KanbanContext,
    column: &str,
    task_id: &str,
    before_id: Option<&str>,
    after_id: Option<&str>,
) -> Result<Ordinal, CommandError> {
    // Placement-based ordering:
    //   before_id = "place me before this task"
    //   after_id  = "place me after this task"
    // We load all tasks in the target column (sorted by ordinal),
    // find the reference task, and compute an ordinal between it
    // and its neighbor. Only ONE entity (the moved task) is updated.
    let col_tasks = load_sorted_column_tasks(kanban, column, task_id).await?;

    let ordinal = if let Some(ref_id) = before_id {
        ordinal_for_before(&col_tasks, ref_id)
    } else if let Some(ref_id) = after_id {
        ordinal_for_after(&col_tasks, ref_id)
    } else {
        // Neither — shouldn't happen, append at end
        crate::task_helpers::compute_ordinal_for_neighbors(None, None)
    };
    Ok(ordinal)
}

/// Compute the ordinal for a drop-index move.
///
/// Loads and sorts the target column, then delegates to
/// [`crate::task_helpers::compute_ordinal_for_drop`] to produce an ordinal
/// that matches the drop position.
async fn compute_drop_ordinal(
    kanban: &KanbanContext,
    column: &str,
    task_id: &str,
    drop_index: usize,
) -> Result<Ordinal, CommandError> {
    let column_tasks = load_sorted_column_tasks(kanban, column, task_id).await?;
    Ok(crate::task_helpers::compute_ordinal_for_drop(
        &column_tasks,
        drop_index,
    ))
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

/// Return the id of the lowest-`order` column on the board.
async fn first_column_id(kanban: &KanbanContext) -> Result<String, CommandError> {
    let ectx = kanban
        .entity_context()
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    let mut columns = ectx
        .list("column")
        .await
        .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
    columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0));
    columns
        .into_iter()
        .next()
        .map(|c| c.id.as_str().to_string())
        .ok_or_else(|| CommandError::ExecutionFailed("no columns on board".into()))
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

        let todo_col_id = first_column_id(&kanban).await?;
        let todo_tasks = load_sorted_column_tasks(&kanban, &todo_col_id, task_id).await?;

        let mut op = crate::task::MoveTask::to_column(task_id, todo_col_id);
        if let Some(first_task) = todo_tasks.first() {
            let first_ord = task_ordinal(first_task);
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

    // =========================================================================
    // DoThisNextCmd
    // =========================================================================

    #[tokio::test]
    async fn do_this_next_moves_to_first_column() {
        use crate::column::UpdateColumn;

        let (_temp, kctx) = setup().await;

        // Swap orders so that `doing` becomes the order-0 (first) column and
        // `todo` becomes order 1. Creation order on disk remains todo -> doing
        // -> done, so any sort that falls back to filesystem iteration order
        // (the buggy `get_str("order")` sort does exactly that) will pick
        // `todo` instead of the correct `doing`.
        UpdateColumn::new("doing")
            .with_order(0)
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();
        UpdateColumn::new("todo")
            .with_order(1)
            .execute(&kctx)
            .await
            .into_result()
            .unwrap();

        // Anchor task already in `doing` (the new first column).
        let mut anchor_op = AddTask::new("Anchor");
        anchor_op.column = Some("doing".into());
        let anchor_result = anchor_op.execute(&kctx).await.into_result().unwrap();
        let anchor_ordinal = anchor_result["position"]["ordinal"]
            .as_str()
            .unwrap()
            .to_string();

        // Target task in `todo` — not the first column.
        let mut target_op = AddTask::new("Target");
        target_op.column = Some("todo".into());
        let target_result = target_op.execute(&kctx).await.into_result().unwrap();
        let target_id = target_result["id"].as_str().unwrap().to_string();

        let kanban = Arc::new(kctx);
        let cmd = DoThisNextCmd;

        let ctx = make_ctx(
            Arc::clone(&kanban),
            vec![format!("task:{target_id}")],
            None,
            HashMap::new(),
        );
        let result = cmd.execute(&ctx).await.unwrap();

        // Target landed in the order-0 column, which is now `doing`.
        assert_eq!(
            result["position"]["column"], "doing",
            "DoThisNextCmd must move to the order-0 column (doing), got {}",
            result["position"]["column"]
        );

        // Target is at the top: its ordinal sorts lexicographically before the anchor's.
        let target_new_ordinal = result["position"]["ordinal"].as_str().unwrap().to_string();
        assert!(
            target_new_ordinal.as_str() < anchor_ordinal.as_str(),
            "target ordinal {target_new_ordinal:?} should sort before anchor ordinal {anchor_ordinal:?}"
        );
    }
}
