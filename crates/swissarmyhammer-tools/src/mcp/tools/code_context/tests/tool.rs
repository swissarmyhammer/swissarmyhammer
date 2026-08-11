//! The `code_context` tool surface: registration, metadata, both schemas, and
//! what dispatch says about an op the tool does not have.

use crate::mcp::tool_registry::{McpTool, ToolRegistry};
use crate::mcp::tools::code_context::support::lsp_degradation_notice;
use crate::mcp::tools::code_context::{register_code_context_tools, CodeContextTool};

#[test]
fn test_register_code_context_tools() {
    let mut registry = ToolRegistry::new();
    assert_eq!(registry.len(), 0);

    register_code_context_tools(&mut registry);

    assert_eq!(registry.len(), 1);
    assert!(registry.get_tool("code_context").is_some());
}

#[test]
fn test_code_context_tool_name() {
    let tool = CodeContextTool::new();
    assert_eq!(<CodeContextTool as McpTool>::name(&tool), "code_context");
}

#[test]
fn test_code_context_tool_has_description() {
    let tool = CodeContextTool::new();
    assert!(!tool.description().is_empty());
}

#[test]
fn test_code_context_tool_has_operations() {
    let tool = CodeContextTool::new();
    let ops = tool.operations();
    assert_eq!(ops.len(), 26);
    assert!(ops.iter().any(|o| o.op_string() == "get symbol"));
    assert!(ops.iter().any(|o| o.op_string() == "search symbol"));
    assert!(ops.iter().any(|o| o.op_string() == "list symbols"));
    assert!(ops.iter().any(|o| o.op_string() == "grep code"));
    assert!(ops.iter().any(|o| o.op_string() == "search code"));
    assert!(ops.iter().any(|o| o.op_string() == "find duplicates"));
    assert!(ops.iter().any(|o| o.op_string() == "query ast"));
    assert!(ops.iter().any(|o| o.op_string() == "find duplication"));
    assert!(ops.iter().any(|o| o.op_string() == "find commented_code"));
    assert!(ops.iter().any(|o| o.op_string() == "get callgraph"));
    assert!(ops.iter().any(|o| o.op_string() == "get blastradius"));
    assert!(ops.iter().any(|o| o.op_string() == "get status"));
    assert!(ops.iter().any(|o| o.op_string() == "rebuild index"));
    assert!(ops.iter().any(|o| o.op_string() == "clear status"));
    assert!(ops.iter().any(|o| o.op_string() == "lsp status"));
    assert!(ops.iter().any(|o| o.op_string() == "detect projects"));
    assert!(ops.iter().any(|o| o.op_string() == "get rename_edits"));
    assert!(ops.iter().any(|o| o.op_string() == "get diagnostics"));
    assert!(ops.iter().any(|o| o.op_string() == "get inbound_calls"));
    assert!(ops
        .iter()
        .any(|o| o.op_string() == "search workspace_symbol"));
    assert!(ops.iter().any(|o| o.op_string() == "get definition"));
    assert!(ops.iter().any(|o| o.op_string() == "get type_definition"));
    assert!(ops.iter().any(|o| o.op_string() == "get hover"));
    assert!(ops.iter().any(|o| o.op_string() == "get references"));
    assert!(ops.iter().any(|o| o.op_string() == "get implementations"));
    assert!(ops.iter().any(|o| o.op_string() == "get code_actions"));
}

#[test]
fn test_code_context_tool_schema_has_op_field() {
    let tool = CodeContextTool::new();
    let schema = tool.schema();

    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["op"].is_object());

    let op_enum = schema["properties"]["op"]["enum"]
        .as_array()
        .expect("op should have enum");
    assert!(op_enum.contains(&serde_json::json!("get symbol")));
    assert!(op_enum.contains(&serde_json::json!("search symbol")));
    assert!(op_enum.contains(&serde_json::json!("list symbols")));
    assert!(op_enum.contains(&serde_json::json!("grep code")));
    assert!(op_enum.contains(&serde_json::json!("query ast")));
    assert!(op_enum.contains(&serde_json::json!("get callgraph")));
    assert!(op_enum.contains(&serde_json::json!("get blastradius")));
    assert!(op_enum.contains(&serde_json::json!("get status")));
    assert!(op_enum.contains(&serde_json::json!("rebuild index")));
    assert!(op_enum.contains(&serde_json::json!("clear status")));
    assert!(op_enum.contains(&serde_json::json!("lsp status")));
    assert!(op_enum.contains(&serde_json::json!("detect projects")));
}

#[test]
fn test_code_context_tool_full_schema_has_operation_schemas() {
    let tool = CodeContextTool::new();
    let schema = tool.schema_full();

    let op_schemas = schema["x-operation-schemas"]
        .as_array()
        .expect("should have x-operation-schemas");
    assert_eq!(op_schemas.len(), 26);

    // The per-op signature map is carried on the full schema.
    assert!(schema["x-op-signatures"].is_object());
}

#[test]
fn test_code_context_tool_wire_schema_omits_operation_schemas() {
    let tool = CodeContextTool::new();
    let schema = tool.schema();

    assert!(
        schema.get("x-operation-schemas").is_none(),
        "wire schema must omit x-operation-schemas"
    );
    // `x-op-signatures` is full-only; the wire surface omits it.
    assert!(
        schema.get("x-op-signatures").is_none(),
        "wire schema must omit x-op-signatures"
    );
}

#[tokio::test]
async fn test_code_context_tool_unknown_op() {
    let tool = CodeContextTool::new();
    let context = crate::test_utils::create_test_context().await;

    let mut args = serde_json::Map::new();
    args.insert(
        "op".to_string(),
        serde_json::Value::String("invalid op".to_string()),
    );

    let result = tool.execute(args, &context).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unknown operation"));
}

#[tokio::test]
async fn test_code_context_tool_missing_op() {
    let tool = CodeContextTool::new();
    let context = crate::test_utils::create_test_context().await;

    let args = serde_json::Map::new();

    let result = tool.execute(args, &context).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing 'op' field"));
}

#[test]
fn test_lsp_degradation_notice_no_supervisor() {
    // When LSP_SUPERVISOR is not set and no projects, should return None
    let tmp = tempfile::tempdir().unwrap();
    assert!(lsp_degradation_notice(tmp.path()).is_none());
}

#[test]
fn test_lsp_degradation_notice_with_project() {
    let tmp = tempfile::tempdir().unwrap();
    std::fs::write(
        tmp.path().join("Cargo.toml"),
        "[package]\nname = \"test\"\n",
    )
    .unwrap();
    let notice = lsp_degradation_notice(tmp.path());
    // If rust-analyzer is installed, notice is None; if not, it should contain the hint
    if let Some(text) = notice {
        assert!(text.contains("tree-sitter only"));
        assert!(text.contains("rust-analyzer"));
    }
}

// -----------------------------------------------------------------------
// Integration tests for operation dispatch and query execution
//
// These tests require access to `index_discovered_files_async` and must
// therefore live in the unit test module rather than the external
// integration test files.
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// Operation dispatch: missing/invalid op
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_dispatch_unknown_op_returns_error() {
    let tool = CodeContextTool::new();
    let ctx = crate::test_utils::create_test_context().await;
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("not an op"));
    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("unknown operation"));
}

#[tokio::test]
async fn test_dispatch_empty_op_returns_error() {
    let tool = CodeContextTool::new();
    let ctx = crate::test_utils::create_test_context().await;
    let args = serde_json::Map::new(); // no "op" key
    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("missing 'op' field"));
}
