//! ListTasks command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{ActorId, ColumnId, SwimlaneId, TagId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List tasks with optional filters
#[operation(verb = "list", noun = "tasks", description = "List tasks with optional filters")]
#[derive(Debug, Default, Deserialize)]
pub struct ListTasks {
    /// Filter by column
    pub column: Option<ColumnId>,
    /// Filter by swimlane
    pub swimlane: Option<SwimlaneId>,
    /// Filter by tag
    pub tag: Option<TagId>,
    /// Filter by assignee
    pub assignee: Option<ActorId>,
    /// Filter by readiness status
    pub ready: Option<bool>,
}

impl ListTasks {
    /// Create a new ListTasks command with no filters
    pub fn new() -> Self {
        Self {
            column: None,
            swimlane: None,
            tag: None,
            assignee: None,
            ready: None,
        }
    }

    /// Filter by column
    pub fn with_column(mut self, column: impl Into<ColumnId>) -> Self {
        self.column = Some(column.into());
        self
    }

    /// Filter by swimlane
    pub fn with_swimlane(mut self, swimlane: impl Into<SwimlaneId>) -> Self {
        self.swimlane = Some(swimlane.into());
        self
    }

    /// Filter by tag
    pub fn with_tag(mut self, tag: impl Into<TagId>) -> Self {
        self.tag = Some(tag.into());
        self
    }

    /// Filter by assignee
    pub fn with_assignee(mut self, assignee: impl Into<ActorId>) -> Self {
        self.assignee = Some(assignee.into());
        self
    }

    /// Filter by readiness
    pub fn with_ready(mut self, ready: bool) -> Self {
        self.ready = Some(ready);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListTasks {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let board = ctx.read_board().await?;
            let all_tasks = ctx.read_all_tasks().await?;

            let terminal_column = board
                .terminal_column()
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            // Filter tasks
            let filtered: Vec<Value> = all_tasks
                .iter()
                .filter(|t| {
                    // Filter by column
                    if let Some(ref col) = self.column {
                        if &t.position.column != col {
                            return false;
                        }
                    }

                    // Filter by swimlane
                    if let Some(ref swimlane) = self.swimlane {
                        if t.position.swimlane.as_ref() != Some(swimlane) {
                            return false;
                        }
                    }

                    // Filter by tag
                    if let Some(ref tag) = self.tag {
                        if !t.tags.contains(tag) {
                            return false;
                        }
                    }

                    // Filter by assignee
                    if let Some(ref assignee) = self.assignee {
                        if !t.assignees.contains(assignee) {
                            return false;
                        }
                    }

                    // Filter by readiness
                    if let Some(ready) = self.ready {
                        let is_ready = t.is_ready(&all_tasks, terminal_column);
                        if is_ready != ready {
                            return false;
                        }
                    }

                    true
                })
                .map(|t| {
                    let ready = t.is_ready(&all_tasks, terminal_column);
                    let blocked_by = t.blocked_by(&all_tasks, terminal_column);
                    let progress = t.progress();

                    let mut result = serde_json::to_value(t).unwrap_or(Value::Null);
                    result["ready"] = serde_json::json!(ready);
                    result["blocked_by"] = serde_json::to_value(&blocked_by).unwrap_or(Value::Null);
                    result["progress"] = serde_json::json!(progress);
                    result
                })
                .collect();

            Ok(serde_json::json!({
                "tasks": filtered,
                "count": filtered.len()
            }))
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
    use crate::types::TaskId;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_list_tasks_empty() {
        let (_temp, ctx) = setup().await;

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 0);
        assert!(result["tasks"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_list_tasks_all() {
        let (_temp, ctx) = setup().await;

        AddTask::new("Task 1").execute(&ctx).await.into_result().unwrap();
        AddTask::new("Task 2").execute(&ctx).await.into_result().unwrap();

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 2);
    }

    #[tokio::test]
    async fn test_list_tasks_by_column() {
        let (_temp, ctx) = setup().await;

        let result1 = AddTask::new("Todo task").execute(&ctx).await.into_result().unwrap();
        let id1 = result1["id"].as_str().unwrap();
        AddTask::new("Another todo").execute(&ctx).await.into_result().unwrap();

        // Move one to done
        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result().unwrap();

        // List only todo column
        let result = ListTasks::new()
            .with_column("todo")
            .execute(&ctx)
            .await
            .into_result().unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Another todo");
    }

    #[tokio::test]
    async fn test_list_tasks_by_ready() {
        let (_temp, ctx) = setup().await;

        let result1 = AddTask::new("Blocker").execute(&ctx).await.into_result().unwrap();
        let id1 = result1["id"].as_str().unwrap();

        AddTask::new("Blocked")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result().unwrap();

        // List only ready tasks
        let result = ListTasks::new()
            .with_ready(true)
            .execute(&ctx)
            .await
            .into_result().unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Blocker");

        // List only blocked tasks
        let result = ListTasks::new()
            .with_ready(false)
            .execute(&ctx)
            .await
            .into_result().unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Blocked");
    }
}
