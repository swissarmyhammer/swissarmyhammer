//! UnassignTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Remove an actor from a task's assignee list
#[operation(
    verb = "unassign",
    noun = "task",
    description = "Remove an actor from a task's assignee list"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UnassignTask {
    /// The task ID to unassign from
    pub id: TaskId,
    /// The actor ID to unassign (the assignee)
    pub assignee: ActorId,
}

impl UnassignTask {
    /// Create a new UnassignTask command
    pub fn new(id: impl Into<TaskId>, assignee: impl Into<ActorId>) -> Self {
        Self {
            id: id.into(),
            assignee: assignee.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UnassignTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Remove assignee (idempotent - no error if not assigned)
            let mut assignees = entity.get_string_list("assignees");
            let was_assigned = assignees.contains(&self.assignee.to_string());
            assignees.retain(|a| a != self.assignee.as_str());
            entity.set("assignees", serde_json::to_value(&assignees)?);

            ectx.write(&entity).await?;

            // Return confirmation
            Ok(serde_json::json!({
                "unassigned": was_assigned,
                "task_id": self.id,
                "assignee": self.assignee,
                "all_assignees": assignees,
            }))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
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
    async fn test_unassign_task() {
        let (_temp, ctx) = setup().await;

        AddActor::new("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UnassignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["unassigned"], true);
        assert_eq!(result["task_id"], task_id);
        assert_eq!(result["all_assignees"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_unassign_task_idempotent() {
        let (_temp, ctx) = setup().await;

        AddActor::new("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UnassignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["unassigned"], false);
        assert_eq!(result["all_assignees"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_unassign_task_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = UnassignTask::new("nonexistent", "assistant")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_unassign_multiple_assignees_keeps_others() {
        let (_temp, ctx) = setup().await;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::new("bob", "Bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let add_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        AssignTask::new(task_id, "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(task_id, "bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Unassign only alice
        let result = UnassignTask::new(task_id, "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["unassigned"], true);
        let remaining: Vec<&str> = result["all_assignees"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str())
            .collect();
        assert_eq!(remaining, vec!["bob"]);
    }
}
