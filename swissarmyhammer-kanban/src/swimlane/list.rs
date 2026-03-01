//! ListSwimlanes command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all swimlanes
#[operation(
    verb = "list",
    noun = "swimlanes",
    description = "List all swimlanes ordered by position"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListSwimlanes;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListSwimlanes {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let mut swimlanes = ctx.read_all_swimlanes().await?;
            swimlanes.sort_by_key(|s| s.order);
            let swimlanes_json: Vec<Value> = swimlanes
                .iter()
                .map(|s| {
                    let mut v = serde_json::to_value(s).unwrap_or(Value::Null);
                    v["id"] = serde_json::json!(&s.id);
                    v
                })
                .collect();

            Ok(serde_json::json!({
                "swimlanes": swimlanes_json,
                "count": swimlanes_json.len()
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
