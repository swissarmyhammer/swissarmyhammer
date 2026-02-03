//! GetBoard command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ColumnId;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get the board with computed task counts
#[operation(verb = "get", noun = "board", description = "Retrieve the board with task counts")]
#[derive(Debug, Default, Deserialize)]
pub struct GetBoard;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetBoard {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let board = ctx.read_board().await?;

            // Compute task counts per column
            let task_ids = ctx.list_task_ids().await?;
            let mut counts: HashMap<ColumnId, usize> = HashMap::new();

            for id in task_ids {
                let task = ctx.read_task(&id).await?;
                *counts.entry(task.position.column.clone()).or_default() += 1;
            }

            // Build response with counts
            let mut result = serde_json::to_value(&board)?;
            let counts_value: HashMap<String, usize> = counts
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect();
            result["task_counts"] = serde_json::to_value(&counts_value)?;

            Ok(result)
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
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

        // Initialize board
        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_get_board() {
        let (_temp, ctx) = setup().await;

        let result = GetBoard.execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["name"], "Test");
        assert!(result["task_counts"].is_object());
    }

    #[tokio::test]
    async fn test_get_board_with_tasks() {
        let (_temp, ctx) = setup().await;

        // Add some tasks
        AddTask::new("Task 1").execute(&ctx).await.into_result().unwrap();
        AddTask::new("Task 2").execute(&ctx).await.into_result().unwrap();

        let result = GetBoard.execute(&ctx).await.into_result().unwrap();
        let counts = result["task_counts"].as_object().unwrap();
        let todo_count = counts.get("todo").and_then(|v| v.as_u64()).unwrap_or(0);
        assert_eq!(todo_count, 2);
    }
}
