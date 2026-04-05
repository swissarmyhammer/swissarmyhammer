//! ListTasks command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task_helpers::{task_entity_to_rich_json, task_is_ready, task_tags};
use crate::types::{ActorId, ColumnId, SwimlaneId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List tasks with optional filters
#[operation(
    verb = "list",
    noun = "tasks",
    description = "List tasks with optional filters"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListTasks {
    /// Filter by column
    pub column: Option<ColumnId>,
    /// Filter by swimlane
    pub swimlane: Option<SwimlaneId>,
    /// Filter by tag name (slug)
    pub tag: Option<String>,
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

    /// Filter by tag name (slug)
    pub fn with_tag(mut self, tag: impl Into<String>) -> Self {
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
            let ectx = ctx.entity_context().await?;
            let all_columns = ectx.list("column").await?;
            let all_tasks = ectx.list("task").await?;

            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            // Filter tasks
            let filtered: Vec<Value> = all_tasks
                .iter()
                .filter(|t| {
                    // Filter by column — when no column is specified, exclude done
                    // (terminal) column by default. Listing all tasks including done
                    // produces huge results and is almost never what you want.
                    if let Some(ref col) = self.column {
                        if t.get_str("position_column") != Some(col.as_str()) {
                            return false;
                        }
                    } else if t.get_str("position_column") == Some(terminal_column) {
                        return false;
                    }

                    // Filter by swimlane
                    if let Some(ref swimlane) = self.swimlane {
                        if t.get_str("position_swimlane") != Some(swimlane.as_str()) {
                            return false;
                        }
                    }

                    // Filter by tag name (computed from body)
                    if let Some(ref tag_name) = self.tag {
                        if !task_tags(t).iter().any(|tag| tag == tag_name) {
                            return false;
                        }
                    }

                    // Filter by assignee
                    if let Some(ref assignee) = self.assignee {
                        if !t
                            .get_string_list("assignees")
                            .contains(&assignee.to_string())
                        {
                            return false;
                        }
                    }

                    // Filter by readiness
                    if let Some(ready) = self.ready {
                        let is_ready = task_is_ready(t, &all_tasks, terminal_column);
                        if is_ready != ready {
                            return false;
                        }
                    }

                    true
                })
                .map(|t| task_entity_to_rich_json(t, &all_tasks, terminal_column))
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

        let result1 = AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();
        AddTask::new("Another todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Move one to done
        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // List only todo column
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
        let id2 = r2["id"].as_str().unwrap();
        let id3 = r3["id"].as_str().unwrap();

        // Move one to doing, one to done
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

        // No filters → should exclude done, return only todo + doing
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
        let id1 = r1["id"].as_str().unwrap();
        AddTask::new("Open task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        MoveTask::to_column(id1, "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Explicit column: "done" → should return only done tasks
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
    async fn test_list_tasks_by_ready() {
        let (_temp, ctx) = setup().await;

        let result1 = AddTask::new("Blocker")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = result1["id"].as_str().unwrap();

        AddTask::new("Blocked")
            .with_depends_on(vec![TaskId::from_string(id1)])
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // List only ready tasks
        let result = ListTasks::new()
            .with_ready(true)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Blocker");

        // List only blocked tasks
        let result = ListTasks::new()
            .with_ready(false)
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

        // Create 3 tasks
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

        // Archive task 2
        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", &id2).await.unwrap();

        // ListTasks should return only the 2 active tasks
        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(
            result["count"], 2,
            "archived task should not appear in list"
        );
        let titles: Vec<&str> = result["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["title"].as_str().unwrap())
            .collect();
        assert!(titles.contains(&"Task 1"));
        assert!(titles.contains(&"Task 3"));
        assert!(!titles.contains(&"Task 2 (to archive)"));
    }

    #[tokio::test]
    async fn test_list_tasks_by_swimlane() {
        let (_temp, ctx) = setup().await;

        use crate::swimlane::AddSwimlane;
        AddSwimlane::new("feature", "Feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a task in the feature swimlane
        let r1 = AddTask::new("Feature task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap();
        MoveTask::to_column_and_swimlane(id1, "todo", "feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Add a task with no swimlane
        AddTask::new("No swimlane task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Filter by swimlane — should only return the feature task
        let result = ListTasks::new()
            .with_swimlane("feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Feature task");
    }

    #[tokio::test]
    async fn test_list_tasks_by_tag() {
        let (_temp, ctx) = setup().await;

        // Add tasks — one with a tag in description, one without
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

        // Filter by tag
        let result = ListTasks::new()
            .with_tag("bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Tagged task");
    }

    #[tokio::test]
    async fn test_list_tasks_by_assignee() {
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
        let id1 = r1["id"].as_str().unwrap();

        let r2 = AddTask::new("Bob's task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id2 = r2["id"].as_str().unwrap();

        AddTask::new("Unassigned task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AssignTask::new(id1, "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(id2, "bob")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Filter by alice
        let result = ListTasks::new()
            .with_assignee("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Alice's task");
    }

    #[tokio::test]
    async fn test_list_tasks_unarchive_restores() {
        let (_temp, ctx) = setup().await;

        // Create a task and archive it
        let r1 = AddTask::new("Task A")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let id1 = r1["id"].as_str().unwrap().to_string();

        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", &id1).await.unwrap();

        // Archived: should not appear
        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 0, "archived task should be hidden");

        // Unarchive it
        ectx.unarchive("task", &id1).await.unwrap();

        // Should reappear
        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(
            result["count"], 1,
            "unarchived task should reappear in list"
        );
        assert_eq!(result["tasks"][0]["title"], "Task A");
    }
}
