//! DeleteSwimlane command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::SwimlaneId;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Delete a swimlane (fails if it has tasks)
#[operation(
    verb = "delete",
    noun = "swimlane",
    description = "Delete an empty swimlane"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct DeleteSwimlane {
    /// The swimlane ID to delete
    pub id: SwimlaneId,
}

impl DeleteSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let ectx = ctx.entity_context().await?;

            // Check swimlane exists
            ectx.read("swimlane", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            // Check for tasks in this swimlane
            let tasks = ectx.list("task").await?;
            let task_count = tasks
                .iter()
                .filter(|t| t.get_str("position_swimlane") == Some(self.id.as_str()))
                .count();

            if task_count > 0 {
                return Err(KanbanError::SwimlaneNotEmpty {
                    id: self.id.to_string(),
                    count: task_count,
                });
            }

            ectx.delete("swimlane", self.id.as_str()).await?;

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
    use crate::board::InitBoard;
    use crate::swimlane::AddSwimlane;
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
    async fn test_delete_swimlane() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = DeleteSwimlane::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["deleted"], true);
        assert_eq!(result["id"], "backend");
    }

    #[tokio::test]
    async fn test_delete_swimlane_not_found() {
        let (_temp, ctx) = setup().await;

        let result = DeleteSwimlane::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::SwimlaneNotFound { .. })));
    }

    #[tokio::test]
    async fn test_delete_swimlane_fails_if_has_tasks() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a task in this swimlane
        let mut add_task = AddTask::new("Task in swimlane");
        add_task.swimlane = Some("backend".to_string());
        add_task.execute(&ctx).await.into_result().unwrap();

        let result = DeleteSwimlane::new("backend")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::SwimlaneNotEmpty { .. })));
    }
}
