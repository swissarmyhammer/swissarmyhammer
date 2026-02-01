//! TagTask command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{TagId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Add a tag to a task
#[operation(verb = "tag", noun = "task", description = "Add a tag to a task")]
#[derive(Debug, Deserialize)]
pub struct TagTask {
    /// The task ID to tag
    pub id: TaskId,
    /// The tag ID to add
    pub tag: TagId,
}

impl TagTask {
    pub fn new(id: impl Into<TaskId>, tag: impl Into<TagId>) -> Self {
        Self {
            id: id.into(),
            tag: tag.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for TagTask {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        // Verify tag exists
        let board = ctx.read_board().await?;
        if !board.tags.iter().any(|t| &t.id == &self.tag) {
            return Err(KanbanError::TagNotFound {
                id: self.tag.to_string(),
            });
        }

        let mut task = ctx.read_task(&self.id).await?;

        // Add tag if not already present
        if !task.tags.contains(&self.tag) {
            task.tags.push(self.tag.clone());
            ctx.write_task(&task).await?;
        }

        Ok(serde_json::json!({
            "tagged": true,
            "task_id": self.id.to_string(),
            "tag_id": self.tag.to_string()
        }))
    }
}
