//! GetComment command

use crate::comment::{comment_member_to_json, find_comment_index, task_comments};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::Value;
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Get a single comment from a task's inline comment log
#[operation(
    verb = "get",
    noun = "comment",
    description = "Get a comment from a task"
)]
#[derive(Debug, Deserialize)]
pub struct GetComment {
    /// The task ID that owns the comment
    pub task_id: TaskId,
    /// The comment member ID
    pub id: String,
}

impl GetComment {
    /// Create a new GetComment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for GetComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let task = ectx.read("task", self.task_id.as_str()).await?;

            let comments = task_comments(&task);
            let index = find_comment_index(&comments, &self.id)?;
            Ok(comment_member_to_json(&comments[index]))
        }
        .await;

        match result {
            Ok(value) => ExecutionResult::Success { value },
            Err(error) => ExecutionResult::Failed { error },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment::testing::setup_with_task;
    use crate::comment::AddComment;
    use crate::error::KanbanError;

    #[tokio::test]
    async fn test_get_comment_returns_member() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let added = AddComment::new(task_id.as_str(), "find me")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let comment_id = added["comment"]["id"].as_str().unwrap();

        let result = GetComment::new(task_id.as_str(), comment_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        assert_eq!(result["id"], comment_id);
        assert_eq!(result["actor"], "alice");
        assert_eq!(result["text"], "find me");
        assert!(result["timestamp"].is_string());
    }

    #[tokio::test]
    async fn test_get_comment_bogus_id_is_comment_not_found() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result = GetComment::new(task_id.as_str(), "bogus")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            matches!(result, Err(KanbanError::CommentNotFound { ref id }) if id == "bogus"),
            "expected CommentNotFound, got: {result:?}"
        );
    }
}
