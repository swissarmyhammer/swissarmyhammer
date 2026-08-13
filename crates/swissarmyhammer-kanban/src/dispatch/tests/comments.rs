//! The comment operations.
//!
//! These tests hold the add, list, get, update and delete round trips. They
//! also hold the actor that dispatch puts on a new comment.

use super::*;

/// `add comment` then `list comments` round-trips through
/// `parse_input` → `execute_operation`: the add returns the mutation ack
/// (top-level `id` = task id) plus the new member, and the list shows it.
#[tokio::test]
async fn dispatch_add_comment_then_list_round_trip() {
    let (_temp, ctx) = setup().await;
    let task_id = add_one_task(&ctx, "Comment target").await;

    let ops = parse_input(json!({"op": "add comment", "task_id": task_id, "text": "hi"})).unwrap();
    let added = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(added["ok"], true);
    assert_eq!(added["id"].as_str().unwrap(), task_id);
    assert_eq!(added["comment"]["text"], "hi");

    let ops = parse_input(json!({"op": "list comments", "task_id": task_id})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    let members = listed["comments"].as_array().unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(members[0]["text"], "hi");
}

/// A dispatching actor (the top-level `actor` key, which `parse_input`
/// lifts onto `op.actor`) is forwarded to `AddComment` and attributed on
/// the resulting member.
#[tokio::test]
async fn dispatch_add_comment_attributes_dispatching_actor() {
    let (_temp, ctx) = setup().await;
    let task_id = add_one_task(&ctx, "Actor attribution").await;

    let ops = parse_input(json!({"op": "add actor", "id": "alice", "name": "Alice"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({
        "op": "add comment",
        "task_id": task_id,
        "text": "from alice",
        "actor": "alice",
    }))
    .unwrap();
    let added = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(added["comment"]["actor"], "alice");
}

/// `get comment`, `update comment`, and `delete comment` all dispatch:
/// get returns the member projection, update edits the text in place,
/// delete removes the member from the log.
#[tokio::test]
async fn dispatch_comment_get_update_delete_round_trip() {
    let (_temp, ctx) = setup().await;
    let task_id = add_one_task(&ctx, "Edit comments").await;

    let ops =
        parse_input(json!({"op": "add comment", "task_id": task_id, "text": "original"})).unwrap();
    let added = execute_operation(&ctx, &ops[0]).await.unwrap();
    let comment_id = added["comment"]["id"].as_str().unwrap().to_string();

    let ops =
        parse_input(json!({"op": "get comment", "task_id": task_id, "id": comment_id})).unwrap();
    let got = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(got["text"], "original");
    assert_eq!(got["id"].as_str().unwrap(), comment_id);

    let ops = parse_input(json!({
        "op": "update comment",
        "task_id": task_id,
        "id": comment_id,
        "text": "edited",
    }))
    .unwrap();
    let updated = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(updated["ok"], true);
    assert_eq!(updated["id"].as_str().unwrap(), task_id);

    let ops =
        parse_input(json!({"op": "get comment", "task_id": task_id, "id": comment_id})).unwrap();
    let got = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(got["text"], "edited");

    let ops =
        parse_input(json!({"op": "delete comment", "task_id": task_id, "id": comment_id})).unwrap();
    let deleted = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(deleted["ok"], true);

    let ops = parse_input(json!({"op": "list comments", "task_id": task_id})).unwrap();
    let listed = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(listed["comments"].as_array().unwrap().len(), 0);
}
