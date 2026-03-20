//! UpdateTask command

use crate::auto_color;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::tag::tag_name_exists_entity;
use crate::task_helpers::task_entity_to_json;
use crate::types::{ActorId, SwimlaneId, TaskId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update an existing task.
///
/// Tags are derived from `#tag` patterns in the description — edit the
/// description to change tags.
#[operation(verb = "update", noun = "task", description = "Update task properties")]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateTask {
    /// The task ID to update
    pub id: TaskId,
    /// New title
    pub title: Option<String>,
    /// New description (may contain #tag patterns)
    pub description: Option<String>,
    /// New swimlane (None = don't change, Some(None) = clear, Some(Some(x)) = set)
    pub swimlane: Option<Option<SwimlaneId>>,
    /// Replace all assignees
    pub assignees: Option<Vec<ActorId>>,
    /// Replace all dependencies
    pub depends_on: Option<Vec<TaskId>>,
    /// Replace all attachment IDs (array of entity ID strings)
    pub attachments: Option<Value>,
}

impl UpdateTask {
    /// Create a new UpdateTask command
    pub fn new(id: impl Into<TaskId>) -> Self {
        Self {
            id: id.into(),
            title: None,
            description: None,
            swimlane: None,
            assignees: None,
            depends_on: None,
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
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateTask {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx
                .read("task", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            // Apply updates
            if let Some(title) = &self.title {
                entity.set("title", serde_json::json!(title));
            }
            if let Some(desc) = &self.description {
                entity.set("body", serde_json::json!(desc));
            }
            if let Some(swimlane) = &self.swimlane {
                match swimlane {
                    Some(s) => entity.set("position_swimlane", serde_json::json!(s)),
                    None => {
                        entity.remove("position_swimlane");
                    }
                }
            }
            if let Some(assignees) = &self.assignees {
                entity.set("assignees", serde_json::to_value(assignees)?);
            }
            if let Some(deps) = &self.depends_on {
                entity.set("depends_on", serde_json::to_value(deps)?);
            }
            if let Some(attachments) = &self.attachments {
                entity.set("attachments", attachments.clone());
            }

            ectx.write(&entity).await?;

            // Auto-create Tag entities for any new #tag patterns in description.
            // Parse directly from body — the computed `tags` field may be stale.
            let body = entity.get_str("body").unwrap_or("");
            let tags = crate::tag_parser::parse_tags(body);
            for tag_name in &tags {
                if !tag_name_exists_entity(ectx, tag_name).await {
                    let color = auto_color::auto_color(tag_name).to_string();
                    let tag_id = ulid::Ulid::new().to_string();
                    let mut tag_entity = Entity::new("tag", tag_id.as_str());
                    tag_entity.set("tag_name", serde_json::json!(tag_name));
                    tag_entity.set("color", serde_json::json!(color));
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
    use crate::task::AddTask;
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
    async fn test_update_task_title() {
        let (_temp, ctx) = setup().await;

        let add_result = AddTask::new("Original")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
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

        let add_result = AddTask::new("Task")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
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
