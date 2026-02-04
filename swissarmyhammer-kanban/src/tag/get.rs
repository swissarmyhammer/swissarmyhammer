//! GetTag command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TagId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a tag by ID
#[operation(verb = "get", noun = "tag", description = "Get a tag by ID")]
#[derive(Debug, Deserialize)]
pub struct GetTag {
    /// The tag ID to retrieve
    pub id: TagId,
}

impl GetTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let tag = ctx.read_tag(&self.id).await?;
            Ok(serde_json::to_value(&tag)?)
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
