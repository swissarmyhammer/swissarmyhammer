//! TagTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{TagId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a tag to a task
#[operation(verb = "tag", noun = "task", description = "Add a tag to a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct TagTask {
    /// The task ID to tag
    pub id: TaskId,
    /// The tag ID to add
    pub tag: TagId,
}

impl TagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for TagTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            // Verify tag exists using file-based storage
            if !ctx.tag_exists(&self.tag).await {
                return Err(KanbanError::TagNotFound {
                    id: self.tag.to_string(),
                });
            }

            let mut task = ctx.read_task(&self.id).await?;

            // Add tag if not already present
            if !task.tags.contains(&self.tag) {
                task.tags.push(self.tag.clone());
                ctx.write_task(&task).await?;
            }

            Ok(serde_json::json!({
                "tagged": true,
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
