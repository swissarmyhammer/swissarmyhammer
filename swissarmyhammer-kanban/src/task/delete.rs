//! DeleteTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a task
#[operation(
    verb = "delete",
    noun = "task",
    description = "Delete a task and clean up dependencies"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteTask {
    /// The task ID to delete
    pub id: TaskId,
}

impl DeleteTask {
    /// Create a new DeleteTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            // Read the task first to verify it exists and get its data
            let task = ctx.read_task(&self.id).await?;

            // Remove this task from the depends_on list of all other tasks
            let task_ids = ctx.list_task_ids().await?;
            for id in task_ids {
                if id == self.id {
                    continue;
                }

                let mut t = ctx.read_task(&id).await?;
                if t.depends_on.contains(&self.id) {
                    t.depends_on.retain(|dep_id| dep_id != &self.id);
                    ctx.write_task(&t).await?;
                }
            }

            // Delete the task file and log
            ctx.delete_task_file(&self.id).await?;

            Ok(serde_json::json!({
                "deleted": true,
                "id": self.id.to_string(),
                "title": task.title
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
            .get("id")
            .and_then(|v| v.as_str())
            .map(|id| vec![id.to_string()])
            .unwrap_or_default()
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
    async fn test_delete_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to delete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = DeleteTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["title"], "Task to delete");

        // Verify task is gone
        let ids = ctx.list_task_ids().await.unwrap();
        assert!(ids.is_empty());
    }

    #[tokio::test]
    async fn test_delete_removes_from_dependencies() {
        let (_temp, ctx) = setup().await;

        // Create first task
        let result1 = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();

        // Create second task depending on first
        let result2 = AddTask::new("Task 2")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = result2["id"].as_str().unwrap();

        // Delete first task
        DeleteTask::new(id1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Verify second task no longer has the dependency
        let task2 = ctx.read_task(&TaskId::from_string(id2)).await.unwrap();
        assert!(task2.depends_on.is_empty());
    }
}
