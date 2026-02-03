//! DeleteActor command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ActorId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete an actor (removes from all task assignee lists)
#[operation(verb = "delete", noun = "actor", description = "Delete an actor and remove from all task assignments")]
#[derive(Debug, Deserialize)]
pub struct DeleteActor {
    /// The actor ID to delete
    pub id: ActorId,
}

impl DeleteActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteActor {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        // Check actor exists
        if !ctx.actor_exists(&self.id).await {
            return Err(KanbanError::ActorNotFound {
                id: self.id.to_string(),
            });
        }

        // Remove actor from all task assignee lists
        let task_ids = ctx.list_task_ids().await?;
        for id in task_ids {
            let mut task = ctx.read_task(&id).await?;
            if task.assignees.contains(&self.id) {
                task.assignees.retain(|a| a != &self.id);
                ctx.write_task(&task).await?;
            }
        }

        // Delete the actor file
        ctx.delete_actor_file(&self.id).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string()
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::actor::AddActor;
    use crate::board::InitBoard;
    use crate::task::{AddTask, AssignTask};
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_delete_actor() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice").execute(&ctx).await.unwrap();

        let result = DeleteActor::new("alice").execute(&ctx).await.unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["id"], "alice");

        // Verify actor is gone
        assert!(!ctx.actor_exists(&"alice".into()).await);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        let result = DeleteActor::new("nonexistent").execute(&ctx).await;

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_actor_removes_from_tasks() {
        let (_temp, ctx) = setup().await;

        // Create actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .unwrap();

        // Create task and assign
        let task_result = AddTask::new("Test task").execute(&ctx).await.unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .unwrap();

        // Verify assignment
        let task = ctx.read_task(&task_id.into()).await.unwrap();
        assert!(task.assignees.contains(&"assistant".into()));

        // Delete actor
        DeleteActor::new("assistant").execute(&ctx).await.unwrap();

        // Verify task no longer has assignment
        let task = ctx.read_task(&task_id.into()).await.unwrap();
        assert!(!task.assignees.contains(&"assistant".into()));
    }
}
