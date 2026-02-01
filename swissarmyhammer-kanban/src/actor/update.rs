//! UpdateActor command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Actor, ActorId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Update an actor
#[operation(verb = "update", noun = "actor", description = "Update an actor's name")]
#[derive(Debug, Deserialize)]
pub struct UpdateActor {
    /// The actor ID to update
    pub id: ActorId,
    /// New actor name
    pub name: Option<String>,
}

impl UpdateActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self {
            id: id.into(),
            name: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateActor {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        let actor_idx = board
            .actors
            .iter()
            .position(|a| a.id() == &self.id)
            .ok_or_else(|| KanbanError::ActorNotFound {
                id: self.id.to_string(),
            })?;

        if let Some(name) = &self.name {
            // Update the name while preserving the type
            let old_actor = &board.actors[actor_idx];
            let new_actor = match old_actor {
                Actor::Human { id, .. } => Actor::Human {
                    id: id.clone(),
                    name: name.clone(),
                },
                Actor::Agent { id, .. } => Actor::Agent {
                    id: id.clone(),
                    name: name.clone(),
                },
            };
            board.actors[actor_idx] = new_actor;
        }

        let result = serde_json::to_value(&board.actors[actor_idx])?;
        ctx.write_board(&board).await?;

        Ok(result)
    }
}
