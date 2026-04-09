//! ListAttachments command

use crate::attachment::attachment_entity_to_json;
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

            // Read each attachment entity referenced by the task
            let attachment_ids = task.get_string_list("attachments");
            let mut attachments = Vec::new();
            for id in &attachment_ids {
                if let Ok(entity) = ectx.read("attachment", id).await {
                    attachments.push(attachment_entity_to_json(&entity));
                }
            }

            Ok(json!({
                "attachments": attachments,
                "count": attachments.len(),
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
    async fn test_list_empty_attachments() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        let attachments = task.get("attachments");
        let is_empty = attachments.is_none()
            || attachments.unwrap().is_null()
            || attachments.unwrap().as_array().is_none_or(|a| a.is_empty());
        assert!(is_empty);
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

        // Create real files
        let f1 = create_temp_file(temp.path(), "file1.txt", b"one");
        let f2 = create_temp_file(temp.path(), "file2.png", b"two");
        let f3 = create_temp_file(temp.path(), "file3.pdf", b"three");

        // Attach all three via entity layer
        let ectx = ctx.entity_context().await.unwrap();
        let mut task = ectx.read("task", task_id).await.unwrap();
        task.set("attachments", json!([f1, f2, f3]));
        ectx.write(&task).await.unwrap();

        // Read back — enriched metadata
        let task = ectx.read("task", task_id).await.unwrap();
        let arr = task.get("attachments").unwrap().as_array().unwrap();
        assert_eq!(arr.len(), 3);
        assert_eq!(arr[0]["name"], "file1.txt");
        assert_eq!(arr[1]["name"], "file2.png");
        assert_eq!(arr[2]["name"], "file3.pdf");
    }

    #[tokio::test]
    async fn test_list_attachments_from_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let ectx = ctx.entity_context().await.unwrap();
        let result = ectx.read("task", "nonexistent").await;
        assert!(result.is_err());
    }
}
