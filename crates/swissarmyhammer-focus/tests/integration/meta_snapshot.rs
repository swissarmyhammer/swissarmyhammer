//! Snapshot test for the `_meta` operations tree of the `focus` tool.
//!
//! Pins the discovery surface so changes to the operation set are visible in
//! code review. The tree shape is the noun->verb->{op} layout produced by
//! `generate_operations_meta`. Every `spatial_*` Tauri command must have a
//! corresponding op here.

use rmcp::ServerHandler;
use serde_json::Value;
use swissarmyhammer_focus::FocusServer;

use super::common::request_context;

/// The `_meta` tree under `io.swissarmyhammer/operations` enumerates every
/// (noun, verb, op) tuple for the `focus` tool. This snapshot pins the current
/// set of eight ops — one per `spatial_*` Tauri command — so a deliberate
/// addition / rename updates this assertion in the same PR as the op struct
/// change.
#[tokio::test]
async fn focus_tool_meta_operations_tree_is_complete() {
    let server = FocusServer::new();

    let listed = server
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    assert_eq!(listed.tools.len(), 1);
    let tool = &listed.tools[0];
    assert_eq!(tool.name.as_ref(), "focus");

    let meta = tool
        .meta
        .as_ref()
        .expect("focus tool advertises a _meta tree");
    let ops_tree = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta carries io.swissarmyhammer/operations");

    // (noun, verb, op-string) — one row per spatial_* Tauri command.
    let expected: Vec<(&str, &str, &str)> = vec![
        ("focus", "set", "set focus"),            // spatial_focus / ui.setFocus
        ("focus", "clear", "clear focus"),        // spatial_clear_focus
        ("focus", "navigate", "navigate focus"),  // spatial_navigate
        ("focus", "lose", "lose focus"),          // spatial_focus_lost
        ("layer", "push", "push layer"),          // spatial_push_layer
        ("layer", "pop", "pop layer"),            // spatial_pop_layer
        ("layer", "drill_in", "drill_in layer"),  // spatial_drill_in
        ("layer", "drill_out", "drill_out layer"), // spatial_drill_out
    ];

    for (noun, verb, op_str) in &expected {
        let leaf = ops_tree
            .get(noun)
            .and_then(|n| n.get(verb))
            .unwrap_or_else(|| panic!("_meta missing tree path {noun}/{verb}"));
        assert_eq!(
            leaf.get("op"),
            Some(&Value::String((*op_str).to_string())),
            "_meta tree {noun}/{verb}.op must equal {op_str:?}",
        );
    }

    // The inputSchema's `op` enum must list every op string exactly once.
    let input_schema_op_enum = tool
        .input_schema
        .get("properties")
        .and_then(|p| p.get("op"))
        .and_then(|o| o.get("enum"))
        .and_then(|e| e.as_array())
        .expect("inputSchema.properties.op.enum is present");
    let mut wire_ops: Vec<&str> = input_schema_op_enum
        .iter()
        .filter_map(|v| v.as_str())
        .collect();
    wire_ops.sort();
    let mut expected_ops: Vec<&str> = expected.iter().map(|(_, _, op)| *op).collect();
    expected_ops.sort();
    assert_eq!(
        wire_ops, expected_ops,
        "inputSchema op enum must match the _meta tree's op strings",
    );
}
