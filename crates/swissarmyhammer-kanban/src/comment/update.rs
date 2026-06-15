//! UpdateComment command

use crate::comment::{find_comment_index, task_comments};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task_helpers::task_mutation_ack;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Edit the text of an existing comment (author and timestamp are immutable)
#[operation(
    verb = "update",
    noun = "comment",
    description = "Edit a comment's text"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct UpdateComment {
    /// The task ID that owns the comment
    pub task_id: TaskId,
    /// The comment member ID to edit
    pub id: String,
    /// The replacement comment text
    pub text: String,
}

impl UpdateComment {
    /// Create a new UpdateComment command
    pub fn new(task_id: impl Into<TaskId>, id: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            id: id.into(),
            text: text.into(),
        }
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for UpdateComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut task = ectx.read("task", self.task_id.as_str()).await?;

            // Edit only `text` — id, actor, and timestamp are immutable.
            let mut comments = task_comments(&task);
            let index = find_comment_index(&comments, &self.id)?;
            comments[index]["text"] = json!(self.text);
            task.set("comments", json!(comments));
            ectx.write(&task).await?;

            // Pure ack — no member echo (the caller already has the text).
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
    use crate::comment::{AddComment, GetComment};
    use crate::error::KanbanError;
    use crate::task_helpers::assert_task_mutation_ack;

    /// Updating a comment returns the pure ack and changes only the text —
    /// id, actor, and timestamp are immutable. Verified via stored state.
    #[tokio::test]
    async fn test_update_comment_changes_text_only() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let added = AddComment::new(task_id.as_str(), "original")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let comment_id = added["comment"]["id"].as_str().unwrap().to_string();
        let original_ts = added["comment"]["timestamp"].as_str().unwrap().to_string();

        let result = UpdateComment::new(task_id.as_str(), comment_id.as_str(), "edited")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_task_mutation_ack(&result, &task_id);

        // Verify the effect via stored state, not the response echo.
        let stored = GetComment::new(task_id.as_str(), comment_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(stored["text"], "edited");
        assert_eq!(stored["id"], comment_id);
        assert_eq!(stored["actor"], "alice");
        assert_eq!(stored["timestamp"], original_ts);
    }

    #[tokio::test]
    async fn test_update_comment_bogus_id_is_comment_not_found() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result = UpdateComment::new(task_id.as_str(), "bogus", "new text")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            matches!(result, Err(KanbanError::CommentNotFound { ref id }) if id == "bogus"),
            "expected CommentNotFound, got: {result:?}"
        );
    }
}
