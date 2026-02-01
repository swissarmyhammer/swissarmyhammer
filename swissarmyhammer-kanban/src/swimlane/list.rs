//! ListSwimlanes command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// List all swimlanes
#[operation(verb = "list", noun = "swimlanes", description = "List all swimlanes ordered by position")]
#[derive(Debug, Default, Deserialize)]
pub struct ListSwimlanes;

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListSwimlanes {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let mut swimlanes = board.swimlanes.clone();
        swimlanes.sort_by_key(|s| s.order);

        Ok(serde_json::json!({
            "swimlanes": swimlanes,
            "count": swimlanes.len()
        }))
    }
}
