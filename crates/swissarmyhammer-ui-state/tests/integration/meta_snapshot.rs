//! Snapshot test for the `_meta` operations tree of the `ui_state` tool.
//!
//! Pins the discovery surface so changes to the operation set are visible in
//! code review. The tree shape is the noun->verb->{op} layout produced by
//! `generate_operations_meta`. Also enforces the hard constraint that the
//! `ui_state` tool exposes **no** spatial-focus op — the spatial focus KERNEL
//! is owned by the separate `focus` server. (`ui.setFocus` records the UI-state
//! focus *scope chain* via `set scope_chain`, which is a `ui_state` concern, not
//! a spatial-focus op.)

use rmcp::ServerHandler;
use serde_json::Value;

use super::common::{request_context, Harness};

/// The 17 operations the `ui_state` tool advertises, as `(noun, verb, op)`.
///
/// A deliberate addition / rename should update this list in the same PR as
/// the operation struct change.
fn expected_operations() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        // inspector
        ("inspector", "inspect", "inspect inspector"),
        ("inspector", "close", "close inspector"),
        ("inspector", "close_all", "close_all inspector"),
        ("inspector", "set_width", "set_width inspector"),
        // palette
        ("palette", "open", "open palette"),
        ("palette", "close", "close palette"),
        // keymap
        ("keymap", "set", "set keymap"),
        // scope chain (ui.setFocus routing target)
        ("scope_chain", "set", "set scope_chain"),
        // active view (view.set)
        ("active_view", "set", "set active_view"),
        // rename
        ("rename", "start", "start rename"),
        // drag
        ("drag", "start", "start drag"),
        ("drag", "cancel", "cancel drag"),
        ("drag", "complete", "complete drag"),
        // app-UI toggles
        ("command", "show", "show command"),
        ("palette", "show", "show palette"),
        ("search", "show", "show search"),
        ("ui", "dismiss", "dismiss ui"),
    ]
}

/// The `_meta` tree under `io.swissarmyhammer/operations` enumerates every
/// (noun, verb, op) tuple for the `ui_state` tool. This snapshot pins the
/// current set of 17 ops and asserts the wire `op` enum matches it exactly.
#[tokio::test]
async fn ui_state_tool_meta_operations_tree_is_complete() {
    let h = Harness::new();
    let service = h.service();

    let listed = service
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    assert_eq!(listed.tools.len(), 1);
    let tool = &listed.tools[0];
    assert_eq!(tool.name.as_ref(), "ui_state");

    let meta = tool
        .meta
        .as_ref()
        .expect("ui_state tool advertises a _meta tree");
    let ops_tree = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta carries io.swissarmyhammer/operations");

    let expected = expected_operations();
    assert_eq!(expected.len(), 17, "ui_state exposes exactly 17 operations");

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

/// Hard constraint: the `ui_state` tool owns no spatial-focus op.
///
/// The spatial focus KERNEL is owned by the separate `focus` MCP server. The
/// `ui_state` tool records only the UI-state focus *scope chain*
/// (`set scope_chain`, the `ui.setFocus` routing target) — never a spatial
/// `set_focus` / `SetFocus` op. This test fails loudly if any op string or noun
/// mentioning `focus` sneaks onto `ui_state`.
#[tokio::test]
async fn ui_state_tool_has_no_set_focus_op() {
    let h = Harness::new();
    let service = h.service();

    let listed = service
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    let tool = &listed.tools[0];

    // No op string may mention focus.
    let input_schema_op_enum = tool
        .input_schema
        .get("properties")
        .and_then(|p| p.get("op"))
        .and_then(|o| o.get("enum"))
        .and_then(|e| e.as_array())
        .expect("inputSchema.properties.op.enum is present");
    for op in input_schema_op_enum.iter().filter_map(|v| v.as_str()) {
        let lower = op.to_lowercase();
        assert!(
            !lower.contains("focus"),
            "ui_state must not expose a focus op, found {op:?}",
        );
    }

    // And the _meta tree must carry no `focus` noun.
    let meta = tool
        .meta
        .as_ref()
        .expect("ui_state advertises a _meta tree");
    let ops_tree = meta
        .0
        .get("io.swissarmyhammer/operations")
        .and_then(Value::as_object)
        .expect("_meta carries io.swissarmyhammer/operations");
    for noun in ops_tree.keys() {
        assert!(
            !noun.to_lowercase().contains("focus"),
            "ui_state _meta must not carry a focus noun, found {noun:?}",
        );
    }
}
