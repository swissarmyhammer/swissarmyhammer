//! AssignTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ActorId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Assign an actor to a task
#[operation(
    verb = "assign",
    noun = "task",
    description = "Assign an actor to a task"
)]
#[derive(Debug, Deserialize, Serialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Verify the actor exists
            if ectx.read("actor", self.assignee.as_str()).await.is_err() {
                return Err(KanbanError::ActorNotFound {
                    id: self.assignee.to_string(),
                });
            }

            // Add assignee if not already assigned
            let mut assignees = entity.get_string_list("assignees");
            if !assignees.contains(&self.assignee.to_string()) {
                assignees.push(self.assignee.to_string());
                entity.set("assignees", serde_json::to_value(&assignees)?);
            }

            ectx.write(&entity).await?;

            // Thin ack — success implies the assignment took effect;
            // `get task` is the escape hatch for the post-op assignee list.
            Ok(crate::task_helpers::task_mutation_ack(&entity))
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
    use crate::task::{AddTask, GetTask};
    use crate::task_helpers::assert_task_mutation_ack;
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

    #[tokio::test]
    async fn test_assign_task() {
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

        let result = AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Mutations acknowledge, they don't echo — the assignee's presence
        // is asserted via `get task` (stored state).
        assert_task_mutation_ack(&result, task_id);

        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(task["assignees"]
            .as_array()
            .unwrap()
            .contains(&json!("assistant")));
    }

    #[tokio::test]
    async fn test_assign_task_idempotent() {
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
        let result = AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_task_mutation_ack(&result, task_id);

        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(
            task["assignees"].as_array().unwrap().len(),
            1,
            "duplicate assignment must not add the actor twice"
        );
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        AddActor::new("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = AssignTask::new("nonexistent", "assistant")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Test task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = AssignTask::new(task_id, "nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }
}
