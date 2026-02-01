//! ListTags command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::Tag;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// List all tags
#[operation(verb = "list", noun = "tags", description = "List all tags on the board")]
#[derive(Debug, Default, Deserialize)]
pub struct ListTags {}

impl ListTags {
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListTags {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let tags: Vec<&Tag> = board.tags.iter().collect();

        Ok(serde_json::json!({
            "tags": tags,
            "count": tags.len()
        }))
    }
}
