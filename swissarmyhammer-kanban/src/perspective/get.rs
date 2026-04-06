//! GetPerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a perspective by ID (ULID) or by name.
///
/// If the `id` field looks like a ULID, it is looked up directly.
/// Otherwise it is treated as a name and searched.
#[operation(
    verb = "get",
    noun = "perspective",
    description = "Get a perspective by ID or name"
)]
#[derive(Debug, Deserialize)]
pub struct GetPerspective {
    /// The perspective ID (ULID) or name to retrieve
    pub id: String,
}

impl GetPerspective {
    /// Create a new GetPerspective lookup.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetPerspective {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let pctx = ctx.perspective_context().await?;
            let pctx = pctx.read().await;

            // Try direct ID lookup first
            if let Some(p) = pctx.get_by_id(&self.id) {
                return Ok(perspective_to_json(p));
            }

            // Fall back to name lookup
            if let Some(p) = pctx.get_by_name(&self.id) {
                return Ok(perspective_to_json(p));
            }

            Err(KanbanError::NotFound {
                resource: "perspective".to_string(),
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
