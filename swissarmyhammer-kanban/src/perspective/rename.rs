//! RenamePerspective command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

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
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: std::result::Result<Value, KanbanError> = async {
            let pctx = ctx.perspective_context().await?;
            let mut pctx = pctx.write().await;

            let updated = pctx.rename(&self.id, &self.new_name).await?;
            Ok(perspective_to_json(&updated))
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
