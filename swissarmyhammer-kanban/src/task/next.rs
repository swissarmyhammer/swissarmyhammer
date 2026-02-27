//! NextTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ActorId, SwimlaneId, Task};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get the next actionable task
#[operation(
    verb = "next",
    noun = "task",
    description = "Get the oldest ready task from the first column"
)]
#[derive(Debug, Default, Deserialize)]
pub struct NextTask {
    /// Filter by swimlane
    pub swimlane: Option<SwimlaneId>,
    /// Filter by assignee
    pub assignee: Option<ActorId>,
}

impl NextTask {
    /// Create a new NextTask command
    pub fn new() -> Self {
        Self {
            swimlane: None,
            assignee: None,
        }
    }

    /// Filter by swimlane
    pub fn with_swimlane(mut self, swimlane: impl Into<SwimlaneId>) -> Self {
        self.swimlane = Some(swimlane.into());
        self
    }

    /// Filter by assignee
    pub fn with_assignee(mut self, assignee: impl Into<ActorId>) -> Self {
        self.assignee = Some(assignee.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for NextTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let all_columns = ctx.read_all_columns().await?;
            let all_tasks = ctx.read_all_tasks().await?;

            // Get first column
            let first_col = all_columns.iter().min_by_key(|c| c.order);
            let first_column = match first_col {
                Some(c) => &c.id,
                None => return Ok(Value::Null),
            };

            // Get terminal column for readiness check
            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.order)
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            // Filter to tasks in first column that are ready
            let mut candidates: Vec<&Task> = all_tasks
                .iter()
                .filter(|t| {
                    // Must be in first column
                    if &t.position.column != first_column {
                        return false;
                    }

                    // Must be ready (all deps complete)
                    if !t.is_ready(&all_tasks, terminal_column) {
                        return false;
                    }

                    // Filter by swimlane if specified
                    if let Some(ref swimlane) = self.swimlane {
                        if t.position.swimlane.as_ref() != Some(swimlane) {
                            return false;
                        }
                    }

                    // Filter by assignee if specified
                    if let Some(ref assignee) = self.assignee {
                        if !t.assignees.contains(assignee) {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            // Sort by ordinal (position within column)
            candidates.sort_by(|a, b| a.position.ordinal.cmp(&b.position.ordinal));

            // Return the first (oldest by position)
            match candidates.first() {
                Some(task) => {
                    // Include computed fields
                    let blocked_by = task.blocked_by(&all_tasks, terminal_column);
                    let blocks = task.blocks(&all_tasks);
                    let progress = task.progress();

                    let mut result = serde_json::to_value(task)?;
                    result["ready"] = serde_json::json!(true);
                    result["blocked_by"] = serde_json::to_value(&blocked_by)?;
                    result["blocks"] = serde_json::to_value(&blocks)?;
                    result["progress"] = serde_json::json!(progress);
                    Ok(result)
                }
                None => Ok(Value::Null),
            }
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
    use crate::task::{AddTask, MoveTask};
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
    async fn test_next_task_empty() {
        let (_temp, ctx) = setup().await;

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_next_task_returns_first() {
        let (_temp, ctx) = setup().await;

        AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task 1");
    }

    #[tokio::test]
    async fn test_next_task_skips_blocked() {
        use crate::types::TaskId;

        let (_temp, ctx) = setup().await;

        // Create a task that blocks another
        let result1 = AddTask::new("Blocker")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();

        // Create a blocked task
        AddTask::new("Blocked")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Next should return the blocker (the blocked one isn't ready)
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Blocker");
    }

    #[tokio::test]
    async fn test_next_task_ignores_done() {
        let (_temp, ctx) = setup().await;

        let result1 = AddTask::new("Done task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();

        AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Move first task to done
        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Next should return the todo task
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Todo task");
    }
}
