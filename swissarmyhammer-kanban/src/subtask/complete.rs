//! CompleteSubtask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{SubtaskId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Mark a subtask as complete (convenience operation)
#[operation(verb = "complete", noun = "subtask", description = "Mark a subtask as complete")]
#[derive(Debug, Deserialize, Serialize)]
pub struct CompleteSubtask {
    /// The task ID containing the subtask
    pub task_id: TaskId,
    /// The subtask ID to complete
    pub id: SubtaskId,
}

impl CompleteSubtask {
    /// Create a new CompleteSubtask command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<SubtaskId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CompleteSubtask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.task_id).await?;
            
            let subtask = task
                .find_subtask_mut(&self.id)
                .ok_or_else(|| KanbanError::NotFound {
                    resource: "subtask".to_string(),
                    id: self.id.to_string(),
                })?;

            // Idempotent - if already complete, that's fine
            subtask.completed = true;
            
            ctx.write_task(&task).await?;
            
            Ok(serde_json::json!({
                "completed": true,
                "subtask_id": self.id,
                "task_id": task.id,
                "task_progress": task.progress()
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

    fn affected_resource_ids(&self, _result: &Value) -> Vec<String> {
        vec![self.task_id.to_string()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::subtask::AddSubtask;
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
    async fn test_complete_subtask() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Subtask")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = CompleteSubtask::new(task_id, subtask_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["completed"], true);
        assert_eq!(result["subtask_id"], subtask_id);
        assert_eq!(result["task_id"], task_id);
        assert_eq!(result["task_progress"], 1.0);
    }

    #[tokio::test]
    async fn test_complete_subtask_idempotent() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Subtask")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        // Complete once
        CompleteSubtask::new(task_id, subtask_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Complete again - should succeed
        let result = CompleteSubtask::new(task_id, subtask_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["completed"], true);
    }

    #[tokio::test]
    async fn test_complete_subtask_updates_progress() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add 3 subtasks
        let sub1 = AddSubtask::new(task_id, "Sub 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let sub1_id = sub1["subtask"]["id"].as_str().unwrap();

        AddSubtask::new(task_id, "Sub 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddSubtask::new(task_id, "Sub 3")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Complete one
        let result = CompleteSubtask::new(task_id, sub1_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Progress should be 1/3 ≈ 0.333...
        let progress = result["task_progress"].as_f64().unwrap();
        assert!((progress - 0.333333).abs() < 0.001);
    }

    #[tokio::test]
    async fn test_complete_nonexistent_subtask() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = CompleteSubtask::new(task_id, "nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }
}
