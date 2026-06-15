//! DeleteComment command

use crate::comment::{find_comment_index, task_comments};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task_helpers::task_mutation_ack;
use crate::types::TaskId;
use serde::Deserialize;
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Remove a comment from a task's inline comment log
#[operation(
    verb = "delete",
    noun = "comment",
    description = "Delete a comment from a task"
)]
#[derive(Debug, Deserialize)]
pub struct DeleteComment {
    /// The task ID that owns the comment
    pub task_id: TaskId,
    /// The comment member ID to remove
    pub id: String,
}

impl DeleteComment {
    /// Create a new DeleteComment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for DeleteComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut task = ectx.read("task", self.task_id.as_str()).await?;

            let mut comments = task_comments(&task);
            let index = find_comment_index(&comments, &self.id)?;
            comments.remove(index);
            task.set("comments", json!(comments));
            ectx.write(&task).await?;

            // Pure ack — no member echo.
            Ok(task_mutation_ack(&task))
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
    use crate::comment::{AddComment, ListComments};
    use crate::error::KanbanError;
    use crate::task_helpers::assert_task_mutation_ack;

    /// Deleting a comment returns the pure ack; the member is gone from the
    /// stored log while the other members survive.
    #[tokio::test]
    async fn test_delete_comment_removes_member() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let first = AddComment::new(task_id.as_str(), "keep me")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let second = AddComment::new(task_id.as_str(), "delete me")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let keep_id = first["comment"]["id"].as_str().unwrap();
        let delete_id = second["comment"]["id"].as_str().unwrap();

        let result = DeleteComment::new(task_id.as_str(), delete_id)
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_task_mutation_ack(&result, &task_id);

        // Verify the effect via stored state, not the response echo.
        let listed = ListComments::new(task_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let members = listed["comments"].as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["id"], keep_id);
        assert_eq!(members[0]["text"], "keep me");
    }

    #[tokio::test]
    async fn test_delete_comment_bogus_id_is_comment_not_found() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result = DeleteComment::new(task_id.as_str(), "bogus")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            matches!(result, Err(KanbanError::CommentNotFound { ref id }) if id == "bogus"),
            "expected CommentNotFound, got: {result:?}"
        );
    }
}
