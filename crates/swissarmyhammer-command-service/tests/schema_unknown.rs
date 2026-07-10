//! Pins the `schema command` error contract for unregistered ids.
//!
//! `schema` for an id no caller has registered must return a structured
//! `UnknownCommand` error so palette / popover code can branch on the
//! `kind` discriminant without parsing the message string.

mod common;

use common::call_tool;
use serde_json::json;
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn schema_for_unregistered_id_returns_unknown_command_error() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    let err = call_tool(
        &service,
        "schema command",
        json!({ "op": "schema command", "id": "does.not.exist" }),
        &caller,
    )
    .await
    .expect_err("schema for unregistered id must be an error");

    let data = err.data.expect("error must carry structured data");
    assert_eq!(data["kind"], json!("UnknownCommand"));
    assert_eq!(data["id"], json!("does.not.exist"));
}

#[tokio::test]
async fn schema_for_id_other_caller_registered_succeeds() {
    // The schema response is global — any caller can ask any command's
    // schema. Pin that by registering as caller A and asking as caller B.
    let service = CommandService::new();
    let caller_a = CallerId::Plugin(PluginId::new("plugin-a"));
    let caller_b = CallerId::Plugin(PluginId::new("plugin-b"));

    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "task.move",
            "name": "Move Task",
            "execute": { "$callback": "cb_move" },
        }),
        &caller_a,
    )
    .await
    .expect("A's register should succeed");

    let result = call_tool(
        &service,
        "schema command",
        json!({ "op": "schema command", "id": "task.move" }),
        &caller_b,
    )
    .await
    .expect("B should be able to look up A's registered command");

    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["schema"]["id"], json!("task.move"));
}
