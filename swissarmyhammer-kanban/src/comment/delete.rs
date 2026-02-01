//! DeleteComment command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{CommentId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Delete a comment
#[operation(verb = "delete", noun = "comment", description = "Delete a comment from a task")]
#[derive(Debug, Deserialize)]
pub struct DeleteComment {
    /// The task ID containing the comment
    pub task_id: TaskId,
    /// The comment ID to delete
    pub id: CommentId,
}

impl DeleteComment {
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<CommentId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteComment {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.task_id).await?;

        // Check comment exists
        if !task.comments.iter().any(|c| c.id == self.id) {
            return Err(KanbanError::CommentNotFound {
                id: self.id.to_string(),
            });
        }

        task.comments.retain(|c| c.id != self.id);
        ctx.write_task(&task).await?;

        Ok(serde_json::json!({
            "deleted": true,
            "id": self.id.to_string()
        }))
    }
}
