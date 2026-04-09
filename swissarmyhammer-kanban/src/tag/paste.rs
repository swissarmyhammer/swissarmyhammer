//! PasteTag operation — apply a tag from the clipboard to a task.

use crate::clipboard;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag::tag_name_exists_entity;
use crate::tag_parser;
use crate::task_helpers::task_tags;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Paste a tag from the clipboard onto a task.
///
/// Reads the clipboard JSON, validates it contains a tag entity, and applies
/// the tag to the target task via `tag_parser::append_tag`. Auto-creates the
/// Tag entity if it doesn't exist. No-op if the task already has the tag.
#[operation(
    verb = "paste",
    noun = "tag",
    description = "Paste a tag from the clipboard onto a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct PasteTag {
    /// The task ID to tag.
    pub task_id: TaskId,
    /// The clipboard JSON string to paste from.
    pub clipboard_json: String,
}

impl PasteTag {
    /// Create a new PasteTag operation.
    pub fn new(task_id: impl Into<TaskId>, clipboard_json: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            clipboard_json: clipboard_json.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for PasteTag {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            // Deserialize and validate clipboard
            let payload =
                clipboard::deserialize_from_clipboard(&self.clipboard_json).ok_or_else(|| {
                    KanbanError::InvalidValue {
                        field: "clipboard".into(),
                        message: "invalid clipboard data".into(),
                    }
                })?;
            let content = &payload.swissarmyhammer_clipboard;

            if content.entity_type != "tag" {
                return Err(KanbanError::InvalidValue {
                    field: "clipboard".into(),
                    message: format!("expected tag on clipboard, got '{}'", content.entity_type),
                });
            }

            // Extract tag name from clipboard fields
            let tag_name = content
                .fields
                .get("tag_name")
                .and_then(|v| v.as_str())
                .ok_or_else(|| KanbanError::InvalidValue {
                    field: "clipboard".into(),
                    message: "clipboard tag has no tag_name field".into(),
                })?;
            let slug = tag_parser::normalize_slug(tag_name);

            let ectx = ctx.entity_context().await?;

            // Check if already tagged — no-op
            let task = ectx.read("task", self.task_id.as_str()).await?;
            if task_tags(&task).iter().any(|t| t == &slug) {
                return Ok(json!({
                    "pasted": false,
                    "already_tagged": true,
                    "task_id": self.task_id.to_string(),
                    "tag": slug,
                }));
            }

            // Auto-create tag entity if it doesn't exist
            if !tag_name_exists_entity(&ectx, &slug).await {
                let color = content
                    .fields
                    .get("color")
                    .and_then(|v| v.as_str())
                    .unwrap_or_else(|| crate::auto_color::auto_color(&slug));
                let tag_id = ulid::Ulid::new().to_string();
                let mut tag_entity = Entity::new("tag", tag_id.as_str());
                tag_entity.set("tag_name", json!(slug));
                tag_entity.set("color", json!(color));
                if let Some(desc) = content.fields.get("description").and_then(|v| v.as_str()) {
                    tag_entity.set("description", json!(desc));
                }
                ectx.write(&tag_entity).await?;
            }

            // Append #tag to task body
            let mut task = ectx.read("task", self.task_id.as_str()).await?;
            let body = task.get_str("body").unwrap_or("").to_string();
            let new_body = tag_parser::append_tag(&body, &slug);
            task.set("body", json!(new_body));
            ectx.write(&task).await?;

            Ok(json!({
                "pasted": true,
                "task_id": self.task_id.to_string(),
                "tag": slug,
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
                        json!({"error": error_msg}),
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
    use crate::clipboard;
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

    fn make_tag_clipboard(tag_name: &str, color: &str) -> String {
        clipboard::serialize_to_clipboard(
            "tag",
            "01FAKE",
            "copy",
            json!({"tag_name": tag_name, "color": color}),
        )
    }

    #[tokio::test]
    async fn test_paste_tag_tags_the_task() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("My task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let clip = make_tag_clipboard("urgent", "ff0000");
        let result = PasteTag::new(task_id, clip)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["pasted"], true);
        assert_eq!(result["tag"], "urgent");

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let body = task.get_str("body").unwrap_or("");
        assert!(body.contains("#urgent"), "task body should contain #urgent");
    }

    #[tokio::test]
    async fn test_paste_tag_noop_if_already_tagged() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Tagged")
            .with_description("Has #bug already")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let clip = make_tag_clipboard("bug", "ff0000");
        let result = PasteTag::new(task_id, clip)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["pasted"], false);
        assert_eq!(result["already_tagged"], true);
    }

    #[tokio::test]
    async fn test_paste_tag_invalid_clipboard_fails() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = PasteTag::new(task_id, "not json")
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_paste_tag_preserves_existing_tags() {
        let (_temp, ctx) = setup().await;

        // Create a task that already has tags in the body
        let task_result = AddTask::new("Multi-tagged task")
            .with_description("Fix #bug and add #feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Verify existing tags are present
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let tags_before = task_tags(&task);
        assert!(
            tags_before.contains(&"bug".to_string()),
            "should have #bug before paste"
        );
        assert!(
            tags_before.contains(&"feature".to_string()),
            "should have #feature before paste"
        );

        // Paste a new tag
        let clip = make_tag_clipboard("urgent", "ff0000");
        let result = PasteTag::new(task_id, clip)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["pasted"], true);

        // Re-read and verify ALL tags are present (old + new)
        let task = ectx.read("task", task_id).await.unwrap();
        let tags_after = task_tags(&task);
        assert!(
            tags_after.contains(&"bug".to_string()),
            "should still have #bug after paste"
        );
        assert!(
            tags_after.contains(&"feature".to_string()),
            "should still have #feature after paste"
        );
        assert!(
            tags_after.contains(&"urgent".to_string()),
            "should have #urgent after paste"
        );
        assert_eq!(tags_after.len(), 3, "should have exactly 3 tags");
    }

    #[tokio::test]
    async fn test_paste_tag_wrong_entity_type_fails() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let clip =
            clipboard::serialize_to_clipboard("task", "01FAKE", "copy", json!({"title": "A task"}));
        let result = PasteTag::new(task_id, clip)
            .execute(&ctx)
            .await
            .into_result();
        assert!(result.is_err());
    }
}
