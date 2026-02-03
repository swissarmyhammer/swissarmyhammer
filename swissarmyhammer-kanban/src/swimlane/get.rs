//! GetSwimlane command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::SwimlaneId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a swimlane by ID
#[operation(verb = "get", noun = "swimlane", description = "Get a swimlane by ID")]
#[derive(Debug, Deserialize)]
pub struct GetSwimlane {
    /// The swimlane ID to retrieve
    pub id: SwimlaneId,
}

impl GetSwimlane {
    pub fn new(id: impl Into<SwimlaneId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetSwimlane {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let board = ctx.read_board().await?;

            let swimlane = board.find_swimlane(&self.id).ok_or_else(|| KanbanError::SwimlaneNotFound {
                id: self.id.to_string(),
            })?;

            Ok(serde_json::to_value(swimlane)?)
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
