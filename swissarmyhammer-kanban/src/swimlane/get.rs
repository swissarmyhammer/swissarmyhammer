//! GetSwimlane command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::swimlane::swimlane_entity_to_json;
use crate::types::SwimlaneId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a swimlane by ID
#[operation(verb = "get", noun = "swimlane", description = "Get a swimlane by ID")]
#[derive(Debug, Deserialize)]
pub struct GetSwimlane {
    /// The swimlane ID to retrieve
    pub id: SwimlaneId,
}

impl GetSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx
                .read("swimlane", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            Ok(swimlane_entity_to_json(&entity))
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
    use crate::error::KanbanError;
    use crate::swimlane::AddSwimlane;
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
    async fn test_get_swimlane() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetSwimlane::new("backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend");
    }

    #[tokio::test]
    async fn test_get_swimlane_not_found() {
        let (_temp, ctx) = setup().await;

        let result = GetSwimlane::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::SwimlaneNotFound { .. })));
    }
}
