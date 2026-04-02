//! ListColumns command

use crate::column::add::column_entity_to_json;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all columns ordered by their `order` field.
#[operation(
    verb = "list",
    noun = "columns",
    description = "List all columns ordered by position"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListColumns;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListColumns {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let mut columns = ectx.list("column").await?;
            columns.sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

            let columns_json: Vec<Value> = columns.iter().map(column_entity_to_json).collect();

            Ok(serde_json::json!({
                "columns": columns_json,
                "count": columns_json.len()
            }))
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::column::add::AddColumn;
    use tempfile::TempDir;

    /// Create a temporary kanban context with a board initialized.
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

    #[tokio::test]
    async fn test_list_columns_returns_defaults() {
        let (_temp, ctx) = setup().await;

        let result = ListColumns.execute(&ctx).await.into_result().unwrap();

        let columns = result["columns"].as_array().unwrap();
        // InitBoard creates todo, doing, done
        assert!(columns.len() >= 3);
        assert!(columns.iter().any(|c| c["id"] == "todo"));
        assert!(columns.iter().any(|c| c["id"] == "doing"));
        assert!(columns.iter().any(|c| c["id"] == "done"));
    }

    #[tokio::test]
    async fn test_list_columns_includes_count() {
        let (_temp, ctx) = setup().await;

        let result = ListColumns.execute(&ctx).await.into_result().unwrap();

        let count = result["count"].as_u64().unwrap();
        let columns_len = result["columns"].as_array().unwrap().len() as u64;
        assert_eq!(count, columns_len);
    }

    #[tokio::test]
    async fn test_list_columns_sorted_by_order() {
        let (_temp, ctx) = setup().await;

        AddColumn::new("zz-last", "ZZ Last")
            .with_order(100)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddColumn::new("aa-first", "AA First")
            .with_order(0)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListColumns.execute(&ctx).await.into_result().unwrap();

        let columns = result["columns"].as_array().unwrap();
        let orders: Vec<u64> = columns
            .iter()
            .map(|c| c["order"].as_u64().unwrap_or(0))
            .collect();

        // Verify that the list is sorted ascending by order
        let mut sorted = orders.clone();
        sorted.sort();
        assert_eq!(orders, sorted);
    }

    #[tokio::test]
    async fn test_list_columns_after_add() {
        let (_temp, ctx) = setup().await;

        AddColumn::new("blocked", "Blocked")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListColumns.execute(&ctx).await.into_result().unwrap();

        let columns = result["columns"].as_array().unwrap();
        assert!(columns.iter().any(|c| c["id"] == "blocked"));
    }
}
