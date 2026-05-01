//! AddTask command

use crate::context::KanbanContext;
use crate::entity::position;
use crate::error::{KanbanError, Result};
use crate::task::shared::{auto_create_body_tags, parse_iso8601_date};
use crate::task_helpers::task_entity_to_json;
use crate::types::{ActorId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new task to the board.
///
/// Tags are derived from `#tag` patterns in the description — no explicit
/// tags parameter needed.
#[operation(
    verb = "add",
    noun = "task",
    description = "Create a new task on the board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddTask {
    /// The task title (required)
    pub title: String,
    /// Detailed task description (may contain #tag patterns)
    pub description: Option<String>,
    /// Initial column (if None, uses first column)
    pub column: Option<String>,
    /// Initial ordinal (if None, appended at end)
    pub ordinal: Option<String>,
    /// Assignees for this task
    #[serde(default)]
    pub assignees: Vec<ActorId>,
    /// Task IDs this task depends on
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
    /// Project this task belongs to
    pub project: Option<String>,
    /// Hard deadline date (ISO 8601 date string, e.g. "2026-04-30").
    ///
    /// Optional user-set date stored alongside the task. Empty string is
    /// rejected — use `None` (omit the field) to leave it unset at creation.
    pub due: Option<String>,
    /// Earliest start date (ISO 8601 date string, e.g. "2026-04-15").
    ///
    /// Optional user-set date stored alongside the task. Empty string is
    /// rejected — use `None` (omit the field) to leave it unset at creation.
    pub scheduled: Option<String>,
}

impl AddTask {
    /// Create a new AddTask command with just a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            column: None,
            ordinal: None,
            assignees: Vec::new(),
            depends_on: Vec::new(),
            project: None,
            due: None,
            scheduled: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the position (column, ordinal) for backward compat
    pub fn with_position(mut self, position: crate::types::Position) -> Self {
        self.column = Some(position.column.to_string());
        self.ordinal = Some(position.ordinal.as_str().to_string());
        self
    }

    /// Set the assignees
    pub fn with_assignees(mut self, assignees: Vec<ActorId>) -> Self {
        self.assignees = assignees;
        self
    }

    /// Set the dependencies
    pub fn with_depends_on(mut self, deps: Vec<TaskId>) -> Self {
        self.depends_on = deps;
        self
    }

    /// Set the project
    pub fn with_project(mut self, project: impl Into<String>) -> Self {
        self.project = Some(project.into());
        self
    }

    /// Set the hard deadline date (ISO 8601).
    pub fn with_due(mut self, due: impl Into<String>) -> Self {
        self.due = Some(due.into());
        self
    }

    /// Set the earliest start date (ISO 8601).
    pub fn with_scheduled(mut self, scheduled: impl Into<String>) -> Self {
        self.scheduled = Some(scheduled.into());
        self
    }

    /// Build the task entity from this command's fields.
    ///
    /// Resolves position (column + ordinal) via [`position::resolve_column`]
    /// and [`position::resolve_ordinal`], applies all user-set fields, and
    /// parses any ISO 8601 date inputs. The resulting entity is not yet
    /// persisted — the caller owns the write.
    async fn build_entity(&self, ectx: &swissarmyhammer_entity::EntityContext) -> Result<Entity> {
        let column = position::resolve_column(ectx, self.column.as_deref(), "task").await?;
        let ordinal =
            position::resolve_ordinal(ectx, "task", &column, self.ordinal.as_deref()).await?;

        let task_id = TaskId::new();
        let mut entity = Entity::new("task", task_id.as_str());
        entity.set("title", json!(self.title));
        entity.set("body", json!(self.description.clone().unwrap_or_default()));
        entity.set("position_column", json!(column));
        entity.set("position_ordinal", json!(ordinal));

        if !self.assignees.is_empty() {
            entity.set("assignees", serde_json::to_value(&self.assignees)?);
        }
        if !self.depends_on.is_empty() {
            entity.set("depends_on", serde_json::to_value(&self.depends_on)?);
        }
        if let Some(ref project) = self.project {
            entity.set("project", json!(project));
        }
        if let Some(ref due) = self.due {
            entity.set("due", json!(parse_iso8601_date(due, "due")?));
        }
        if let Some(ref scheduled) = self.scheduled {
            entity.set(
                "scheduled",
                json!(parse_iso8601_date(scheduled, "scheduled")?),
            );
        }

        Ok(entity)
    }

