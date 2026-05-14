//! RenamePerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Rename a perspective atomically.
///
/// Looks up the perspective by ID and changes its name in a single write,
/// avoiding the non-atomic delete-then-create pattern.
#[operation(
    verb = "rename",
    noun = "perspective",
    description = "Rename a perspective"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct RenamePerspective {
    /// The perspective ID (ULID) to rename.
    pub id: String,
    /// The new name for the perspective.
    pub new_name: String,
}

impl RenamePerspective {
    /// Create a new RenamePerspective targeting the given ID with a new name.
    pub fn new(id: impl Into<String>, new_name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            new_name: new_name.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for RenamePerspective {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            // Read, mutate, and persist through `write` (instead of the
            // bare `rename` helper) so the legacy-view-id migration helper
            // gets a chance to pin `view_id` on the same save.
            let mut updated = pctx
                .get_by_id(&self.id)
                .ok_or_else(|| KanbanError::NotFound {
                    resource: "perspective".to_string(),
                    id: self.id.clone(),
                })?
                .clone();
            updated.name = self.new_name.clone();

            if let Some(views_lock) = ctx.views() {
                let views = views_lock.read().await;
                crate::perspective::migrate::maybe_pin_view_id_on_save(&mut updated, &views);
            }

            pctx.write(&updated).await?;
            Ok(perspective_to_json(&updated))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}
