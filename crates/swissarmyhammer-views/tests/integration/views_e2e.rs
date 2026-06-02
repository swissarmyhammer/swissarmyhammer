//! End-to-end tests for the `views` MCP server, per sub-domain.
//!
//! Stands up a real `ViewsServer` over a `PerspectiveContext` + `ViewsContext`
//! wired to a shared `StoreContext`, then exercises each sub-domain: lifecycle
//! (save/load/list/rename/delete), filter, group, sort, navigation, and the
//! `set view` path. Mutations are observed through the persisted perspective /
//! view state the server reads back.

use serde_json::json;

use super::common::{call_tool, Harness};

/// Create a perspective via the server and return its id.
async fn save_perspective(h: &Harness, name: &str, view: &str) -> String {
    let server = h.server();
    let saved = call_tool(
        &server,
        "save perspective",
        json!({ "op": "save perspective", "name": name, "view": view }),
    )
    .await
    .unwrap();
    saved["perspective"]["id"].as_str().unwrap().to_string()
}

/// save → load round-trips the perspective, and the id resolves both ways.
#[tokio::test]
async fn save_then_load_round_trip() {
    let h = Harness::new().await;
    let server = h.server();

    let saved = call_tool(
        &server,
        "save perspective",
        json!({ "op": "save perspective", "name": "Sprint", "view": "board" }),
    )
    .await
    .unwrap();
    assert_eq!(saved["ok"], json!(true));
    let id = saved["perspective"]["id"].as_str().unwrap().to_string();
    assert!(saved["entry_id"].is_string(), "save should be undoable");

    // Load by name.
    let by_name = call_tool(
        &server,
        "load perspective",
        json!({ "op": "load perspective", "name": "Sprint" }),
    )
    .await
    .unwrap();
    assert_eq!(by_name["perspective"]["id"], json!(id));
    assert_eq!(by_name["perspective"]["view"], json!("board"));

    // Load by id.
    let by_id = call_tool(
        &server,
        "load perspective",
        json!({ "op": "load perspective", "name": id }),
    )
    .await
    .unwrap();
    assert_eq!(by_id["perspective"]["name"], json!("Sprint"));
}

/// list reflects saved perspectives; rename and delete mutate the set.
#[tokio::test]
async fn list_rename_delete_lifecycle() {
    let h = Harness::new().await;
    let server = h.server();

    let id = save_perspective(&h, "One", "grid").await;
    save_perspective(&h, "Two", "grid").await;

    let listed = call_tool(
        &server,
        "list perspective",
        json!({ "op": "list perspective" }),
    )
    .await
    .unwrap();
    assert_eq!(listed["count"], json!(2));

    // Rename.
    let renamed = call_tool(
        &server,
        "rename perspective",
        json!({ "op": "rename perspective", "id": id, "new_name": "Renamed" }),
    )
    .await
    .unwrap();
    assert_eq!(renamed["perspective"]["name"], json!("Renamed"));

    // Delete.
    let deleted = call_tool(
        &server,
        "delete perspective",
        json!({ "op": "delete perspective", "id": id }),
    )
    .await
    .unwrap();
    assert_eq!(deleted["ok"], json!(true));

    let after = call_tool(
        &server,
        "list perspective",
        json!({ "op": "list perspective" }),
    )
    .await
    .unwrap();
    assert_eq!(after["count"], json!(1));
}

/// set filter stores the expression; clear filter drops it.
#[tokio::test]
async fn set_and_clear_filter() {
    let h = Harness::new().await;
    let server = h.server();
    let id = save_perspective(&h, "Filtered", "board").await;

    let set = call_tool(
        &server,
        "set filter",
        json!({ "op": "set filter", "perspective_id": id, "filter": "#bug && @will" }),
    )
    .await
    .unwrap();
    assert_eq!(set["perspective"]["filter"], json!("#bug && @will"));

    // Re-read via load to confirm persistence.
    let loaded = call_tool(
        &server,
        "load perspective",
        json!({ "op": "load perspective", "name": id }),
    )
    .await
    .unwrap();
    assert_eq!(loaded["perspective"]["filter"], json!("#bug && @will"));

    let cleared = call_tool(
        &server,
        "clear filter",
        json!({ "op": "clear filter", "perspective_id": id }),
    )
    .await
    .unwrap();
    assert!(cleared["perspective"]["filter"].is_null());
}

