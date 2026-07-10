//! Pins the override-stack invariant for the `list command` verb: when
//! caller A registers an id and caller B overrides it, `list` must return
//! exactly one entry for that id — B's. The shadowed A entry is hidden
//! from the discovery surface.
//!
//! This is the discovery-surface complement to `registry_stack.rs` —
//! the registry's own `list()` accessor already enforces top-of-stack-only;
//! this test pins that the service's verb projection preserves it.

mod common;

use common::{call_tool, register_payload};
use serde_json::json;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn override_hides_shadowed_entry_from_list() {
    let service = CommandService::new();
    let caller_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let caller_b = CallerId::Plugin(PluginId::new("plugin-b"));

    // A registers `foo` first.
    call_tool(
        &service,
        "register command",
        register_payload("foo", "Foo from A", "cb_a"),
        &caller_a,
    )
    .await
    .expect("A's register should succeed");

    // B overrides — B is now top of stack.
    call_tool(
        &service,
        "register command",
        register_payload("foo", "Foo from B", "cb_b"),
        &caller_b,
    )
    .await
    .expect("B's register should succeed");

    // List from either caller must return one entry, and it must be B's.
    let result = call_tool(
        &service,
        "list command",
        json!({ "op": "list command" }),
        &caller_a,
    )
    .await
    .expect("list should succeed");
    let structured = result.structured_content.expect("structured response");
    let commands = structured["commands"].as_array().expect("commands array");

    assert_eq!(
        commands.len(),
        1,
        "shadowed entry must not appear; got {commands:?}"
    );
    assert_eq!(commands[0]["id"], json!("foo"));
    assert_eq!(
        commands[0]["name"],
        json!("Foo from B"),
        "the active entry must be B's, not A's",
    );
}
