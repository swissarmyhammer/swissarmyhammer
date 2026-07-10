//! Pins the soft latency budget on the `available command` verb:
//!
//! - When the `available` callback exceeds [`AVAILABLE_HARD_DEADLINE`]
//!   (50ms), the verb force-cancels the dispatch and returns
//!   `{ ok: false, reason: "available timeout" }`.
//! - When the latency is between the warn threshold (5ms) and the hard
//!   deadline, the real result still surfaces.

mod common;

use std::sync::Arc;
use std::time::Duration;

use common::{
    call_tool, register_payload_with_available, service_with_dispatcher, FakeDispatcher, Reply,
};
use serde_json::json;
use swissarmyhammer_command_service::AVAILABLE_TIMEOUT_REASON;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn available_callback_over_hard_deadline_yields_timeout_reason() {
    // 60ms is well past the 50ms hard deadline; the verb must give up
    // waiting and surface the canned timeout reason.
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program(
        "cb_avail_slow",
        Reply::ok_after(json!(true), Duration::from_millis(60)),
    );
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::Plugin(PluginId::new("plugin-slow"));

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.slow", "Slow", "cb_exec", "cb_avail_slow"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "available command",
        json!({ "op": "available command", "id": "task.slow" }),
        &caller,
    )
    .await
    .expect("available should succeed even when the callback was force-cancelled");

    let structured = result
        .structured_content
        .expect("available response should carry structured content");
    assert_eq!(
        structured["ok"],
        json!(false),
        "force-cancelled callback must surface ok:false, got {structured}",
    );
    assert_eq!(
        structured["reason"],
        json!(AVAILABLE_TIMEOUT_REASON),
        "reason must be the canned timeout string",
    );
}

#[tokio::test(start_paused = true)]
async fn available_callback_just_past_warn_threshold_still_returns_real_result() {
    // 10ms sits in the warn band (>5ms, <50ms): the warn fires but the
    // real result must surface unchanged.
    //
    // Runs with `start_paused = true` so tokio's auto-advance virtual clock
    // makes the assertion deterministic: the dispatcher's `sleep(10ms)` and
    // the latency budget's `timeout(50ms)` both observe virtual time, so a
    // heavily loaded CI runner cannot slip the real wall-clock past the
    // 50ms hard deadline and flip the result to `ok: false`.
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program(
        "cb_avail_slowish",
        Reply::ok_after(json!(true), Duration::from_millis(10)),
    );
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.slowish", "Slowish", "cb_exec", "cb_avail_slowish"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "available command",
        json!({ "op": "available command", "id": "task.slowish" }),
        &caller,
    )
    .await
    .expect("available should succeed inside the warn band");

    let structured = result
        .structured_content
        .expect("available response should carry structured content");
    assert_eq!(
        structured["ok"],
        json!(true),
        "warn-band latency must NOT change the result, got {structured}",
    );
}

#[tokio::test]
async fn execute_recheck_returns_timeout_reason_when_available_overruns_budget() {
    // The execute path reuses the same budget enforcement; a 60ms available
    // callback must surface the timeout reason onto CommandUnavailable.
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program(
        "cb_avail_slow",
        Reply::ok_after(json!(true), Duration::from_millis(60)),
    );
    dispatcher.program("cb_exec", Reply::ok(json!("ran")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload_with_available("task.slow", "Slow", "cb_exec", "cb_avail_slow"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let err = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "task.slow" }),
        &caller,
    )
    .await
    .expect_err("execute must reject when the available recheck timed out");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("CommandUnavailable"));
    assert_eq!(data["reason"], json!(AVAILABLE_TIMEOUT_REASON));
}
