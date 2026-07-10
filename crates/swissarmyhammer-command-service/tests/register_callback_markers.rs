//! Asserts the service stores both the required `execute` callback
//! marker and the optional `available` callback marker on the registry
//! stack entry, using the SDK's `{"$callback": "cb_..."}` wire shape.

mod common;

use common::{call_tool, register_payload_with_available};
use serde_json::json;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn register_stores_both_callback_markers_on_stack_entry() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    let payload = register_payload_with_available(
        "task.archive",
        "Archive Task",
        "cb_execute_archive",
        "cb_available_archive",
    );

    let result = call_tool(&service, "register command", payload, &caller)
        .await
        .expect("register should succeed");

    let structured = result
        .structured_content
        .expect("register response must carry structured content");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["active"]["id"], json!("task.archive"));

    service.with_registry(|reg| {
        let active = reg.active("task.archive").expect("entry should be present");
        assert_eq!(
            active.registration.execute.callback_id, "cb_execute_archive",
            "execute marker id must round-trip from the $callback wire shape"
        );
        let available = active
            .registration
            .available
            .as_ref()
            .expect("optional available marker should be stored");
        assert_eq!(available.callback_id, "cb_available_archive");
    });
}

#[tokio::test]
async fn register_without_available_callback_leaves_field_none() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    // Only the required execute marker — no `available` field at all.
    let payload = serde_json::json!({
        "op": "register command",
        "id": "task.move",
        "name": "Move Task",
        "execute": { "$callback": "cb_execute_only" },
    });

    call_tool(&service, "register command", payload, &caller)
        .await
        .expect("register should succeed");

    service.with_registry(|reg| {
        let active = reg.active("task.move").expect("entry should be present");
        assert!(
            active.registration.available.is_none(),
            "available marker stays None when not supplied"
        );
        assert_eq!(active.registration.execute.callback_id, "cb_execute_only");
    });
}
