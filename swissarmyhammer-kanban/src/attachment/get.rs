//! GetAttachment command

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

            // Read the task — attachment field is already enriched to metadata objects
            let task = ectx.read("task", self.task_id.as_str()).await?;
            let attachments = task
                .fields
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            // Find the attachment by its ID in the enriched metadata
            let attachment = attachments.iter().find(|a| {
                a.get("id")
                    .and_then(|v| v.as_str())
                    .is_some_and(|id| id == self.id)
            });

            match attachment {
                Some(att) => Ok(att.clone()),
                None => Err(KanbanError::NotFound {
                    resource: "attachment".to_string(),
                    id: self.id.to_string(),
                }),
            }
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
    async fn test_get_attachment() {
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

        let result = GetAttachment::new(task_id, attachment_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], attachment_id);
        assert_eq!(result["name"], "file.txt");
        assert!(result["path"].as_str().is_some());
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

        let result = GetAttachment::new(task_id, "nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_get_attachment_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = GetAttachment::new("nonexistent", "some-id")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }
}
