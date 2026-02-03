//! UpdateAttachment command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{AttachmentId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update attachment metadata (name, mime type, size)
#[operation(verb = "update", noun = "attachment", description = "Update an attachment's metadata")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateAttachment {
    /// The task ID
    pub task_id: TaskId,
    /// The attachment ID to update
    pub id: AttachmentId,
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
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<AttachmentId>) -> Self {
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
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.task_id).await?;

            let attachment = task
                .find_attachment_mut(&self.id)
                .ok_or_else(|| KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                })?;

            // Update only provided fields
            if let Some(name) = &self.name {
                attachment.name = name.clone();
            }
            if let Some(mime_type) = &self.mime_type {
                attachment.mime_type = Some(mime_type.clone());
            }
            if let Some(size) = self.size {
                attachment.size = Some(size);
            }

            let updated_attachment = attachment.clone();

            ctx.write_task(&task).await?;

            Ok(serde_json::json!({
                "attachment": updated_attachment,
                "task_id": task.id
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::attachment::AddAttachment;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_attachment_name() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddAttachment::new(task_id, "old-name.txt", "./file.txt")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let attachment_id = add_result["attachment"]["id"].as_str().unwrap();

        let result = UpdateAttachment::new(task_id, attachment_id)
            .with_name("new-name.txt")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["attachment"]["name"], "new-name.txt");
        assert_eq!(result["attachment"]["path"], "./file.txt"); // Path unchanged
    }

    #[tokio::test]
    async fn test_update_attachment_mime_and_size() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddAttachment::new(task_id, "file", "./file")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let attachment_id = add_result["attachment"]["id"].as_str().unwrap();

        let result = UpdateAttachment::new(task_id, attachment_id)
            .with_mime_type("application/octet-stream")
            .with_size(999)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["attachment"]["mime_type"], "application/octet-stream");
        assert_eq!(result["attachment"]["size"], 999);
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

        let result = UpdateAttachment::new(task_id, "nonexistent")
            .with_name("new-name")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = UpdateAttachment::new("nonexistent", "some-id")
            .with_name("new-name")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }
}
