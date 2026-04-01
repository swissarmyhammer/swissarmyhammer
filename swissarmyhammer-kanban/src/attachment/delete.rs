//! DeleteAttachment command

use crate::attachment::extract_stored_filenames;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete an attachment from a task
#[operation(
    verb = "delete",
    noun = "attachment",
    description = "Delete an attachment from a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteAttachment {
    /// The task ID
    pub task_id: TaskId,
    /// The attachment ID to delete
    pub id: String,
}

impl DeleteAttachment {
    /// Create a new DeleteAttachment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Read the task — attachments field is enriched to metadata objects
            let mut task = ectx.read("task", self.task_id.as_str()).await?;
            let attachments = task
                .fields
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Check the attachment exists in this task's metadata
            let found = attachments.iter().any(|a| {
                a.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == self.id)
            });

            if !found {
                return Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                });
            }

            // Extract stored filenames, excluding the one being deleted.
            // The entity layer will trash the removed file on write.
            let remaining: Vec<Value> =
                extract_stored_filenames(task.fields.get("attachments").unwrap_or(&json!([])))
                    .into_iter()
                    .filter(|filename| {
                        // Stored filenames are `{id}-{name}` — match by ID prefix
                        !filename.starts_with(&format!("{}-", self.id))
                    })
                    .map(Value::String)
                    .collect();

            task.set("attachments", json!(remaining));
            ectx.write(&task).await?;

            Ok(json!({
                "deleted": true,
                "attachment_id": self.id,
                "task_id": self.task_id.to_string()
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
    async fn test_delete_attachment() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file to attach
        let source_file = temp.path().join("file.txt");
        std::fs::write(&source_file, b"hello").unwrap();

        let add_result = AddAttachment::new(task_id, "file.txt", source_file.to_str().unwrap())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let attachment_id = add_result["attachment"]["id"].as_str().unwrap();

        let result = DeleteAttachment::new(task_id, attachment_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["attachment_id"], attachment_id);
        assert_eq!(result["task_id"], task_id);

        // Verify the attachment file was moved to trash
        let att_dir = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments");
        let live_entries: Vec<_> = std::fs::read_dir(&att_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name();
                let name = name.to_str().unwrap_or("");
                !name.starts_with('.') && e.path().is_file()
            })
            .collect();
        assert_eq!(live_entries.len(), 0, "attachment file should be trashed");

        // Verify the task's attachments list is empty
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task
            .fields
            .get("attachments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert!(attachments.is_empty());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_attachment() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = DeleteAttachment::new(task_id, "nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = DeleteAttachment::new("nonexistent", "some-id")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_delete_one_of_multiple_attachments() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create real files
        let source1 = temp.path().join("file1.txt");
        std::fs::write(&source1, b"content1").unwrap();
        let source2 = temp.path().join("file2.txt");
        std::fs::write(&source2, b"content2").unwrap();

        // Add two attachments
        let add1 = AddAttachment::new(task_id, "file1.txt", source1.to_str().unwrap())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let attachment_id1 = add1["attachment"]["id"].as_str().unwrap();

        let add2 = AddAttachment::new(task_id, "file2.txt", source2.to_str().unwrap())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let attachment_id2 = add2["attachment"]["id"].as_str().unwrap().to_string();

        // Delete the first one
        DeleteAttachment::new(task_id, attachment_id1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify only one attachment remains
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task
            .fields
            .get("attachments")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();
        assert_eq!(attachments.len(), 1);
        assert_eq!(attachments[0]["id"].as_str().unwrap(), attachment_id2);
    }
}
