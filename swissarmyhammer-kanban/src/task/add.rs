//! AddTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, Ordinal, Position, TagId, Task, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Add a new task to the board
#[operation(verb = "add", noun = "task", description = "Create a new task on the board")]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddTask {
    /// The task title (required)
    pub title: String,
    /// Detailed task description
    pub description: Option<String>,
    /// Initial position (column, swimlane, ordinal)
    pub position: Option<Position>,
    /// Tags to apply
    #[serde(default)]
    pub tags: Vec<TagId>,
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
            position: None,
            tags: Vec::new(),
            assignees: Vec::new(),
            depends_on: Vec::new(),
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the position
    pub fn with_position(mut self, position: Position) -> Self {
        self.position = Some(position);
        self
    }

    /// Set the tags
    pub fn with_tags(mut self, tags: Vec<TagId>) -> Self {
        self.tags = tags;
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
            let board = ctx.read_board().await?;

            // Determine position
            let position = match &self.position {
                Some(pos) => pos.clone(),
                None => {
                    // Default to first column, no swimlane, at the end
                    let column = board
                        .first_column()
                        .expect("board must have at least one column");

                    // Find the last ordinal in that column
                    let task_ids = ctx.list_task_ids().await?;
                    let mut last_ordinal: Option<Ordinal> = None;

                    for id in &task_ids {
                        let t = ctx.read_task(id).await?;
                        if t.position.column == column.id && t.position.swimlane.is_none() {
                            last_ordinal = Some(match last_ordinal {
                                None => t.position.ordinal.clone(),
                                Some(ref o) if t.position.ordinal > *o => t.position.ordinal.clone(),
                                Some(o) => o,
                            });
                        }
                    }

                    Position {
                        column: column.id.clone(),
                        swimlane: None,
                        ordinal: match last_ordinal {
                            Some(last) => Ordinal::after(&last),
                            None => Ordinal::first(),
                        },
                    }
                }
            };

            let task = Task {
                id: TaskId::new(),
                title: self.title.clone(),
                description: self.description.clone().unwrap_or_default(),
                tags: self.tags.clone(),
                position,
                depends_on: self.depends_on.clone(),
                assignees: self.assignees.clone(),
                comments: Vec::new(),
                subtasks: Vec::new(),
                attachments: Vec::new(),
            };

            ctx.write_task(&task).await?;
            Ok(serde_json::to_value(&task)?)
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

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

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
        let result1 = AddTask::new("Task 1").execute(&ctx).await.into_result().unwrap();
        let ordinal1 = result1["position"]["ordinal"].as_str().unwrap();

        // Add second task
        let result2 = AddTask::new("Task 2").execute(&ctx).await.into_result().unwrap();
        let ordinal2 = result2["position"]["ordinal"].as_str().unwrap();

        // Second should be after first
        assert!(ordinal2 > ordinal1);
    }
}
