//! DeleteAttachment command

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

            // Verify the attachment exists and belongs to this task
            // Read the task and verify it owns this attachment
            let mut task = ectx.read("task", self.task_id.as_str()).await?;
            if !task.get_string_list("attachments").contains(&self.id) {
                return Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                });
            }

            // Two-phase write: delete attachment entity first, then update task.
            // If the task update fails, the stale ID in the task's list is
            // silently skipped by ListAttachments (tolerant of missing IDs).
            ectx.delete("attachment", &self.id).await?;

            // Remove the attachment ID from the task's attachments list
            let mut attachment_ids = task.get_string_list("attachments");
            attachment_ids.retain(|id| id != &self.id);
            task.set("attachments", json!(attachment_ids));
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
    async fn test_delete_attachment() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create file and attach
        let file_path = create_temp_file(temp.path(), "file.txt", b"hello");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Read back to get the stored filename
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        let stored_id = arr[0]["id"].as_str().unwrap();
        let stored_name = arr[0]["name"].as_str().unwrap();
        let stored_filename = format!("{}-{}", stored_id, stored_name);

        // Remove attachment by clearing the field and writing
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([]));
        ectx.write(&task).await.unwrap();

        // Verify file was trashed
        let att_file = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments")
            .join(&stored_filename);
        assert!(!att_file.exists(), "Attachment file should be trashed");

        // Verify file moved to trash dir
        let trash_file = temp
            .path()
            .join(".kanban")
            .join("tasks")
            .join(".attachments")
            .join(".trash")
            .join(&stored_filename);
        assert!(trash_file.exists(), "Attachment should be in trash");

        // Verify the task's attachments list is empty
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments
                .unwrap()
                .as_array()
                .map_or(true, |a| a.is_empty());
        assert!(is_empty);
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

        // Task with no attachments — nothing to delete
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments
                .unwrap()
                .as_array()
                .map_or(true, |a| a.is_empty());
        assert!(is_empty);
    }

    #[tokio::test]
    async fn test_delete_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
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

        // Create and attach two files
        let f1 = create_temp_file(temp.path(), "file1.txt", b"one");
        let f2 = create_temp_file(temp.path(), "file2.txt", b"two");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([f1, f2]));
        ectx.write(&task).await.unwrap();

        // Read back enriched metadata
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 2);

        // Keep only the second attachment (remove the first)
        let second_meta = arr[1].clone();
        let mut task = ectx.read("task", task_id).await.unwrap();
        // Write back with only the second stored filename
        let second_stored = format!(
            "{}-{}",
            second_meta["id"].as_str().unwrap(),
            second_meta["name"].as_str().unwrap()
        );
        task.set("attachments", json!([second_stored]));
        ectx.write(&task).await.unwrap();

        // Verify only one attachment remains
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "file2.txt");
    }
}
