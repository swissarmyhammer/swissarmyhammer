//! UpdateAttachment command
//!
//! With the migration to `kind: attachment`, attachment metadata (name, mime_type,
//! size, path) is derived from the stored file by the entity layer on read.
//! There is no separate attachment entity to update.
//!
//! Renaming is not supported at the storage level — use delete + re-add instead.
//! This command is kept for API compatibility and returns the current metadata.

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update attachment metadata (name, mime type, size)
///
/// With `kind: attachment`, metadata is derived from the stored file.
/// This command currently returns the attachment's current metadata.
/// To change the file, delete and re-add the attachment.
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
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task — attachment field is enriched to metadata objects
            let task = ectx.read("task", self.task_id.as_str()).await?;
            let attachments = task
                .fields
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Find the attachment by ID
            let attachment = attachments.iter().find(|a| {
                a.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == self.id)
            });

            match attachment {
                Some(att) => Ok(json!({
                    "attachment": att,
                    "task_id": self.task_id.to_string()
                })),
                None => Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                }),
            }
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

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_attachment_returns_metadata() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file to attach
        let source_file = temp.path().join("file.txt");
        std::fs::write(&source_file, b"hello world").unwrap();

        let add_result = AddAttachment::new(task_id, "file.txt", source_file.to_str().unwrap())
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

        // Returns current metadata (name is derived from stored file, not the update request)
        assert_eq!(result["attachment"]["name"], "file.txt");
        assert!(result["attachment"]["path"].as_str().is_some());
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

        assert!(result.is_err());
    }
}
