//! ListPerspectives command

use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::perspective::add::perspective_to_json;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// List all perspectives on the board.
#[operation(
    verb = "list",
    noun = "perspectives",
    description = "List all perspectives on the board"
)]
#[derive(Debug, Default, Deserialize)]
pub struct ListPerspectives {}

impl ListPerspectives {
    /// Create a new ListPerspectives query.
    pub fn new() -> Self {
        Self {}
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListPerspectives {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let pctx = ctx.perspective_context().await?;
            let pctx = pctx.read().await;
            let all = pctx.all();
            let perspectives_json: Vec<Value> = all.iter().map(perspective_to_json).collect();
            let count = perspectives_json.len();

            Ok(serde_json::json!({
                "perspectives": perspectives_json,
                "count": count,
            }))
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
