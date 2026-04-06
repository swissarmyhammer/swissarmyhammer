//! GetProject command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::project::add::project_entity_to_json;
use crate::types::ProjectId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a project by ID
#[operation(verb = "get", noun = "project", description = "Get a project by ID")]
#[derive(Debug, Deserialize)]
pub struct GetProject {
    /// The project ID to retrieve
    pub id: ProjectId,
}

impl GetProject {
    /// Create a new GetProject command for the given project ID.
    pub fn new(id: impl Into<ProjectId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetProject {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx
                .read("project", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            Ok(project_entity_to_json(&entity))
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
    use crate::project::add::AddProject;
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
    async fn test_get_project_existing() {
        let (_temp, ctx) = setup().await;

        AddProject::new("backend", "Backend")
            .with_description("The backend")
            .with_color("00ff00")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetProject::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend");
        assert_eq!(result["description"], "The backend");
        assert_eq!(result["color"], "00ff00");
    }

    #[tokio::test]
    async fn test_get_project_not_found() {
        let (_temp, ctx) = setup().await;

        let result = GetProject::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ProjectNotFound { .. })));
    }
}
