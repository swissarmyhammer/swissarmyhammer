//! DeletePerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a perspective by ID.
///
/// Reads the perspective for a changelog snapshot, removes it from storage,
/// and logs the deletion.
#[operation(
    verb = "delete",
    noun = "perspective",
    description = "Delete a perspective"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeletePerspective {
    /// The perspective ID (ULID) to delete
    pub id: String,
}

impl DeletePerspective {
    /// Create a new DeletePerspective targeting the given ID.
    pub fn new(id: impl Into<String>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeletePerspective {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            // Delete returns the removed perspective for changelog snapshot
            let deleted = pctx.delete(&self.id).await?;

            // Log to changelog
            if let Err(e) = ctx.perspective_changelog().log_delete(&deleted).await {
                tracing::warn!(%e, "failed to log perspective delete");
            }

            Ok(serde_json::json!({
                "deleted": true,
                "id": deleted.id,
                "name": deleted.name,
            }))
        }
        .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        match result {
            Ok(value) => ExecutionResult::Logged {
                value: value.clone(),
                log_entry: LogEntry::new(self.op_string(), input, value, None, duration_ms),
            },
            Err(error) => {
                let error_msg = error.to_string();
                ExecutionResult::Failed {
                    error,
                    log_entry: Some(LogEntry::new(
                        self.op_string(),
                        input,
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
    }
}
