//! ListColumns command

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
            let mut columns = ctx.read_all_columns().await?;
            columns.sort_by_key(|c| c.order);
            let columns_json: Vec<Value> = columns
                .iter()
                .map(|c| {
                    let mut v = serde_json::to_value(c).unwrap_or(Value::Null);
                    v["id"] = serde_json::json!(&c.id);
                    v
                })
                .collect();

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