    /// Persist the task entity and run post-write hooks (auto-tag creation).
    async fn persist(
        &self,
        ectx: &swissarmyhammer_entity::EntityContext,
        entity: &Entity,
    ) -> Result<()> {
        ectx.write(entity).await?;
        auto_create_body_tags(ectx, entity).await?;
        Ok(())
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let entity = self.build_entity(&ectx).await?;
            self.persist(&ectx, &entity).await?;
            Ok(task_entity_to_json(&entity))
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
    async fn test_add_task() {
        let (_temp, ctx) = setup().await;

        let cmd = AddTask::new("Test task").with_description("A test task");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["title"], "Test task");
        assert_eq!(result["description"], "A test task");
        assert_eq!(result["position"]["column"], "todo");
    }

    #[tokio::test]
    async fn test_add_task_places_in_first_column_by_default() {
        // The default board has todo/doing/done columns (in that order).
        // Adding a task without specifying a column should place it in "todo".
        let (_temp, ctx) = setup().await;

        let cmd = AddTask::new("My new task");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(
            result["position"]["column"], "todo",
            "task should be placed in first column (todo) when no column specified"
        );
    }

    #[tokio::test]
    async fn test_add_task_with_explicit_column_uses_that_column() {
        // When an explicit column is provided, the task should land there, not in todo.
        let (_temp, ctx) = setup().await;

        let mut cmd = AddTask::new("Task in doing");
        cmd.column = Some("doing".to_string());
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(
            result["position"]["column"], "doing",
            "task should be placed in the explicitly specified column"
        );
    }

    #[tokio::test]
    async fn test_add_task_on_board_with_no_columns_returns_error() {
        // Set up a board without going through InitBoard (which creates default columns).
        // Instead, create the directory structure and a board entity manually,
        // leaving the columns directory empty.
        use crate::board::InitBoard;
        use crate::column::DeleteColumn;

        let temp = tempfile::TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(&kanban_dir);

        // Initialize the board (creates todo/doing/done columns)
        InitBoard::new("Empty Board")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Delete all three default columns
        for col_id in &["todo", "doing", "done"] {
            DeleteColumn::new(*col_id)
                .execute(&ctx)
                .await
                .into_result()
                .unwrap();
        }

        // Now attempt to add a task — should return an error, not panic
        let cmd = AddTask::new("Task on empty board");
        let result = cmd.execute(&ctx).await.into_result();

        assert!(
            result.is_err(),
            "adding a task to a board with no columns should return an error"
        );
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("no columns"),
            "error message should mention missing columns, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_add_multiple_tasks_ordering() {
        let (_temp, ctx) = setup().await;

        // Add first task
        let result1 = AddTask::new("Task 1")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let ordinal1 = result1["position"]["ordinal"].as_str().unwrap();

        // Add second task
        let result2 = AddTask::new("Task 2")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let ordinal2 = result2["position"]["ordinal"].as_str().unwrap();

        // Second should be after first
        assert!(ordinal2 > ordinal1);
    }

    #[tokio::test]
    async fn test_add_task_project_field_null_when_unset() {
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("Task without project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert!(
            result.get("project").is_some(),
            "project field should be present in task JSON"
        );
        assert!(
            result["project"].is_null(),
            "project should be null when unset"
        );
    }

