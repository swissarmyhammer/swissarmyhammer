//! Happy-path `execute command` tests through the `rmcp::ServerHandler`
//! surface of [`CommandService`].
//!
//! Registers a command whose `execute` callback echoes its first argument,
//! then drives `execute command` through the verb dispatcher and asserts
//! that:
//!
//! - the dispatcher saw exactly one invocation, routed at the registering
//!   caller's id and the registered execute callback id;
//! - the verb response surfaced the callback's return value under
//!   `result`;
//! - the `CommandContext` round-tripped as the positional first argument.

mod common;

use std::sync::Arc;

use common::{call_tool, register_payload, service_with_dispatcher, FakeDispatcher, Reply};
use serde_json::json;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn execute_invokes_execute_callback_and_returns_its_result() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_exec", Reply::ok(json!({ "moved": true })));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        register_payload("task.move", "Move Task", "cb_exec"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "execute command",
        json!({
            "op": "execute command",
            "id": "task.move",
        }),
        &caller,
    )
    .await
    .expect("execute should succeed");

    let structured = result
        .structured_content
        .expect("execute response should carry structured content");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(
        structured["result"],
        json!({ "moved": true }),
        "execute must surface the callback's return value verbatim",
    );

    let recorded = dispatcher.recorded();
    assert_eq!(recorded.len(), 1, "exactly one dispatch expected");
    let invocation = &recorded[0];
    assert_eq!(
        invocation.caller, caller,
        "dispatch must be routed at the registering caller",
    );
    assert_eq!(
        invocation.callback_id, "cb_exec",
        "dispatch must use the registered execute callback id",
    );
}

#[tokio::test]
async fn execute_passes_command_context_as_positional_first_arg() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_exec", Reply::ok(json!("ok")));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload("task.archive", "Archive Task", "cb_exec"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let ctx = json!({
        "scope_chain": ["board:01ABC", "task:42"],
        "target": "task:42",
        "args": { "reason": "stale" },
    });

    call_tool(
        &service,
        "execute command",
        json!({
            "op": "execute command",
            "id": "task.archive",
            "ctx": ctx,
        }),
        &caller,
    )
    .await
    .expect("execute should succeed");

    let recorded = dispatcher.recorded();
    assert_eq!(recorded.len(), 1, "exactly one dispatch expected");
    let args = &recorded[0].args;
    let arr = args
        .as_array()
        .expect("dispatcher args should be a positional JSON array");
    assert_eq!(
        arr.len(),
        1,
        "execute callback receives the context as its single positional arg",
    );
    assert_eq!(
        arr[0], ctx,
        "context must round-trip verbatim through the dispatch path",
    );
}

#[tokio::test]
async fn execute_surfaces_callback_failure_as_callback_failed() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    dispatcher.program("cb_exec", Reply::err("isolate exploded"));
    let service = service_with_dispatcher(dispatcher.clone());
    let caller = CallerId::HostInternal;

    call_tool(
        &service,
        "register command",
        register_payload("task.boom", "Detonate", "cb_exec"),
        &caller,
    )
    .await
    .expect("register should succeed");

    let err = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "task.boom" }),
        &caller,
    )
    .await
    .expect_err("dispatcher failure must surface as an error");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("CallbackFailed"));
    assert!(
        data["message"]
            .as_str()
            .expect("CallbackFailed.message must be a string")
            .contains("isolate exploded"),
        "underlying dispatcher message must propagate, got {data}",
    );
}
