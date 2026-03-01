//! UntagTask command — removes `#tag` from task description

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag_parser;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Remove a tag from a task by removing `#tag` from its description.
///
/// The `tag` field is the tag name/slug (e.g. "bug").
#[operation(
    verb = "untag",
    noun = "task",
    description = "Remove a tag from a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UntagTask {
    /// The task ID to untag
    pub id: TaskId,
    /// The tag name (slug) to remove
    pub tag: String,
}

impl UntagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<String>) -> Self {
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
            let slug = tag_parser::normalize_slug(&self.tag);
            let mut task = ctx.read_task(&self.id).await?;

            // Check if tag is present in description
            let was_present = task.tags().iter().any(|t| t == &slug);

            // Remove #tag from description
            let new_desc = tag_parser::remove_tag(&task.description, &slug);
            if new_desc != task.description {
                task.description = new_desc;
                ctx.write_task(&task).await?;
            }

            Ok(serde_json::json!({
                "untagged": was_present,
                "task_id": self.id.to_string(),
                "tag": slug
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
