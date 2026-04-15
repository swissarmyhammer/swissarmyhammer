//! NextTask command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::task::shared::parse_filter_expr;
use crate::task_helpers::{enrich_all_task_entities, task_entity_to_rich_json, TaskFilterAdapter};
use crate::types::Ordinal;
use crate::virtual_tags::default_virtual_tag_registry;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get the next actionable task.
#[operation(
    verb = "next",
    noun = "task",
    description = "Get the oldest ready task not in the done column"
)]
#[derive(Debug, Default, Deserialize)]
pub struct NextTask {
    /// Filter DSL expression (e.g. `#bug`).
    pub filter: Option<String>,
}

impl NextTask {
    /// Create a new NextTask command with no filter.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set a filter DSL expression.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }
}

/// Build a column-id to ordering-index map for positional sorting.
fn build_column_order(
    columns: &[swissarmyhammer_entity::Entity],
) -> std::collections::HashMap<&str, usize> {
    columns
        .iter()
        .enumerate()
        .map(|(i, c)| (c.id.as_str(), i))
        .collect()
}

/// Check whether a task is actionable: not done, ready, and passes the DSL filter.
fn is_actionable(
    t: &swissarmyhammer_entity::Entity,
    terminal_column: &str,
    expr: &Option<swissarmyhammer_filter_expr::Expr>,
) -> bool {
    if t.get_str("position_column") == Some(terminal_column) {
        return false;
    }
    if !t.get("ready").and_then(|v| v.as_bool()).unwrap_or(true) {
        return false;
    }
    if let Some(ref e) = expr {
        if !e.matches(&TaskFilterAdapter { entity: t }) {
            return false;
        }
    }
    true
}

/// Compare two tasks by column order, then by ordinal within column.
fn compare_by_position(
    a: &swissarmyhammer_entity::Entity,
    b: &swissarmyhammer_entity::Entity,
    column_order: &std::collections::HashMap<&str, usize>,
) -> std::cmp::Ordering {
    let col_a = column_order
        .get(a.get_str("position_column").unwrap_or(""))
        .unwrap_or(&0);
    let col_b = column_order
        .get(b.get_str("position_column").unwrap_or(""))
        .unwrap_or(&0);
    let ord_a = Ordinal::from_string(
        a.get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR),
    );
    let ord_b = Ordinal::from_string(
        b.get_str("position_ordinal")
            .unwrap_or(Ordinal::DEFAULT_STR),
    );
    col_a.cmp(col_b).then(ord_a.cmp(&ord_b))
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for NextTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            // Sort by the declared `order` field so `build_column_order` below
            // derives a stable index map. `list("column")` returns entities in
            // completion order from concurrent reads, which is non-deterministic
            // and would otherwise cause tasks to be ranked by arbitrary column
            // position. Matches the pattern used by GetBoard/ListColumns.
            let mut all_columns = ectx.list("column").await?;
            all_columns
                .sort_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0) as usize);
            let mut all_tasks = ectx.list("task").await?;

            let terminal_column = all_columns
                .iter()
                .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                .map(|c| c.id.as_str())
                .unwrap_or("done");

            let registry = default_virtual_tag_registry();
            enrich_all_task_entities(&mut all_tasks, terminal_column, registry);

            let expr = parse_filter_expr(self.filter.as_deref())?;
            let column_order = build_column_order(&all_columns);

            let mut candidates: Vec<&swissarmyhammer_entity::Entity> = all_tasks
                .iter()
                .filter(|t| is_actionable(t, terminal_column, &expr))
                .collect();

            candidates.sort_by(|a, b| compare_by_position(a, b, &column_order));

            match candidates.first() {
                Some(task) => Ok(task_entity_to_rich_json(task)),
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

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Blocker");
    }

    #[tokio::test]
    async fn test_next_task_filter_by_tag() {
        let (_temp, ctx) = setup().await;
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

        // Without filter, returns first task
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Untagged task");

        // With filter, skips untagged
        let result = NextTask::new()
            .with_filter("#bug")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["title"], "Bug task");

        // Non-matching filter returns null
        let result = NextTask::new()
            .with_filter("#feature")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_next_task_filter_by_project() {
        use crate::project::AddProject;

        let (_temp, ctx) = setup().await;
        AddProject::new("myproj", "My Project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddTask::new("Unrelated task")
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

        // Without filter, returns first task
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Unrelated task");

        // With project filter, skips unrelated
        let result = NextTask::new()
            .with_filter("$myproj")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["title"], "Project task");

        // Non-matching project filter returns null
        let result = NextTask::new()
            .with_filter("$other")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_next_task_ignores_done() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Done task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r1["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Todo task");
    }

    #[tokio::test]
    async fn test_next_task_searches_all_non_done_columns() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Task in todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Task in doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r3 = AddTask::new("Task in done")
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

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task in todo");

        MoveTask::to_column(r1["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task in doing");

        MoveTask::to_column(r2["id"].as_str().unwrap(), "done")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_next_task_prefers_earlier_column() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Doing task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Todo task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        MoveTask::to_column(r1["id"].as_str().unwrap(), "doing")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Todo task");
    }

    #[tokio::test]
    async fn test_next_task_skips_archived() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Task 1 (to archive)")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Task 2 (active)")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", r1["id"].as_str().unwrap())
            .await
            .unwrap();

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert_eq!(result["title"], "Task 2 (active)");
    }

    #[tokio::test]
    async fn test_next_task_all_archived_returns_null() {
        let (_temp, ctx) = setup().await;
        let r1 = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r2 = AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let ectx = ctx.entity_context().await.unwrap();
        ectx.archive("task", r1["id"].as_str().unwrap())
            .await
            .unwrap();
        ectx.archive("task", r2["id"].as_str().unwrap())
            .await
            .unwrap();

        let result = NextTask::new().execute(&ctx).await.into_result().unwrap();
        assert!(result.is_null());
    }

    #[tokio::test]
    async fn test_next_task_filter_with_assignee() {
        let (_temp, ctx) = setup().await;
        use crate::actor::AddActor;
        use crate::task::AssignTask;

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let r1 = AddTask::new("Alice's task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AddTask::new("Unassigned")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        AssignTask::new(r1["id"].as_str().unwrap(), "alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = NextTask::new()
            .with_filter("@alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["title"], "Alice's task");
    }
}
