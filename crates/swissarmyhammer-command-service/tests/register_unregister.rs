//! End-to-end `register` + `unregister command` tests through the
//! `rmcp::ServerHandler` surface of [`CommandService`].
//!
//! Each test mints a [`CallerId`], stuffs it into the request context's
//! extensions (mirroring what the in-process transport does in
//! production), and invokes [`CommandService::call_tool`] for the verb
//! under test. State assertions go through the read-only
//! [`CommandService::with_registry`] accessor — the public `list command`
//! verb is a stub in this layer and gets filled in by the follow-up
//! list/schema task.

mod common;

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use common::{call_tool, register_payload, request_context_for};
use serde_json::json;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn register_with_valid_payload_returns_ok_and_stores_entry() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    let payload = register_payload("task.move", "Move Task", "cb_execute_1");
    let result = call_tool(&service, "register command", payload, &caller)
        .await
        .expect("register should succeed");

    let structured = result
        .structured_content
        .as_ref()
        .expect("register response should carry structured content");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["stack_depth"], json!(1));
    assert_eq!(structured["active"]["id"], json!("task.move"));
    assert_eq!(structured["active"]["name"], json!("Move Task"));
    // The metadata projection must NOT carry the callback marker fields —
    // they are dispatch-time concerns, not registration data.
    assert!(structured["active"].get("execute").is_none());

    service.with_registry(|reg| {
        let active = reg.active("task.move").expect("entry should be present");
        assert_eq!(active.caller, caller);
        assert_eq!(active.registration.name, "Move Task");
        assert_eq!(active.registration.execute.callback_id, "cb_execute_1");
    });
}

#[tokio::test]
async fn register_with_empty_id_returns_structured_error() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    let mut payload = register_payload("", "Anonymous", "cb_x");
    // Sanity: the helper sets id to empty already, but make it explicit
    // so the test reads top-to-bottom without consulting the helper.
    payload["id"] = json!("");

    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("empty id must be rejected");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("EmptyId"));
    service.with_registry(|reg| assert!(reg.is_empty()));
}

#[tokio::test]
async fn register_with_empty_name_returns_structured_error() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    let payload = register_payload("task.move", "", "cb_x");
    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("empty name must be rejected");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("EmptyName"));
    assert_eq!(data["id"], json!("task.move"));
}

#[tokio::test]
async fn register_with_missing_execute_callback_returns_structured_error() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    // Empty callback id models the SDK-bug case the task calls out: the
    // marker shape arrives but `cb_<n>` is missing.
    let payload = register_payload("task.move", "Move Task", "");
    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("missing execute callback id must be rejected");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("MissingExecuteCallback"));
    assert_eq!(data["id"], json!("task.move"));
}

#[tokio::test]
async fn re_register_same_caller_same_id_replaces_in_place() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload("task.move", "Move Task", "cb_v1"),
        &caller,
    )
    .await
    .expect("first register should succeed");

    let result = call_tool(
        &service,
        "register command",
        register_payload("task.move", "Move Task v2", "cb_v2"),
        &caller,
    )
    .await
    .expect("second register should succeed");

    // Stack depth stays at 1 — the same caller's prior entry is replaced,
    // not stacked on top.
    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["stack_depth"], json!(1));
    assert_eq!(structured["active"]["name"], json!("Move Task v2"));

    service.with_registry(|reg| {
        let stack = reg.stack_for("task.move");
        assert_eq!(stack.len(), 1, "re-register must replace in place");
        assert_eq!(stack[0].registration.execute.callback_id, "cb_v2");
    });
}

#[tokio::test]
async fn unregister_removes_caller_entry() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload("task.move", "Move Task", "cb_x"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "task.move" }),
        &caller,
    )
    .await
    .expect("unregister should succeed");

    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["removed"], json!(true));

    service.with_registry(|reg| {
        assert!(
            reg.active("task.move").is_none(),
            "entry must be gone after unregister"
        );
    });
}

