//! Pins that `schema command` returns the exact `params` array a command
//! was registered with — round-trip through the service's own
//! `register` verb so the test exercises the full dispatch path.

mod common;

use common::call_tool;
use serde_json::{json, Value};
use swissarmyhammer_command_service::CommandService;
use swissarmyhammer_plugin::{CallerId, PluginId};

#[tokio::test]
async fn schema_returns_registered_params_array() {
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    // Register a command with one scope-chain-resolved `task` param so the
    // schema response can be compared against the exact registered shape.
    let params = json!([
        {
            "name": "task",
            "from": "scope_chain",
            "entity_type": "task",
        }
    ]);
    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "task.move",
            "name": "Move Task",
            "execute": { "$callback": "cb_move" },
            "params": params,
        }),
        &caller,
    )
    .await
    .expect("register with params should succeed");

    let result = call_tool(
        &service,
        "schema command",
        json!({ "op": "schema command", "id": "task.move" }),
        &caller,
    )
    .await
    .expect("schema should succeed");

    let structured = result.structured_content.expect("structured response");
    assert_eq!(structured["ok"], json!(true));
    assert_eq!(structured["schema"]["id"], json!("task.move"));
    assert_eq!(
        structured["schema"]["params"], params,
        "schema must return the exact params array we registered",
    );
}

#[tokio::test]
async fn schema_for_command_with_no_params_omits_params_field() {
    // Commands registered without a `params` array carry `params: None` —
    // the schema response uses `skip_serializing_if = Option::is_none` so
    // the field is absent from the JSON, not present as `null`. Pin that
    // shape so palette code doesn't need to special-case `null` vs absent.
    let service = CommandService::new();
    let caller = CallerId::Plugin(PluginId::new("plugin-a"));

    call_tool(
        &service,
        "register command",
        json!({
            "op": "register command",
            "id": "task.touch",
            "name": "Touch Task",
            "execute": { "$callback": "cb_touch" },
        }),
        &caller,
    )
    .await
    .expect("register should succeed");

    let result = call_tool(
        &service,
        "schema command",
        json!({ "op": "schema command", "id": "task.touch" }),
        &caller,
    )
    .await
    .expect("schema should succeed");

    let structured = result.structured_content.expect("structured response");
    let schema: &Value = &structured["schema"];
    assert_eq!(schema["id"], json!("task.touch"));
    assert!(
        schema.get("params").is_none(),
        "schema for a no-params command must omit `params`, got {schema}",
    );
}
