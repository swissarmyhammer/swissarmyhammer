//! ListTags command

use crate::context::KanbanContext;
use crate::error::KanbanError;
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
            let tags = ctx.read_all_tags().await?;

            Ok(serde_json::json!({
                "tags": tags,
                "count": tags.len()
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
