//! AddTask command

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag::tag_name_exists_entity;
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
    /// Initial swimlane
    pub swimlane: Option<String>,
    /// Initial ordinal (if None, appended at end)
    pub ordinal: Option<String>,
    /// Assignees for this task
    #[serde(default)]
    pub assignees: Vec<ActorId>,
    /// Task IDs this task depends on
    #[serde(default)]
    pub depends_on: Vec<TaskId>,
}

impl AddTask {
    /// Create a new AddTask command with just a title
    pub fn new(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            description: None,
            column: None,
            swimlane: None,
            ordinal: None,
            assignees: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the position (column, swimlane, ordinal) for backward compat
    pub fn with_position(mut self, position: crate::types::Position) -> Self {
        self.column = Some(position.column.to_string());
        self.swimlane = position.swimlane.map(|s| s.to_string());
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
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Determine column
            let column = match &self.column {
                Some(col) => col.clone(),
                None => {
                    let columns = ectx.list("column").await?;
                    let first = columns
                        .iter()
                        .min_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                        .expect("board must have at least one column");
                    first.id.to_string()
                }
            };

            // Calculate ordinal at end of target column/swimlane
            let ordinal = match &self.ordinal {
                Some(ord) => ord.clone(),
                None => {
                    let tasks = ectx.list("task").await?;
                    let mut last_ordinal: Option<Ordinal> = None;

                    for t in &tasks {
                        let t_col = t.get_str("position_column").unwrap_or("");
                        let t_swim = t.get_str("position_swimlane");
                        if t_col == column && t_swim == self.swimlane.as_deref() {
                            let ord_str = t.get_str("position_ordinal").unwrap_or(Ordinal::DEFAULT_STR);
                            let ord = Ordinal::from_string(ord_str);
                            last_ordinal = Some(match last_ordinal {
                                None => ord,
                                Some(ref o) if ord > *o => ord,
                                Some(o) => o,
                            });
                        }
                    }

                    match last_ordinal {
                        Some(last) => Ordinal::after(&last).as_str().to_string(),
                        None => Ordinal::first().as_str().to_string(),
                    }
                }
            };

            // Create entity
            let task_id = TaskId::new();
            let mut entity = Entity::new("task", task_id.as_str());
            entity.set("title", json!(self.title));
            entity.set("body", json!(self.description.clone().unwrap_or_default()));
            entity.set("position_column", json!(column));
            if let Some(ref swimlane) = self.swimlane {
                entity.set("position_swimlane", json!(swimlane));
            }
            entity.set("position_ordinal", json!(ordinal));

            if !self.assignees.is_empty() {
                entity.set("assignees", serde_json::to_value(&self.assignees)?);
            }
            if !self.depends_on.is_empty() {
                entity.set("depends_on", serde_json::to_value(&self.depends_on)?);
            }

            ectx.write(&entity).await?;

            // Auto-create Tag entities for any #tag patterns in description
            let tags = crate::task_helpers::task_tags(&entity);
            for tag_name in &tags {
                if !tag_name_exists_entity(ectx, tag_name).await {
                    let color = auto_color::auto_color(tag_name).to_string();
                    let tag_id = ulid::Ulid::new().to_string();
                    let mut tag_entity = Entity::new("tag", tag_id.as_str());
                    tag_entity.set("tag_name", json!(tag_name));
                    tag_entity.set("color", json!(color));
                    ectx.write(&tag_entity).await?;
                }
            }

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
