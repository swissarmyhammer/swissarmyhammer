//! End-to-end tests for the `views` MCP server, per sub-domain.
//!
//! Stands up a real `ViewsServer` over a `PerspectiveContext` + `ViewsContext`
//! wired to a shared `StoreContext`, then exercises each sub-domain: lifecycle
//! (save/load/list/rename/delete), filter, group, sort, navigation, and the
//! `set view` path. Mutations are observed through the persisted perspective /
//! view state the server reads back.

use serde_json::json;
use swissarmyhammer_views::{ViewCommand, ViewDef, ViewKind};

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

/// list reflects saved perspectives; rename mutates the set. (Delete is NOT a
/// views op — it routes to the `entity` server, which holds the per-window
/// UIState the active-selection fallback writes; see
/// `swissarmyhammer-entity-mcp`.)
#[tokio::test]
async fn list_rename_lifecycle() {
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

/// goto resolves a perspective by id.
///
/// Activation ops (next / prev / switch) moved to the `entity` tool — this
/// server exposes only the `goto` RESOLUTION op (card 01KTYQY0ZB62KHN6BPK3FBMBD7).
#[tokio::test]
async fn nav_goto_resolves_by_id() {
    let h = Harness::new().await;
    let server = h.server();

    save_perspective(&h, "A", "board").await;
    let b = save_perspective(&h, "B", "board").await;
    save_perspective(&h, "C", "board").await;

    // goto by id returns it.
    let goto = call_tool(
        &server,
        "goto perspective",
        json!({ "op": "goto perspective", "id": b }),
    )
    .await
    .unwrap();
    assert_eq!(goto["perspective"]["id"], json!(b));
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

/// A partial `set view` against an EXISTING view must merge: omitting
/// `icon`/`card_fields`/`entity_type` preserves their on-disk value rather than
/// silently stripping them. Regression for the full-replace bug that wrote
/// `{id, name, kind}` over a real view file and wiped icon/card_fields/commands.
#[tokio::test]
async fn set_view_partial_update_preserves_omitted_fields() {
    let h = Harness::new().await;
    let server = h.server();

    // Create a fully-specified view.
    let created = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "name": "My Grid",
            "kind": "grid",
            "icon": "folder",
            "entity_type": "task",
            "card_fields": ["title", "status"],
        }),
    )
    .await
    .unwrap();
    let id = created["view"]["id"].as_str().unwrap().to_string();

    // Partial update: only name + kind. icon / entity_type / card_fields omitted.
    let updated = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "id": id,
            "name": "My Grid Renamed",
            "kind": "grid",
        }),
    )
    .await
    .unwrap();

    assert_eq!(updated["view"]["name"], json!("My Grid Renamed"));
    assert_eq!(
        updated["view"]["icon"],
        json!("folder"),
        "omitted icon must be preserved, not stripped"
    );
    assert_eq!(
        updated["view"]["entity_type"],
        json!("task"),
        "omitted entity_type must be preserved, not stripped"
    );
    assert_eq!(
        updated["view"]["card_fields"],
        json!(["title", "status"]),
        "omitted card_fields must be preserved, not stripped"
    );
}

/// An explicit empty `card_fields: []` and explicit `icon: null` on an existing
/// view must still CLEAR the field — merge semantics preserve omitted fields
/// but must not make explicit clearing impossible.
#[tokio::test]
async fn set_view_partial_update_explicit_empty_clears() {
    let h = Harness::new().await;
    let server = h.server();

    let created = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "name": "My Grid",
            "kind": "grid",
            "icon": "folder",
            "entity_type": "task",
            "card_fields": ["title", "status"],
        }),
    )
    .await
    .unwrap();
    let id = created["view"]["id"].as_str().unwrap().to_string();

    let updated = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "id": id,
            "name": "My Grid",
            "kind": "grid",
            "icon": null,
            "entity_type": null,
            "card_fields": [],
        }),
    )
    .await
    .unwrap();

    assert!(
        updated["view"]["icon"].is_null(),
        "explicit icon: null must clear the icon"
    );
    assert!(
        updated["view"]["entity_type"].is_null(),
        "explicit entity_type: null must clear it"
    );
    // An empty `card_fields` serializes as absent (skip_serializing_if =
    // Vec::is_empty), so the cleared list shows up as null in the output JSON.
    assert!(
        updated["view"]["card_fields"].is_null(),
        "explicit card_fields: [] must clear the list (serializes as absent)"
    );
}

/// A partial `set view` must preserve a view's `commands`, which have no wire
/// surface and so can only be set out-of-band (builtin YAML / direct write).
/// This is the most subtle clause of the merge contract: `set view` always
/// built `commands: Vec::new()` before the fix, silently wiping them. The
/// other regression tests cannot catch a regression here because every
/// `set view`-created view starts with empty `commands`, so this test seeds a
/// view carrying commands directly through the kernel first.
#[tokio::test]
async fn set_view_partial_update_preserves_commands() {
    let h = Harness::new().await;

    // Seed a view WITH commands directly through the views kernel (the wire
    // `set view` op has no `commands` field, mirroring builtin YAML views).
    let seeded = ViewDef {
        id: "01AAAAAAAAAAAAAAAAAAAAAAAA".to_string(),
        name: "Board".to_string(),
        icon: Some("kanban".to_string()),
        kind: ViewKind::Board,
        entity_type: Some("task".to_string()),
        card_fields: vec!["title".to_string()],
        commands: vec![ViewCommand {
            id: "board.newCard".to_string(),
            name: "New Card".to_string(),
            description: None,
            keys: None,
        }],
    };
    h.views.write().await.write_view(&seeded).await.unwrap();

    // Partial `set view` touching only name; commands omitted (no wire field).
    let server = h.server();
    let updated = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "id": "01AAAAAAAAAAAAAAAAAAAAAAAA",
            "name": "Board Renamed",
        }),
    )
    .await
    .unwrap();

    assert_eq!(updated["view"]["name"], json!("Board Renamed"));
    let commands = updated["view"]["commands"]
        .as_array()
        .expect("commands must survive a partial update, not be stripped to empty");
    assert_eq!(commands.len(), 1, "the seeded command must be preserved");
    assert_eq!(commands[0]["id"], json!("board.newCard"));
}

/// A partial `set view` against a NON-EXISTENT view creates it fresh — omitted
/// optional fields default to empty/none (current create behavior is unchanged).
#[tokio::test]
async fn set_view_partial_update_creates_fresh_when_absent() {
    let h = Harness::new().await;
    let server = h.server();

    let created = call_tool(
        &server,
        "set view",
        json!({
            "op": "set view",
            "id": "01AAAAAAAAAAAAAAAAAAAAAAAA",
            "name": "Brand New",
            "kind": "list",
        }),
    )
    .await
    .unwrap();

    assert_eq!(created["view"]["name"], json!("Brand New"));
    assert_eq!(created["view"]["kind"], json!("list"));
    assert!(created["view"]["icon"].is_null());
    // Empty card_fields serializes as absent (skip_serializing_if).
    assert!(created["view"]["card_fields"].is_null());
}
