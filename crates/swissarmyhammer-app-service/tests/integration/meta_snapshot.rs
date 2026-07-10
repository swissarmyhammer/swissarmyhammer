//! Snapshot test for the `_meta` operations tree of the `app` tool.
//!
//! Pins the discovery surface so changes to the operation set are visible in
//! code review. The tree shape is the noun->verb->{op} layout produced by
//! `generate_operations_meta`.

use rmcp::ServerHandler;
use serde_json::Value;

use super::common::{request_context, Harness};

/// The `_meta` tree under `io.swissarmyhammer/operations` enumerates every
/// (noun, verb, op) tuple for the `app` tool. This snapshot pins the current
/// set of three shell ops; a deliberate addition / rename should update this
/// assertion in the same PR as the operation struct change.
#[tokio::test]
async fn app_tool_meta_operations_tree_is_complete() {
    let h = Harness::new();
    let service = h.service();

    let listed = service
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    assert_eq!(listed.tools.len(), 1);
    let tool = &listed.tools[0];
    assert_eq!(tool.name.as_ref(), "app");

    let meta = tool
        .meta
        .as_ref()
        .expect("app tool advertises a _meta tree");
    let ops_tree = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta carries io.swissarmyhammer/operations");

    let expected: Vec<(&str, &str, &str)> = vec![
        // (noun, verb, op-string)
        ("app", "quit", "quit app"),
        ("about", "show", "show about"),
        ("help", "show", "show help"),
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
    // Catches drift between the wire schema and the _meta tree (both generated
    // from the same operation slice).
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
