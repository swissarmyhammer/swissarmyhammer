//! ListComments command


use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::{Comment, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute};

/// List all comments on a task
#[operation(verb = "list", noun = "comments", description = "List all comments on a task")]
#[derive(Debug, Deserialize)]
pub struct ListComments {
    /// The task ID to list comments for
    pub task_id: TaskId,
}

impl ListComments {
    pub fn new(task_id: impl Into<TaskId>) -> Self {
        Self {
            task_id: task_id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for ListComments {
    async fn execute(&self, ctx: &KanbanContext) -> Result<Value> {
        let task = ctx.read_task(&self.task_id).await?;

        let comments: Vec<&Comment> = task.comments.iter().collect();

        Ok(serde_json::json!({
            "comments": comments,
            "count": comments.len()
        }))
    }
}
