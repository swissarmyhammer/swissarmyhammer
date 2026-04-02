//! CompleteTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task_helpers::task_entity_to_json;
use crate::types::{Ordinal, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Mark a task as complete by moving it to the done column
#[operation(
    verb = "complete",
    noun = "task",
    description = "Mark a task as complete"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct CompleteTask {
    /// The task ID to complete
    pub id: TaskId,
}

impl CompleteTask {
    /// Create a new CompleteTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for CompleteTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx.read("task", self.id.as_str()).await?;

            // Get terminal column (highest order = done)
            let all_columns = ectx.list("column").await?;
            let terminal = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .ok_or_else(|| KanbanError::ColumnNotFound {
                    id: "done".to_string(),
                })?;

            // Calculate ordinal at end of done column
            let all_tasks = ectx.list("task").await?;
            let mut last_ordinal: Option<Ordinal> = None;

            for t in &all_tasks {
                if t.id == self.id.as_str() {
                    continue; // Skip the task being completed
                }
                if t.get_str("position_column") == Some(terminal.id.as_str()) {
                    let ord = Ordinal::from_string(
                        t.get_str("position_ordinal")
                            .unwrap_or(Ordinal::DEFAULT_STR),
                    );
                    last_ordinal = Some(match last_ordinal {
                        None => ord,
                        Some(ref o) if ord > *o => ord,
                        Some(o) => o,
                    });
                }
            }

            // Update position to done column (preserving swimlane)
            entity.set("position_column", serde_json::json!(terminal.id.as_str()));
            entity.set(
                "position_ordinal",
                serde_json::json!(match last_ordinal {
                    Some(last) => Ordinal::after(&last).as_str().to_string(),
                    None => Ordinal::first().as_str().to_string(),
                }),
            );

            ectx.write(&entity).await?;
            Ok(task_entity_to_json(&entity))
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
    async fn test_complete_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to complete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Task should start in todo column
        assert_eq!(add_result["position"]["column"], "todo");

        // Complete the task
        let result = CompleteTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Task should now be in done column
        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn test_complete_task_preserves_swimlane() {
        let (_temp, ctx) = setup().await;

        // Add a swimlane
        use crate::swimlane::AddSwimlane;
        AddSwimlane::new("feature", "Feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a task and move it to the swimlane
        let add_result = AddTask::new("Task with swimlane")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Move to doing column with swimlane
        use crate::task::MoveTask;
        MoveTask::to_column_and_swimlane(task_id, "doing", "feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Complete the task
        let result = CompleteTask::new(task_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Should be in done column but preserve swimlane
        assert_eq!(result["position"]["column"], "done");
        assert_eq!(result["position"]["swimlane"], "feature");
    }

    #[tokio::test]
    async fn test_complete_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = CompleteTask::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_complete_task_affected_resource_ids() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to complete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let op = CompleteTask::new(task_id);
        let exec_result = op.execute(&ctx).await;
        let value = exec_result.into_result().unwrap();

        let ids = op.affected_resource_ids(&value);
        assert_eq!(ids.len(), 1);
        assert_eq!(ids[0], task_id);
    }

    #[tokio::test]
    async fn test_complete_task_ordering_multiple_done() {
        let (_temp, ctx) = setup().await;

        // Complete two tasks; the second should be ordered after the first in done column.
        let r1 = AddTask::new("First to complete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();

        let r2 = AddTask::new("Second to complete")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = r2["id"].as_str().unwrap();

        let done1 = CompleteTask::new(id1)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let done2 = CompleteTask::new(id2)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(done1["position"]["column"], "done");
        assert_eq!(done2["position"]["column"], "done");

        // Second completed task should have a higher ordinal than the first.
        let ord1 = done1["position"]["ordinal"].as_str().unwrap();
        let ord2 = done2["position"]["ordinal"].as_str().unwrap();
        assert!(
            ord2 > ord1,
            "second done task ({}) should sort after first ({})",
            ord2,
            ord1
        );
    }
}
