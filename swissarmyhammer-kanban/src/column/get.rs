//! GetColumn command


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
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetColumn {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let board = ctx.read_board().await?;

            let column = board.find_column(&self.id).ok_or_else(|| KanbanError::ColumnNotFound {
                id: self.id.to_string(),
            })?;

            Ok(serde_json::to_value(column)?)
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
