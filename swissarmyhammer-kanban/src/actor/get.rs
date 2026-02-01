//! GetActor command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ActorId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Get an actor by ID
#[operation(verb = "get", noun = "actor", description = "Get an actor by ID")]
#[derive(Debug, Deserialize)]
pub struct GetActor {
    /// The actor ID to retrieve
    pub id: ActorId,
}

impl GetActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetActor {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let actor = board
            .actors
            .iter()
            .find(|a| a.id() == &self.id)
            .ok_or_else(|| KanbanError::ActorNotFound {
                id: self.id.to_string(),
            })?;

        Ok(serde_json::to_value(actor)?)
    }
}
