//! UpdateComment command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{CommentId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Update a comment
#[operation(verb = "update", noun = "comment", description = "Update a comment's body")]
#[derive(Debug, Deserialize)]
pub struct UpdateComment {
    /// The task ID containing the comment
    pub task_id: TaskId,
    /// The comment ID to update
    pub id: CommentId,
    /// New comment body
    pub body: Option<String>,
}

impl UpdateComment {
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<CommentId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
            body: None,
        }
    }

    pub fn with_body(mut self, body: impl Into<String>) -> Self {
        self.body = Some(body.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateComment {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.task_id).await?;

        let comment = task
            .comments
            .iter_mut()
            .find(|c| c.id == self.id)
            .ok_or_else(|| KanbanError::CommentNotFound {
                id: self.id.to_string(),
            })?;

        if let Some(body) = &self.body {
            comment.body = body.clone();
        }

        let result = serde_json::to_value(&*comment)?;
        ctx.write_task(&task).await?;

        Ok(result)
    }
}
