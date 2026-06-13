//! AddComment command

use crate::comment::{
    build_comment_member, comment_member_to_json, resolve_comment_author, task_comments,
};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::task_helpers::task_mutation_ack;
use crate::types::TaskId;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use swissarmyhammer_operations::{async_trait, operation, Execute, ExecutionResult};

/// Add a comment to a task's inline comment log
#[operation(
    verb = "add",
    noun = "comment",
    description = "Add a comment to a task"
)]
#[derive(Debug, Deserialize, Serialize)]
pub struct AddComment {
    /// The task ID to comment on
    pub task_id: TaskId,
    /// Optional author actor id; when omitted, the OS-level user is
    /// resolved and ensured as the author
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,
    /// The comment text
    pub text: String,
}

impl AddComment {
    /// Create a new AddComment command with no explicit author.
    pub fn new(task_id: impl Into<TaskId>, text: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            actor: None,
            text: text.into(),
        }
    }

    /// Attribute the comment to an explicit actor id.
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }
}

#[async_trait]
impl Execute<KanbanContext, KanbanError> for AddComment {
    async fn execute(&self, ctx: &KanbanContext) -> ExecutionResult<Value, KanbanError> {
        let result: Result<Value> = async {
            let ectx = ctx.entity_context().await?;
            let mut task = ectx.read("task", self.task_id.as_str()).await?;

            let author = resolve_comment_author(ctx, self.actor.as_deref()).await?;
            let member = build_comment_member(&self.text, author.as_str());

            // Append to the inline log, preserving existing members.
            let mut comments = task_comments(&task);
            comments.push(member.clone());
            task.set("comments", json!(comments));
            ectx.write(&task).await?;

            // Mutation ack plus the new member — the member is genuinely new
            // information (server-assigned id/timestamp/resolved author).
            let mut ack = task_mutation_ack(&task);
            ack["comment"] = comment_member_to_json(&member);
            Ok(ack)
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
    use crate::comment::ListComments;
    use crate::error::KanbanError;
    use crate::task_helpers::assert_task_mutation_ack_with;

    /// Adding two comments with an explicit actor: both members are stored
    /// with who/what/when + stable ids, the second add preserves the first,
    /// and the add response is the mutation ack plus the new member.
    #[tokio::test]
    async fn test_add_two_comments_preserves_existing() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result1 = AddComment::new(task_id.as_str(), "first comment")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_task_mutation_ack_with(&result1, &task_id, &["comment"]);
        assert_eq!(result1["comment"]["actor"], "alice");
        assert_eq!(result1["comment"]["text"], "first comment");

        let result2 = AddComment::new(task_id.as_str(), "second comment")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_task_mutation_ack_with(&result2, &task_id, &["comment"]);

        // Re-read via list comments — stored state, not response echo.
        let listed = ListComments::new(task_id.as_str())
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let members = listed["comments"].as_array().unwrap();
        assert_eq!(members.len(), 2, "second add must preserve the first");

        for (member, text) in members.iter().zip(["first comment", "second comment"]) {
            assert_eq!(member["actor"], "alice");
            assert_eq!(member["text"], text);
            let id = member["id"].as_str().unwrap();
            assert_eq!(id.len(), 26);
            assert_eq!(id, id.to_lowercase());

            let ts = member["timestamp"].as_str().unwrap();
            let parsed = chrono::DateTime::parse_from_rfc3339(ts)
                .expect("stored timestamp must parse as RFC3339");
            assert_eq!(
                parsed.offset().local_minus_utc(),
                0,
                "stored timestamp must be UTC, got: {ts}"
            );
        }
    }

    /// An explicit author that doesn't exist as an actor entity errors
    /// clearly instead of silently mis-attributing the comment.
    #[tokio::test]
    async fn test_add_comment_unknown_actor_errors() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let result = AddComment::new(task_id.as_str(), "hello")
            .with_actor("ghost")
            .execute(&ctx)
            .await
            .into_result();

        assert!(
            matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "ghost"),
            "expected ActorNotFound, got: {result:?}"
        );
    }

    /// `actor: None` resolves the OS user, ensures that actor entity exists,
    /// and attributes the comment to it — idempotent on repeat.
    #[tokio::test]
    async fn test_add_comment_without_actor_resolves_os_user() {
        let (_temp, ctx, task_id) = setup_with_task().await;

        let expected_id = swissarmyhammer_common::slug(&whoami::username());

        let result = AddComment::new(task_id.as_str(), "from the OS user")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result["comment"]["actor"], expected_id.as_str());

        // The ensured actor entity now exists.
        let ectx = ctx.entity_context().await.unwrap();
        let actor = ectx.read("actor", &expected_id).await.unwrap();
        assert_eq!(actor.id.as_ref(), expected_id.as_str());

        // Repeat add is idempotent on the actor and still succeeds.
        let result2 = AddComment::new(task_id.as_str(), "again")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        assert_eq!(result2["comment"]["actor"], expected_id.as_str());
    }

    /// Comments are dependent members, not their own entity kind: adding
    /// comments must never write a standalone comment entity file.
    #[tokio::test]
    async fn test_add_comment_writes_no_comment_entity() {
        let (temp, ctx, task_id) = setup_with_task().await;

        AddComment::new(task_id.as_str(), "inline only")
            .with_actor("alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let kanban_dir = temp.path().join(".kanban");
        assert!(
            !kanban_dir.join("comments").exists(),
            "no comments entity directory may be created"
        );

        // The tasks dir holds exactly the one task file — no comment files.
        let tasks_dir = kanban_dir.join("tasks");
        let entries: Vec<String> = std::fs::read_dir(&tasks_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_file())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        assert!(
            entries
                .iter()
                .all(|name| name.contains(&task_id.to_lowercase()) || name.contains(&task_id)),
            "tasks dir must only contain the task file, got: {entries:?}"
        );
    }
}
