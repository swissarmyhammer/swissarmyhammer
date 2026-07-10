//! Caller-A and caller-B cannot pop each other's registry entries.
//!
//! This pins the rule that `unregister` is scoped to the registering
//! caller: B's `unregister` for an id A registered is a no-op, and only
//! A's `unregister` actually removes A's entry.

mod common;

use common::{call_tool, register_payload};
use serde_json::json;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn unregister_from_a_different_caller_is_a_noop() {
    let service = CommandService::new();
    let caller_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let caller_b = CallerId::Plugin(PluginId::new("plugin-b"));

    // A registers `foo`.
    call_tool(
        &service,
        "register command",
        register_payload("foo", "Foo", "cb_a"),
        &caller_a,
    )
    .await
    .expect("A's register should succeed");

    // B unregisters `foo` — must be a no-op because B never registered it.
    let result = call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "foo" }),
        &caller_b,
    )
    .await
    .expect("B's unregister must NOT error");

    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(
        structured["removed"],
        json!(false),
        "B had no entry to remove"
    );

    // A's entry must still be present.
    service.with_registry(|reg| {
        let active = reg.active("foo").expect("A's entry must still be there");
        assert_eq!(active.caller, caller_a);
        assert_eq!(active.registration.execute.callback_id, "cb_a");
    });
}

#[tokio::test]
async fn only_the_registering_caller_can_unregister_its_own_entry() {
    let service = CommandService::new();
    let caller_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let caller_b = CallerId::Plugin(PluginId::new("plugin-b"));

    // Both callers register the same id — the stack now holds two
    // entries, B on top.
    call_tool(
        &service,
        "register command",
        register_payload("shared", "Shared A", "cb_a"),
        &caller_a,
    )
    .await
    .expect("A's register should succeed");

    call_tool(
        &service,
        "register command",
        register_payload("shared", "Shared B", "cb_b"),
        &caller_b,
    )
    .await
    .expect("B's register should succeed");

    service.with_registry(|reg| {
        assert_eq!(reg.stack_for("shared").len(), 2);
    });

    // A unregisters its own entry — B remains on top.
    let removed = call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "shared" }),
        &caller_a,
    )
    .await
    .expect("A's unregister should succeed");
    assert_eq!(removed.structured_content.unwrap()["removed"], json!(true));

    service.with_registry(|reg| {
        let stack = reg.stack_for("shared");
        assert_eq!(stack.len(), 1, "only B's entry remains");
        assert_eq!(stack[0].caller, caller_b);
    });

    // B unregisters its own entry — stack is now empty.
    call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "shared" }),
        &caller_b,
    )
    .await
    .expect("B's unregister should succeed");

    service.with_registry(|reg| {
        assert!(
            reg.active("shared").is_none(),
            "both entries gone after each caller unregistered its own"
        );
    });
}
