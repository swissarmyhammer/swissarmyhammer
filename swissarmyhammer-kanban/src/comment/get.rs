//! GetComment command


use crate::context::KanbanContext;
use crate::error::KanbanError;
use crate::types::{CommentId, TaskId};
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a comment by ID
#[operation(verb = "get", noun = "comment", description = "Get a comment by ID from a task")]
#[derive(Debug, Deserialize)]
pub struct GetComment {
    /// The task ID containing the comment
    pub task_id: TaskId,
    /// The comment ID to retrieve
    pub id: CommentId,
}

impl GetComment {
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<CommentId>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        match async {
            let task = ctx.read_task(&self.task_id).await?;

            let comment = task
                .comments
                .iter()
                .find(|c| c.id == self.id)
                .ok_or_else(|| KanbanError::CommentNotFound {
                    id: self.id.to_string(),
                })?;

            Ok(serde_json::to_value(comment)?)
        }
        .await
        {
            Ok(value) => ExecutionResult::Unlogged { value },
            Err(error) => ExecutionResult::Failed {
                error,
                log_entry: None,
            },
        }
    }
}
