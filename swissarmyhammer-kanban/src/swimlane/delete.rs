//! DeleteSwimlane command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::SwimlaneId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Delete a swimlane (fails if it has tasks)
#[operation(verb = "delete", noun = "swimlane", description = "Delete an empty swimlane")]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteSwimlane {
    /// The swimlane ID to delete
    pub id: SwimlaneId,
}

impl DeleteSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let mut board = ctx.read_board().await?;

            // Check swimlane exists
            if board.find_swimlane(&self.id).is_none() {
                return Err(KanbanError::SwimlaneNotFound {
                    id: self.id.to_string(),
                });
            }

            // Check for tasks in this swimlane
            let tasks = ctx.read_all_tasks().await?;
            let task_count = tasks
                .iter()
                .filter(|t| t.position.swimlane.as_ref() == Some(&self.id))
                .count();

            if task_count > 0 {
                return Err(KanbanError::SwimlaneNotEmpty {
                    id: self.id.to_string(),
                    count: task_count,
                });
            }

            board.swimlanes.retain(|s| s.id != self.id);
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
