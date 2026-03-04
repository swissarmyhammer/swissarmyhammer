//! ListSwimlanes command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::swimlane::swimlane_entity_to_json;
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
            let ectx = ctx.entity_context().await?;
            let mut swimlanes = ectx.list("swimlane").await?;
            swimlanes
                .sort_by_key(|s| s.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);

            let swimlanes_json: Vec<Value> =
                swimlanes.iter().map(swimlane_entity_to_json).collect();

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
