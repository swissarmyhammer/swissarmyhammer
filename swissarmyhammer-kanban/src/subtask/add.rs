//! AddSubtask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Subtask, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a checklist item to an existing task
#[operation(
    verb = "add",
    noun = "subtask",
    description = "Add a subtask to a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddSubtask {
    /// The task ID to add the subtask to
    pub task_id: TaskId,
    /// The subtask title
    pub title: String,
}

impl AddSubtask {
    /// Create a new AddSubtask command
    pub fn new(task_id: impl Into<TaskId>, title: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            title: title.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddSubtask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.task_id).await?;

            let subtask = Subtask::new(&self.title);
            task.subtasks.push(subtask.clone());

            ctx.write_task(&task).await?;

            Ok(serde_json::json!({
                "subtask": subtask,
                "task_id": task.id
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
    use crate::task::AddTask;
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
    async fn test_add_subtask() {
        let (_temp, ctx) = setup().await;

        // Create a task
        let task_result = AddTask::new("Task with subtasks")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        // Add a subtask
        let result = AddSubtask::new(task_id, "Write tests")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["subtask"]["title"], "Write tests");
        assert_eq!(result["subtask"]["completed"], false);
        assert_eq!(result["task_id"], task_id);

        // Verify the task has the subtask
        use crate::task::GetTask;
        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(task["subtasks"].as_array().unwrap().len(), 1);
        assert_eq!(task["subtasks"][0]["title"], "Write tests");
    }

    #[tokio::test]
    async fn test_add_multiple_subtasks() {
        let (_temp, ctx) = setup().await;

        let task_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task_result["id"].as_str().unwrap();

        AddSubtask::new(task_id, "Subtask 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddSubtask::new(task_id, "Subtask 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        use crate::task::GetTask;
        let task = GetTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(task["subtasks"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_add_subtask_to_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = AddSubtask::new("nonexistent", "Subtask")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }
}
