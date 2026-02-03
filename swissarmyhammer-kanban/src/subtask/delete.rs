//! DeleteSubtask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{SubtaskId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Remove a subtask from a task
#[operation(verb = "delete", noun = "subtask", description = "Delete a subtask from a task")]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteSubtask {
    /// The task ID containing the subtask
    pub task_id: TaskId,
    /// The subtask ID to delete
    pub id: SubtaskId,
}

impl DeleteSubtask {
    /// Create a new DeleteSubtask command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<SubtaskId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteSubtask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.task_id).await?;
            
            let original_len = task.subtasks.len();
            task.subtasks.retain(|s| s.id != self.id);
            
            if task.subtasks.len() == original_len {
                return Err(KanbanError::NotFound {
                    resource: "subtask".to_string(),
                    id: self.id.to_string(),
                });
            }
            
            ctx.write_task(&task).await?;
            
            Ok(serde_json::json!({
                "deleted": true,
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
    use crate::task::{AddTask, GetTask};
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_delete_subtask() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Subtask")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = DeleteSubtask::new(task_id, subtask_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["subtask_id"], subtask_id);

        // Verify the subtask is gone
        let task = GetTask::new(task_id).execute(&ctx).await.into_result().unwrap();
        assert_eq!(task["subtasks"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_delete_subtask_updates_progress() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add 2 subtasks, complete one
        let sub1 = AddSubtask::new(task_id, "Sub 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let sub1_id = sub1["subtask"]["id"].as_str().unwrap();

        let sub2 = AddSubtask::new(task_id, "Sub 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let sub2_id = sub2["subtask"]["id"].as_str().unwrap();

        // Complete first subtask
        use crate::subtask::CompleteSubtask;
        CompleteSubtask::new(task_id, sub1_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Progress is now 0.5 (1 of 2 complete)

        // Delete the incomplete subtask
        let result = DeleteSubtask::new(task_id, sub2_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Progress should now be 1.0 (1 of 1 complete)
        assert_eq!(result["task_progress"], 1.0);
    }

    #[tokio::test]
    async fn test_delete_all_subtasks_progress_zero() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Subtask")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = DeleteSubtask::new(task_id, subtask_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // When no subtasks remain, progress is 0.0
        assert_eq!(result["task_progress"], 0.0);
    }

    #[tokio::test]
    async fn test_delete_nonexistent_subtask() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = DeleteSubtask::new(task_id, "nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_from_multiple_subtasks() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add 3 subtasks
        AddSubtask::new(task_id, "Sub 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let sub2 = AddSubtask::new(task_id, "Sub 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let sub2_id = sub2["subtask"]["id"].as_str().unwrap();

        AddSubtask::new(task_id, "Sub 3")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Delete the middle one
        DeleteSubtask::new(task_id, sub2_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify only 2 remain
        let task = GetTask::new(task_id).execute(&ctx).await.into_result().unwrap();
        assert_eq!(task["subtasks"].as_array().unwrap().len(), 2);
        
        // Verify the right ones remain
        let titles: Vec<&str> = task["subtasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s["title"].as_str().unwrap())
            .collect();
        assert!(titles.contains(&"Sub 1"));
        assert!(titles.contains(&"Sub 3"));
        assert!(!titles.contains(&"Sub 2"));
    }
}
