//! End-to-end filter tests for `list command` through the
//! `rmcp::ServerHandler` surface of [`CommandService`].
//!
//! Registers eight commands across two scopes (`entity:task`,
//! `entity:column`) and two categories (`Cleanup`, `Navigation`) using the
//! real `register command` verb, then drives every filter combination
//! through `list command` and asserts on the returned id set.
//!
//! Going through the service's own `register` verb (rather than poking the
//! registry directly) proves the entire dispatch path holds — that's the
//! workflow the task description calls out.

mod common;

use std::collections::BTreeSet;

use common::call_tool;
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Build a `register command` payload with scope + category set, so the
/// list-filter tests can register the eight fixture commands compactly.
fn register_payload_with_scope_and_category(
    id: &str,
    name: &str,
    scope: &[&str],
    category: &str,
) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": "cb_x" },
        "scope": scope,
        "category": category,
    })
}

/// Register the eight-command fixture used by every test in this file.
///
/// The matrix is `{task, column} × {Cleanup, Navigation} × {a, b}` —
/// two commands per scope/category bucket so prefix-only filters return
/// more than one entry and intersection filters narrow correctly.
async fn register_fixture(service: &CommandService, caller: &CallerId) {
    let entries: &[(&str, &str, &[&str], &str)] = &[
        (
            "task.cleanup.a",
            "Task Cleanup A",
            &["entity:task"],
            "Cleanup",
        ),
        (
            "task.cleanup.b",
            "Task Cleanup B",
            &["entity:task"],
            "Cleanup",
        ),
        ("task.nav.a", "Task Nav A", &["entity:task"], "Navigation"),
        ("task.nav.b", "Task Nav B", &["entity:task"], "Navigation"),
        (
            "column.cleanup.a",
            "Column Cleanup A",
            &["entity:column"],
            "Cleanup",
        ),
        (
            "column.cleanup.b",
            "Column Cleanup B",
            &["entity:column"],
            "Cleanup",
        ),
        (
            "column.nav.a",
            "Column Nav A",
            &["entity:column"],
            "Navigation",
        ),
        (
            "column.nav.b",
            "Column Nav B",
            &["entity:column"],
            "Navigation",
        ),
    ];
    for (id, name, scope, category) in entries {
        call_tool(
            service,
            "register command",
            register_payload_with_scope_and_category(id, name, scope, category),
            caller,
        )
        .await
        .expect("fixture register should succeed");
    }
}

/// Drive `list command` with the given filter arguments and return the
/// set of ids in the response. Sorting via a `BTreeSet` keeps assertions
/// independent of HashMap iteration order.
async fn list_ids(
    service: &CommandService,
    arguments: Value,
    caller: &CallerId,
) -> BTreeSet<String> {
    let result = call_tool(service, "list command", arguments, caller)
        .await
        .expect("list should succeed");
    let structured = result
        .structured_content
        .expect("list response should carry structured content");
    assert_eq!(structured["ok"], json!(true));
    let commands = structured["commands"]
        .as_array()
        .expect("`commands` should be an array");
    commands
        .iter()
        .map(|entry| {
            entry["id"]
                .as_str()
                .expect("each entry should carry a string id")
                .to_string()
        })
        .collect()
}

/// Helper that turns a slice of static ids into a `BTreeSet<String>` so
/// the per-test "expected ids" literals stay terse.
fn ids(slice: &[&str]) -> BTreeSet<String> {
    slice.iter().map(|s| s.to_string()).collect()
}

#[tokio::test]
async fn list_with_no_filters_returns_all_eight_commands() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(&service, json!({ "op": "list command" }), &caller).await;
    assert_eq!(
        got,
        ids(&[
            "column.cleanup.a",
            "column.cleanup.b",
            "column.nav.a",
            "column.nav.b",
            "task.cleanup.a",
            "task.cleanup.b",
            "task.nav.a",
            "task.nav.b",
        ]),
    );
}

#[tokio::test]
async fn list_with_scope_filter_returns_only_matching_scope() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({ "op": "list command", "scope": "entity:task" }),
        &caller,
    )
    .await;
    assert_eq!(
        got,
        ids(&[
            "task.cleanup.a",
            "task.cleanup.b",
            "task.nav.a",
            "task.nav.b",
        ]),
    );
}

#[tokio::test]
async fn list_with_category_filter_returns_only_matching_category() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({ "op": "list command", "category": "Cleanup" }),
        &caller,
    )
    .await;
    assert_eq!(
        got,
        ids(&[
            "column.cleanup.a",
            "column.cleanup.b",
            "task.cleanup.a",
            "task.cleanup.b",
        ]),
    );
}

#[tokio::test]
async fn list_with_id_prefix_filter_returns_only_matching_prefix() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({ "op": "list command", "id_prefix": "task." }),
        &caller,
    )
    .await;
    assert_eq!(
        got,
        ids(&[
            "task.cleanup.a",
            "task.cleanup.b",
            "task.nav.a",
            "task.nav.b",
        ]),
    );
}

#[tokio::test]
async fn list_with_scope_and_category_filters_returns_intersection() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "scope": "entity:task",
            "category": "Cleanup",
        }),
        &caller,
    )
    .await;
    assert_eq!(got, ids(&["task.cleanup.a", "task.cleanup.b"]));
}

#[tokio::test]
async fn list_with_all_three_filters_returns_full_intersection() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "scope": "entity:task",
            "category": "Navigation",
            "id_prefix": "task.nav.",
        }),
        &caller,
    )
    .await;
    assert_eq!(got, ids(&["task.nav.a", "task.nav.b"]));
}

#[tokio::test]
async fn list_scope_filter_includes_global_commands_with_no_scope() {
    // Commands with `scope: None` or `scope: Some(vec![])` are global —
    // they apply in every scope, so a scope filter must return them too.
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    // Register a global command (no scope field).
    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "global.help",
            "name": "Help",
            "execute": { "$callback": "cb_help" },
        }),
        &caller,
    )
    .await
    .expect("global register should succeed");

    let got = list_ids(
        &service,
        json!({ "op": "list command", "scope": "entity:task" }),
        &caller,
    )
    .await;
    assert!(
        got.contains("global.help"),
        "global commands (no scope) must match any scope filter, got {got:?}",
    );
}

#[tokio::test]
async fn list_returns_callback_free_metadata_projection() {
    // The `list` response must carry the public CommandMetadata projection
    // — no `execute` / `available` callback markers. Pin the projection
    // shape so a future change can't silently leak callback ids.
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let result = call_tool(
        &service,
        "list command",
        json!({ "op": "list command" }),
        &caller,
    )
    .await
    .expect("list should succeed");
    let structured = result.structured_content.expect("structured response");
    let commands = structured["commands"].as_array().expect("commands array");
    for entry in commands {
        assert!(
            entry.get("execute").is_none(),
            "list entry must not carry execute marker, got {entry}",
        );
        assert!(
            entry.get("available").is_none(),
            "list entry must not carry available marker, got {entry}",
        );
    }
}

#[tokio::test]
async fn list_with_unmatched_filter_returns_empty_array() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_fixture(&service, &caller).await;

    let got = list_ids(
        &service,
        json!({ "op": "list command", "category": "DoesNotExist" }),
        &caller,
    )
    .await;
    assert!(got.is_empty(), "no commands match the filter; got {got:?}");
}
