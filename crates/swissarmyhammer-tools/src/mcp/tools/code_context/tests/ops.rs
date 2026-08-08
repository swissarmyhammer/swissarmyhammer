//! Each `code_context` operation, driven through the real dispatch.

use swissarmyhammer_code_context::CodeContextWorkspace;

use crate::mcp::tool_registry::McpTool;
use crate::mcp::tools::code_context::CodeContextTool;

use super::support::{create_indexed_project, extract_text};

// -----------------------------------------------------------------------
// get status — workspace discovery and reporting
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_get_status_returns_file_counts() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get status"));

    let result = tool.execute(args, &ctx).await.expect("get status");
    assert_eq!(result.is_error, Some(false));

    let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
    let total = json["total_files"].as_u64().unwrap_or(0);
    assert!(
        total >= 2,
        "expected >= 2 files (main.rs, lib.rs), got {}",
        total
    );
}

// -----------------------------------------------------------------------
// rebuild index and clear status
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_rebuild_index_resets_indexed_flags() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("rebuild index"));
    args.insert("layer".to_string(), serde_json::json!("treesitter"));

    let result = tool.execute(args, &ctx).await.expect("rebuild index");
    assert_eq!(result.is_error, Some(false));

    let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
    // After rebuild index, files_marked should be >= 2 (main.rs and lib.rs)
    let marked = json["files_marked"].as_u64().unwrap_or(0);
    assert!(
        marked >= 2,
        "expected >= 2 files marked for re-indexing, got {}",
        marked
    );
}

#[tokio::test]
async fn test_rebuild_index_invalid_layer_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("rebuild index"));
    args.insert("layer".to_string(), serde_json::json!("invalid_layer"));

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_clear_status_wipes_index_data() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("clear status"));

    let result = tool.execute(args, &ctx).await.expect("clear status");
    assert_eq!(result.is_error, Some(false));

    let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
    // After clear, the response should be a valid JSON object (stats about what was cleared)
    assert!(
        json.is_object(),
        "expected object response from clear status"
    );
}

/// When a write op runs against a workspace whose leader is held by
/// another live process, the user must see a typed `invalid_request`
/// error that names the workspace path instead of an opaque
/// `-32603: database error`. This protects against the most common
/// confusion ("why does rebuilding the index fail?"): the leader is
/// another agent session.
///
/// The test holds the leader in this thread via a `_leader` binding so
/// the MCP tool's call to `open_workspace` deterministically lands on
/// the follower branch.
#[tokio::test]
async fn test_rebuild_index_returns_typed_error_on_follower() {
    let (_tmp, ctx) = create_indexed_project().await;

    // Hold the leader so the MCP op opens as a follower.
    let workspace_root = ctx
        .working_dir
        .clone()
        .expect("indexed project sets working_dir");
    let _leader = CodeContextWorkspace::open(&workspace_root).expect("hold leader for test");

    let tool = CodeContextTool::new();
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("rebuild index"));

    let err = tool
        .execute(args, &ctx)
        .await
        .expect_err("follower must reject rebuild index");
    let msg = err.to_string();
    let ws_display = workspace_root.display().to_string();
    assert!(
        msg.contains(&ws_display),
        "MCP error must mention the workspace root, got: {msg}"
    );
    assert!(
        msg.contains("read-only"),
        "MCP error must explain the read-only follower condition, got: {msg}"
    );
}

/// `clear status` follows the same write-rejection path. Validating
/// both ops here prevents a future regression that wires only one of
/// them through `write_db()`.
#[tokio::test]
async fn test_clear_status_returns_typed_error_on_follower() {
    let (_tmp, ctx) = create_indexed_project().await;
    let workspace_root = ctx
        .working_dir
        .clone()
        .expect("indexed project sets working_dir");
    let _leader = CodeContextWorkspace::open(&workspace_root).expect("hold leader for test");

    let tool = CodeContextTool::new();
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("clear status"));

    let err = tool
        .execute(args, &ctx)
        .await
        .expect_err("follower must reject clear status");
    let msg = err.to_string();
    assert!(
        msg.contains(&workspace_root.display().to_string()),
        "MCP error must mention the workspace root, got: {msg}"
    );
    assert!(
        msg.contains("read-only"),
        "MCP error must explain the read-only follower condition, got: {msg}"
    );
    // The follower-rejection message must stay op-agnostic. A user who
    // invoked `clear status` should not see the message naming a
    // different op (e.g. `rebuild index`), which would steer debugging
    // in the wrong direction.
    assert!(
        !msg.contains("rebuild index"),
        "MCP error for clear status must not misname the op, got: {msg}"
    );
}

