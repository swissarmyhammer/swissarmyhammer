//! Pins the `UnknownCommand` rejection path for both `execute` and
//! `available` verbs when no caller has registered the requested id.

mod common;

use std::sync::Arc;

use common::{call_tool, service_with_dispatcher, FakeDispatcher};
use serde_json::json;
use swissarmyhammer_plugin::CallerId;

#[tokio::test]
async fn execute_for_unknown_id_returns_unknown_command_error() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    let service = service_with_dispatcher(dispatcher.clone());

    let err = call_tool(
        &service,
        "execute command",
        json!({ "op": "execute command", "id": "does.not.exist" }),
        &CallerId::HostInternal,
    )
    .await
    .expect_err("execute for an unregistered id must fail");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("UnknownCommand"));
    assert_eq!(data["id"], json!("does.not.exist"));

    assert!(
        dispatcher.recorded().is_empty(),
        "the dispatcher must not see any invocation when the id is unknown",
    );
}

#[tokio::test]
async fn available_for_unknown_id_returns_unknown_command_error() {
    let dispatcher = Arc::new(FakeDispatcher::new());
    let service = service_with_dispatcher(dispatcher.clone());

    let err = call_tool(
        &service,
        "available command",
        json!({ "op": "available command", "id": "does.not.exist" }),
        &CallerId::HostInternal,
    )
    .await
    .expect_err("available for an unregistered id must fail");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("UnknownCommand"));
    assert_eq!(data["id"], json!("does.not.exist"));

    assert!(
        dispatcher.recorded().is_empty(),
        "the dispatcher must not see any invocation when the id is unknown",
    );
}
