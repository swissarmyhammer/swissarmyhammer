//! GetColumn command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Get a column by ID
#[operation(verb = "get", noun = "column", description = "Get a column by ID")]
#[derive(Debug, Deserialize)]
pub struct GetColumn {
    /// The column ID to retrieve
    pub id: ColumnId,
}

impl GetColumn {
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetColumn {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let column = board.find_column(&self.id).ok_or_else(|| KanbanError::ColumnNotFound {
            id: self.id.to_string(),
        })?;

        Ok(serde_json::to_value(column)?)
    }
}
