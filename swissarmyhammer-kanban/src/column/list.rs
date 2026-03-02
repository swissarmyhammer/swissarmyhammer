//! ListColumns command

use crate::column::add::column_entity_to_json;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all columns
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
            columns.sort_by_key(|c| {
                c.get("order")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as usize
            });

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
