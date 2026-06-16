//! Comment commands
//!
//! Comments are dependent members of a task — stored inline on the task's
//! `comments` field as a JSON array of `{id, actor, text, timestamp}`
//! objects, never as standalone entities (contrast with attachments, which
//! create their own `attachment` entity).
//!
//! The member `id` is a lowercased ULID. ULIDs are time-ordered, so
//! ascending id order is creation order, and ids are unique (no
//! same-millisecond timestamp ties). Edits preserve `id`, so order is
//! stable under edit.

mod add;
mod delete;
mod get;
mod list;
mod normalize;
mod update;

pub(crate) use normalize::normalize_comment_log;

pub use add::AddComment;
pub use delete::DeleteComment;
pub use get::GetComment;
pub use list::ListComments;
pub use update::UpdateComment;

use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use crate::types::ActorId;
use serde_json::{json, Value};
use swissarmyhammer_entity::Entity;

/// Construct a new comment member.
///
/// Mints a fresh lowercased-ULID `id` and a UTC RFC3339 `timestamp`
/// (boards sync across machines via git, and mixed-offset ISO strings
/// don't sort lexically — UTC only).
///
/// Single source of truth for the member shape: used by both the agent
/// ops in this module and the UI field-set normalization path.
pub(crate) fn build_comment_member(text: &str, author_id: &str) -> Value {
    json!({
        "id": ulid::Ulid::new().to_string().to_lowercase(),
        "actor": author_id,
        "text": text,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    })
}

/// Project a stored comment member down to the canonical
/// `{id, actor, text, timestamp}` wire shape, dropping any stray keys.
pub(crate) fn comment_member_to_json(member: &Value) -> Value {
    json!({
        "id": member.get("id").cloned().unwrap_or(Value::Null),
        "actor": member.get("actor").cloned().unwrap_or(Value::Null),
        "text": member.get("text").cloned().unwrap_or(Value::Null),
        "timestamp": member.get("timestamp").cloned().unwrap_or(Value::Null),
    })
}

/// Resolve the author for a new comment.
///
/// - `Some(actor)` — validate that the actor entity exists; errors with
///   [`KanbanError::ActorNotFound`] when it doesn't.
/// - `None` — resolve the OS-level user identity and idempotently ensure
///   that actor exists (see [`crate::actor::ensure_os_user_actor`]).
///
/// Single source of truth for author rules: used by both the agent ops in
/// this module and the UI field-set normalization path.
pub(crate) async fn resolve_comment_author(
    ctx: &KanbanContext,
    explicit: Option<&str>,
) -> Result<ActorId> {
    match explicit {
        Some(actor_id) => {
            let ectx = ctx.entity_context().await?;
            ectx.read("actor", actor_id)
                .await
                .map_err(KanbanError::from_entity_error)?;
            Ok(ActorId::from_string(actor_id))
        }
        None => crate::actor::ensure_os_user_actor(ctx).await,
    }
}

/// Read the inline `comments` array off a task entity.
///
/// Missing or null fields read as an empty log.
pub(crate) fn task_comments(task: &Entity) -> Vec<Value> {
    task.get("comments")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

/// Find the index of the member with `id` in a comment log.
///
/// # Errors
///
/// Returns [`KanbanError::CommentNotFound`] when no member matches.
pub(crate) fn find_comment_index(comments: &[Value], id: &str) -> Result<usize> {
    comments
        .iter()
        .position(|member| member.get("id").and_then(Value::as_str) == Some(id))
        .ok_or_else(|| KanbanError::CommentNotFound { id: id.to_string() })
}

#[cfg(test)]
pub(crate) mod testing {
    use crate::actor::AddActor;
    use crate::board::InitBoard;
    use crate::context::KanbanContext;
    use crate::task::AddTask;
    use swissarmyhammer_operations::Execute;
    use tempfile::TempDir;

    /// Init a board with an `alice` actor and one task; returns the task id.
    pub(crate) async fn setup_with_task() -> (TempDir, KanbanContext, String) {
        let temp = TempDir::new().unwrap();
        let kanban_dir = temp.path().join(".kanban");
        let ctx = KanbanContext::new(kanban_dir);

        InitBoard::new("Test")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        AddActor::new("alice", "Alice")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();

        let task = AddTask::new("Task with comments")
            .execute(&ctx)
            .await
            .into_result()
            .unwrap();
        let task_id = task["id"].as_str().unwrap().to_string();

        (temp, ctx, task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// A freshly built member carries exactly the canonical keys: a 26-char
    /// lowercased ULID id, the author, the text, and a UTC RFC3339 timestamp.
    #[test]
    fn test_build_comment_member_shape() {
        let member = build_comment_member("hello", "alice");
        let obj = member.as_object().expect("member is an object");

        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["actor", "id", "text", "timestamp"]);

        let id = member["id"].as_str().unwrap();
        assert_eq!(id.len(), 26);
        assert_eq!(id, id.to_lowercase(), "member id must be lowercased");

        assert_eq!(member["actor"], "alice");
        assert_eq!(member["text"], "hello");

        let ts = member["timestamp"].as_str().unwrap();
        let parsed =
            chrono::DateTime::parse_from_rfc3339(ts).expect("timestamp must parse as RFC3339");
        assert_eq!(
            parsed.offset().local_minus_utc(),
            0,
            "timestamp must be UTC, got: {ts}"
        );
    }

    /// Member ids are time-ordered: builds in distinct milliseconds sort
    /// ascending. (Within the same millisecond ULID low bits are random, so
    /// the guarantee — and this test — is across millisecond boundaries.)
    #[test]
    fn test_build_comment_member_ids_ascend() {
        let first = build_comment_member("one", "alice");
        std::thread::sleep(std::time::Duration::from_millis(2));
        let second = build_comment_member("two", "alice");
        assert!(
            first["id"].as_str().unwrap() < second["id"].as_str().unwrap(),
            "ULID ids must ascend with creation order"
        );
    }

    /// Projection keeps the canonical keys and drops stray ones.
    #[test]
    fn test_comment_member_to_json_drops_stray_keys() {
        let stored = json!({
            "id": "01abc",
            "actor": "alice",
            "text": "hi",
            "timestamp": "2026-01-01T00:00:00+00:00",
            "stray": "drop me",
        });
        let projected = comment_member_to_json(&stored);
        let obj = projected.as_object().unwrap();
        let mut keys: Vec<&str> = obj.keys().map(String::as_str).collect();
        keys.sort_unstable();
        assert_eq!(keys, vec!["actor", "id", "text", "timestamp"]);
        assert_eq!(projected["text"], "hi");
    }

    #[test]
    fn test_find_comment_index_found_and_not_found() {
        let comments = vec![json!({"id": "01a"}), json!({"id": "01b"})];
        assert_eq!(find_comment_index(&comments, "01b").unwrap(), 1);

        let err = find_comment_index(&comments, "bogus").unwrap_err();
        assert!(
            matches!(err, KanbanError::CommentNotFound { ref id } if id == "bogus"),
            "expected CommentNotFound, got: {err:?}"
        );
    }

    #[test]
    fn test_task_comments_missing_field_reads_empty() {
        let task = Entity::new("task", "t1");
        assert!(task_comments(&task).is_empty());
    }
}
