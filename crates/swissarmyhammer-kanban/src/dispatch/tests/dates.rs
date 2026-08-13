//! The `due` and `scheduled` fields.
//!
//! These tests hold the dates that dispatch accepts, the invalid dates that it
//! refuses, the null that clears a date, and the date fields that get task and
//! list tasks give back.

use super::*;

// -----------------------------------------------------------------------
// Date field dispatch tests
// -----------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_task_accepts_due_and_scheduled() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Dated task",
        "due": "2026-04-30",
        "scheduled": "2026-04-15",
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(result["due"], "2026-04-30");
    assert_eq!(result["scheduled"], "2026-04-15");
}

#[tokio::test]
async fn dispatch_add_task_rejects_invalid_date() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Bad date",
        "due": "not-a-date",
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(result.is_err(), "invalid due must be rejected");
}

#[tokio::test]
async fn dispatch_add_task_rejects_non_string_date() {
    // Non-string JSON values for `due` must not silently vanish — they need
    // to produce a clear downstream parse error, mirroring the behaviour of
    // `dispatch_update_task`. Otherwise a caller that accidentally sends
    // `42` or `true` would silently get no date set with no feedback.
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Bad date type",
        "due": 42,
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        result.is_err(),
        "non-string due must be rejected, got: {result:?}"
    );
    let err = result.unwrap_err().to_string();
    assert!(
        err.to_lowercase().contains("due"),
        "error should mention the failing field, got: {err}"
    );
}

#[tokio::test]
async fn dispatch_add_task_rejects_non_string_scheduled() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task",
        "title": "Bad scheduled type",
        "scheduled": true,
    }))
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;
    assert!(
        result.is_err(),
        "non-string scheduled must be rejected, got: {result:?}"
    );
}

#[tokio::test]
async fn dispatch_update_task_sets_due() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({"op": "add task", "title": "Set due"})).unwrap();
    let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
    let id = add["id"].as_str().unwrap();

    let update_ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "due": "2026-05-01",
    }))
    .unwrap();
    execute_operation(&ctx, &update_ops[0]).await.unwrap();
    assert_eq!(get_task(&ctx, id).await["due"], "2026-05-01");
}

#[tokio::test]
async fn dispatch_update_task_clears_due_with_null() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({
        "op": "add task",
        "title": "Clear me",
        "due": "2026-05-01",
    }))
    .unwrap();
    let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
    let id = add["id"].as_str().unwrap();
    assert_eq!(add["due"], "2026-05-01");

    let update_ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "due": null,
    }))
    .unwrap();
    execute_operation(&ctx, &update_ops[0]).await.unwrap();
    assert!(
        get_task(&ctx, id).await["due"].is_null(),
        "due must be null after clearing via null"
    );
}

#[tokio::test]
async fn dispatch_update_task_clears_scheduled_with_empty_string() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({
        "op": "add task",
        "title": "Clear me",
        "scheduled": "2026-05-01",
    }))
    .unwrap();
    let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
    let id = add["id"].as_str().unwrap();

    let update_ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "scheduled": "",
    }))
    .unwrap();
    execute_operation(&ctx, &update_ops[0]).await.unwrap();
    assert!(
        get_task(&ctx, id).await["scheduled"].is_null(),
        "scheduled must be null after clearing via empty string"
    );
}

#[tokio::test]
async fn dispatch_update_task_ignores_missing_date_fields() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({
        "op": "add task",
        "title": "Keep my date",
        "due": "2026-05-01",
    }))
    .unwrap();
    let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
    let id = add["id"].as_str().unwrap();

    // Update a different field; date must be preserved.
    let update_ops = parse_input(json!({
        "op": "update task",
        "id": id,
        "title": "New title",
    }))
    .unwrap();
    execute_operation(&ctx, &update_ops[0]).await.unwrap();
    let task = get_task(&ctx, id).await;
    assert_eq!(task["title"], "New title");
    assert_eq!(
        task["due"], "2026-05-01",
        "missing date param must not touch the field"
    );
}

#[tokio::test]
async fn dispatch_get_task_emits_all_date_fields() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({
        "op": "add task",
        "title": "All dates",
        "due": "2026-05-01",
        "scheduled": "2026-04-15",
    }))
    .unwrap();
    let add = execute_operation(&ctx, &add_ops[0]).await.unwrap();
    let id = add["id"].as_str().unwrap();

    let get_ops = parse_input(json!({"op": "get task", "id": id})).unwrap();
    let result = execute_operation(&ctx, &get_ops[0]).await.unwrap();

    assert_eq!(result["due"], "2026-05-01");
    assert_eq!(result["scheduled"], "2026-04-15");
    // System dates are populated by the changelog-backed derivations.
    assert!(
        result["created"].is_string(),
        "created must be populated after write"
    );
    assert!(
        result["updated"].is_string(),
        "updated must be populated after write"
    );
}

#[tokio::test]
async fn dispatch_list_tasks_emits_date_fields() {
    let (_temp, ctx) = setup().await;

    let add_ops = parse_input(json!({
        "op": "add task",
        "title": "In list",
        "due": "2026-05-01",
    }))
    .unwrap();
    execute_operation(&ctx, &add_ops[0]).await.unwrap();

    let list_ops = parse_input(json!({"op": "list tasks"})).unwrap();
    let result = execute_operation(&ctx, &list_ops[0]).await.unwrap();

    let tasks = result["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["due"], "2026-05-01");
    assert!(tasks[0]["scheduled"].is_null());
    assert!(tasks[0].get("created").is_some());
}
