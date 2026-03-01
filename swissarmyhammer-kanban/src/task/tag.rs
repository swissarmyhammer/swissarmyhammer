//! TagTask command — appends `#tag` to task description

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::tag_parser;
use crate::types::{Tag, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a tag to a task by appending `#tag` to its description.
///
/// The `tag` field is the tag name/slug (e.g. "bug").
/// If the Tag object doesn't exist yet, it is auto-created with an auto-color.
#[operation(verb = "tag", noun = "task", description = "Add a tag to a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct TagTask {
    /// The task ID to tag
    pub id: TaskId,
    /// The tag name (slug) to add (e.g. "bug")
    pub tag: String,
}

impl TagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<String>) -> Self {
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

        let result: std::result::Result<Value, KanbanError> = async {
            let slug = tag_parser::normalize_slug(&self.tag);

            // Auto-create Tag object if it doesn't exist
            if !ctx.tag_name_exists(&slug).await? {
                let color = auto_color::auto_color(&slug).to_string();
                let tag = Tag::new(&slug).with_color(&color);
                ctx.write_tag(&tag).await?;
            }

            let mut task = ctx.read_task(&self.id).await?;

            // Append #tag to description if not already present
            let new_desc = tag_parser::append_tag(&task.description, &slug);
            if new_desc != task.description {
                task.description = new_desc;
                ctx.write_task(&task).await?;
            }

            Ok(serde_json::json!({
                "tagged": true,
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
