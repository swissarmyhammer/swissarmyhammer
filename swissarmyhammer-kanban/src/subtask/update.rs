//! UpdateSubtask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{SubtaskId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update subtask title or completion status
#[operation(verb = "update", noun = "subtask", description = "Update a subtask")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateSubtask {
    /// The task ID containing the subtask
    pub task_id: TaskId,
    /// The subtask ID to update
    pub id: SubtaskId,
    /// New title (optional)
    pub title: Option<String>,
    /// New completion status (optional)
    pub completed: Option<bool>,
}

impl UpdateSubtask {
    /// Create a new UpdateSubtask command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<SubtaskId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
            title: None,
            completed: None,
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the completed status
    pub fn with_completed(mut self, completed: bool) -> Self {
        self.completed = Some(completed);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateSubtask {
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

            if let Some(ref title) = self.title {
                subtask.title = title.clone();
            }

            if let Some(completed) = self.completed {
                subtask.completed = completed;
            }

            let updated_subtask = subtask.clone();
            
            ctx.write_task(&task).await?;
            
            Ok(serde_json::json!({
                "subtask": updated_subtask,
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
    async fn test_update_subtask_title() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Original title")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = UpdateSubtask::new(task_id, subtask_id)
            .with_title("Updated title")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["subtask"]["title"], "Updated title");
        assert_eq!(result["subtask"]["completed"], false);
    }

    #[tokio::test]
    async fn test_update_subtask_completed() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Subtask")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = UpdateSubtask::new(task_id, subtask_id)
            .with_completed(true)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["subtask"]["completed"], true);
        assert_eq!(result["task_progress"], 1.0);
    }

    #[tokio::test]
    async fn test_update_subtask_both() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let add_result = AddSubtask::new(task_id, "Original")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let subtask_id = add_result["subtask"]["id"].as_str().unwrap();

        let result = UpdateSubtask::new(task_id, subtask_id)
            .with_title("New title")
            .with_completed(true)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["subtask"]["title"], "New title");
        assert_eq!(result["subtask"]["completed"], true);
    }

    #[tokio::test]
    async fn test_update_nonexistent_subtask() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        let result = UpdateSubtask::new(task_id, "nonexistent")
            .with_title("Title")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::NotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_task_progress() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add two subtasks
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
        let result1 = UpdateSubtask::new(task_id, sub1_id)
            .with_completed(true)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result1["task_progress"], 0.5);

        // Complete second subtask
        let result2 = UpdateSubtask::new(task_id, sub2_id)
            .with_completed(true)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result2["task_progress"], 1.0);
    }
}
