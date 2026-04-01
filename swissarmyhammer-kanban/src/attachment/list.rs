//! ListAttachments command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all attachments on a task
#[operation(
    verb = "list",
    noun = "attachments",
    description = "List all attachments on a task"
)]
#[derive(Debug, Deserialize)]
pub struct ListAttachments {
    /// The task ID to list attachments for
    pub task_id: TaskId,
}

impl ListAttachments {
    /// Create a new ListAttachments command
    pub fn new(task_id: impl Into<TaskId>) -> Self {
        Self {
            task_id: task_id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListAttachments {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let task = ectx.read("task", self.task_id.as_str()).await?;

            // The attachment field is already enriched to metadata objects
            let attachments = task
                .fields
                .get("attachments")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default();

            let count = attachments.len();

            Ok(json!({
                "attachments": attachments,
                "count": count,
                "task_id": self.task_id.to_string()
            }))
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
    async fn test_list_empty_attachments() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = ListAttachments::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 0);
        assert_eq!(result["attachments"].as_array().unwrap().len(), 0);
        assert_eq!(result["task_id"], task_id);
    }

    #[tokio::test]
    async fn test_list_multiple_attachments() {
        let (temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Create real files to attach
        for name in &["file1.txt", "file2.png", "file3.pdf"] {
            let source = temp.path().join(name);
            std::fs::write(&source, format!("content of {}", name)).unwrap();
            AddAttachment::new(task_id, *name, source.to_str().unwrap())
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        let result = ListAttachments::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 3);
        let attachments = result["attachments"].as_array().unwrap();
        assert_eq!(attachments.len(), 3);
        assert_eq!(attachments[0]["name"], "file1.txt");
        assert_eq!(attachments[1]["name"], "file2.png");
        assert_eq!(attachments[2]["name"], "file3.pdf");
    }

    #[tokio::test]
    async fn test_list_attachments_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = ListAttachments::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }
}
