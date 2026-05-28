//! End-to-end tests for the `entity` MCP server's generic CRUD + archive
//! verbs.
//!
//! Builds a kernel wired for two entity types (`tag` plain-YAML, `task`
//! frontmatter+body), wraps it in an `EntityServer`, and exercises every
//! verb the `_meta` tree advertises: add → get → update field → delete, plus
//! an archive → unarchive round-trip.

use serde_json::json;

use super::common::{call_tool, Harness};

/// Full CRUD lifecycle across two distinct entity types proves the verbs are
/// type-agnostic: the same ops route the `tag` (`.yaml`) and `task` (`.md`)
/// types through the one kernel.
#[tokio::test]
async fn add_get_update_delete_across_two_types() {
    let h = Harness::new().await;
    let server = h.server();

    // --- tag (plain YAML) -------------------------------------------------
    let added = call_tool(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "tag",
            "id": "blue",
            "fields": { "tag_name": "Blue", "color": "#0000ff" },
        }),
    )
    .await
    .unwrap();
    assert_eq!(added["ok"], json!(true));
    assert_eq!(added["id"], json!("blue"));
    // A registered store handle produces an undo entry.
    assert!(added["entry_id"].is_string(), "write should be undoable");

    // The file landed on disk as YAML.
    assert!(h.dir.path().join("tags/blue.yaml").exists());

    let got = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "tag", "id": "blue" }),
    )
    .await
    .unwrap();
    assert_eq!(got["entity"]["tag_name"], json!("Blue"));
    assert_eq!(got["entity"]["color"], json!("#0000ff"));
    assert_eq!(got["entity"]["entity_type"], json!("tag"));

    // --- task (frontmatter + body) ---------------------------------------
    let added_task = call_tool(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "task",
            "id": "t1",
            "fields": { "title": "Write docs", "body": "Initial body" },
        }),
    )
    .await
    .unwrap();
    assert_eq!(added_task["id"], json!("t1"));
    assert!(h.dir.path().join("tasks/t1.md").exists());

    // Update a single field; read it back through a fresh get.
    let updated = call_tool(
        &server,
        "update field",
        json!({
            "op": "update field",
            "type": "task",
            "id": "t1",
            "field": "title",
            "value": "Write better docs",
        }),
    )
    .await
    .unwrap();
    assert_eq!(updated["ok"], json!(true));
    assert_eq!(updated["id"], json!("t1"));

    let got_task = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "task", "id": "t1" }),
    )
    .await
    .unwrap();
    assert_eq!(got_task["entity"]["title"], json!("Write better docs"));
    assert_eq!(got_task["entity"]["body"], json!("Initial body"));

    // list returns both live entities (one per type).
    let listed_tags = call_tool(
        &server,
        "list entities",
        json!({ "op": "list entities", "type": "tag" }),
    )
    .await
    .unwrap();
    assert_eq!(listed_tags["entities"].as_array().unwrap().len(), 1);

    // Delete the tag; it disappears from disk and from list.
    let deleted = call_tool(
        &server,
        "delete entity",
        json!({ "op": "delete entity", "type": "tag", "id": "blue" }),
    )
    .await
    .unwrap();
    assert_eq!(deleted["ok"], json!(true));
    assert!(!h.dir.path().join("tags/blue.yaml").exists());

    let listed_after = call_tool(
        &server,
        "list entities",
        json!({ "op": "list entities", "type": "tag" }),
    )
    .await
    .unwrap();
    assert_eq!(listed_after["entities"].as_array().unwrap().len(), 0);
}

/// Archive moves an entity out of `list`; unarchive brings it back. Both
/// steps round-trip through the kernel's `archive` / `unarchive` methods.
#[tokio::test]
async fn archive_unarchive_round_trip() {
    let h = Harness::new().await;
    let server = h.server();

    call_tool(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "tag",
            "id": "green",
            "fields": { "tag_name": "Green", "color": "#00ff00" },
        }),
    )
    .await
    .unwrap();

    // Archive — entity leaves the live list.
    let archived = call_tool(
        &server,
        "archive entity",
        json!({ "op": "archive entity", "type": "tag", "id": "green" }),
    )
    .await
    .unwrap();
    assert_eq!(archived["ok"], json!(true));

    let listed = call_tool(
        &server,
        "list entities",
        json!({ "op": "list entities", "type": "tag" }),
    )
    .await
    .unwrap();
    assert_eq!(
        listed["entities"].as_array().unwrap().len(),
        0,
        "archived entity must not appear in the live list"
    );

    // Unarchive — entity returns to the live list and is readable again.
    let unarchived = call_tool(
        &server,
        "unarchive entity",
        json!({ "op": "unarchive entity", "type": "tag", "id": "green" }),
    )
    .await
    .unwrap();
    assert_eq!(unarchived["ok"], json!(true));

    let got = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "tag", "id": "green" }),
    )
    .await
    .unwrap();
    assert_eq!(got["entity"]["tag_name"], json!("Green"));
}

/// An unknown entity type surfaces a structured error carrying the type.
#[tokio::test]
async fn unknown_entity_type_returns_structured_error() {
    let h = Harness::new().await;
    let server = h.server();

    let err = call_tool(
        &server,
        "list entities",
        json!({ "op": "list entities", "type": "nonexistent" }),
    )
    .await
    .expect_err("unknown entity type should error");

    let data = err.data.as_ref().expect("error carries structured data");
    assert_eq!(data["type"], json!("nonexistent"));
}

/// `add entity` without an explicit id mints a fresh ULID.
#[tokio::test]
async fn add_without_id_mints_ulid() {
    let h = Harness::new().await;
    let server = h.server();

    let added = call_tool(
        &server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "tag",
            "fields": { "tag_name": "Auto", "color": "#abcdef" },
        }),
    )
    .await
    .unwrap();
    let id = added["id"].as_str().expect("minted id is a string");
    assert_eq!(id.len(), 26, "minted id should be a 26-char ULID");

    let got = call_tool(
        &server,
        "get entity",
        json!({ "op": "get entity", "type": "tag", "id": id }),
    )
    .await
    .unwrap();
    assert_eq!(got["entity"]["tag_name"], json!("Auto"));
}
