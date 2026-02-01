//! ListColumns command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// List all columns
#[operation(verb = "list", noun = "columns", description = "List all columns ordered by position")]
#[derive(Debug, Default, Deserialize)]
pub struct ListColumns;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListColumns {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let mut columns = board.columns.clone();
        columns.sort_by_key(|c| c.order);

        Ok(serde_json::json!({
            "columns": columns,
            "count": columns.len()
        }))
    }
}
