//! AddColumn command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{Column, ColumnId};
use serde::{Deserialize, Serialize};
use serde_json::Value;
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

        let result = async {
            // Check for duplicate ID
            if ctx.column_exists(&self.id).await {
                return Err(KanbanError::duplicate_id("column", self.id.to_string()));
            }

            // Determine order
            let order = self.order.unwrap_or_else(|| {
                // Synchronous fallback: we'll compute after reading all columns
                0 // placeholder, overridden below
            });

            let order = if self.order.is_some() {
                order
            } else {
                let columns = ctx.read_all_columns().await?;
                columns
                    .iter()
                    .map(|c| c.order)
                    .max()
                    .map(|o| o + 1)
                    .unwrap_or(0)
            };

            let column = Column {
                id: self.id.clone(),
                name: self.name.clone(),
                order,
            };

            ctx.write_column(&column).await?;

            let mut result = serde_json::to_value(&column)?;
            result["id"] = serde_json::json!(&column.id);
            Ok(result)
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
