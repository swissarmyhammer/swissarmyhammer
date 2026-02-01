//! UpdateColumn command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Update a column
#[operation(verb = "update", noun = "column", description = "Update a column's name or order")]
#[derive(Debug, Deserialize)]
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
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        let column = board
            .columns
            .iter_mut()
            .find(|c| c.id == self.id)
            .ok_or_else(|| KanbanError::ColumnNotFound {
                id: self.id.to_string(),
            })?;

        if let Some(name) = &self.name {
            column.name = name.clone();
        }
        if let Some(order) = self.order {
            column.order = order;
        }

        let result = serde_json::to_value(&*column)?;
        ctx.write_board(&board).await?;

        Ok(result)
    }
}
