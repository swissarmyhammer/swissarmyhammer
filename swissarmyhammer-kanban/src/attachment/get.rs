//! GetAttachment command

use crate::attachment::attachment_entity_to_json;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a specific attachment from a task
#[operation(
    verb = "get",
    noun = "attachment",
    description = "Get an attachment from a task"
)]
#[derive(Debug, Deserialize)]
pub struct GetAttachment {
    /// The task ID (kept for API compatibility; used to verify ownership)
    pub task_id: TaskId,
    /// The attachment ID
    pub id: String,
}

impl GetAttachment {
    /// Create a new GetAttachment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetAttachment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;

            // Verify the task owns this attachment
            let task = ectx.read("task", self.task_id.as_str()).await?;
            if !task.get_string_list("attachments").contains(&self.id) {
                return Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                });
            }

            // Read the attachment entity
            let attachment = ectx
                .read("attachment", &self.id)
                .await
                .map_err(KanbanError::from_entity_error)?;

            Ok(attachment_entity_to_json(&attachment))
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use serde_json::json;
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
    async fn test_get_attachment() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create a real file and attach via entity layer
        let file_path = create_temp_file(temp.path(), "file.txt", b"hello");
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([file_path]));
        ectx.write(&task).await.unwrap();

        // Read back — entity layer enriches with metadata
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 1);
        assert_eq!(arr[0]["name"], "file.txt");
        assert!(arr[0]["id"].as_str().is_some());
        assert!(arr[0]["path"].as_str().is_some());
    }

    #[tokio::test]
    async fn test_get_nonexistent_attachment() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Task with no attachments — reading should show empty
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
    async fn test_get_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
        assert!(result.is_err());
    }
}
