//! End-to-end tests for the `applies_to` capability gate on `list command`.
//!
//! `applies_to` is a declarative capability set on a command registration:
//! the entity types the command supports. When present, `list command`
//! filters the command out unless the focused object (from `ctx.target` /
//! `ctx.scope_chain`) is one of the declared types. When absent, the
//! command is unconstrained (global) exactly as before.
//!
//! This is the metadata-driven seam that stops cross-cutting clipboard
//! commands (`entity.cut` / `entity.copy` / `entity.paste`) from surfacing
//! on entity types that don't support them (views, perspectives): the
//! capability is DATA on the declaration, interpreted by one code path in
//! `list_filter_matches` — never a hardcoded `if (type === "view")` branch
//! in the UI.

mod common;

use std::collections::BTreeSet;

use common::call_tool;
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Register a command that declares it applies to the given entity types.
fn register_with_applies_to(id: &str, name: &str, applies_to: &[&str]) -> Value {
    json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": "cb_x" },
        "context_menu": true,
        "params": [{ "name": "moniker", "from": "target" }],
        "applies_to": applies_to,
    })
}

/// Drive `list command` with the given arguments and return the id set.
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
    structured["commands"]
        .as_array()
        .expect("`commands` should be an array")
        .iter()
        .map(|entry| {
            entry["id"]
                .as_str()
                .expect("each entry should carry a string id")
                .to_string()
        })
        .collect()
}

/// Register the clipboard-shaped fixture: three cross-cutting commands that
/// declare they apply only to clipboard-capable entity types (task, tag,
/// column, board, attachment) — never to views or perspectives.
async fn register_clipboard_fixture(service: &CommandService, caller: &CallerId) {
    // Deliberately REDUCED fixture, NOT the canonical production set: the real
    // clipboard capability is `COPYABLE_ENTITY_TYPES`
    // (`swissarmyhammer-kanban::commands::clipboard_commands`), which also
    // includes `actor` and `project`. This isolated unit only needs a
    // representative supported set vs the unsupported view/perspective to
    // exercise the list-time gate; the production set is pinned against the
    // Rust constant by `builtin_entity_commands_e2e::assert_clipboard_applies_to`.
    let copyable = &["task", "tag", "column", "board", "attachment"];
    for (id, name) in [
        ("entity.cut", "Cut {{entity.type}}"),
        ("entity.copy", "Copy {{entity.type}}"),
        ("entity.paste", "Paste {{entity.type}}"),
    ] {
        call_tool(
            service,
            "register command",
            register_with_applies_to(id, name, copyable),
            caller,
        )
        .await
        .expect("fixture register should succeed");
    }
}

#[tokio::test]
async fn clipboard_commands_absent_when_a_view_is_the_context_menu_target() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // Right-click on a view button: the context menu fires over `view:v1`.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "view:v1", "scope_chain": ["view:v1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("entity.cut")
            && !got.contains("entity.copy")
            && !got.contains("entity.paste"),
        "clipboard commands must NOT appear when a view is focused; got {got:?}",
    );
}

#[tokio::test]
async fn clipboard_commands_absent_when_a_perspective_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // A focused perspective tab: the scope chain leaf is `perspective:p1`,
    // no target moniker (palette semantics).
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["perspective:p1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        !got.contains("entity.cut")
            && !got.contains("entity.copy")
            && !got.contains("entity.paste"),
        "clipboard commands must NOT appear when a perspective is focused; got {got:?}",
    );
}

#[tokio::test]
async fn clipboard_commands_present_when_a_task_is_focused() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_clipboard_fixture(&service, &caller).await;

    // A focused task: task is a clipboard-capable type, so the commands
    // must still surface and work — the fix must not regress them.
    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "task:01X", "scope_chain": ["task:01X", "column:todo"] },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("entity.cut") && got.contains("entity.copy") && got.contains("entity.paste"),
        "clipboard commands MUST appear when a task is focused; got {got:?}",
    );
}

#[tokio::test]
async fn applies_to_absent_command_is_global_in_every_focus() {
    // A command with NO `applies_to` is unconstrained — it must still list
    // for a view focus, exactly as before. This pins that the gate only
    // restricts commands that opt in to the capability set.
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "app.quit",
            "name": "Quit",
            "execute": { "$callback": "cb_quit" },
        }),
        &caller,
    )
    .await
    .expect("global register should succeed");

    let got = list_ids(
        &service,
        json!({
            "op": "list command",
            "ctx": { "target": "view:v1", "scope_chain": ["view:v1"] },
        }),
        &caller,
    )
    .await;

    assert!(
        got.contains("app.quit"),
        "a command with no applies_to must remain global; got {got:?}",
    );
}