/// focus filter is a UI-only no-op that still reports success.
#[tokio::test]
async fn focus_filter_is_noop() {
    let h = Harness::new().await;
    let server = h.server();

    let res = call_tool(&server, "focus filter", json!({ "op": "focus filter" }))
        .await
        .unwrap();
    assert_eq!(res["ok"], json!(true));
}

/// set group stores the field; clear group drops it.
#[tokio::test]
async fn set_and_clear_group() {
    let h = Harness::new().await;
    let server = h.server();
    let id = save_perspective(&h, "Grouped", "grid").await;

    let set = call_tool(
        &server,
        "set group",
        json!({ "op": "set group", "perspective_id": id, "group": "status" }),
    )
    .await
    .unwrap();
    assert_eq!(set["perspective"]["group"], json!("status"));

    let cleared = call_tool(
        &server,
        "clear group",
        json!({ "op": "clear group", "perspective_id": id }),
    )
    .await
    .unwrap();
    assert!(cleared["perspective"]["group"].is_null());
}

/// set sort appends/replaces; toggle cycles asc→desc→none; clear empties.
#[tokio::test]
async fn sort_set_toggle_clear() {
    let h = Harness::new().await;
    let server = h.server();
    let id = save_perspective(&h, "Sorted", "grid").await;

    // Set ascending.
    let set = call_tool(
        &server,
        "set sort",
        json!({ "op": "set sort", "perspective_id": id, "field": "title", "direction": "asc" }),
    )
    .await
    .unwrap();
    let sort = set["perspective"]["sort"].as_array().unwrap();
    assert_eq!(sort.len(), 1);
    assert_eq!(sort[0]["field"], json!("title"));
    assert_eq!(sort[0]["direction"], json!("asc"));

    // Set the same field descending — replaces, not appends.
    let replaced = call_tool(
        &server,
        "set sort",
        json!({ "op": "set sort", "perspective_id": id, "field": "title", "direction": "desc" }),
    )
    .await
    .unwrap();
    let sort = replaced["perspective"]["sort"].as_array().unwrap();
    assert_eq!(sort.len(), 1);
    assert_eq!(sort[0]["direction"], json!("desc"));

    // Toggle title: desc -> none (removed).
    let toggled = call_tool(
        &server,
        "toggle sort",
        json!({ "op": "toggle sort", "perspective_id": id, "field": "title" }),
    )
    .await
    .unwrap();
    assert!(
        toggled["perspective"]["sort"]
            .as_array()
            .map(|a| a.is_empty())
            .unwrap_or(true),
        "toggle from desc should remove the entry"
    );

    // Toggle priority: none -> asc.
    let toggled2 = call_tool(
        &server,
        "toggle sort",
        json!({ "op": "toggle sort", "perspective_id": id, "field": "priority" }),
    )
    .await
    .unwrap();
    let sort = toggled2["perspective"]["sort"].as_array().unwrap();
    assert_eq!(sort[0]["field"], json!("priority"));
    assert_eq!(sort[0]["direction"], json!("asc"));

    // Clear everything.
    let cleared = call_tool(
        &server,
        "clear sort",
        json!({ "op": "clear sort", "perspective_id": id }),
    )
    .await
    .unwrap();
    assert!(cleared["perspective"]["sort"]
        .as_array()
        .map(|a| a.is_empty())
        .unwrap_or(true));
}