#[tokio::test]
async fn unregister_for_unknown_id_is_a_noop_success() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    let result = call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "never.registered" }),
        &caller,
    )
    .await
    .expect("unregister for unknown id must NOT error");

    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(
        structured["removed"],
        json!(false),
        "no entry was actually removed"
    );
}

#[tokio::test]
async fn missing_caller_in_context_defaults_to_unknown() {
    let service = CommandService::new();

    // Build a context WITHOUT inserting a CallerId — mirrors the case
    // where the service is reached via an external rmcp transport that
    // doesn't thread a caller through.
    let context = request_context_for(None);

    let payload = register_payload("task.move", "Move Task", "cb_x");
    let map = payload
        .as_object()
        .cloned()
        .expect("payload helper must produce a JSON object");
    let request = rmcp::model::CallToolRequestParams::new(std::borrow::Cow::Borrowed("command"))
        .with_arguments(map);
    use rmcp::ServerHandler;
    service
        .call_tool(request, context)
        .await
        .expect("register should succeed even without a caller in context");

    service.with_registry(|reg| {
        let active = reg.active("task.move").expect("entry must be present");
        assert_eq!(
            active.caller,
            CallerId::Unknown,
            "missing caller in context defaults to Unknown"
        );
    });
}

#[tokio::test]
async fn register_with_empty_available_callback_returns_structured_error() {
    let service = CommandService::new();
    let caller = CallerId::HostInternal;

    // Mirrors the SDK-bug case for the optional `available` marker: the
    // `$callback` wire shape arrives but the id is empty. The service
    // must reject it at registration time so it cannot surface later as
    // an opaque dispatch failure.
    let payload = json!({
        "op": "register command",
        "id": "task.move",
        "name": "Move Task",
        "execute": { "$callback": "cb_execute" },
        "available": { "$callback": "" },
    });

    let err = call_tool(&service, "register command", payload, &caller)
        .await
        .expect_err("empty available callback id must be rejected");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("MissingAvailableCallback"));
    assert_eq!(data["id"], json!("task.move"));
    service.with_registry(|reg| assert!(reg.is_empty()));
}

#[tokio::test]
async fn register_and_unregister_each_schedule_a_change_notification() {
    // Pins the "both verbs schedule a `commands/changed` notification"
    // acceptance criterion through the actual service wiring (not just
    // the notifier module's own tests). A register + an unregister, with
    // a flush between, must drive the counter to exactly 2.
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_sink = counter.clone();
    let service = CommandService::with_notifier_sink(move || {
        counter_for_sink.fetch_add(1, Ordering::SeqCst);
    });
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload("task.move", "Move Task", "cb_x"),
        &caller,
    )
    .await
    .expect("register should succeed");
    // Sleep past the debounce window so the worker task gets a chance to
    // wake and emit. The default window is 100ms — 200ms is comfortably
    // past it without making the test slow.
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "register must schedule one notification"
    );

    call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "task.move" }),
        &caller,
    )
    .await
    .expect("unregister should succeed");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        counter.load(Ordering::SeqCst),
        2,
        "unregister must schedule a second notification",
    );
}

#[tokio::test]
async fn no_op_unregister_does_not_schedule_a_notification() {
    // Pins the intentional `if removed { notify }` guard in
    // `handle_unregister` — a no-op unregister (caller had no entry for
    // the id) must NOT bump the notification counter, because the
    // registry's observable state did not change.
    let counter = Arc::new(AtomicUsize::new(0));
    let counter_for_sink = counter.clone();
    let service = CommandService::with_notifier_sink(move || {
        counter_for_sink.fetch_add(1, Ordering::SeqCst);
    });
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "unregister command",
        json!({ "op": "unregister command", "id": "never.registered" }),
        &caller,
    )
    .await
    .expect("unregister for unknown id must NOT error");
    tokio::time::sleep(Duration::from_millis(200)).await;
    assert_eq!(
        counter.load(Ordering::SeqCst),
        0,
        "no-op unregister must not emit a notification",
    );
}
