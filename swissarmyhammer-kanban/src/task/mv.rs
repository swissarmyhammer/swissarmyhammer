//! MoveTask command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ColumnId, Ordinal, Position, SwimlaneId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Move a task to a new position
#[operation(verb = "move", noun = "task", description = "Move a task to a different column or position")]
#[derive(Debug, Deserialize, Serialize)]
pub struct MoveTask {
    /// The task ID to move
    pub id: TaskId,
    /// The new position (column, optional swimlane, optional ordinal)
    pub position: Position,
}

impl MoveTask {
    /// Create a new MoveTask command with full position
    pub fn new(id: impl Into<TaskId>, position: Position) -> Self {
        Self {
            id: id.into(),
            position,
        }
    }

    /// Create a MoveTask command to move to a column (at the end)
    pub fn to_column(id: impl Into<TaskId>, column: impl Into<ColumnId>) -> Self {
        Self {
            id: id.into(),
            position: Position::in_column(column.into()),
        }
    }

    /// Create a MoveTask command with column and swimlane
    pub fn to_column_and_swimlane(
        id: impl Into<TaskId>,
        column: impl Into<ColumnId>,
        swimlane: impl Into<SwimlaneId>,
    ) -> Self {
        Self {
            id: id.into(),
            position: Position::new(column.into(), Some(swimlane.into()), Ordinal::first()),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for MoveTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result = async {
            let mut task = ctx.read_task(&self.id).await?;
            let board = ctx.read_board().await?;

            // Validate column exists
            if board.find_column(&self.position.column).is_none() {
                return Err(KanbanError::ColumnNotFound {
                    id: self.position.column.to_string(),
                });
            }

            // Validate swimlane exists if specified
            if let Some(ref swimlane_id) = self.position.swimlane {
                if board.find_swimlane(swimlane_id).is_none() {
                    return Err(KanbanError::SwimlaneNotFound {
                        id: swimlane_id.to_string(),
                    });
                }
            }

            // Calculate ordinal if not specified (default = at end)
            let ordinal = if self.position.ordinal == Ordinal::first() {
                // Find the last ordinal in the target column/swimlane
                let task_ids = ctx.list_task_ids().await?;
                let mut last_ordinal: Option<Ordinal> = None;

                for id in &task_ids {
                    if id == &self.id {
                        continue; // Skip the task being moved
                    }
                    let t = ctx.read_task(id).await?;
                    if t.position.column == self.position.column
                        && t.position.swimlane == self.position.swimlane
                    {
                        last_ordinal = Some(match last_ordinal {
                            None => t.position.ordinal.clone(),
                            Some(ref o) if t.position.ordinal > *o => t.position.ordinal.clone(),
                            Some(o) => o,
                        });
                    }
                }

                match last_ordinal {
                    Some(last) => Ordinal::after(&last),
                    None => Ordinal::first(),
                }
            } else {
                self.position.ordinal.clone()
            };

            // Update position
            task.position = Position {
                column: self.position.column.clone(),
                swimlane: self.position.swimlane.clone(),
                ordinal,
            };

            ctx.write_task(&task).await?;
            Ok(serde_json::to_value(&task)?)
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

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_move_task_to_column() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = MoveTask::to_column(task_id, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["position"]["column"], "done");
    }

    #[tokio::test]
    async fn test_move_task_invalid_column() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = MoveTask::to_column(task_id, "nonexistent").execute(&ctx).await.into_result();

        assert!(matches!(result, Err(KanbanError::ColumnNotFound { .. })));
    }
}
