//! CompleteTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Ordinal, Position, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Mark a task as complete by moving it to the done column
#[operation(
    verb = "complete",
    noun = "task",
    description = "Mark a task as complete"
)]
#[derive(Debug, Deserialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.id).await?;
        let board = ctx.read_board().await?;

        // Get terminal column (highest order = done)
        let terminal = board.terminal_column().ok_or_else(|| KanbanError::ColumnNotFound {
            id: "done".to_string(),
        })?;

        // Calculate ordinal at end of done column
        let task_ids = ctx.list_task_ids().await?;
        let mut last_ordinal: Option<Ordinal> = None;

        for id in &task_ids {
            if id == &self.id {
                continue; // Skip the task being completed
            }
            let t = ctx.read_task(id).await?;
            if t.position.column == terminal.id {
                last_ordinal = Some(match last_ordinal {
                    None => t.position.ordinal.clone(),
                    Some(ref o) if t.position.ordinal > *o => t.position.ordinal.clone(),
                    Some(o) => o,
                });
            }
        }

        // Update position to done column (preserving swimlane)
        task.position = Position {
            column: terminal.id.clone(),
            swimlane: task.position.swimlane.clone(),
            ordinal: match last_ordinal {
                Some(last) => Ordinal::after(&last),
                None => Ordinal::first(),
            },
        };

        ctx.write_task(&task).await?;
        Ok(serde_json::to_value(&task)?)
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

        InitBoard::new("Test").execute(&ctx).await.unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_complete_task() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task to complete").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Task should start in todo column
        assert_eq!(add_result["position"]["column"], "todo");

        // Complete the task
        let result = CompleteTask::new(task_id).execute(&ctx).await.unwrap();

        // Task should now be in done column
        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn test_complete_task_preserves_swimlane() {
        let (_temp, ctx) = setup().await;

        // Add a swimlane
        use crate::swimlane::AddSwimlane;
        AddSwimlane::new("feature", "Feature").execute(&ctx).await.unwrap();

        // Add a task and move it to the swimlane
        let add_result = AddTask::new("Task with swimlane").execute(&ctx).await.unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        // Move to doing column with swimlane
        use crate::task::MoveTask;
        use crate::types::Position;
        MoveTask::new(
            task_id,
            Position::new(
                "doing".into(),
                Some("feature".into()),
                Ordinal::first(),
            ),
        )
        .execute(&ctx)
        .await
        .unwrap();

        // Complete the task
        let result = CompleteTask::new(task_id).execute(&ctx).await.unwrap();

        // Should be in done column but preserve swimlane
        assert_eq!(result["position"]["column"], "done");
        assert_eq!(result["position"]["swimlane"], "feature");
    }

    #[tokio::test]
    async fn test_complete_nonexistent_task() {
        let (_temp, ctx) = setup().await;

        let result = CompleteTask::new("nonexistent").execute(&ctx).await;

        assert!(matches!(result, Err(KanbanError::TaskNotFound { .. })));
    }
}
