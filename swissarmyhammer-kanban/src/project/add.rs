//! AddProject command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new project to the board
#[operation(
    verb = "add",
    noun = "project",
    description = "Add a new project to the board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddProject {
    /// The project ID (slug)
    pub id: ProjectId,
    /// The project display name
    pub name: String,
    /// Optional project description
    pub description: Option<String>,
    /// Optional display color (6-char hex without #)
    pub color: Option<String>,
}

impl AddProject {
    /// Create a new AddProject command
    pub fn new(id: impl Into<ProjectId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            description: None,
            color: None,
        }
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the display color
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddProject {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Check for duplicate ID
            if ectx.read("project", self.id.as_str()).await.is_ok() {
                return Err(KanbanError::duplicate_id("project", self.id.to_string()));
            }

            let mut entity = Entity::new("project", self.id.as_str());
            entity.set("name", json!(self.name));

            if let Some(ref description) = self.description {
                entity.set("description", json!(description));
            }
            if let Some(ref color) = self.color {
                entity.set("color", json!(color));
            }

            ectx.write(&entity).await?;

            Ok(project_entity_to_json(&entity))
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
}

/// Convert a project Entity to the API JSON format
pub(crate) fn project_entity_to_json(entity: &Entity) -> Value {
    json!({
        "id": entity.id,
        "name": entity.get_str("name").unwrap_or(""),
        "description": entity.get_str("description").unwrap_or(""),
        "color": entity.get_str("color").unwrap_or(""),
    })
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
    async fn test_add_project() {
        let (_temp, ctx) = setup().await;

        let result = AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend");
    }

    #[tokio::test]
    async fn test_add_project_with_all_fields() {
        let (_temp, ctx) = setup().await;

        let result = AddProject::new("frontend", "Frontend")
            .with_description("The frontend project")
            .with_color("ff0000")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "frontend");
        assert_eq!(result["name"], "Frontend");
        assert_eq!(result["description"], "The frontend project");
        assert_eq!(result["color"], "ff0000");
    }

    #[tokio::test]
    async fn test_add_project_duplicate() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = AddProject::new("backend", "Duplicate")
            .execute(&ctx)
            .await
            .into_result();
        assert!(matches!(result, Err(KanbanError::DuplicateId { .. })));
    }

    /// Regression: the JSON returned by `AddProject` must NOT include an `order` key.
    /// The `order` field was removed from the project entity schema — see the
    /// `task-card-fields` kanban project.
    #[tokio::test]
    async fn test_add_project_result_has_no_order_key() {
        let (_temp, ctx) = setup().await;

        let result = AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let obj = result.as_object().expect("result should be a JSON object");
        assert!(
            !obj.contains_key("order"),
            "AddProject result should not contain an `order` key, got: {:?}",
            obj.keys().collect::<Vec<_>>()
        );
    }
}
