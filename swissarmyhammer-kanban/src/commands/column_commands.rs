//! Column command implementations: reorder.

use super::run_op;
use crate::context::KanbanContext;
use async_trait::async_trait;
use serde_json::{json, Value};
use swissarmyhammer_commands::{Command, CommandContext, CommandError};

/// Reorder columns by moving a single column to a target index.
///
/// The backend lists all columns sorted by their current order, removes the
/// moved column, inserts it at `target_index`, then assigns sequential order
/// values (0, 1, 2, …) and persists all changes.
pub struct ColumnReorderCmd;

#[async_trait]
impl Command for ColumnReorderCmd {
    fn available(&self, _ctx: &CommandContext) -> bool {
        true
    }

    async fn execute(&self, ctx: &CommandContext) -> swissarmyhammer_commands::Result<Value> {
        let kanban = ctx.require_extension::<KanbanContext>()?;

        let id = ctx
            .arg("id")
            .and_then(|v| v.as_str().map(String::from))
            .ok_or_else(|| CommandError::MissingArg("id".into()))?;

        let target_index = ctx
            .arg("target_index")
            .and_then(|v| v.as_u64().map(|n| n as usize))
            .ok_or_else(|| CommandError::MissingArg("target_index".into()))?;

        let ectx = kanban
            .entity_context()
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;

        // 1. List all columns sorted by current order
        let mut columns = ectx
            .list("column")
            .await
            .map_err(|e| CommandError::ExecutionFailed(e.to_string()))?;
        columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0));

        // 2. Find the column being moved
        let from_index = columns
            .iter()
            .position(|c| c.id == id)
            .ok_or_else(|| CommandError::ExecutionFailed(format!("column '{}' not found", id)))?;

        if from_index == target_index {
            return Ok(json!({ "updated": 0, "operation_ids": [] }));
        }

        // 3. Remove from current position and insert at target
        let moved = columns.remove(from_index);
        let insert_at = target_index.min(columns.len());
        columns.insert(insert_at, moved);

        // 4. Assign sequential order values and persist
        let mut operation_ids: Vec<String> = Vec::new();
        for (i, col) in columns.iter().enumerate() {
            let op = crate::column::UpdateColumn::new(col.id.as_str()).with_order(i);
            let result = run_op(&op, &kanban).await?;
            if let Some(op_id) = result.get("operation_id").and_then(|v| v.as_str()) {
                operation_ids.push(op_id.to_string());
            }
        }

        Ok(json!({ "updated": columns.len(), "operation_ids": operation_ids }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use serde_json::Value;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer_commands::CommandContext;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

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

    fn make_ctx(kanban: Arc<KanbanContext>, args: HashMap<String, Value>) -> CommandContext {
        let mut ctx = CommandContext::new("test", vec![], None, args);
        ctx.set_extension(kanban);
        ctx
    }

    #[tokio::test]
    async fn reorder_moves_first_to_last() {
        let (_temp, ctx) = setup().await;

        // InitBoard creates todo, doing, done columns
        // Move todo (index 0) to index 2
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("todo".into()));
        args.insert("target_index".into(), Value::Number(2.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let cmd = ColumnReorderCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["updated"], 3);

        // Verify order: doing=0, done=1, todo=2
        let ectx = kanban.entity_context().await.unwrap();
        let mut cols = ectx.list("column").await.unwrap();
        cols.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0));
        let ids: Vec<&str> = cols.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["doing", "done", "todo"]);
    }

    #[tokio::test]
    async fn reorder_moves_last_to_first() {
        let (_temp, ctx) = setup().await;

        // Move done (index 2) to index 0
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("done".into()));
        args.insert("target_index".into(), Value::Number(0.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let cmd = ColumnReorderCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["updated"], 3);

        let ectx = kanban.entity_context().await.unwrap();
        let mut cols = ectx.list("column").await.unwrap();
        cols.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0));
        let ids: Vec<&str> = cols.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(ids, vec!["done", "todo", "doing"]);
    }

    #[tokio::test]
    async fn reorder_same_index_is_noop() {
        let (_temp, ctx) = setup().await;

        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("todo".into()));
        args.insert("target_index".into(), Value::Number(0.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let cmd = ColumnReorderCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["updated"], 0);
    }

    #[tokio::test]
    async fn reorder_missing_id_errors() {
        let (_temp, ctx) = setup().await;

        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("nonexistent".into()));
        args.insert("target_index".into(), Value::Number(0.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let cmd = ColumnReorderCmd;
        let result = cmd.execute(&cmd_ctx).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reorder_clamps_target_index_to_end() {
        let (_temp, ctx) = setup().await;

        // target_index=99 should clamp to last position
        let kanban = Arc::new(ctx);
        let mut args = HashMap::new();
        args.insert("id".into(), Value::String("todo".into()));
        args.insert("target_index".into(), Value::Number(99.into()));

        let cmd_ctx = make_ctx(Arc::clone(&kanban), args);
        let cmd = ColumnReorderCmd;
        let result = cmd.execute(&cmd_ctx).await.unwrap();
        assert_eq!(result["updated"], 3);

        let ectx = kanban.entity_context().await.unwrap();
        let mut cols = ectx.list("column").await.unwrap();
        cols.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0));
        // todo should be at the end
        assert_eq!(cols.last().unwrap().id, "todo");
    }
}
