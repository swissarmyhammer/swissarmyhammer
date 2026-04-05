//! UpdateSwimlane command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::swimlane::swimlane_entity_to_json;
use crate::types::SwimlaneId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a swimlane
#[operation(
    verb = "update",
    noun = "swimlane",
    description = "Update a swimlane's name or order"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateSwimlane {
    /// The swimlane ID to update
    pub id: SwimlaneId,
    /// New swimlane name
    pub name: Option<String>,
    /// New position in swimlane order
    pub order: Option<usize>,
}

impl UpdateSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
        Self {
            id: id.into(),
            name: None,
            order: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_order(mut self, order: usize) -> Self {
        self.order = Some(order);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx
                .read("swimlane", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            if let Some(name) = &self.name {
                entity.set("name", json!(name));
            }
            if let Some(order) = self.order {
                entity.set("order", json!(order));
            }

            ectx.write(&entity).await?;
            Ok(swimlane_entity_to_json(&entity))
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
    async fn test_update_swimlane_name() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateSwimlane::new("backend")
            .with_name("Backend Services")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend Services");
    }

    #[tokio::test]
    async fn test_update_swimlane_order() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .with_order(0)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateSwimlane::new("backend")
            .with_order(5)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["order"], 5);
    }

    #[tokio::test]
    async fn test_update_swimlane_not_found() {
        let (_temp, ctx) = setup().await;

        let result = UpdateSwimlane::new("nonexistent")
            .with_name("New Name")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::SwimlaneNotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_swimlane_name_and_order() {
        let (_temp, ctx) = setup().await;

        AddSwimlane::new("backend", "Backend")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateSwimlane::new("backend")
            .with_name("Backend Services")
            .with_order(3)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "backend");
        assert_eq!(result["name"], "Backend Services");
        assert_eq!(result["order"], 3);
    }
}
