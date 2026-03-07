//! NextTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::{task_entity_to_rich_json, task_is_ready, task_tags};
use crate::types::{ActorId, Ordinal, SwimlaneId, TagId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get the next actionable task
#[operation(
    verb = "next",
    noun = "task",
    description = "Get the oldest ready task not in the done column"
)]
#[derive(Debug, Default, Deserialize)]
pub struct NextTask {
    /// Filter by swimlane
    pub swimlane: Option<SwimlaneId>,
    /// Filter by assignee
    pub assignee: Option<ActorId>,
    /// Filter by tag
    pub tag: Option<TagId>,
}

impl NextTask {
    /// Create a new NextTask command
    pub fn new() -> Self {
        Self {
            swimlane: None,
            assignee: None,
            tag: None,
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

    /// Filter by tag
    pub fn with_tag(mut self, tag: impl Into<TagId>) -> Self {
        self.tag = Some(tag.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for NextTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let all_columns = ectx.list("column").await?;
            let all_tasks = ectx.list("task").await?;

            // Get terminal column (highest order) — tasks here are done
            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            // Get column ordering for sorting candidates
            let column_order: std::collections::HashMap<&str, usize> = all_columns
                .iter()
                .enumerate()
                .map(|(i, c)| (c.id.as_str(), i))
                .collect();

            // Filter to tasks not in terminal column that are ready
            let mut candidates: Vec<&swissarmyhammer_entity::Entity> = all_tasks
                .iter()
                .filter(|t| {
                    // Must not be in terminal column
                    if t.get_str("position_column") == Some(terminal_column) {
                        return false;
                    }

                    // Must be ready (all deps complete)
                    if !task_is_ready(t, &all_tasks, terminal_column) {
                        return false;
                    }

                    // Filter by swimlane if specified
                    if let Some(ref swimlane) = self.swimlane {
                        if t.get_str("position_swimlane") != Some(swimlane.as_str()) {
                            return false;
                        }
                    }

                    // Filter by assignee if specified
                    if let Some(ref assignee) = self.assignee {
                        if !t
                            .get_string_list("assignees")
                            .contains(&assignee.to_string())
                        {
                            return false;
                        }
                    }

                    // Filter by tag if specified
                    if let Some(ref tag) = self.tag {
                        if !task_tags(t).contains(&tag.to_string()) {
                            return false;
                        }
                    }

                    true
                })
                .collect();

            // Sort by column order first, then ordinal within column
            candidates.sort_by(|a, b| {
                let col_a = column_order.get(a.get_str("position_column").unwrap_or("")).unwrap_or(&0);
                let col_b = column_order.get(b.get_str("position_column").unwrap_or("")).unwrap_or(&0);
                let ord_a = Ordinal::from_string(a.get_str("position_ordinal").unwrap_or("a0"));
                let ord_b = Ordinal::from_string(b.get_str("position_ordinal").unwrap_or("a0"));
                col_a.cmp(col_b).then(ord_a.cmp(&ord_b))
            });

            // Return the first (oldest by position)
            match candidates.first() {
                Some(task) => Ok(task_entity_to_rich_json(task, &all_tasks, terminal_column)),
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
    async fn test_next_task_filters_by_tag() {
        let (_temp, ctx) = setup().await;

        // Create tasks - one with a #bug tag in description, one without
        AddTask::new("Untagged task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Bug task")
            .with_description("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Without tag filter, returns first task (untagged)
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Untagged task");

        // With tag filter, skips untagged and returns the bug task
        let result = NextTask::new()
            .with_tag("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["title"], "Bug task");

        // With non-matching tag, returns null
        let result = NextTask::new()
            .with_tag("feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(result.is_null());
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

    #[tokio::test]
    async fn test_next_task_searches_all_non_done_columns() {
        let (_temp, ctx) = setup().await;

        // Create three tasks
        let r1 = AddTask::new("Task in todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();

        let r2 = AddTask::new("Task in doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = r2["id"].as_str().unwrap();

        let r3 = AddTask::new("Task in done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id3 = r3["id"].as_str().unwrap();

        // Spread tasks across columns: todo, doing, done
        MoveTask::to_column(id2, "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(id3, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // next task should return the todo task first (earlier column)
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task in todo");

        // Move the todo task to done — now only "doing" has a live task
        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // next task should return the doing task (not skip it)
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task in doing");

        // Move the doing task to done — board is empty of actionable tasks
        MoveTask::to_column(id2, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // next task should return null — all tasks are done
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert!(result.is_null(), "Expected null when all tasks are done, got: {result}");
    }

    #[tokio::test]
    async fn test_next_task_prefers_earlier_column() {
        let (_temp, ctx) = setup().await;

        // Create two tasks and put them in different non-done columns
        let r1 = AddTask::new("Doing task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();

        AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Move first task to doing (column index 1), second stays in todo (column index 0)
        MoveTask::to_column(id1, "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // next task should return the todo task (earlier column wins)
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Todo task");
    }
}
