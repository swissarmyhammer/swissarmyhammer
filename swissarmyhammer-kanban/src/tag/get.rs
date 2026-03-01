//! GetTag command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TagId;
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
            // Try direct ULID lookup first
            let tag_id = TagId::from_string(&self.id);
            if ctx.tag_exists(&tag_id).await {
                let tag = ctx.read_tag(&tag_id).await?;
                let mut result = serde_json::to_value(&tag)?;
                result["id"] = serde_json::json!(&tag.id);
                return Ok(result);
            }

            // Fall back to name lookup
            if let Some(tag) = ctx.find_tag_by_name(&self.id).await? {
                let mut result = serde_json::to_value(&tag)?;
                result["id"] = serde_json::json!(&tag.id);
                return Ok(result);
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
