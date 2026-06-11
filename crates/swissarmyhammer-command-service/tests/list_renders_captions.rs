//! Display-time caption rendering tests for `list command`.
//!
//! Plugins declare caption templates (e.g. `"Inspect {{entity.type}}"`); the
//! service renders them against the focused object at list time, driven by
//! the `ctx` (scope chain / target) the listing surface supplies. A raw
//! `{{...}}` placeholder must NEVER reach a display surface: with context the
//! placeholder resolves (e.g. "Inspect Task"), without context it falls back
//! to a clean generic form (e.g. "Inspect").
//!
//! Regression for kanban card 01KTRMXRNH66GZCWSNR1YGE28E — the command
//! palette showed `Inspect {{entity.type}}` verbatim.

mod common;

use common::call_tool;
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

/// Register a command whose `name` (and optionally `menu_name`) carries a
/// caption template, exactly as the builtin plugins declare them.
async fn register_templated(
    service: &CommandService,
    caller: &CallerId,
    id: &str,
    name: &str,
    menu_name: Option<&str>,
) {
    let mut payload = json!({
        "op": "register command",
        "id": id,
        "name": name,
        "execute": { "$callback": "cb_x" },
    });
    if let Some(menu_name) = menu_name {
        payload["menu_name"] = json!(menu_name);
    }
    call_tool(service, "register command", payload, caller)
        .await
        .expect("register should succeed");
}

/// Drive `list command` with `arguments` and return the listed command
/// entries keyed by id.
async fn list_commands(
    service: &CommandService,
    arguments: Value,
    caller: &CallerId,
) -> Vec<Value> {
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
        .clone()
}

/// Find the entry for `id` in a listed command array.
fn find<'a>(commands: &'a [Value], id: &str) -> &'a Value {
    commands
        .iter()
        .find(|cmd| cmd["id"] == json!(id))
        .unwrap_or_else(|| panic!("command {id:?} should be listed"))
}

#[tokio::test]
async fn list_renders_entity_type_from_focused_scope_chain() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "app.inspect",
        "Inspect {{entity.type}}",
        None,
    )
    .await;

    // The palette lists for the current focus: the innermost scope-chain
    // moniker identifies the focused object (a task here).
    let commands = list_commands(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["task:01HTASK", "view:01HVIEW", "board:01HBOARD"] },
        }),
        &caller,
    )
    .await;

    assert_eq!(
        find(&commands, "app.inspect")["name"],
        json!("Inspect Task"),
        "the {{{{entity.type}}}} placeholder must render against the focused object",
    );
}

#[tokio::test]
async fn list_without_context_falls_back_to_clean_generic_caption() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "app.inspect",
        "Inspect {{entity.type}}",
        None,
    )
    .await;

    // No ctx at all — the OS menu / a context-free surface. The caption must
    // degrade to a clean generic form, never the raw placeholder.
    let commands = list_commands(&service, json!({ "op": "list command" }), &caller).await;

    assert_eq!(
        find(&commands, "app.inspect")["name"],
        json!("Inspect"),
        "without context the placeholder must be dropped cleanly (trimmed)",
    );
}

#[tokio::test]
async fn list_prefers_target_over_scope_chain_for_entity_context() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "entity.delete",
        "Delete {{entity.type}}",
        None,
    )
    .await;

    // Context menus fire over an explicit target — the entity the menu
    // targets wins over the ambient focus chain.
    let commands = list_commands(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["task:01HTASK"], "target": "tag:01HTAG" },
        }),
        &caller,
    )
    .await;

    assert_eq!(
        find(&commands, "entity.delete")["name"],
        json!("Delete Tag"),
        "the explicit target moniker must take precedence over the scope chain",
    );
}

#[tokio::test]
async fn list_renders_menu_name_too() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "entity.copy",
        "Copy {{entity.type}}",
        Some("Copy {{entity.type}}"),
    )
    .await;

    let commands = list_commands(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["column:01HCOL"] },
        }),
        &caller,
    )
    .await;

    let cmd = find(&commands, "entity.copy");
    assert_eq!(cmd["name"], json!("Copy Column"), "name renders");
    assert_eq!(cmd["menu_name"], json!("Copy Column"), "menu_name renders");
}

#[tokio::test]
async fn list_tolerates_whitespace_inside_placeholder_braces() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "entity.cut",
        "Cut {{ entity.type }}",
        None,
    )
    .await;

    let commands = list_commands(
        &service,
        json!({
            "op": "list command",
            "ctx": { "scope_chain": ["task:01HTASK"] },
        }),
        &caller,
    )
    .await;

    assert_eq!(
        find(&commands, "entity.cut")["name"],
        json!("Cut Task"),
        "`{{{{ entity.type }}}}` (inner spaces) must resolve like the tight form",
    );
}

#[tokio::test]
async fn list_never_surfaces_raw_placeholders_even_for_unknown_keys() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));
    register_templated(
        &service,
        &caller,
        "x.future",
        "Reticulate {{entity.frobnicate}}",
        None,
    )
    .await;

    // Even with full context, an unknown placeholder must be dropped — the
    // defined fallback is a clean caption, never raw `{{...}}` passthrough.
    for ctx in [json!({ "scope_chain": ["task:01HTASK"] }), json!({})] {
        let commands = list_commands(
            &service,
            json!({ "op": "list command", "ctx": ctx }),
            &caller,
        )
        .await;
        let name = find(&commands, "x.future")["name"]
            .as_str()
            .expect("name is a string")
            .to_string();
        assert_eq!(name, "Reticulate", "unknown placeholders drop cleanly");
        assert!(
            !name.contains("{{"),
            "no raw placeholder may ever surface, got {name:?}",
        );
    }
}
