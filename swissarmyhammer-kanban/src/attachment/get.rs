//! GetAttachment command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{AttachmentId, TaskId};
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
    /// The task ID
    pub task_id: TaskId,
    /// The attachment ID
    pub id: AttachmentId,
}

impl GetAttachment {
    /// Create a new GetAttachment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<AttachmentId>) -> Self {
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
            let task = ctx.read_task(&self.task_id).await?;

            let attachment =
                task.find_attachment(&self.id)
                    .ok_or_else(|| KanbanError::NotFound {
                        resource: "attachment".to_string(),
                        id: self.id.to_string(),
                    })?;

            Ok(serde_json::to_value(attachment)?)
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
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddAttachment::new(task_id, "file.txt", "./file.txt")
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
        assert_eq!(result["path"], "./file.txt");
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

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }
}