    #[tokio::test]
    async fn test_add_task_with_project() {
        let (_temp, ctx) = setup().await;

        // Create a project entity first
        use crate::project::AddProject;
        let project = AddProject::new("my-project", "My Project")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let project_id = project["id"].as_str().unwrap();

        // Create a task with that project
        let result = AddTask::new("Task with project")
            .with_project(project_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(
            result["project"].as_str().unwrap(),
            project_id,
            "task should have the project set"
        );
    }

    // -----------------------------------------------------------------------
    // Date field tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_add_task_emits_null_dates_by_default() {
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("Task with no dates")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // User-set dates must be null when unset.
        assert!(result["due"].is_null(), "due should be null when unset");
        assert!(
            result["scheduled"].is_null(),
            "scheduled should be null when unset"
        );
        // System dates may be null immediately on write (no changelog read
        // happens until the next read), but the field keys must be present.
        assert!(result.get("created").is_some(), "created key should exist");
        assert!(result.get("updated").is_some(), "updated key should exist");
        assert!(result.get("started").is_some(), "started key should exist");
        assert!(
            result.get("completed").is_some(),
            "completed key should exist"
        );
    }

    #[tokio::test]
    async fn test_add_task_with_due_and_scheduled() {
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("Task with dates")
            .with_due("2026-04-30")
            .with_scheduled("2026-04-15")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["due"], "2026-04-30");
        assert_eq!(result["scheduled"], "2026-04-15");
    }

    #[tokio::test]
    async fn test_add_task_rejects_invalid_due_date() {
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("Task with bad date")
            .with_due("not-a-date")
            .execute(&ctx)
            .await
            .into_result();

        assert!(result.is_err(), "invalid date should be rejected");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("due"),
            "error should mention the failing field, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_add_task_rfc3339_datetime_normalized_to_date() {
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("RFC3339 task")
            .with_scheduled("2026-05-01T08:00:00Z")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(
            result["scheduled"], "2026-05-01",
            "RFC 3339 datetime should be truncated to the date portion"
        );
    }

    #[tokio::test]
    async fn test_add_task_empty_due_is_rejected() {
        // On AddTask, empty string has no "clear" semantics (the task is
        // new) — it must be rejected so callers can't paper over mistakes.
        let (_temp, ctx) = setup().await;

        let result = AddTask::new("Empty due task")
            .with_due("")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            result.is_err(),
            "empty string due should be rejected on AddTask"
        );
    }

    #[tokio::test]
    async fn test_add_task_does_not_accept_system_date_params() {
        // The AddTask struct does not declare created/updated/started/completed.
        // Feeding them through JSON deserialization must not write those fields —
        // under any name, anywhere in the stored entity.
        let (_temp, ctx) = setup().await;

        // Use a distinctive sentinel date so the search is unambiguous.
        let sentinel = "1999-01-01";
        let cmd_json = serde_json::json!({
            "title": "Sneaky task",
            "created": sentinel,
            "updated": sentinel,
            "started": sentinel,
            "completed": sentinel,
        });
        let cmd: AddTask = serde_json::from_value(cmd_json).unwrap();
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        // None of the four system date fields in the enriched output may equal
        // the sentinel — that catches the obvious case.
        for field in ["created", "updated", "started", "completed"] {
            if let Some(value) = result[field].as_str() {
                assert_ne!(
                    value, sentinel,
                    "{field} must not be settable via AddTask params"
                );
            }
        }

        // Stronger check: read the raw stored entity and scan every field
        // value for the sentinel. A misrouted system date could land under
        // any field name, so searching the whole entity catches the subtler
        // case the per-field assertions miss.
        let id = result["id"].as_str().unwrap();
        let ectx = ctx.entity_context().await.unwrap();
        let stored = ectx.read("task", id).await.unwrap();
        for (field_name, value) in &stored.fields {
            let serialized = value.to_string();
            assert!(
                !serialized.contains(sentinel),
                "system date sentinel {sentinel:?} must not appear in any \
                 stored entity field, but was found in {field_name:?} = {serialized}"
            );
        }
    }
}
