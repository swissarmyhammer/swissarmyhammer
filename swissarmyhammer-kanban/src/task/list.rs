//! ListTasks command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task::shared::parse_filter_expr;
use crate::task_helpers::{enrich_all_task_entities, task_entity_to_rich_json, TaskFilterAdapter};
use crate::types::ColumnId;
use crate::virtual_tags::default_virtual_tag_registry;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List tasks with optional column and DSL filter.
#[operation(
    verb = "list",
    noun = "tasks",
    description = "List tasks with optional filters"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListTasks {
    /// Filter by column (structural — when absent, done column is excluded).
    pub column: Option<ColumnId>,
    /// Filter DSL expression (e.g. `#bug && @alice`).
    pub filter: Option<String>,
}

impl ListTasks {
    /// Create a new ListTasks command with no filters.
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by column.
    pub fn with_column(mut self, column: impl Into<ColumnId>) -> Self {
        self.column = Some(column.into());
        self
    }

    /// Set a filter DSL expression.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListTasks {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let all_columns = ectx.list("column").await?;
            let mut all_tasks = ectx.list("task").await?;

            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            let registry = default_virtual_tag_registry();
            enrich_all_task_entities(&mut all_tasks, terminal_column, registry);

            let expr = parse_filter_expr(self.filter.as_deref())?;
            let column = &self.column;

            let filtered: Vec<Value> = all_tasks
                .iter()
                .filter(|t| {
                    if let Some(ref col) = column {
                        if t.get_str("position_column") != Some(col.as_str()) {
                            return false;
                        }
                    } else if t.get_str("position_column") == Some(terminal_column) {
                        return false;
                    }
                    if let Some(ref e) = expr {
                        if !e.matches(&TaskFilterAdapter { entity: t }) {
                            return false;
                        }
                    }
                    true
                })
                .map(task_entity_to_rich_json)
                .collect();

            Ok(serde_json::json!({ "tasks": filtered, "count": filtered.len() }))
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

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

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

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 2);
    }

    #[tokio::test]
    async fn test_list_tasks_by_column() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();
        AddTask::new("Another todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_column("todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Another todo");
    }

    #[tokio::test]
    async fn test_list_tasks_excludes_done_by_default() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Still todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("In progress")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r3 = AddTask::new("Finished")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        MoveTask::to_column(r2["id"].as_str().unwrap(), "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r3["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 2);
        let titles: Vec<&str> = result["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect();
        assert!(titles.contains(&"Still todo"));
        assert!(titles.contains(&"In progress"));
        assert!(!titles.contains(&"Finished"));
    }

    #[tokio::test]
    async fn test_list_tasks_explicit_done_column_returns_done() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Finished task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Open task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r1["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_column("done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Finished task");
    }

    #[tokio::test]
    async fn test_list_tasks_filter_by_ready() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Blocker")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();
        AddTask::new("Blocked")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // #READY virtual tag matches only ready tasks
        let result = ListTasks::new()
            .with_filter("#READY")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Blocker");

        // #BLOCKED virtual tag matches only blocked tasks
        let result = ListTasks::new()
            .with_filter("#BLOCKED")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Blocked");
    }

    #[tokio::test]
    async fn test_list_tasks_excludes_archived() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Task 2 (to archive)")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = r2["id"].as_str().unwrap().to_string();
        AddTask::new("Task 3")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", &id2).await.unwrap();

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 2);
        let titles: Vec<&str> = result["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect();
        assert!(titles.contains(&"Task 1"));
        assert!(titles.contains(&"Task 3"));
    }

    #[tokio::test]
    async fn test_list_tasks_filter_by_tag() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Tagged task")
            .with_description("This task has a #bug tag")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Untagged task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_filter("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Tagged task");
    }

    #[tokio::test]
    async fn test_list_tasks_filter_by_project() {
        use crate::project::AddProject;

        let (_temp, ctx) = setup().await;
        AddProject::new("myproj", "My Project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddTask::new("Project task")
            .with_project("myproj")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Unrelated task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_filter("$myproj")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Project task");
    }

    #[tokio::test]
    async fn test_list_tasks_filter_by_project_case_insensitive() {
        use crate::project::AddProject;

        let (_temp, ctx) = setup().await;
        AddProject::new("myproj", "My Project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddTask::new("Project task")
            .with_project("myproj")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_filter("$MYPROJ")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Project task");
    }

    #[tokio::test]
    async fn test_list_tasks_filter_by_assignee() {
        let (_temp, ctx) = setup().await;
        use crate::actor::AddActor;
        use crate::task::AssignTask;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddActor::new("bob", "Bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let r1 = AddTask::new("Alice's task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Bob's task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Unassigned task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AssignTask::new(r1["id"].as_str().unwrap(), "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(r2["id"].as_str().unwrap(), "bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_filter("@alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Alice's task");
    }

    #[tokio::test]
    async fn test_list_tasks_filter_boolean_logic() {
        let (_temp, ctx) = setup().await;
        use crate::actor::AddActor;
        use crate::task::AssignTask;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let r1 = AddTask::new("Bug by Alice")
            .with_description("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(r1["id"].as_str().unwrap(), "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddTask::new("Bug unassigned")
            .with_description("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Feature by Alice")
            .with_description("#feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_filter("#bug && @alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Bug by Alice");
    }

    #[tokio::test]
    async fn test_list_tasks_unarchive_restores() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Task A")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();

        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", &id1).await.unwrap();
        assert_eq!(
            ListTasks::new().execute(&ctx).await.into_result().unwrap()["count"],
            0
        );

        ectx.unarchive("task", &id1).await.unwrap();
        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Task A");
    }
}
