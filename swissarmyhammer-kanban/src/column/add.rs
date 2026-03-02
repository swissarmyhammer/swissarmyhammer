//! AddColumn command

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Add a new column to the board
#[operation(
    verb = "add",
    noun = "column",
    description = "Add a new column to the board"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddColumn {
    /// The column ID (slug)
    pub id: ColumnId,
    /// The column display name
    pub name: String,
    /// Optional position in column order
    pub order: Option<usize>,
}

impl AddColumn {
    /// Create a new AddColumn command
    pub fn new(id: impl Into<ColumnId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            order: None,
        }
    }

    /// Set the order (position in column list)
    pub fn with_order(mut self, order: usize) -> Self {
        self.order = Some(order);
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;

            // Check for duplicate ID
            if ectx.read("column", self.id.as_str()).await.is_ok() {
                return Err(KanbanError::duplicate_id("column", self.id.to_string()));
            }

            // Determine order
            let order = if let Some(order) = self.order {
                order
            } else {
                let columns = ectx.list("column").await?;
                columns
                    .iter()
                    .filter_map(|c| c.get("order").and_then(|v| v.as_u64()))
                    .max()
                    .map(|o| o as usize + 1)
                    .unwrap_or(0)
            };

            let mut entity = Entity::new("column", self.id.as_str());
            entity.set("name", json!(self.name));
            entity.set("order", json!(order));

            ectx.write(&entity).await?;

            Ok(column_entity_to_json(&entity))
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

/// Convert a column Entity to the API JSON format
pub(crate) fn column_entity_to_json(entity: &Entity) -> Value {
    json!({
        "id": entity.id,
        "name": entity.get_str("name").unwrap_or(""),
        "order": entity.get("order").and_then(|v| v.as_u64()).unwrap_or(0),
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
    async fn test_add_column() {
        let (_temp, ctx) = setup().await;

        let result = AddColumn::new("blocked", "Blocked")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "blocked");
        assert_eq!(result["name"], "Blocked");
    }

    #[tokio::test]
    async fn test_add_column_duplicate() {
        let (_temp, ctx) = setup().await;

        let result = AddColumn::new("todo", "Duplicate")
            .execute(&ctx)
            .await
            .into_result();
        assert!(matches!(result, Err(KanbanError::DuplicateId { .. })));
    }
}
