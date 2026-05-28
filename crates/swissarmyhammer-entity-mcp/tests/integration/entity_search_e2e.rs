//! End-to-end tests for the `entity` server's `Search` op.
//!
//! The `Search` verb builds an `EntitySearchIndex` from the kernel's live
//! entities on each call, so a write made through the same server is
//! immediately searchable. These tests add entities of two types (`task` and
//! `tag`) through the server, then exercise both the unfiltered query and the
//! `type`-narrowed query.

use serde_json::json;

use super::common::{call_tool, Harness};

/// Pull the list of `(id, type)` pairs out of a `search entities` response,
/// preserving best-match-first order.
fn result_pairs(value: &serde_json::Value) -> Vec<(String, String)> {
    value["results"]
        .as_array()
        .expect("results is an array")
        .iter()
        .map(|hit| {
            (
                hit["id"].as_str().expect("hit id is a string").to_string(),
                hit["type"]
                    .as_str()
                    .expect("hit type is a string")
                    .to_string(),
            )
        })
        .collect()
}

/// Seed the kernel with two tasks and one tag through the server so every
/// `Search` call indexes live, on-disk entities.
async fn seed(server: &swissarmyhammer_entity_mcp::EntityServer) {
    call_tool(
        server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "task",
            "id": "t1",
            "fields": { "title": "Fix the login page", "body": "auth bug" },
        }),
    )
    .await
    .unwrap();

    call_tool(
        server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "task",
            "id": "t2",
            "fields": { "title": "Add dashboard widgets", "body": "" },
        }),
    )
    .await
    .unwrap();

    // A tag whose name also mentions "login" so a type filter is observable.
    call_tool(
        server,
        "add entity",
        json!({
            "op": "add entity",
            "type": "tag",
            "id": "login-tag",
            "fields": { "tag_name": "login", "color": "#0000ff" },
        }),
    )
    .await
    .unwrap();
}

/// `Search { query }` (no type filter) finds entities of any type by text.
#[tokio::test]
async fn search_finds_by_text_across_types() {
    let h = Harness::new().await;
    let server = h.server();
    seed(&server).await;

    let found = call_tool(
        &server,
        "search entities",
        json!({ "op": "search entities", "query": "login" }),
    )
    .await
    .unwrap();

    assert_eq!(found["ok"], json!(true));
    let pairs = result_pairs(&found);
    let ids: Vec<&str> = pairs.iter().map(|(id, _)| id.as_str()).collect();

    // The login task and the login tag both match; the dashboard task does not.
    assert!(ids.contains(&"t1"), "login task should match: {ids:?}");
    assert!(ids.contains(&"login-tag"), "login tag should match: {ids:?}");
    assert!(
        !ids.contains(&"t2"),
        "unrelated dashboard task must not match: {ids:?}"
    );

    // Every hit carries the originating entity and its type.
    let login_task = pairs.iter().find(|(id, _)| id == "t1").unwrap();
    assert_eq!(login_task.1, "task");
}

/// `Search { query, type }` narrows results to the named type only — the
/// matching tag is excluded when the filter pins `task`.
#[tokio::test]
async fn search_type_filter_narrows_results() {
    let h = Harness::new().await;
    let server = h.server();
    seed(&server).await;

    let found = call_tool(
        &server,
        "search entities",
        json!({ "op": "search entities", "query": "login", "type": "task" }),
    )
    .await
    .unwrap();

    let pairs = result_pairs(&found);
    assert!(
        pairs.iter().all(|(_, ty)| ty == "task"),
        "type filter must exclude non-task hits: {pairs:?}"
    );
    let ids: Vec<&str> = pairs.iter().map(|(id, _)| id.as_str()).collect();
    assert!(ids.contains(&"t1"), "login task still matches: {ids:?}");
    assert!(
        !ids.contains(&"login-tag"),
        "login tag must be filtered out by type=task: {ids:?}"
    );
}

/// A query that matches nothing returns an empty result set, not an error.
#[tokio::test]
async fn search_no_match_returns_empty() {
    let h = Harness::new().await;
    let server = h.server();
    seed(&server).await;

    let found = call_tool(
        &server,
        "search entities",
        json!({ "op": "search entities", "query": "zzzznevermatches" }),
    )
    .await
    .unwrap();

    assert_eq!(found["ok"], json!(true));
    assert_eq!(found["results"].as_array().unwrap().len(), 0);
}
