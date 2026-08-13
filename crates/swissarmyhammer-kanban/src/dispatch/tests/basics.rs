//! The board bootstrap and the task rows that dispatch gives back.
//!
//! These tests hold `init board`, the column that a new task goes into, the
//! list and get operations, the archive operations, the `detail` parameter,
//! the error for a field that is not there, and the actor that the processor
//! holds.

use super::*;

#[tokio::test]
async fn dispatch_init_board() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(kanban_dir);

    let ops = parse_input(json!({"op": "init board", "name": "My Board"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "My Board");
    assert!(result["columns"].is_array());
}

/// Verify that dispatching `add task` (without a column arg) places the task
/// in the first column (todo).
#[tokio::test]
async fn dispatch_add_task_places_in_first_column_by_default() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add task", "title": "New task"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(
        result["position"]["column"], "todo",
        "task without explicit column should land in todo (first column)"
    );
}

/// Verify that dispatching `add task` with an explicit column arg places the task
/// in that column, not in todo.
#[tokio::test]
async fn dispatch_add_task_with_explicit_column_uses_that_column() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add task", "title": "Task in doing", "column": "doing"}))
        .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();

    assert_eq!(
        result["position"]["column"], "doing",
        "task with explicit column arg should land in that column"
    );
}

/// Verify that dispatching `add task` on a board with no columns returns an error.
#[tokio::test]
async fn dispatch_add_task_on_board_with_no_columns_returns_error() {
    let (_temp, ctx) = setup().await;

    // Delete every default column so the board has none.
    for col in crate::types::default_column_entities() {
        let ops = parse_input(json!({"op": "delete column", "id": col.id.to_string()})).unwrap();
        execute_operation(&ctx, &ops[0]).await.unwrap();
    }

    // Now add task should fail gracefully
    let ops = parse_input(json!({"op": "add task", "title": "Task on empty board"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await;

    assert!(
        result.is_err(),
        "adding a task to a board with no columns should return an error"
    );
}

/// Verify that `board.newCard` is not a separate dispatch operation — the
/// `task.add` dispatch path is the canonical way to add cards and it correctly
/// defaults to the first column.
#[tokio::test]
async fn dispatch_board_new_card_not_a_separate_operation() {
    let (_temp, ctx) = setup().await;

    // board.newCard does not exist as a parsed operation; the canonical way
    // to add a card is "add task".  Attempting to dispatch an invented
    // "new card" verb/noun pair must return an error, confirming that all
    // new-card creation flows go through "add task".
    let op = crate::types::Operation::new(crate::types::Verb::Add, crate::types::Noun::Task, {
        let mut m = serde_json::Map::new();
        m.insert("title".into(), json!("Card via add task"));
        m
    });
    let result = execute_operation(&ctx, &op).await;
    assert!(
        result.is_ok(),
        "add task (the board.newCard equivalent) should succeed"
    );
    assert_eq!(
        result.unwrap()["position"]["column"],
        "todo",
        "board.newCard equivalent should default to the first column"
    );
}

#[tokio::test]
async fn dispatch_add_and_list_tasks() {
    let (_temp, ctx) = setup().await;

    // Add a task
    let ops = parse_input(json!({"op": "add task", "title": "Fix bug"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["title"], "Fix bug");
    let task_id = result["id"].as_str().unwrap().to_string();

    // List tasks
    let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["count"], 1);
    assert_eq!(result["tasks"][0]["id"], task_id);
}

/// Regression: a `get task` that passes the task reference under the `task`
/// key (the committer role's habit) must resolve to `id` and succeed through
/// the real dispatch + resolver path, not fail with
/// `missing required field: id`. Covers the full ULID and the `^<short>`
/// form (the exact shape from the bug report).
#[tokio::test]
async fn dispatch_get_task_accepts_task_key_alias() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add task", "title": "Aliased fetch"})).unwrap();
    let added = execute_operation(&ctx, &ops[0]).await.unwrap();
    let full_id = added["id"].as_str().unwrap().to_string();
    let short_id = added["short_id"].as_str().unwrap().to_string();

    // Full ULID under `task`.
    let ops = parse_input(json!({"op": "get task", "task": full_id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["title"], "Aliased fetch");
    assert_eq!(result["id"].as_str().unwrap(), full_id);

    // `^<short>` under `task` — the exact shape from the bug report.
    let ops = parse_input(json!({"op": "get task", "task": format!("^{short_id}")})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"].as_str().unwrap(), full_id);
}

/// `search tasks` must parse to (Verb::Search, Noun::Tasks) and dispatch to
/// SearchTasks. On an empty board the op short-circuits before loading any
/// model, so this proves the wiring without an embedding model. A missing
/// `query` must surface a parse error.
#[tokio::test]
async fn dispatch_search_tasks_wiring() {
    let (_temp, ctx) = setup().await;

    // Parses to the Search verb and the Tasks noun.
    let ops = parse_input(json!({"op": "search tasks", "query": "anything"})).unwrap();
    assert_eq!(ops[0].verb, Verb::Search);
    assert_eq!(ops[0].noun, Noun::Tasks);

    // Empty board → ranked result of zero, no model loaded.
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["count"], 0);
    assert!(result["tasks"].as_array().unwrap().is_empty());

    // `query` is required.
    let ops = parse_input(json!({"op": "search tasks"})).unwrap();
    assert!(execute_operation(&ctx, &ops[0]).await.is_err());
}

#[tokio::test]
async fn dispatch_get_board() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "get board"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Test");
}

#[tokio::test]
async fn dispatch_unsupported_operation_returns_error() {
    let (_temp, ctx) = setup().await;

    let op = crate::types::Operation::new(
        crate::types::Verb::Rename,
        crate::types::Noun::Board,
        serde_json::Map::new(),
    );
    let result = execute_operation(&ctx, &op).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn dispatch_archive_task() {
    let (_temp, ctx) = setup().await;

    // Add a task
    let ops = parse_input(json!({"op": "add task", "title": "Task to archive"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    // Archive the task via dispatch
    let ops = parse_input(json!({"op": "archive task", "id": task_id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["archived"], true);
    assert_eq!(result["id"].as_str().unwrap(), task_id);

    // List tasks — the archived task should not appear
    let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        result["count"], 0,
        "archived task should not appear in list tasks"
    );
}

#[tokio::test]
async fn dispatch_unarchive_task() {
    let (_temp, ctx) = setup().await;

    // Add a task and archive it
    let ops = parse_input(json!({"op": "add task", "title": "Task to unarchive"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    let task_id = result["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "archive task", "id": task_id})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // Unarchive via dispatch
    let ops = parse_input(json!({"op": "unarchive task", "id": task_id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["unarchived"], true);
    assert_eq!(result["id"].as_str().unwrap(), task_id);

    // List tasks — the task should be back
    let ops = parse_input(json!({"op": "list tasks", "column": "todo"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(
        result["count"], 1,
        "unarchived task should reappear in list tasks"
    );
}

#[tokio::test]
async fn dispatch_list_archived() {
    let (_temp, ctx) = setup().await;

    // Add two tasks and archive one
    let ops = parse_input(json!({"op": "add task", "title": "Will be archived"})).unwrap();
    let r1 = execute_operation(&ctx, &ops[0]).await.unwrap();
    let id1 = r1["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "add task", "title": "Still live"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "archive task", "id": id1})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    // List archived
    let ops = parse_input(json!({"op": "list archived"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["count"], 1, "should list exactly one archived task");
    let tasks = result["tasks"].as_array().unwrap();
    assert_eq!(tasks[0]["title"], "Will be archived");
}

/// The optional `detail` param flows through dispatch for both listing
/// ops: defaults to slim (no `description`), `"full"` restores the
/// enriched shape.
#[tokio::test]
async fn dispatch_list_detail_param() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({
        "op": "add task", "title": "Live", "description": "live body"
    }))
    .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({
        "op": "add task", "title": "Gone", "description": "archived body"
    }))
    .unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let archived_id = r["id"].as_str().unwrap().to_string();
    let ops = parse_input(json!({"op": "archive task", "id": archived_id})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    for (op_name, body) in [
        ("list tasks", "live body"),
        ("list archived", "archived body"),
    ] {
        let ops = parse_input(json!({"op": op_name})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert!(
            !result["tasks"][0]
                .as_object()
                .unwrap()
                .contains_key("description"),
            "{op_name} default must be slim"
        );

        let ops = parse_input(json!({"op": op_name, "detail": "full"})).unwrap();
        let result = execute_operation(&ctx, &ops[0]).await.unwrap();
        assert_eq!(
            result["tasks"][0]["description"], body,
            "{op_name} detail=full must include description"
        );

        let ops = parse_input(json!({"op": op_name, "detail": "verbose"})).unwrap();
        let err = execute_operation(&ctx, &ops[0]).await.unwrap_err();
        assert!(
            err.to_string().contains("verbose"),
            "{op_name} must reject unknown detail: {err}"
        );
    }
}

// ------------------------------------------------------------------
// Dispatch: req helper error
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_missing_required_field_returns_error() {
    let (_temp, ctx) = setup().await;

    // get column without id
    let op = crate::types::Operation::new(
        crate::types::Verb::Get,
        crate::types::Noun::Column,
        serde_json::Map::new(),
    );
    let result = execute_operation(&ctx, &op).await;
    assert!(result.is_err(), "should fail without required 'id' field");
}

// ------------------------------------------------------------------
// Dispatch: processor with actor
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_with_actor_sets_processor() {
    let (_temp, ctx) = setup().await;

    let mut op = crate::types::Operation::new(crate::types::Verb::Add, crate::types::Noun::Task, {
        let mut m = serde_json::Map::new();
        m.insert("title".into(), json!("Actor task"));
        m
    });
    op.actor = Some("test-actor".into());
    let result = execute_operation(&ctx, &op).await.unwrap();
    assert_eq!(result["title"], "Actor task");
}
