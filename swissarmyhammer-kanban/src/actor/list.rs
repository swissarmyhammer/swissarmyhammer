//! ListActors command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::Actor;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// List all actors
#[operation(verb = "list", noun = "actors", description = "List all actors with optional type filter")]
#[derive(Debug, Default, Deserialize)]
pub struct ListActors {
    /// Filter by actor type (human or agent)
    #[serde(rename = "type")]
    pub actor_type: Option<String>,
}

impl ListActors {
    pub fn new() -> Self {
        Self { actor_type: None }
    }

    pub fn humans() -> Self {
        Self {
            actor_type: Some("human".to_string()),
        }
    }

    pub fn agents() -> Self {
        Self {
            actor_type: Some("agent".to_string()),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListActors {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let board = ctx.read_board().await?;

        let actors: Vec<&Actor> = board
            .actors
            .iter()
            .filter(|a| match &self.actor_type {
                None => true,
                Some(t) if t == "human" => matches!(a, Actor::Human { .. }),
                Some(t) if t == "agent" => matches!(a, Actor::Agent { .. }),
                Some(_) => true, // Unknown type, include all
            })
            .collect();

        Ok(serde_json::json!({
            "actors": actors,
            "count": actors.len()
        }))
    }
}
