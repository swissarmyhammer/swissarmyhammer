//! AddActor command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Actor, ActorId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Actor type for creation
#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActorType {
    Human,
    Agent,
}

/// Add a new actor (person or agent) to the board
#[operation(verb = "add", noun = "actor", description = "Add a new actor (person or agent) to the board")]
#[derive(Debug, Deserialize)]
pub struct AddActor {
    /// The actor ID (slug)
    pub id: ActorId,
    /// The actor display name
    pub name: String,
    /// The actor type (human or agent)
    #[serde(rename = "type")]
    pub actor_type: ActorType,
}

impl AddActor {
    /// Create a new AddActor command for a human
    pub fn human(id: impl Into<ActorId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            actor_type: ActorType::Human,
        }
    }

    /// Create a new AddActor command for an agent
    pub fn agent(id: impl Into<ActorId>, name: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            actor_type: ActorType::Agent,
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddActor {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check for duplicate ID
        if board.actors.iter().any(|a| a.id() == &self.id) {
            return Err(KanbanError::duplicate_id("actor", self.id.to_string()));
        }

        let actor = match self.actor_type {
            ActorType::Human => Actor::Human {
                id: self.id.clone(),
                name: self.name.clone(),
            },
            ActorType::Agent => Actor::Agent {
                id: self.id.clone(),
                name: self.name.clone(),
            },
        };

        board.actors.push(actor.clone());
        ctx.write_board(&board).await?;

        Ok(serde_json::to_value(&actor)?)
    }
}
