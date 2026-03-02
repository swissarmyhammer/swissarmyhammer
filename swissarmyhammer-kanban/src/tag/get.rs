//! GetTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag::{find_tag_entity_by_name, tag_entity_to_json};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a tag by ID (ULID) or by name (slug).
///
/// If the `id` field looks like a ULID (26 chars), it's looked up directly.
/// Otherwise it's treated as a name and searched.
#[operation(verb = "get", noun = "tag", description = "Get a tag by ID or name")]
#[derive(Debug, Deserialize)]
pub struct GetTag {
    /// The tag ID (ULID) or name (slug) to retrieve
    pub id: String,
}

impl GetTag {
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;

            // Try direct ULID lookup first
            if let Ok(entity) = ectx.read("tag", &self.id).await {
                return Ok(tag_entity_to_json(&entity));
            }

            // Fall back to name lookup
            if let Some(entity) = find_tag_entity_by_name(&ectx, &self.id).await {
                return Ok(tag_entity_to_json(&entity));
            }

            Err(KanbanError::TagNotFound {
                id: self.id.clone(),
            })
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
