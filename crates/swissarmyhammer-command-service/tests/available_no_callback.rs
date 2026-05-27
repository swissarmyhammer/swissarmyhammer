//! Pins the "no available callback → always available" shortcut on both
//! the `available` verb and the `execute` recheck path.
//!
//! When a command is registered without an `available` marker:
//!
//! - `available command` returns `{ ok: true }` without dispatching.
//! - `execute command` skips the recheck and goes straight to invoking
//!   the `execute` callback.

mod common;

use std::sync::Arc;

use common::{call_tool, register_payload, service_with_dispatcher, FakeDispatcher, Reply};
use serde_json::json;
use swissarmyhammer_plugin::CallerId;

#[tokio::test]
async fn available_without_callback_returns_ok_true_without_dispatch() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload("task.always", "Always Available", "cb_exec"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "available command",
        json!({ "op": "available command", "id": "task.always" }),
        &caller,
    )
    .await
    .expect("available should succeed");

    let structured = result
        .structured_content
        .expect("available response should carry structured content");
    assert_eq!(structured, json!({ "ok": true }));

    assert!(
        dispatcher.recorded().is_empty(),
        "no available callback registered ⇒ no dispatch",
    );
}

#[tokio::test]
async fn execute_without_available_callback_skips_recheck() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_exec", Reply::ok(json!("ran")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload("task.always", "Always Available", "cb_exec"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "task.always" }),
        &caller,
    )
    .await
    .expect("execute should succeed when no available callback is registered");

    let structured = result
        .structured_content
        .expect("execute response should carry structured content");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["result"], json!("ran"));

    let recorded = dispatcher.recorded();
    assert_eq!(
        recorded.len(),
        1,
        "only the execute callback should have been dispatched, got {recorded:?}",
    );
    assert_eq!(recorded[0].callback_id, "cb_exec");
}
