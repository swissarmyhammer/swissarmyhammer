//! DeleteColumn command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ColumnId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete a column (fails if it has tasks)
#[operation(verb = "delete", noun = "column", description = "Delete an empty column")]
#[derive(Debug, Deserialize)]
pub struct DeleteColumn {
    /// The column ID to delete
    pub id: ColumnId,
}

impl DeleteColumn {
    pub fn new(id: impl Into<ColumnId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteColumn {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check column exists
        if board.find_column(&self.id).is_none() {
            return Err(KanbanError::ColumnNotFound {
                id: self.id.to_string(),
            });
        }

        // Check for tasks in this column
        let tasks = ctx.read_all_tasks().await?;
        let task_count = tasks
            .iter()
            .filter(|t| t.position.column == self.id)
            .count();

        if task_count > 0 {
            return Err(KanbanError::ColumnNotEmpty {
                id: self.id.to_string(),
                count: task_count,
            });
        }

        board.columns.retain(|c| c.id != self.id);
        ctx.write_board(&board).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string()
        }))
    }
}
