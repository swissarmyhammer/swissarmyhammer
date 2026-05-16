//! Runtime tests for the `operation_tool!` macro.
//!
//! These assertions live in a `tests/` integration target rather than inside
//! the proc-macro crate's own `src/` because a proc-macro crate cannot link
//! the runtime crates the generated code references (`rmcp`, `serde_json`,
//! `swissarmyhammer-operations`). An integration test target compiles as an
//! ordinary library, so it can pull those crates in as `[dev-dependencies]`,
//! invoke the macro for real, and inspect the `rmcp::model::Tool` it produces.

use rmcp::model::Tool;
use serde_json::Value;
use swissarmyhammer_operations::schema::generate_operations_meta;
use swissarmyhammer_operations::{operation, operation_tool, Operation};
use swissarmyhammer_operations_macros as _;

/// A minimal "add task" operation declared with the existing `#[operation]`
/// macro — the macro under test consumes a slice of these.
///
/// The fields ARE the operation parameters: `#[operation]` reads their names,
/// types, doc comments, and `Option` wrapping to synthesize `ParamMeta`. The
/// values themselves are never read at runtime, hence `#[allow(dead_code)]`.
#[operation(verb = "add", noun = "task", description = "Create a new task")]
#[allow(dead_code)]
struct AddTask {
    /// The task title
    title: String,
    /// Optional description
    description: Option<String>,
}

/// A minimal "get task" operation sharing the `task` noun with `AddTask`.
///
/// See [`AddTask`] for why the field carries `#[allow(dead_code)]`.
#[operation(verb = "get", noun = "task", description = "Get a task by id")]
#[allow(dead_code)]
struct GetTask {
    /// The task id
    id: String,
}

/// Build the canonical operation slice for the demo tool.
fn demo_operations() -> Vec<&'static dyn Operation> {
    vec![
        Box::leak(Box::new(AddTask {
            title: String::new(),
            description: None,
        })) as &dyn Operation,
        Box::leak(Box::new(GetTask { id: String::new() })) as &dyn Operation,
    ]
}

#[test]
fn operation_tool_attaches_operations_meta() {
    let tool: Tool = operation_tool! {
        name: "demo",
        description: "Demo operation tool",
        operations: demo_operations(),
    };

    // The tool name and description flow straight through.
    assert_eq!(tool.name, "demo");
    assert_eq!(tool.description.as_deref(), Some("Demo operation tool"));

    // `_meta` must carry the discovery tree under the well-known key.
    let meta = tool.meta.expect("operation tool must have _meta");
    let ops_meta = meta
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta must carry io.swissarmyhammer/operations");

    // noun -> verb -> { op, ... }
    assert_eq!(ops_meta["task"]["add"]["op"], "add task");
    assert_eq!(ops_meta["task"]["get"]["op"], "get task");
    assert_eq!(ops_meta["task"]["add"]["description"], "Create a new task");
    assert_eq!(
        ops_meta["task"]["add"]["parameters"]["title"]["required"],
        true
    );
}

#[test]
fn operation_tool_input_schema_is_flat_op_enum() {
    let tool: Tool = operation_tool! {
        name: "demo",
        description: "Demo operation tool",
        operations: demo_operations(),
    };

    // The flat wire schema keeps `op` as the single selector with an enum
    // listing every operation.
    let schema: &serde_json::Map<String, Value> = &tool.input_schema;
    let op_enum = schema["properties"]["op"]["enum"]
        .as_array()
        .expect("inputSchema.properties.op.enum must be an array");
    let ops: Vec<&str> = op_enum.iter().filter_map(|v| v.as_str()).collect();
    assert!(ops.contains(&"add task"), "op enum should list 'add task'");
    assert!(ops.contains(&"get task"), "op enum should list 'get task'");
}

#[test]
fn operation_tool_meta_is_byte_identical_to_generator() {
    // There must be no second source of truth: the `_meta` the macro emits
    // must equal calling `generate_operations_meta` over the same slice.
    let tool: Tool = operation_tool! {
        name: "demo",
        description: "Demo operation tool",
        operations: demo_operations(),
    };

    let macro_meta = tool
        .meta
        .expect("operation tool must have _meta")
        .0
        .get("io.swissarmyhammer/operations")
        .expect("_meta must carry io.swissarmyhammer/operations")
        .clone();

    let direct_meta = generate_operations_meta(&demo_operations());

    assert_eq!(
        serde_json::to_string(&macro_meta).unwrap(),
        serde_json::to_string(&direct_meta).unwrap(),
        "macro-generated _meta must be byte-identical to generate_operations_meta",
    );
}

#[test]
fn flat_tool_is_not_mistagged_as_operation_tool() {
    // Negative control: a hand-built flat tool that never went through the
    // `operation_tool!` macro must not carry the operations `_meta` key, so a
    // consumer can reliably distinguish flat tools from operation tools.
    let schema = serde_json::json!({
        "type": "object",
        "properties": { "path": { "type": "string" } },
    });
    let schema_map = match schema {
        Value::Object(map) => map,
        _ => unreachable!(),
    };
    let flat_tool = Tool::new("flat", "A flat, non-operation tool", schema_map);

    match &flat_tool.meta {
        None => {}
        Some(meta) => assert!(
            !meta.0.contains_key("io.swissarmyhammer/operations"),
            "a flat tool must not be tagged with operations _meta",
        ),
    }
}
