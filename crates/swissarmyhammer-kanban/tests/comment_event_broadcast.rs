//! Production-path integration tests: comment mutations close the
//! command → event → store loop.
//!
//! Comments mutate the task's `comments` field via `ectx.write(&task)`, so
//! the attached `EntityCache` must emit a thin field-level
//! `EntityEvent::EntityChanged` carrying a `FieldChange { field: "comments",
//! value: <new array> }` on its broadcast channel. These tests subscribe to
//! the real cache wired by `KanbanContext::entity_context()` and run the
//! mutations through the real command paths — both the agent op path
//! (`AddComment` / `UpdateComment` / `DeleteComment`) and the UI field-set
//! path (`UpdateEntityField` on the `comments` field, which is what the
//! React comment editor actually dispatches).
//!
//! No production change was needed: the existing cache diff already emits
//! the `comments` field change for every path. These tests pin that
//! contract.

use serde_json::{json, Value};
use swissarmyhammer_entity::EntityEvent;
use swissarmyhammer_kanban::{
    actor::AddActor,
    board::InitBoard,
    comment::{AddComment, DeleteComment, UpdateComment},
    entity::UpdateEntityField,
    task::AddTask,
    Execute, KanbanContext,
};
use tempfile::TempDir;
use tokio::sync::broadcast::Receiver;

/// Init a board with an `alice` actor and one task; returns the task id.
///
/// Mirrors the comment module's unit-test harness, but through the public
/// API so the integration test exercises exactly what external callers see.
async fn setup_with_task() -> (TempDir, KanbanContext, String) {
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

/// Subscribe to the entity cache's broadcast channel.
///
/// The cache is the one `entity_context()` attached during setup — the same
/// instance production subscribers (the kanban-app bridge) listen on.
fn subscribe(ctx: &KanbanContext) -> Receiver<EntityEvent> {
    ctx.entity_cache()
        .expect("entity cache is initialized by setup ops")
        .subscribe()
}

/// Drain every event currently buffered on the receiver.
///
/// Events are emitted synchronously inside `ectx.write`, so by the time an
/// awaited command returns, everything it produced is already buffered.
fn drain_events(rx: &mut Receiver<EntityEvent>) -> Vec<EntityEvent> {
    let mut events = Vec::new();
    while let Ok(evt) = rx.try_recv() {
        events.push(evt);
    }
    events
}

/// Assert the drained events are exactly one `EntityChanged` for `task_id`
/// whose `changes` contains a `comments` field-level change; returns that
/// change's `value` (the new comments array).
///
/// The event is thin by construction — `FieldChange` is `{field, value}`
/// only — so receiving the array here proves no enrichment round-trip is
/// needed to land the comment in a subscriber's store.
fn expect_single_comments_change(events: Vec<EntityEvent>, task_id: &str) -> Value {
    assert_eq!(
        events.len(),
        1,
        "expected exactly one event, got: {events:?}"
    );
    match events.into_iter().next().unwrap() {
        EntityEvent::EntityChanged {
            entity_type,
            id,
            changes,
            ..
        } => {
            assert_eq!(entity_type, "task");
            assert_eq!(id, task_id);
            changes
                .into_iter()
                .find(|change| change.field == "comments")
                .map(|change| change.value)
                .expect("changes must contain a FieldChange for `comments`")
        }
        other => panic!("expected EntityChanged, got {other:?}"),
    }
}

/// Agent op path: `AddComment` produces exactly one `EntityChanged` whose
/// `changes` carries the new `comments` array with the appended member.
#[tokio::test]
async fn comment_add_emits_field_change_event() {
    let (_temp, ctx, task_id) = setup_with_task().await;

    let mut rx = subscribe(&ctx);

    AddComment::new(task_id.as_str(), "first comment")
        .with_actor("alice")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let value = expect_single_comments_change(drain_events(&mut rx), &task_id);
    let members = value.as_array().expect("comments value is an array");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["text"], "first comment");
    assert_eq!(members[0]["actor"], "alice");
}

/// Agent op path: `UpdateComment` and `DeleteComment` each produce exactly
/// one `EntityChanged` with a `comments` field-level change reflecting the
/// stored log after the mutation.
#[tokio::test]
async fn comment_edit_and_delete_emit_field_change_events() {
    let (_temp, ctx, task_id) = setup_with_task().await;

    // Seed one comment before subscribing — its event is not under test.
    let added = AddComment::new(task_id.as_str(), "original")
        .with_actor("alice")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();
    let comment_id = added["comment"]["id"].as_str().unwrap().to_string();

    let mut rx = subscribe(&ctx);

    // Edit: one event, the member text updated in the carried array.
    UpdateComment::new(task_id.as_str(), comment_id.as_str(), "edited")
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let value = expect_single_comments_change(drain_events(&mut rx), &task_id);
    let members = value.as_array().expect("comments value is an array");
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["id"], comment_id.as_str());
    assert_eq!(members[0]["text"], "edited");

    // Delete: one event, the carried array is now empty.
    DeleteComment::new(task_id.as_str(), comment_id.as_str())
        .execute(&ctx)
        .await
        .into_result()
        .unwrap();

    let value = expect_single_comments_change(drain_events(&mut rx), &task_id);
    assert_eq!(
        value.as_array().expect("comments value is an array").len(),
        0,
        "deleted member must be gone from the event payload"
    );
}

/// UI field-set path: `UpdateEntityField` committing a new `comments` array
/// (the React editor's dispatch) produces a `comments` `FieldChange`
/// carrying the normalized array — server-assigned ULID id, resolved
/// author, UTC RFC3339 timestamp.
#[tokio::test]
async fn comment_field_set_emits_field_change_event() {
    let (_temp, ctx, task_id) = setup_with_task().await;

    let mut rx = subscribe(&ctx);

    UpdateEntityField::new(
        "task",
        &task_id,
        "comments",
        json!([{"text": "from the editor", "actor": "alice"}]),
    )
    .execute(&ctx)
    .await
    .into_result()
    .unwrap();

    let value = expect_single_comments_change(drain_events(&mut rx), &task_id);
    let members = value.as_array().expect("comments value is an array");
    assert_eq!(members.len(), 1);

    // The event carries the NORMALIZED member, not the raw UI input.
    let member = &members[0];
    assert_eq!(member["text"], "from the editor");
    assert_eq!(member["actor"], "alice");
    let id = member["id"].as_str().expect("normalized member has an id");
    assert_eq!(id.len(), 26, "member id must be a ULID");
    assert_eq!(id, id.to_lowercase(), "member id must be lowercased");
    chrono::DateTime::parse_from_rfc3339(member["timestamp"].as_str().unwrap())
        .expect("normalized timestamp must be RFC3339");
}
