//! Snapshot test for the `_meta` operations tree of the `window` tool.
//!
//! Pins the discovery surface so changes to the operation set are visible in
//! code review. The tree shape is the noun->verb->{op} layout produced by
//! `generate_operations_meta`.

use rmcp::ServerHandler;
use serde_json::Value;

use super::common::{request_context, Harness};

/// The `_meta` tree under `io.swissarmyhammer/operations` enumerates every
/// (noun, verb, op) tuple for the `window` tool. This snapshot pins the current
/// set of fifteen ops across five groups (window + OS-file actions + board
/// lifecycle + app-wide window affordances + board-management reads); a
/// deliberate addition / rename should update this assertion in the same PR as
/// the operation struct change.
#[tokio::test]
async fn window_tool_meta_operations_tree_is_complete() {
    let h = Harness::new();
    let service = h.service();

    let listed = service
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    assert_eq!(listed.tools.len(), 1);
    let tool = &listed.tools[0];
    assert_eq!(tool.name.as_ref(), "window");

    let meta = tool
        .meta
        .as_ref()
        .expect("window tool advertises a _meta tree");
    let ops_tree = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta carries io.swissarmyhammer/operations");

    let expected: Vec<(&str, &str, &str)> = vec![
        // (noun, verb, op-string) — window group
        ("window", "new", "new window"),
        ("window", "activate", "activate window"),
        ("position", "set", "set position"),
        ("position", "get", "get position"),
        ("monitors", "get", "get monitors"),
        ("window", "close", "close window"),
        // OS-file actions group
        ("path", "open", "open path"),
        ("path", "reveal", "reveal path"),
        // board-lifecycle group
        ("board", "switch", "switch board"),
        ("board", "close", "close board"),
        ("board", "new", "new board"),
        ("board", "open", "open board"),
        // app-wide window affordances group
        ("context menu", "show", "show context menu"),
        // board-management reads group
        ("open boards", "list", "list open boards"),
        ("board data", "get", "get board data"),
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

/// The `_meta` tree under `io.swissarmyhammer/notifications` advertises every
/// board-lifecycle event a plugin resolves with `this.window.on("board.opened")`
/// (and `board.switched` / `board.closed`).
///
/// Production-path assertion: it drives the real `WindowService::list_tools`,
/// not just the `_meta` generator, so it pins the discovery surface the SDK's
/// `.on()` resolves against — complementing the declared ⟺ raised coverage
/// guard in `operations.rs`. The `_meta` tree is keyed by the SHORT event name
/// (each leaf's `method` is the wire notification method the host re-broadcasts
/// as a Tauri event of the same name); the explicit `board.*` short events keep
/// the `closed` event from colliding with the sibling raw-window lifecycle's
/// `window.closed` in this shared `window` tool.
#[tokio::test]
async fn window_tool_meta_advertises_board_lifecycle_notifications() {
    let h = Harness::new();
    let service = h.service();

    let listed = service
        .list_tools(None, request_context())
        .await
        .expect("list_tools should succeed");
    let tool = &listed.tools[0];
    assert_eq!(tool.name.as_ref(), "window");

    let meta = tool
        .meta
        .as_ref()
        .expect("window tool advertises a _meta tree");
    let notifications_tree = meta
        .0
        .get("io.swissarmyhammer/notifications")
        .and_then(Value::as_object)
        .expect("_meta carries io.swissarmyhammer/notifications");

    // (explicit two-segment short event → declared wire method)
    let expected: Vec<(&str, &str)> = vec![
        ("board.opened", "notifications/board/opened"),
        ("board.switched", "notifications/board/switched"),
        ("board.closed", "notifications/board/closed"),
    ];
    for (event, method) in &expected {
        let leaf = notifications_tree
            .get(*event)
            .unwrap_or_else(|| panic!("_meta must declare the {event:?} board event"));
        assert_eq!(
            leaf.get("method"),
            Some(&Value::String((*method).to_string())),
            "_meta notification {event:?}.method must equal {method:?}",
        );
    }
}
