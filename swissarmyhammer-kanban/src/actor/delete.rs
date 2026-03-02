//! DeleteActor command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ActorId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete an actor (removes from all task assignee lists)
#[operation(
    verb = "delete",
    noun = "actor",
    description = "Delete an actor and remove from all task assignments"
)]
#[derive(Debug, Deserialize, Serialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: crate::error::Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Check actor exists
            ectx.read("actor", self.id.as_str())
                .await
                .map_err(|_| KanbanError::ActorNotFound {
                    id: self.id.to_string(),
                })?;

            // Remove actor from all task assignee lists
            let all_tasks = ectx.list("task").await?;
            for mut task in all_tasks {
                let mut assignees = task.get_string_list("assignees");
                if assignees.contains(&self.id.to_string()) {
                    assignees.retain(|a| a != self.id.as_str());
                    task.set("assignees", serde_json::to_value(&assignees)?);
                    ectx.write(&task).await?;
                }
            }

            // Delete the actor entity
            ectx.delete("actor", self.id.as_str()).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string()
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
                        serde_json::json!({"error": error_msg}),
                        None,
                        duration_ms,
                    )),
                }
            }
        }
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

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_delete_actor() {
        let (_temp, ctx) = setup().await;

        AddActor::human("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = DeleteActor::new("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["id"], "alice");

        // Verify actor is gone
        let ectx = ctx.entity_context().await.unwrap();
        assert!(ectx.read("actor", "alice").await.is_err());
    }

    #[tokio::test]
    async fn test_delete_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        let result = DeleteActor::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_actor_removes_from_tasks() {
        let (_temp, ctx) = setup().await;

        // Create actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Create task and assign
        let task_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify assignment
        let ectx = ctx.entity_context().await.unwrap();
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(task.get_string_list("assignees").contains(&"assistant".to_string()));

        // Delete actor
        DeleteActor::new("assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify task no longer has assignment
        let task = ectx.read("task", task_id).await.unwrap();
        assert!(!task.get_string_list("assignees").contains(&"assistant".to_string()));
    }
}