/// An invalid sort direction is rejected with invalid_params.
#[tokio::test]
async fn set_sort_rejects_bad_direction() {
    let h = Harness::new().await;
    let server = h.server();
    let id = save_perspective(&h, "Bad", "grid").await;

    let err = call_tool(
        &server,
        "set sort",
        json!({ "op": "set sort", "perspective_id": id, "field": "title", "direction": "sideways" }),
    )
    .await
    .expect_err("bad direction should error");
    assert!(err.message.contains("invalid sort direction"));
}

/// next/prev cycle through perspectives matching a view kind, wrapping.
#[tokio::test]
async fn nav_next_prev_goto_switch() {
    let h = Harness::new().await;
    let server = h.server();

    let a = save_perspective(&h, "A", "board").await;
    let b = save_perspective(&h, "B", "board").await;
    let c = save_perspective(&h, "C", "board").await;

    // next from A -> B.
    let next = call_tool(
        &server,
        "next perspective",
        json!({ "op": "next perspective", "view": "board", "current": a }),
    )
    .await
    .unwrap();
    assert_eq!(next["perspective"]["id"], json!(b));

    // next from C wraps to A.
    let wrap = call_tool(
        &server,
        "next perspective",
        json!({ "op": "next perspective", "view": "board", "current": c }),
    )
    .await
    .unwrap();
    assert_eq!(wrap["perspective"]["id"], json!(a));

    // prev from A wraps to C.
    let prev = call_tool(
        &server,
        "prev perspective",
        json!({ "op": "prev perspective", "view": "board", "current": a }),
    )
    .await
    .unwrap();
    assert_eq!(prev["perspective"]["id"], json!(c));

    // goto by id returns it.
    let goto = call_tool(
        &server,
        "goto perspective",
        json!({ "op": "goto perspective", "id": b }),
    )
    .await
    .unwrap();
    assert_eq!(goto["perspective"]["id"], json!(b));

    // switch returns perspective + its (empty) filter.
    let switch = call_tool(
        &server,
        "switch perspective",
        json!({ "op": "switch perspective", "perspective_id": c }),
    )
    .await
    .unwrap();
    assert_eq!(switch["perspective"]["id"], json!(c));
    assert_eq!(switch["filter"], json!(""));
}

/// next is a no-op (null perspective) when fewer than two match the view.
#[tokio::test]
async fn nav_next_noop_with_single_match() {
    let h = Harness::new().await;
    let server = h.server();
    let only = save_perspective(&h, "Solo", "board").await;

    let res = call_tool(
        &server,
        "next perspective",
        json!({ "op": "next perspective", "view": "board", "current": only }),
    )
    .await
    .unwrap();
    assert!(res["perspective"].is_null());
}

/// goto validates view membership and errors on mismatch.
#[tokio::test]
async fn goto_rejects_view_mismatch() {
    let h = Harness::new().await;
    let server = h.server();
    let id = save_perspective(&h, "Board persp", "board").await;

    let err = call_tool(
        &server,
        "goto perspective",
        json!({ "op": "goto perspective", "id": id, "view": "grid" }),
    )
    .await
    .expect_err("view mismatch should error");
    assert!(err.message.contains("does not belong to view"));
}

/// set view creates a view def and writes it to disk; minted id round-trips.
#[tokio::test]
async fn set_view_creates_and_persists() {
    let h = Harness::new().await;
    let server = h.server();

    let saved = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "name": "My Grid",
            "kind": "grid",
            "entity_type": "task",
            "card_fields": ["title", "status"],
        }),
    )
    .await
    .unwrap();
    assert_eq!(saved["ok"], json!(true));
    assert_eq!(saved["view"]["kind"], json!("grid"));
    assert_eq!(saved["view"]["name"], json!("My Grid"));
    assert!(saved["entry_id"].is_string(), "set view should be undoable");

    let id = saved["view"]["id"].as_str().unwrap();
    assert_eq!(id.len(), 26, "minted id should be a 26-char ULID");
    assert!(h.dir.path().join(format!("views/{id}.yaml")).exists());
}
