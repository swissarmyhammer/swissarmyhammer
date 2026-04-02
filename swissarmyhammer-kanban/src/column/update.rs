//! UpdateColumn command

use crate::column::add::column_entity_to_json;
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{
    async_trait, operation, Execute, ExecutionResult, LogEntry, Operation,
};

/// Update a column
#[operation(
    verb = "update",
    noun = "column",
    description = "Update a column's name or order"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateColumn {
    /// The column ID to update
    pub id: ColumnId,
    /// New column name
    pub name: Option<String>,
    /// New position in column order
    pub order: Option<usize>,
}

impl UpdateColumn {
    pub fn new(id: impl Into<ColumnId>) -> Self {
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
impl Execute<KanbanContext, KanbanError> for UpdateColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let start = std::time::Instant::now();
        let input = serde_json::to_value(self).unwrap();

        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut entity = ectx
                .read("column", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;

            if let Some(name) = &self.name {
                entity.set("name", json!(name));
            }
            if let Some(order) = self.order {
                entity.set("order", json!(order));
            }

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::InitBoard;
    use crate::column::add::AddColumn;
    use crate::column::get::GetColumn;
    use crate::error::KanbanError;
    use tempfile::TempDir;

    /// Create a temporary kanban context with a board initialized.
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
    async fn test_update_column_name() {
        let (_temp, ctx) = setup().await;

        let result = UpdateColumn::new("todo")
            .with_name("Backlog")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "todo");
        assert_eq!(result["name"], "Backlog");

        // Verify via get
        let fetched = GetColumn::new("todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(fetched["name"], "Backlog");
    }

    #[tokio::test]
    async fn test_update_column_order() {
        let (_temp, ctx) = setup().await;

        let result = UpdateColumn::new("todo")
            .with_order(42)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "todo");
        assert_eq!(result["order"], 42);

        // Verify via get
        let fetched = GetColumn::new("todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(fetched["order"], 42);
    }

    #[tokio::test]
    async fn test_update_column_name_and_order() {
        let (_temp, ctx) = setup().await;

        AddColumn::new("review", "Review")
            .with_order(5)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = UpdateColumn::new("review")
            .with_name("Code Review")
            .with_order(10)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["name"], "Code Review");
        assert_eq!(result["order"], 10);
    }

    #[tokio::test]
    async fn test_update_column_not_found() {
        let (_temp, ctx) = setup().await;

        let result = UpdateColumn::new("nonexistent")
            .with_name("Whatever")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ColumnNotFound { .. })));
    }

    #[tokio::test]
    async fn test_update_column_no_changes() {
        let (_temp, ctx) = setup().await;

        // Updating with no fields set should succeed and return current values
        let result = UpdateColumn::new("todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "todo");
    }
}
