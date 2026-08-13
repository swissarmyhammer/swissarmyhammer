//! The board and the column operations.
//!
//! These tests hold `update board`, the column CRUD operations, the `column`
//! alias, the column order, the board description, and the `include_counts`
//! parameter.

use super::*;

// Board operations
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_update_board() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(
        json!({"op": "update board", "name": "Updated Board", "description": "A description"}),
    )
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Updated Board");
    assert_eq!(result["description"], "A description");
}

// ------------------------------------------------------------------
// Column operations
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_column() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add column", "id": "qa", "name": "QA"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"], "qa");
    assert_eq!(result["name"], "QA");
}

#[tokio::test]
async fn dispatch_get_column() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "get column", "id": "todo"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"], "todo");
}

#[tokio::test]
async fn dispatch_get_column_accepts_column_alias() {
    let (_temp, ctx) = setup().await;

    // `column` is the natural field name for a column id; it must be
    // accepted as an alias for `id` and return the identical result.
    let by_alias = execute_operation(
        &ctx,
        &parse_input(json!({"op": "get column", "column": "todo"})).unwrap()[0],
    )
    .await
    .unwrap();
    let by_id = execute_operation(
        &ctx,
        &parse_input(json!({"op": "get column", "id": "todo"})).unwrap()[0],
    )
    .await
    .unwrap();
    assert_eq!(by_alias["id"], "todo");
    assert_eq!(by_alias, by_id);
}

#[tokio::test]
async fn dispatch_get_column_missing_field_names_both_aliases() {
    let (_temp, ctx) = setup().await;

    // Neither `id` nor `column` present → parse error naming both.
    let op = crate::types::Operation::new(
        crate::types::Verb::Get,
        crate::types::Noun::Column,
        serde_json::Map::new(),
    );
    let err = execute_operation(&ctx, &op).await.unwrap_err().to_string();
    assert!(err.contains("id"), "error should name `id`: {err}");
    assert!(err.contains("column"), "error should name `column`: {err}");
}

#[tokio::test]
async fn dispatch_update_column_accepts_column_alias() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "update column", "column": "todo", "name": "Backlog"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Backlog");
}

#[tokio::test]
async fn dispatch_delete_column_accepts_column_alias() {
    let (_temp, ctx) = setup().await;

    // Add a new empty column then delete it via the `column` alias.
    let ops = parse_input(json!({"op": "add column", "id": "temp", "name": "Temp"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "delete column", "column": "temp"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["deleted"], true);
}

#[tokio::test]
async fn dispatch_update_column() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "update column", "id": "todo", "name": "Backlog"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Backlog");
}

#[tokio::test]
async fn dispatch_delete_column() {
    let (_temp, ctx) = setup().await;

    // Add a new empty column then delete it
    let ops = parse_input(json!({"op": "add column", "id": "temp", "name": "Temp"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "delete column", "id": "temp"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["deleted"], true);
}

#[tokio::test]
async fn dispatch_list_columns() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "list columns"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    let columns = result["columns"].as_array().unwrap();
    let ids: Vec<&str> = columns.iter().filter_map(|c| c["id"].as_str()).collect();

    // Derive the expected columns from the single source of truth
    // (`default_column_entities`) rather than hardcoding ids, so this
    // test can never drift from the default set (e.g. when `review`
    // was added between `doing` and `done`).
    let expected = crate::types::default_column_entities();
    assert!(columns.len() >= expected.len());
    for col in &expected {
        assert!(
            ids.contains(&col.id.as_str()),
            "default column `{}` missing from list columns result; got {ids:?}",
            col.id
        );
    }
}

// ------------------------------------------------------------------
// Dispatch: column with order
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_column_with_order() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "add column", "id": "qa", "name": "QA", "order": 1})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"], "qa");
    assert_eq!(result["order"], 1);
}

#[tokio::test]
async fn dispatch_update_column_with_order() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "update column", "id": "todo", "order": 5})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["order"], 5);
}

// ------------------------------------------------------------------
// Dispatch: init board with description
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_init_board_with_description() {
    let temp = TempDir::new().unwrap();
    let kanban_dir = temp.path().join(".kanban");
    let ctx = KanbanContext::new(kanban_dir);

    let ops = parse_input(
        json!({"op": "init board", "name": "Described Board", "description": "A nice board"}),
    )
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Described Board");
    assert_eq!(result["description"], "A nice board");
}

// ------------------------------------------------------------------
// Dispatch: get board with include_counts=false
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_get_board_without_counts() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "get board", "include_counts": false})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Test");
}
