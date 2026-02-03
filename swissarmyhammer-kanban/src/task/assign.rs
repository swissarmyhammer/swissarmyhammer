//! AssignTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Assign an actor to a task
#[operation(
    verb = "assign",
    noun = "task",
    description = "Assign an actor to a task"
)]
#[derive(Debug, Deserialize)]
pub struct AssignTask {
    /// The task ID to assign
    pub id: TaskId,
    /// The actor ID to assign (the assignee)
    pub assignee: ActorId,
}

impl AssignTask {
    /// Create a new AssignTask command
    pub fn new(id: impl Into<TaskId>, assignee: impl Into<ActorId>) -> Self {
        Self {
            id: id.into(),
            assignee: assignee.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AssignTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.id).await?;

        // Verify the actor exists using file-based storage
        if !ctx.actor_exists(&self.assignee).await {
            return Err(KanbanError::ActorNotFound {
                id: self.assignee.to_string(),
            });
        }

        // Add assignee if not already assigned
        if !task.assignees.contains(&self.assignee) {
            task.assignees.push(self.assignee.clone());
        }

        ctx.write_task(&task).await?;

        // Return confirmation with task info
        Ok(serde_json::json!({
            "assigned": true,
            "task_id": self.id,
            "assignee": self.assignee,
            "all_assignees": task.assignees,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AddActor;
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
    async fn test_assign_task() {
        let (_temp, ctx) = setup().await;

        // Create an actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign the task
        let result = AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .unwrap();

        assert_eq!(result["assigned"], true);
        assert_eq!(result["task_id"], task_id);
        assert_eq!(result["assignee"], "assistant");
        assert!(result["all_assignees"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("assistant")));
    }

    #[tokio::test]
    async fn test_assign_task_idempotent() {
        let (_temp, ctx) = setup().await;

        // Create an actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign twice
        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .unwrap();
        let result = AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .unwrap();

        // Should still only have one assignee
        assert_eq!(result["all_assignees"].as_array().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_assign_task_multiple_assignees() {
        let (_temp, ctx) = setup().await;

        // Create two actors
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .unwrap();
        AddActor::human("user", "User").execute(&ctx).await.unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign both
        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .unwrap();
        let result = AssignTask::new(task_id, "user")
            .execute(&ctx)
            .await
            .unwrap();

        assert_eq!(result["all_assignees"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        // Create an actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .unwrap();

        let result = AssignTask::new("nonexistent", "assistant")
            .execute(&ctx)
            .await;

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Try to assign to nonexistent actor
        let result = AssignTask::new(task_id, "nonexistent").execute(&ctx).await;

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }
}
