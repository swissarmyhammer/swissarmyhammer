//! The actor and the board-level tag operations.
//!
//! These tests hold the actor CRUD operations and the `ensure` parameter. They
//! also hold the tag CRUD operations with their color and their description.

use super::*;

// ------------------------------------------------------------------
// Actor operations
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_actor() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(
        json!({"op": "add actor", "id": "alice", "name": "Alice Smith", "type": "human"}),
    )
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    // AddActor wraps the actor under an "actor" key
    assert_eq!(result["actor"]["id"], "alice");
    assert_eq!(result["actor"]["name"], "Alice Smith");
    assert_eq!(result["created"], true);
}

#[tokio::test]
async fn dispatch_get_actor() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "add actor", "id": "bob", "name": "Bob Jones", "type": "human"}))
            .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "get actor", "id": "bob"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["id"], "bob");
    assert_eq!(result["name"], "Bob Jones");
}

#[tokio::test]
async fn dispatch_update_actor() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "add actor", "id": "carol", "name": "Carol", "type": "human"}))
            .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops =
        parse_input(json!({"op": "update actor", "id": "carol", "name": "Carol Updated"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "Carol Updated");
}

#[tokio::test]
async fn dispatch_delete_actor() {
    let (_temp, ctx) = setup().await;

    let ops =
        parse_input(json!({"op": "add actor", "id": "dave", "name": "Dave", "type": "human"}))
            .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "delete actor", "id": "dave"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["deleted"], true);
}

#[tokio::test]
async fn dispatch_list_actors() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add actor", "id": "eve", "name": "Eve", "type": "human"}))
        .unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "list actors"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    let actors = result["actors"].as_array().unwrap();
    let ids: Vec<&str> = actors.iter().filter_map(|a| a["id"].as_str()).collect();
    assert!(ids.contains(&"eve"));
}

// ------------------------------------------------------------------
// Tag operations (board-level)
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_tag() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "urgent"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "urgent");
}

#[tokio::test]
async fn dispatch_get_tag() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "blocker"})).unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tag_id = r["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "get tag", "id": tag_id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "blocker");
}

#[tokio::test]
async fn dispatch_update_tag() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "old-tag"})).unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tag_id = r["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "update tag", "id": tag_id, "name": "new-tag"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "new-tag");
}

#[tokio::test]
async fn dispatch_delete_tag() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "remove-me"})).unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tag_id = r["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "delete tag", "id": tag_id})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["deleted"], true);
}

#[tokio::test]
async fn dispatch_list_tags() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "mytag"})).unwrap();
    execute_operation(&ctx, &ops[0]).await.unwrap();

    let ops = parse_input(json!({"op": "list tags"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tags = result["tags"].as_array().unwrap();
    let names: Vec<&str> = tags.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(names.contains(&"mytag"));
}

// ------------------------------------------------------------------
// Dispatch: tag with optional fields
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_tag_with_color() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "red-tag", "color": "ff0000"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "red-tag");
    assert_eq!(result["color"], "ff0000");
}

#[tokio::test]
async fn dispatch_add_tag_with_description() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(
        json!({"op": "add tag", "name": "documented", "description": "A documented tag"}),
    )
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "documented");
    assert_eq!(result["description"], "A documented tag");
}

#[tokio::test]
async fn dispatch_add_tag_by_id_field() {
    let (_temp, ctx) = setup().await;

    // The dispatch code also accepts "id" as a fallback for "name"
    let ops = parse_input(json!({"op": "add tag", "id": "id-based-tag"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["name"], "id-based-tag");
}

#[tokio::test]
async fn dispatch_update_tag_with_color() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "colorful"})).unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tag_id = r["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "update tag", "id": tag_id, "color": "00ff00"})).unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["color"], "00ff00");
}

#[tokio::test]
async fn dispatch_update_tag_with_description() {
    let (_temp, ctx) = setup().await;

    let ops = parse_input(json!({"op": "add tag", "name": "desc-tag"})).unwrap();
    let r = execute_operation(&ctx, &ops[0]).await.unwrap();
    let tag_id = r["id"].as_str().unwrap().to_string();

    let ops = parse_input(json!({"op": "update tag", "id": tag_id, "description": "Updated desc"}))
        .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["description"], "Updated desc");
}

// ------------------------------------------------------------------
// Dispatch: actor with ensure
// ------------------------------------------------------------------

#[tokio::test]
async fn dispatch_add_actor_with_ensure() {
    let (_temp, ctx) = setup().await;

    // First add
    let ops =
        parse_input(json!({"op": "add actor", "id": "ensured", "name": "Ensured", "ensure": true}))
            .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["actor"]["id"], "ensured");
    assert_eq!(result["created"], true);

    // Second add with ensure should not fail
    let ops = parse_input(
        json!({"op": "add actor", "id": "ensured", "name": "Ensured Again", "ensure": true}),
    )
    .unwrap();
    let result = execute_operation(&ctx, &ops[0]).await.unwrap();
    assert_eq!(result["actor"]["id"], "ensured");
    assert_eq!(result["created"], false);
}
