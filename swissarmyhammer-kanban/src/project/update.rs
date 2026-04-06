//! UpdateProject command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::project::add::project_entity_to_json;
use crate::types::ProjectId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a project
#[operation(
    verb = "update",
    noun = "project",
    description = "Update a project's name, description, color, or order"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateProject {
    /// The project ID to update
    pub id: ProjectId,
    /// New project name
    pub name: Option<String>,
    /// New project description
    pub description: Option<String>,
    /// New display color
    pub color: Option<String>,
    /// New position in project order
    pub order: Option<usize>,
}

impl UpdateProject {
    /// Create a new UpdateProject command
    pub fn new(id: impl Into<ProjectId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            description: None,
            color: None,
            order: None,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Set the description
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Set the color
    pub fn with_color(mut self, color: impl Into<String>) -> Self {
        self.color = Some(color.into());
        self
    }

    /// Set the order
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = Some(order);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateProject {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx
                .read("project", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            if let Some(name) = &self.name {
                entity.set("name", json!(name));
            }
            if let Some(description) = &self.description {
                entity.set("description", json!(description));
            }
            if let Some(color) = &self.color {
                entity.set("color", json!(color));
            }
            if let Some(order) = self.order {
                entity.set("order", json!(order));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::error::KanbanError;
    use crate::project::add::AddProject;
    use crate::project::get::GetProject;
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
    async fn test_update_project_name() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateProject::new("backend")
            .with_name("Backend Service")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend Service");

        // Verify via get
        let fetched = GetProject::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(fetched["name"], "Backend Service");
    }

    #[tokio::test]
    async fn test_update_project_all_fields() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateProject::new("backend")
            .with_name("New Name")
            .with_description("New description")
            .with_color("aabbcc")
            .with_order(42)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "New Name");
        assert_eq!(result["description"], "New description");
        assert_eq!(result["color"], "aabbcc");
        assert_eq!(result["order"], 42);
    }

    #[tokio::test]
    async fn test_update_project_not_found() {
        let (_temp, ctx) = setup().await;

        let result = UpdateProject::new("nonexistent")
            .with_name("Whatever")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ProjectNotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_project_no_changes() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        // Updating with no fields set should succeed and return current values
        let result = UpdateProject::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend");
    }
}
