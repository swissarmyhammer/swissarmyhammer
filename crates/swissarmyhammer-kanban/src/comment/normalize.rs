//! Comment-log normalization for the UI field-set path.
//!
//! The UI never dispatches comment ops — the comment editor commits the
//! whole `comments` array through the generic `entity.update_field`
//! command. This module merges that (possibly stale) incoming array
//! against the stored log so concurrent agent appends survive.
//!
//! Merge semantics are explicitly NOT diff-as-delete: an old member being
//! absent from the incoming array is preserved, never deleted. Deletion
//! happens only via an explicit wire-only tombstone `{id, deleted: true}`
//! emitted by the editor; tombstones are never stored.

use crate::comment::{build_comment_member, comment_member_to_json, resolve_comment_author};
use crate::context::KanbanContext;
use crate::error::{KanbanError, Result};
use serde_json::Value;

/// Merge an incoming comment-log array against the stored one.
///
/// Keyed on member `id`:
/// - incoming member without an `id` (or with an `id` unknown to `old`)
///   is NEW: mints a fresh ULID id + UTC RFC3339 timestamp and resolves
///   the author via [`resolve_comment_author`] (explicit actor validated,
///   else OS-user fallback);
/// - incoming member whose `id` matches an old member is a text-only
///   edit: the old `actor` and `timestamp` are immutable, only `text` is
///   taken from the incoming member;
/// - incoming tombstone `{id, deleted: true}` removes that member
///   (unknown id → no-op); tombstones are never stored;
/// - old members absent from `incoming` are PRESERVED (concurrent-append
///   protection).
///
/// Returns the normalized array sorted by member `id` ascending (ULIDs
/// are time-ordered, so id order is creation order).
///
/// # Errors
///
/// - [`KanbanError::InvalidValue`] when `incoming` is not an array;
/// - [`KanbanError::ActorNotFound`] when a new member names an explicit
///   actor that doesn't exist.
pub(crate) async fn normalize_comment_log(
    ctx: &KanbanContext,
    old: &Value,
    incoming: &Value,
) -> Result<Value> {
    let incoming_members = incoming
        .as_array()
        .ok_or_else(|| KanbanError::InvalidValue {
            field: "comments".to_string(),
            message: "comment log value must be an array".to_string(),
        })?;

    // BTreeMap keyed on member id keeps the result sorted by id ascending
    // (ULIDs are time-ordered, so id order is creation order).
    let mut merged: std::collections::BTreeMap<String, Value> = old
        .as_array()
        .map(|members| {
            members
                .iter()
                .filter_map(|member| {
                    member
                        .get("id")
                        .and_then(Value::as_str)
                        .map(|id| (id.to_string(), member.clone()))
                })
                .collect()
        })
        .unwrap_or_default();

    for member in incoming_members {
        let id = member.get("id").and_then(Value::as_str);

        if member.get("deleted").and_then(Value::as_bool) == Some(true) {
            // Wire-only tombstone: explicit delete, never stored.
            if let Some(id) = id {
                merged.remove(id);
            }
            continue;
        }

        match id.filter(|id| merged.contains_key(*id)) {
            Some(id) => {
                // Existing member: text-only edit; actor/timestamp immutable.
                let mut edited = merged[id].clone();
                if let Some(text) = member.get("text") {
                    edited["text"] = text.clone();
                }
                merged.insert(id.to_string(), comment_member_to_json(&edited));
            }
            None => {
                // New member (no id, or id unknown to the stored log):
                // mint a fresh id/timestamp and resolve the author.
                let author =
                    resolve_comment_author(ctx, member.get("actor").and_then(Value::as_str))
                        .await?;
                let text = member.get("text").and_then(Value::as_str).unwrap_or("");
                let new_member = build_comment_member(text, author.as_str());
                let new_id = new_member["id"].as_str().expect("minted id").to_string();
                merged.insert(new_id, new_member);
            }
        }
    }

    Ok(Value::Array(merged.into_values().collect()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comment::testing::setup_with_task;
    use serde_json::json;

    /// Build a stored-shape member with fixed id/actor/text/timestamp.
    fn stored_member(id: &str, actor: &str, text: &str, timestamp: &str) -> Value {
        json!({"id": id, "actor": actor, "text": text, "timestamp": timestamp})
    }

    /// A new member (no `id`) gets a minted 26-char lowercased ULID id, a
    /// UTC RFC3339 timestamp, and the explicit actor when one is given.
    #[tokio::test]
    async fn test_new_member_without_id_gets_id_timestamp_author() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([]);
        let incoming = json!([{"text": "hello", "actor": "alice"}]);
        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();

        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        let member = &members[0];

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

    /// A new member with no explicit actor falls back to the OS-level user.
    #[tokio::test]
    async fn test_new_member_without_actor_resolves_os_user() {
        let (_temp, ctx, _task_id) = setup_with_task().await;
        let expected_id = swissarmyhammer_common::slug(&whoami::username());

        let result = normalize_comment_log(&ctx, &json!([]), &json!([{"text": "from os user"}]))
            .await
            .unwrap();

        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["actor"], expected_id.as_str());
    }

    /// A new member naming an actor that doesn't exist errors clearly
    /// instead of silently mis-attributing the comment.
    #[tokio::test]
    async fn test_new_member_unknown_actor_errors() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let result =
            normalize_comment_log(&ctx, &json!([]), &json!([{"text": "hi", "actor": "ghost"}]))
                .await;

        assert!(
            matches!(result, Err(KanbanError::ActorNotFound { ref id }) if id == "ghost"),
            "expected ActorNotFound, got: {result:?}"
        );
    }

    /// Editing an existing member's text keeps its original actor and
    /// timestamp — only the text changes.
    #[tokio::test]
    async fn test_existing_member_text_edit_preserves_who_and_when() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([stored_member(
            "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "alice",
            "original",
            "2026-01-01T00:00:00+00:00"
        )]);
        let incoming = json!([{
            "id": "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "actor": "mallory",
            "text": "edited",
            "timestamp": "2030-01-01T00:00:00+00:00"
        }]);

        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();
        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["id"], "01aaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert_eq!(members[0]["actor"], "alice", "actor is immutable");
        assert_eq!(
            members[0]["timestamp"], "2026-01-01T00:00:00+00:00",
            "timestamp is immutable"
        );
        assert_eq!(members[0]["text"], "edited");
    }

    /// A tombstone removes the member, and the tombstone itself is never
    /// stored in the result.
    #[tokio::test]
    async fn test_tombstone_removes_member_and_is_not_stored() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([
            stored_member(
                "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
                "alice",
                "keep me",
                "2026-01-01T00:00:00+00:00"
            ),
            stored_member(
                "01bbbbbbbbbbbbbbbbbbbbbbbbbb",
                "alice",
                "delete me",
                "2026-01-02T00:00:00+00:00"
            ),
        ]);
        let incoming = json!([
            {"id": "01aaaaaaaaaaaaaaaaaaaaaaaaaa", "text": "keep me"},
            {"id": "01bbbbbbbbbbbbbbbbbbbbbbbbbb", "deleted": true},
        ]);

        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();
        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["id"], "01aaaaaaaaaaaaaaaaaaaaaaaaaa");
        assert!(
            members[0].get("deleted").is_none(),
            "tombstone markers must never be stored"
        );
    }

    /// A tombstone for an id not present in the stored log is a no-op.
    #[tokio::test]
    async fn test_tombstone_unknown_id_is_noop() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([stored_member(
            "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
            "alice",
            "still here",
            "2026-01-01T00:00:00+00:00"
        )]);
        let incoming = json!([{"id": "01zzzzzzzzzzzzzzzzzzzzzzzzzz", "deleted": true}]);

        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();
        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        assert_eq!(members[0]["text"], "still here");
    }

    /// The concurrent-append race: the stored log contains an agent
    /// comment the incoming (stale) snapshot lacks — it must survive.
    #[tokio::test]
    async fn test_absent_old_member_is_preserved() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([
            stored_member(
                "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
                "alice",
                "user comment",
                "2026-01-01T00:00:00+00:00"
            ),
            stored_member(
                "01bbbbbbbbbbbbbbbbbbbbbbbbbb",
                "agent",
                "concurrent agent append",
                "2026-01-02T00:00:00+00:00"
            ),
        ]);
        // Stale UI snapshot: only knows about the first member.
        let incoming = json!([{"id": "01aaaaaaaaaaaaaaaaaaaaaaaaaa", "text": "user comment"}]);

        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();
        let members = result.as_array().unwrap();
        assert_eq!(
            members.len(),
            2,
            "absence is not deletion — the agent comment must survive"
        );
        assert_eq!(members[1]["text"], "concurrent agent append");
        assert_eq!(members[1]["actor"], "agent");
    }

    /// A non-tombstone incoming member with an id unknown to the stored
    /// log is treated as new: its supplied id is discarded and a fresh
    /// one minted.
    #[tokio::test]
    async fn test_incoming_unknown_id_treated_as_new_with_fresh_id() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let incoming = json!([{
            "id": "01zzzzzzzzzzzzzzzzzzzzzzzzzz",
            "text": "smuggled id",
            "actor": "alice"
        }]);
        let result = normalize_comment_log(&ctx, &json!([]), &incoming)
            .await
            .unwrap();

        let members = result.as_array().unwrap();
        assert_eq!(members.len(), 1);
        let id = members[0]["id"].as_str().unwrap();
        assert_ne!(
            id, "01zzzzzzzzzzzzzzzzzzzzzzzzzz",
            "supplied unknown id must be discarded"
        );
        assert_eq!(id.len(), 26);
        assert_eq!(members[0]["text"], "smuggled id");
        assert_eq!(members[0]["actor"], "alice");
    }

    /// The normalized result is sorted by member id ascending (creation
    /// order), regardless of input order.
    #[tokio::test]
    async fn test_result_sorted_by_id_ascending() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let old = json!([
            stored_member(
                "01cccccccccccccccccccccccccc",
                "alice",
                "third",
                "2026-01-03T00:00:00+00:00"
            ),
            stored_member(
                "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
                "alice",
                "first",
                "2026-01-01T00:00:00+00:00"
            ),
        ]);
        let incoming = json!([
            {"id": "01cccccccccccccccccccccccccc", "text": "third"},
            {"id": "01aaaaaaaaaaaaaaaaaaaaaaaaaa", "text": "first"},
        ]);

        let result = normalize_comment_log(&ctx, &old, &incoming).await.unwrap();
        let ids: Vec<&str> = result
            .as_array()
            .unwrap()
            .iter()
            .map(|m| m["id"].as_str().unwrap())
            .collect();
        assert_eq!(
            ids,
            vec![
                "01aaaaaaaaaaaaaaaaaaaaaaaaaa",
                "01cccccccccccccccccccccccccc"
            ]
        );
    }

    /// The wire convention is the editor always commits an array — a
    /// non-array value is rejected loudly.
    #[tokio::test]
    async fn test_non_array_incoming_errors() {
        let (_temp, ctx, _task_id) = setup_with_task().await;

        let result = normalize_comment_log(&ctx, &json!([]), &json!("not an array")).await;
        assert!(
            matches!(result, Err(KanbanError::InvalidValue { ref field, .. }) if field == "comments"),
            "expected InvalidValue, got: {result:?}"
        );
    }
}
