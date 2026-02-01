//! DeleteActor command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ActorId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete an actor (removes from all task assignee lists)
#[operation(verb = "delete", noun = "actor", description = "Delete an actor and remove from all task assignments")]
#[derive(Debug, Deserialize)]
pub struct DeleteActor {
    /// The actor ID to delete
    pub id: ActorId,
}

impl DeleteActor {
    pub fn new(id: impl Into<ActorId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteActor {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check actor exists
        if !board.actors.iter().any(|a| a.id() == &self.id) {
            return Err(KanbanError::ActorNotFound {
                id: self.id.to_string(),
            });
        }

        // Remove actor from all task assignee lists
        let task_ids = ctx.list_task_ids().await?;
        for id in task_ids {
            let mut task = ctx.read_task(&id).await?;
            if task.assignees.contains(&self.id) {
                task.assignees.retain(|a| a != &self.id);
                ctx.write_task(&task).await?;
            }
        }

        // Remove from board
        board.actors.retain(|a| a.id() != &self.id);
        ctx.write_board(&board).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string()
        }))
    }
}
