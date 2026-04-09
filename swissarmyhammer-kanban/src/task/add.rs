//! AddTask command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task::shared::auto_create_body_tags;
use crate::task_helpers::task_entity_to_json;
use crate::types::{ActorId, Ordinal, TaskId};
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

    /// Resolve the target column, falling back to the first board column.
    async fn resolve_column(&self, ectx: &swissarmyhammer_entity::EntityContext) -> Result<String> {
        match &self.column {
            Some(col) => Ok(col.clone()),
            None => {
                let columns = ectx.list("column").await?;
                let first = columns
                    .iter()
                    .min_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                    .ok_or_else(|| KanbanError::parse("board has no columns — cannot add task"))?;
                Ok(first.id.to_string())
            }
        }
    }

    /// Resolve the ordinal, falling back to appending at the end of the column.
    async fn resolve_ordinal(
        &self,
        ectx: &swissarmyhammer_entity::EntityContext,
        column: &str,
    ) -> Result<String> {
        match &self.ordinal {
            Some(ord) => Ok(ord.clone()),
            None => {
                let tasks = ectx.list("task").await?;
                let last_ordinal = tasks
                    .iter()
                    .filter(|t| t.get_str("position_column").unwrap_or("") == column)
                    .filter_map(|t| t.get_str("position_ordinal").map(Ordinal::from_string))
                    .max();
                Ok(match last_ordinal {
                    Some(last) => Ordinal::after(&last).as_str().to_string(),
                    None => Ordinal::first().as_str().to_string(),
                })
            }
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let column = self.resolve_column(&ectx).await?;
            let ordinal = self.resolve_ordinal(&ectx, &column).await?;

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

            ectx.write(&entity).await?;
            auto_create_body_tags(&ectx, &entity).await?;
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
    async fn test_add_task_with_project() {
        let (_temp, ctx) = setup().await;

        // Create a project first
        use crate::project::AddProject;
        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let cmd = AddTask::new("Task with project").with_project("backend");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["title"], "Task with project");
        assert_eq!(result["project"], "backend");
    }

    #[tokio::test]
    async fn test_add_task_without_project_has_empty_project() {
        let (_temp, ctx) = setup().await;

        let cmd = AddTask::new("Task without project");
        let result = cmd.execute(&ctx).await.into_result().unwrap();

        assert_eq!(result["project"], "");
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
}
