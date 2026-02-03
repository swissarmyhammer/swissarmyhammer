//! AssignTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ActorId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

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
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
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

    fn affected_resource_ids(&self, result: &Value) -> Vec<String> {
        result
            .get("task_id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
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

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_assign_task() {
        let (_temp, ctx) = setup().await;

        // Create an actor
        AddActor::agent("assistant", "Assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign the task
        let result = AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
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
            .into_result()
            .unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign twice
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
            .into_result()
            .unwrap();
        AddActor::human("user", "User").execute(&ctx).await.into_result().unwrap();

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Assign both
        AssignTask::new(task_id, "assistant")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let result = AssignTask::new(task_id, "user")
            .execute(&ctx)
            .await
            .into_result()
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
            .into_result()
            .unwrap();

        let result = AssignTask::new("nonexistent", "assistant")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }

    #[tokio::test]
    async fn test_assign_task_nonexistent_actor() {
        let (_temp, ctx) = setup().await;

        // Create a task
        let add_result = AddTask::new("Test task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Try to assign to nonexistent actor
        let result = AssignTask::new(task_id, "nonexistent").execute(&ctx).await.into_result();

        assert!(matches!(result, Err(KanbanError::ActorNotFound { .. })));
    }
}
