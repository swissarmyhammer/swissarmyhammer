//! UntagTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{TagId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Remove a tag from a task
#[operation(verb = "untag", noun = "task", description = "Remove a tag from a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UntagTask {
    /// The task ID to untag
    pub id: TaskId,
    /// The tag ID to remove
    pub tag: TagId,
}

impl UntagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UntagTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.id).await?;

            // Remove tag if present
            let was_present = task.tags.contains(&self.tag);
            task.tags.retain(|t| t != &self.tag);

            if was_present {
                ctx.write_task(&task).await?;
            }

            Ok(serde_json::json!({
                "untagged": was_present,
                "task_id": self.id.to_string(),
                "tag_id": self.tag.to_string()
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

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
    }
}
