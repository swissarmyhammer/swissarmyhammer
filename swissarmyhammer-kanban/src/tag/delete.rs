//! DeleteTag command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TagId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Delete a tag (removes from all tasks)
#[operation(verb = "delete", noun = "tag", description = "Delete a tag and remove from all tasks")]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteTag {
    /// The tag ID to delete
    pub id: TagId,
}

impl DeleteTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let mut board = ctx.read_board().await?;

            // Check tag exists
            if !board.tags.iter().any(|t| t.id == self.id) {
                return Err(KanbanError::TagNotFound {
                    id: self.id.to_string(),
                });
            }

            // Remove tag from all tasks
            let task_ids = ctx.list_task_ids().await?;
            for id in task_ids {
                let mut task = ctx.read_task(&id).await?;
                if task.tags.contains(&self.id) {
                    task.tags.retain(|t| t != &self.id);
                    ctx.write_task(&task).await?;
                }
            }

            // Remove from board
            board.tags.retain(|t| t.id != self.id);
            ctx.write_board(&board).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string()
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
