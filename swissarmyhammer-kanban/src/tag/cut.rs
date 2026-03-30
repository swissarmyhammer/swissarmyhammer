//! CutTag operation — copy tag to clipboard and untag from the source task.

use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag_parser;
use crate::task_helpers::task_tags;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Cut a tag: snapshot it to clipboard JSON and remove it from the source task.
///
/// The untag mutation (removing `#tag` from the task body) is logged and
/// undoable. The clipboard JSON is returned for the Command layer to write
/// to the system clipboard.
#[operation(
    verb = "cut",
    noun = "tag",
    description = "Cut a tag (copy to clipboard and untag from task)"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CutTag {
    /// The task ID to untag from.
    pub task_id: TaskId,
    /// The tag name/slug to cut.
    pub tag: String,
}

impl CutTag {
    /// Create a new CutTag operation.
    pub fn new(task_id: impl Into<TaskId>, tag: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CutTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let slug = tag_parser::normalize_slug(&self.tag);
            let ectx = ctx.entity_context().await?;

            // Find the tag entity to snapshot its fields for the clipboard
            let tag_entity = crate::tag::find_tag_entity_by_name(&ectx, &slug)
                .await
                .ok_or_else(|| KanbanError::TagNotFound { id: slug.clone() })?;
            let tag_id = tag_entity.id.to_string();
            let fields = serde_json::to_value(&tag_entity.fields)?;
            let clipboard_json = clipboard::serialize_to_clipboard("tag", &tag_id, "cut", fields);

            // Remove #tag from the task body (same logic as UntagTask)
            let mut task = ectx.read("task", self.task_id.as_str()).await?;
            let was_present = task_tags(&task).iter().any(|t| t == &slug);
            let body = task.get_str("body").unwrap_or("").to_string();
            let new_body = tag_parser::remove_tag(&body, &slug);
            if new_body != body {
                task.set("body", serde_json::json!(new_body));
                ectx.write(&task).await?;
            }

            Ok(serde_json::json!({
                "cut": true,
                "tag": slug,
                "tag_id": tag_id,
                "task_id": self.task_id.to_string(),
                "was_present": was_present,
                "clipboard_json": clipboard_json,
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let ctx = KanbanContext::new(temp.path().join(".kanban"));
        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_cut_tag_untags_and_returns_clipboard() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tagged task")
            .with_description("A task #bug to fix")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = CutTag::new(task_id, "bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["cut"], true);
        assert_eq!(result["tag"], "bug");
        assert!(result["clipboard_json"].as_str().is_some());

        // Verify tag removed from task body
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let body = task.get_str("body").unwrap_or("");
        assert!(!body.contains("#bug"), "tag should be removed from body");
    }
}
