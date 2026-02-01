//! DeleteTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete a task
#[operation(verb = "delete", noun = "task", description = "Delete a task and clean up dependencies")]
#[derive(Debug, Deserialize)]
pub struct DeleteTask {
    /// The task ID to delete
    pub id: TaskId,
}

impl DeleteTask {
    /// Create a new DeleteTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        // Read the task first to verify it exists and get its data
        let task = ctx.read_task(&self.id).await?;

        // Remove this task from the depends_on list of all other tasks
        let task_ids = ctx.list_task_ids().await?;
        for id in task_ids {
            if id == self.id {
                continue;
            }

            let mut t = ctx.read_task(&id).await?;
            if t.depends_on.contains(&self.id) {
                t.depends_on.retain(|dep_id| dep_id != &self.id);
                ctx.write_task(&t).await?;
            }
        }

        // Delete the task file and log
        ctx.delete_task_file(&self.id).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string(),
            "title": task.title
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::task::AddTask;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_delete_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to delete")
            .execute(&ctx)
            .await
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = DeleteTask::new(task_id).execute(&ctx).await.unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["title"], "Task to delete");

        // Verify task is gone
        let ids = ctx.list_task_ids().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_delete_removes_from_dependencies() {
        let (_temp, ctx) = setup().await;

        // Create first task
        let result1 = AddTask::new("Task 1").execute(&ctx).await.unwrap();
        let id1 = result1["id"].as_str().unwrap();

        // Create second task depending on first
        let result2 = AddTask::new("Task 2")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .unwrap();
        let id2 = result2["id"].as_str().unwrap();

        // Delete first task
        DeleteTask::new(id1).execute(&ctx).await.unwrap();

        // Verify second task no longer has the dependency
        let task2 = ctx.read_task(&TaskId::from_string(id2)).await.unwrap();
        assert!(task2.depends_on.is_empty());
    }
}
