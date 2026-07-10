//! Pins the `available` → `execute` gating semantics described in the task:
//!
//! - When the registered command has an `available` callback that returns
//!   `false`, `execute` rejects with `CommandUnavailable { reason }` and
//!   does NOT invoke the `execute` callback.
//! - When the same scenario is invoked with `force: true`, the recheck is
//!   skipped and the `execute` callback runs anyway.
//! - When the `available` callback returns an `{ ok: false, reason }`
//!   object, the supplied reason propagates onto the `CommandUnavailable`
//!   error.

mod common;

use std::sync::Arc;

use common::{
    call_tool, register_payload_with_available, service_with_dispatcher, FakeDispatcher, Reply,
};
use serde_json::json;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn execute_rejects_when_available_returns_false() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_avail", Reply::ok(json!(false)));
    dispatcher.program("cb_exec", Reply::ok(json!("ran")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.gated", "Gated", "cb_exec", "cb_avail"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let err = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "task.gated" }),
        &caller,
    )
    .await
    .expect_err("execute must reject when available returned false");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("CommandUnavailable"));
    assert!(
        !data["reason"]
            .as_str()
            .expect("reason should be a string")
            .is_empty(),
        "CommandUnavailable must carry a non-empty reason, got {data}",
    );

    let recorded = dispatcher.recorded();
    assert_eq!(
        recorded.len(),
        1,
        "only the available callback should have been invoked",
    );
    assert_eq!(recorded[0].callback_id, "cb_avail");
}

#[tokio::test]
async fn execute_with_force_true_skips_available_recheck() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_avail", Reply::ok(json!(false)));
    dispatcher.program("cb_exec", Reply::ok(json!("ran")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.forced", "Forced", "cb_exec", "cb_avail"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "execute command",
        json!({
            "op": "execute command",
            "id": "task.forced",
            "force": true,
        }),
        &caller,
    )
    .await
    .expect("execute with force: true should succeed despite available=false");

    let structured = result
        .structured_content
        .expect("execute response should carry structured content");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["result"], json!("ran"));

    let recorded = dispatcher.recorded();
    assert_eq!(
        recorded.len(),
        1,
        "force: true must skip the available recheck",
    );
    assert_eq!(
        recorded[0].callback_id, "cb_exec",
        "the single dispatch must be the execute callback",
    );
}

#[tokio::test]
async fn execute_surfaces_available_reason_string_on_unavailable_object() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program(
        "cb_avail",
        Reply::ok(json!({ "ok": false, "reason": "no selection" })),
    );
    dispatcher.program("cb_exec", Reply::ok(json!("ran")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.picky", "Picky", "cb_exec", "cb_avail"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let err = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "task.picky" }),
        &caller,
    )
    .await
    .expect_err("execute must reject when available returned {ok:false}");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("CommandUnavailable"));
    assert_eq!(
        data["reason"],
        json!("no selection"),
        "supplied reason must propagate verbatim onto CommandUnavailable",
    );

    let recorded = dispatcher.recorded();
    assert_eq!(recorded.len(), 1, "execute must not have run");
    assert_eq!(recorded[0].callback_id, "cb_avail");
}
