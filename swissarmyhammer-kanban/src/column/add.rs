//! AddColumn command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Column, ColumnId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Add a new column to the board
#[operation(verb = "add", noun = "column", description = "Add a new column to the board")]
#[derive(Debug, Deserialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check for duplicate ID
        if board.find_column(&self.id).is_some() {
            return Err(KanbanError::duplicate_id("column", self.id.to_string()));
        }

        // Determine order
        let order = self.order.unwrap_or_else(|| {
            board
                .columns
                .iter()
                .map(|c| c.order)
                .max()
                .map(|o| o + 1)
                .unwrap_or(0)
        });

        let column = Column {
            id: self.id.clone(),
            name: self.name.clone(),
            order,
        };

        board.columns.push(column.clone());
        ctx.write_board(&board).await?;

        Ok(serde_json::to_value(&column)?)
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
        InitBoard::new("Test").execute(&ctx).await.unwrap();
        (temp, ctx)
    }

    #[tokio::test]
    async fn test_add_column() {
        let (_temp, ctx) = setup().await;

        let result = AddColumn::new("blocked", "Blocked")
            .execute(&ctx)
            .await
            .unwrap();

        assert_eq!(result["id"], "blocked");
        assert_eq!(result["name"], "Blocked");
    }

    #[tokio::test]
    async fn test_add_column_duplicate() {
        let (_temp, ctx) = setup().await;

        let result = AddColumn::new("todo", "Duplicate").execute(&ctx).await;
        assert!(matches!(result, Err(KanbanError::DuplicateId { .. })));
    }
}