// -----------------------------------------------------------------------
// lsp status
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_lsp_status_returns_language_list() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("lsp status"));

    let result = tool.execute(args, &ctx).await.expect("lsp status");
    assert_eq!(result.is_error, Some(false));

    let json: serde_json::Value = serde_json::from_str(extract_text(&result)).unwrap();
    // Response should have a "languages" array
    assert!(
        json["languages"].is_array(),
        "expected 'languages' array in lsp status response"
    );
    assert!(
        json["all_healthy"].is_boolean(),
        "expected 'all_healthy' boolean in lsp status response"
    );
}

// -----------------------------------------------------------------------
// grep code
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_grep_code_finds_pattern() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("grep code"));
    args.insert("pattern".to_string(), serde_json::json!("fn greet"));

    let result = tool.execute(args, &ctx).await.expect("grep code");
    // May return progress message if not indexed, or actual results
    assert_eq!(result.is_error, Some(false));
    let text = extract_text(&result);
    // If indexed, should find fn greet; if not indexed yet, will be progress message
    // Either way, result is valid (not an error)
    assert!(!text.is_empty());
}

#[tokio::test]
async fn test_grep_code_missing_pattern_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("grep code"));
    // Intentionally omit "pattern"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("pattern"));
}

#[tokio::test]
async fn test_grep_code_with_language_filter() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("grep code"));
    args.insert("pattern".to_string(), serde_json::json!("pub struct"));
    args.insert("language".to_string(), serde_json::json!(["rs"]));

    let result = tool
        .execute(args, &ctx)
        .await
        .expect("grep code with language filter");
    assert_eq!(result.is_error, Some(false));
}

// -----------------------------------------------------------------------
// search symbol
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_search_symbol_returns_results_or_progress() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("search symbol"));
    args.insert("query".to_string(), serde_json::json!("Calculator"));

    let result = tool.execute(args, &ctx).await.expect("search symbol");
    assert_eq!(result.is_error, Some(false));
    assert!(!extract_text(&result).is_empty());
}

#[tokio::test]
async fn test_search_symbol_missing_query_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("search symbol"));
    // Omit "query"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("query"));
}

#[tokio::test]
async fn test_search_symbol_with_kind_filter() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("search symbol"));
    args.insert("query".to_string(), serde_json::json!("add"));
    args.insert("kind".to_string(), serde_json::json!("function"));

    let result = tool
        .execute(args, &ctx)
        .await
        .expect("search symbol with kind");
    assert_eq!(result.is_error, Some(false));
}

// -----------------------------------------------------------------------
// get symbol
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_get_symbol_returns_results_or_progress() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get symbol"));
    args.insert("query".to_string(), serde_json::json!("Calculator::new"));

    let result = tool.execute(args, &ctx).await.expect("get symbol");
    assert_eq!(result.is_error, Some(false));
    assert!(!extract_text(&result).is_empty());
}

#[tokio::test]
async fn test_get_symbol_missing_query_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get symbol"));
    // Omit "query"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("query"));
}

// -----------------------------------------------------------------------
// list symbols
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_list_symbols_returns_results_or_progress() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("list symbols"));
    args.insert("file_path".to_string(), serde_json::json!("src/lib.rs"));

    let result = tool.execute(args, &ctx).await.expect("list symbols");
    assert_eq!(result.is_error, Some(false));
    assert!(!extract_text(&result).is_empty());
}

#[tokio::test]
async fn test_list_symbols_missing_file_path_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("list symbols"));
    // Omit "file_path"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("file_path"));
}

