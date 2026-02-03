//! DeleteComment command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{CommentId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Delete a comment
#[operation(verb = "delete", noun = "comment", description = "Delete a comment from a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteComment {
    /// The task ID containing the comment
    pub task_id: TaskId,
    /// The comment ID to delete
    pub id: CommentId,
}

impl DeleteComment {
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<CommentId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let mut task = ctx.read_task(&self.task_id).await?;

            // Check comment exists
            if !task.comments.iter().any(|c| c.id == self.id) {
                return Err(KanbanError::CommentNotFound {
                    id: self.id.to_string(),
                });
            }

            task.comments.retain(|c| c.id != self.id);
            ctx.write_task(&task).await?;

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

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.task_id.to_string()]
    }
}
