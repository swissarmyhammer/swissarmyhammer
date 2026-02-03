//! UpdateTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, Attachment, SwimlaneId, Subtask, TagId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult, LogEntry, Operation};

/// Update an existing task
#[operation(verb = "update", noun = "task", description = "Update task properties")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTask {
    /// The task ID to update
    pub id: TaskId,
    /// New title
    pub title: Option<String>,
    /// New description
    pub description: Option<String>,
    /// New swimlane (None = don't change, Some(None) = clear, Some(Some(x)) = set)
    pub swimlane: Option<Option<SwimlaneId>>,
    /// Replace all tags
    pub tags: Option<Vec<TagId>>,
    /// Replace all assignees
    pub assignees: Option<Vec<ActorId>>,
    /// Replace all dependencies
    pub depends_on: Option<Vec<TaskId>>,
    /// Replace all subtasks
    pub subtasks: Option<Vec<Subtask>>,
    /// Replace all attachments
    pub attachments: Option<Vec<Attachment>>,
}

impl UpdateTask {
    /// Create a new UpdateTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            description: None,
            swimlane: None,
            tags: None,
            assignees: None,
            depends_on: None,
            subtasks: None,
            attachments: None,
        }
    }

    /// Set the title
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the swimlane
    pub fn with_swimlane(mut self, swimlane: Option<SwimlaneId>) -> Self {
        self.swimlane = Some(swimlane);
        self
    }

    /// Set the tags (replaces all existing tags)
    pub fn with_tags(mut self, tags: Vec<TagId>) -> Self {
        self.tags = Some(tags);
        self
    }

    /// Set the assignees (replaces all existing assignees)
    pub fn with_assignees(mut self, assignees: Vec<ActorId>) -> Self {
        self.assignees = Some(assignees);
        self
    }

    /// Set the dependencies (replaces all existing dependencies)
    pub fn with_depends_on(mut self, deps: Vec<TaskId>) -> Self {
        self.depends_on = Some(deps);
        self
    }

    /// Set the subtasks (replaces all existing subtasks)
    pub fn with_subtasks(mut self, subtasks: Vec<Subtask>) -> Self {
        self.subtasks = Some(subtasks);
        self
    }

    /// Set the attachments (replaces all existing attachments)
    pub fn with_attachments(mut self, attachments: Vec<Attachment>) -> Self {
        self.attachments = Some(attachments);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let mut task = ctx.read_task(&self.id).await?;

            // Apply updates
            if let Some(title) = &self.title {
                task.title = title.clone();
            }
            if let Some(desc) = &self.description {
                task.description = desc.clone();
            }
            if let Some(swimlane) = &self.swimlane {
                task.position.swimlane = swimlane.clone();
            }
            if let Some(tags) = &self.tags {
                task.tags = tags.clone();
            }
            if let Some(assignees) = &self.assignees {
                task.assignees = assignees.clone();
            }
            if let Some(deps) = &self.depends_on {
                task.depends_on = deps.clone();
            }
            if let Some(subtasks) = &self.subtasks {
                task.subtasks = subtasks.clone();
            }
            if let Some(attachments) = &self.attachments {
                task.attachments = attachments.clone();
            }

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
    use crate::task::AddTask;
    use tempfile::TempDir;

    async fn setup() -> (TempDir, KanbanContext) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test").execute(&ctx).await.into_result().unwrap();

        (temp, ctx)
    }

    #[tokio::test]
    async fn test_update_task_title() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Original").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UpdateTask::new(task_id)
            .with_title("Updated")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["title"], "Updated");
    }

    #[tokio::test]
    async fn test_update_task_description() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Task").execute(&ctx).await.into_result().unwrap();
        let task_id = add_result["id"].as_str().unwrap();

        let result = UpdateTask::new(task_id)
            .with_description("New description")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["description"], "New description");
    }
}
