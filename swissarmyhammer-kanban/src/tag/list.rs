//! ListTags command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::tag_entity_to_json;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all tags
#[operation(
    verb = "list",
    noun = "tags",
    description = "List all tags on the board"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListTags {}

impl ListTags {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListTags {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let tags = ectx.list("tag").await?;
            let tags_json: Vec<Value> = tags.iter().map(tag_entity_to_json).collect();

            Ok(serde_json::json!({
                "tags": tags_json,
                "count": tags_json.len()
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
