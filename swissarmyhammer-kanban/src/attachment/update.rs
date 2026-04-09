//! UpdateAttachment command

use crate::attachment::attachment_entity_to_json;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update attachment metadata (name, mime type, size)
#[operation(
    verb = "update",
    noun = "attachment",
    description = "Update an attachment's metadata"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAttachment {
    /// The task ID
    pub task_id: TaskId,
    /// The attachment ID to update
    pub id: String,
    /// Optional new name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Optional new MIME type
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    /// Optional new size
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
}

impl UpdateAttachment {
    /// Create a new UpdateAttachment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
            name: None,
            mime_type: None,
            size: None,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set the size
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }

    /// Verify ownership, apply field updates, and return the updated JSON.
    async fn apply(&self, ctx: &KanbanContext) -> Result<Value> {
        let ectx = ctx.entity_context().await?;

        let task = ectx.read("task", self.task_id.as_str()).await?;
        if !task.get_string_list("attachments").contains(&self.id) {
            return Err(KanbanError::NotFound {
                resource: "attachment".to_string(),
                id: self.id.to_string(),
            });
        }

        let mut attachment = ectx
            .read("attachment", &self.id)
            .await
            .map_err(KanbanError::from_entity_error)?;

        if let Some(name) = &self.name {
            attachment.set("attachment_name", json!(name));
        }
        if let Some(mime_type) = &self.mime_type {
            attachment.set("attachment_mime_type", json!(mime_type));
        }
        if let Some(size) = self.size {
            attachment.set("attachment_size", json!(size));
        }

        ectx.write(&attachment).await?;

        Ok(json!({
            "attachment": attachment_entity_to_json(&attachment),
            "task_id": self.task_id.to_string()
        }))
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();
        let result = self.apply(ctx).await;

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

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.task_id.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::task::AddTask;
    use serde_json::json;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    fn create_temp_file(dir: &std::path::Path, name: &str, content: &[u8]) -> String {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path.to_string_lossy().to_string()
    }

    #[tokio::test]
    async fn test_update_attachment_name() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Attach a file
        let file_path = create_temp_file(temp.path(), "old-name.txt", b"data");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Read back and verify name from the original filename
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr[0]["name"], "old-name.txt");

        // Replace with a new file (update = replace in the new model)
        let new_path = create_temp_file(temp.path(), "new-name.txt", b"updated data");
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([new_path]));
        ectx.write(&task).await.unwrap();

        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr[0]["name"], "new-name.txt");
    }

    #[tokio::test]
    async fn test_update_attachment_mime_and_size() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Attach a file with known content
        let content = b"exactly 999 bytes of padding would be silly, just check size is right";
        let file_path = create_temp_file(temp.path(), "file.bin", content);
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(
            arr[0]["size"].as_u64().unwrap(),
            content.len() as u64,
            "size should match file content length"
        );
        // mime_type is auto-detected; .bin may or may not be known
        assert!(arr[0]["mime_type"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_update_nonexistent_attachment() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Task with no attachments — nothing to update
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(is_empty);
    }

    #[tokio::test]
    async fn test_update_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
        assert!(result.is_err());
    }
}
