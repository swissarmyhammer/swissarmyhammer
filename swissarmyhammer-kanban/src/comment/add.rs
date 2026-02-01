//! AddComment command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{ActorId, Comment, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// Add a comment to a task
#[operation(verb = "add", noun = "comment", description = "Add a comment to a task")]
#[derive(Debug, Deserialize)]
pub struct AddComment {
    /// The task ID to comment on
    pub task_id: TaskId,
    /// The comment body
    pub body: String,
    /// The author of the comment
    pub author: ActorId,
}

impl AddComment {
    pub fn new(
        task_id: impl Into<TaskId>,
        body: impl Into<String>,
        author: impl Into<ActorId>,
    ) -> Self {
        Self {
            task_id: task_id.into(),
            body: body.into(),
            author: author.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddComment {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let mut task = ctx.read_task(&self.task_id).await?;

        let comment = Comment::new(&self.body, self.author.clone());
        let result = serde_json::to_value(&comment)?;

        task.comments.push(comment);
        ctx.write_task(&task).await?;

        Ok(result)
    }
}
