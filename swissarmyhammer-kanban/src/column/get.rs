//! GetColumn command

use crate::column::add::column_entity_to_json;
use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::ColumnId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a column by ID
#[operation(verb = "get", noun = "column", description = "Get a column by ID")]
#[derive(Debug, Deserialize)]
pub struct GetColumn {
    /// The column ID to retrieve
    pub id: ColumnId,
}

impl GetColumn {
    /// Create a new GetColumn command for the given column ID.
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let ectx = ctx.entity_context().await?;
            let entity = ectx
                .read("column", self.id.as_str())
                .await
                .map_err(KanbanError::from_entity_error)?;
            Ok(column_entity_to_json(&entity))
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
    use crate::column::add::AddColumn;
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
    async fn test_get_column_existing() {
        let (_temp, ctx) = setup().await;

        // "todo" is created by InitBoard
        let result = GetColumn::new("todo")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "todo");
        assert!(result["name"].is_string());
    }

    #[tokio::test]
    async fn test_get_column_returns_name_and_order() {
        let (_temp, ctx) = setup().await;

        AddColumn::new("review", "Review")
            .with_order(99)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let result = GetColumn::new("review")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], "review");
        assert_eq!(result["name"], "Review");
        assert_eq!(result["order"], 99);
    }

    #[tokio::test]
    async fn test_get_column_not_found() {
        let (_temp, ctx) = setup().await;

        let result = GetColumn::new("nonexistent")
            .execute(&ctx)
            .await
            .into_result();

        assert!(matches!(result, Err(KanbanError::ColumnNotFound { .. })));
    }
}
