//! ListTasks command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task::shared::parse_filter_expr;
use crate::task_helpers::{
    enrich_all_task_entities, task_entity_to_rich_json, EntitySlugRegistry, TaskFilterAdapter,
};
use crate::types::ColumnId;
use crate::virtual_tags::default_virtual_tag_registry;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Default number of tasks returned per page when the caller does not
/// specify `page_size`. Picked to keep AI-driven `list tasks` calls cheap
/// — at ~200 prompt tokens per enriched task, 10 tasks is well under 2k
/// tokens and avoids the multi-tens-of-thousands-of-tokens tool result
/// that an unpaginated list of a busy board produces.
pub const DEFAULT_PAGE_SIZE: usize = 10;

/// Upper bound on a single `list tasks` response. A caller asking for an
/// unreasonably large page is clamped down rather than silently surprised
/// by a partial result — and the bound keeps prompt-eating tool results
/// bounded regardless of caller behaviour.
pub const MAX_PAGE_SIZE: usize = 100;

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
    /// 1-indexed page number. Defaults to 1 when unset; values < 1 are
    /// treated as 1.
    pub page: Option<usize>,
    /// Tasks per page. Defaults to [`DEFAULT_PAGE_SIZE`] (10) when unset;
    /// clamped to `1..=MAX_PAGE_SIZE` otherwise.
    pub page_size: Option<usize>,
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

    /// Request a specific page (1-indexed).
    pub fn with_page(mut self, page: usize) -> Self {
        self.page = Some(page);
        self
    }

    /// Override the page size (clamped to `1..=MAX_PAGE_SIZE`).
    pub fn with_page_size(mut self, page_size: usize) -> Self {
        self.page_size = Some(page_size);
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

            // Build the id-or-slug registry so `$project`, `@user`, and
            // `^task` predicates resolve display-name slugs to entity ids.
            let all_projects = ectx.list("project").await?;
            let all_actors = ectx.list("actor").await?;
            let slug_registry = EntitySlugRegistry::build(&all_projects, &all_actors, &all_tasks);

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
                        if !e.matches(&TaskFilterAdapter::with_registry(t, &slug_registry)) {
                            return false;
                        }
                    }
                    true
                })
                .map(task_entity_to_rich_json)
                .collect();

            // Pagination — applied AFTER filtering so the page metadata
            // reflects the filtered set, not the raw board. Without this
            // the kanban MCP `list tasks` op returned the entire board on
            // every call: a busy board's enriched JSON could blow past
            // 25k prompt tokens per response, eating the AI's context
            // budget on a single tool call.
            let total = filtered.len();
            let page = self.page.unwrap_or(1).max(1);
            let page_size = self
                .page_size
                .unwrap_or(DEFAULT_PAGE_SIZE)
                .clamp(1, MAX_PAGE_SIZE);
            let total_pages = total.div_ceil(page_size).max(1);
            let start = (page - 1).saturating_mul(page_size);
            let paginated: Vec<Value> =
                filtered.into_iter().skip(start).take(page_size).collect();

            Ok(serde_json::json!({
                "tasks": paginated,
                // `count` continues to mean "number of items in the returned
                // `tasks` array" — the most common consumer of the field —
                // and matches `paginated.len()` exactly. Callers wanting
                // the unpaginated total use `total`.
                "count": paginated.len(),
                "total": total,
                "page": page,
                "page_size": page_size,
                "total_pages": total_pages,
            }))
        }
        .await
        {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
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

    /// End-to-end regression of the concrete reproducer in
    /// `.kanban/tasks/01KPDWC4F4QPVTJZNN1NQKJAPJ.md`: project id
    /// `task-card-fields` with name "Task card & field polish" must
    /// match a filter of `$task-card-field-polish` (the slug of the
    /// display name, which is what the frontend autocomplete offers).
    #[tokio::test]
    async fn test_list_tasks_filter_by_project_slug_of_name() {
        use crate::project::AddProject;

        let (_temp, ctx) = setup().await;
        AddProject::new("task-card-fields", "Task card & field polish")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddTask::new("Project task")
            .with_project("task-card-fields")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Unrelated task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Filter by the slug of the project's display name, not the id.
        let result = ListTasks::new()
            .with_filter("$task-card-field-polish")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Project task");

        // The id-based filter still works (backwards compat).
        let result = ListTasks::new()
            .with_filter("$task-card-fields")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Project task");
    }

    /// Assignee filter must match the slug of an actor's display name as
    /// well as the actor's id — the frontend autocomplete offers the
    /// name-slug for `@user` mentions.
    #[tokio::test]
    async fn test_list_tasks_filter_by_assignee_slug_of_name() {
        use crate::actor::AddActor;
        use crate::task::AssignTask;

        let (_temp, ctx) = setup().await;
        AddActor::new("alice", "Alice Smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let r1 = AddTask::new("Alice's task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(r1["id"].as_str().unwrap(), "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Unassigned")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Match by slug of the actor's display name.
        let result = ListTasks::new()
            .with_filter("@alice-smith")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 1);
        assert_eq!(result["tasks"][0]["title"], "Alice's task");
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

    // --- Pagination ---------------------------------------------------------

    /// Default page size of 10 must be applied even when the caller passes
    /// neither `page` nor `page_size`. This is the behaviour that keeps the
    /// AI tool result bounded on a busy board.
    #[tokio::test]
    async fn test_list_tasks_default_page_size_is_10() {
        let (_temp, ctx) = setup().await;
        for i in 0..15 {
            AddTask::new(format!("Task {i}"))
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 10, "default page returns 10 tasks");
        assert_eq!(result["total"], 15, "total reports unpaginated size");
        assert_eq!(result["page"], 1);
        assert_eq!(result["page_size"], 10);
        assert_eq!(result["total_pages"], 2);
        assert_eq!(result["tasks"].as_array().unwrap().len(), 10);
    }

    /// `page=2` returns the second slice with the correct metadata; a partial
    /// final page is shorter than `page_size` but is still page=2.
    #[tokio::test]
    async fn test_list_tasks_second_page() {
        let (_temp, ctx) = setup().await;
        for i in 0..15 {
            AddTask::new(format!("Task {i:02}"))
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        let result = ListTasks::new()
            .with_page(2)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 5, "remainder on the second page");
        assert_eq!(result["total"], 15);
        assert_eq!(result["page"], 2);
        assert_eq!(result["total_pages"], 2);
        assert_eq!(result["tasks"].as_array().unwrap().len(), 5);
    }

    /// Explicit `page_size` overrides the default and is honoured by both the
    /// slice math and the metadata.
    #[tokio::test]
    async fn test_list_tasks_explicit_page_size() {
        let (_temp, ctx) = setup().await;
        for i in 0..7 {
            AddTask::new(format!("Task {i}"))
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        let result = ListTasks::new()
            .with_page_size(3)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 3);
        assert_eq!(result["page_size"], 3);
        assert_eq!(result["total_pages"], 3, "7 items / 3 per page = 3 pages");

        let last_page = ListTasks::new()
            .with_page(3)
            .with_page_size(3)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(last_page["count"], 1, "final partial page has 1 task");
        assert_eq!(last_page["page"], 3);
    }

    /// A page past the last page returns an empty `tasks` array but still
    /// reports accurate metadata — callers can safely paginate forward
    /// without a pre-emptive total fetch.
    #[tokio::test]
    async fn test_list_tasks_page_beyond_range_is_empty() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Only task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_page(5)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["count"], 0);
        assert_eq!(result["total"], 1);
        assert_eq!(result["page"], 5);
        assert_eq!(result["total_pages"], 1);
        assert!(result["tasks"].as_array().unwrap().is_empty());
    }

    /// An empty board returns `total_pages: 1` (not 0) so callers can
    /// branch on `total === 0` rather than special-casing zero-page math.
    #[tokio::test]
    async fn test_list_tasks_empty_pagination_metadata() {
        let (_temp, ctx) = setup().await;
        let result = ListTasks::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["count"], 0);
        assert_eq!(result["total"], 0);
        assert_eq!(result["page"], 1);
        assert_eq!(result["page_size"], 10);
        assert_eq!(result["total_pages"], 1);
    }

    /// `page_size` over `MAX_PAGE_SIZE` is clamped so a caller cannot
    /// blow up the response by passing an absurd value.
    #[tokio::test]
    async fn test_list_tasks_page_size_clamped_to_max() {
        let (_temp, ctx) = setup().await;
        AddTask::new("Only")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = ListTasks::new()
            .with_page_size(MAX_PAGE_SIZE * 10)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(
            result["page_size"].as_u64().unwrap() as usize,
            MAX_PAGE_SIZE
        );
    }

    /// Filter is applied BEFORE pagination, so `total` reflects only the
    /// matched set.
    #[tokio::test]
    async fn test_list_tasks_filter_then_paginate() {
        let (_temp, ctx) = setup().await;
        for i in 0..12 {
            AddTask::new(format!("Task {i}"))
                .with_description(if i % 2 == 0 { "#bug" } else { "" })
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        let result = ListTasks::new()
            .with_filter("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["total"], 6, "6 of 12 tasks have #bug");
        assert_eq!(result["count"], 6, "fits on default page");
        assert_eq!(result["total_pages"], 1);
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