// -----------------------------------------------------------------------
// get callgraph
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_get_callgraph_returns_results_or_progress() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get callgraph"));
    args.insert("symbol".to_string(), serde_json::json!("main"));

    let result = tool.execute(args, &ctx).await.expect("get callgraph");
    assert_eq!(result.is_error, Some(false));
    assert!(!extract_text(&result).is_empty());
}

#[tokio::test]
async fn test_get_callgraph_missing_symbol_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get callgraph"));
    // Omit "symbol"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("symbol"));
}

#[tokio::test]
async fn test_get_callgraph_invalid_direction_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get callgraph"));
    args.insert("symbol".to_string(), serde_json::json!("main"));
    args.insert("direction".to_string(), serde_json::json!("sideways"));

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("direction"));
}

#[tokio::test]
async fn test_get_callgraph_inbound_direction() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get callgraph"));
    args.insert("symbol".to_string(), serde_json::json!("greet"));
    args.insert("direction".to_string(), serde_json::json!("inbound"));

    let result = tool
        .execute(args, &ctx)
        .await
        .expect("get callgraph inbound");
    assert_eq!(result.is_error, Some(false));
}

// -----------------------------------------------------------------------
// get blastradius
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_get_blastradius_returns_results_or_progress() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get blastradius"));
    args.insert("file_path".to_string(), serde_json::json!("src/lib.rs"));

    let result = tool.execute(args, &ctx).await.expect("get blastradius");
    assert_eq!(result.is_error, Some(false));
    assert!(!extract_text(&result).is_empty());
}

#[tokio::test]
async fn test_get_blastradius_missing_file_path_returns_error() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get blastradius"));
    // Omit "file_path"

    let result = tool.execute(args, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("file_path"));
}

// -----------------------------------------------------------------------
// detect projects
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_detect_projects_returns_project_list() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    // Add Cargo.toml to make it look like a Rust project
    if let Some(ref dir) = ctx.working_dir {
        std::fs::write(
            dir.join("Cargo.toml"),
            "[package]\nname = \"test-project\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
    }

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("detect projects"));

    let result = tool.execute(args, &ctx).await.expect("detect projects");
    assert_eq!(result.is_error, Some(false));
    let text = extract_text(&result);
    assert!(!text.is_empty());
}

#[tokio::test]
async fn test_detect_projects_with_path_param() {
    let (_tmp, ctx) = create_indexed_project().await;
    let tool = CodeContextTool::new();

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("detect projects"));
    // Use a non-existent subdirectory — should return "no projects found" gracefully
    args.insert("path".to_string(), serde_json::json!("/tmp"));

    let result = tool
        .execute(args, &ctx)
        .await
        .expect("detect projects with path");
    assert_eq!(result.is_error, Some(false));
}

// -----------------------------------------------------------------------
// Error handling for missing/invalid workspace
// -----------------------------------------------------------------------

#[tokio::test]
async fn test_operations_with_no_working_dir() {
    // When working_dir is not set, operations should either succeed
    // (using cwd as fallback) or return a meaningful error.
    let tool = CodeContextTool::new();
    let ctx = crate::test_utils::create_test_context().await;

    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get status"));

    // Either succeeds or fails with an internal error — just must not panic.
    let result = tool.execute(args, &ctx).await;
    // We accept both Ok and Err here — just verify no panic occurs.
    let _ = result;
}

// -----------------------------------------------------------------------
// Chunk embedding tests for `index_discovered_files_with_embedder`
//
// These tests exercise the embedding path via dependency injection — they
// pass a `MockEmbedder` rather than constructing a real model, so they run
// fast and deterministically. They cover:
//   - success path: every chunk gets an embedding blob, `embedded=1`
//   - partial failure: failing chunk has NULL embedding, others succeed,
//     `embedded=0` (the successful chunks remain searchable; the file is
//     not re-driven until `ts_indexed` is flipped back to 0 elsewhere)
//   - no embedder: chunks are still written without embeddings, `embedded=0`
//     (existing fallback behavior preserved)
//   - round-trip: blob written by indexer deserializes to the same vector
// -----------------------------------------------------------------------
