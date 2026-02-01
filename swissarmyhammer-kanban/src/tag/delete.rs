//! DeleteTag command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TagId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete a tag (removes from all tasks)
#[operation(verb = "delete", noun = "tag", description = "Delete a tag and remove from all tasks")]
#[derive(Debug, Deserialize)]
pub struct DeleteTag {
    /// The tag ID to delete
    pub id: TagId,
}

impl DeleteTag {
    pub fn new(id: impl Into<TagId>) -> Self {
        Self { id: id.into() }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteTag {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut board = ctx.read_board().await?;

        // Check tag exists
        if !board.tags.iter().any(|t| &t.id == &self.id) {
            return Err(KanbanError::TagNotFound {
                id: self.id.to_string(),
            });
        }

        // Remove tag from all tasks
        let task_ids = ctx.list_task_ids().await?;
        for id in task_ids {
            let mut task = ctx.read_task(&id).await?;
            if task.tags.contains(&self.id) {
                task.tags.retain(|t| t != &self.id);
                ctx.write_task(&task).await?;
            }
        }

        // Remove from board
        board.tags.retain(|t| &t.id != &self.id);
        ctx.write_board(&board).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string()
        }))
    }
}
