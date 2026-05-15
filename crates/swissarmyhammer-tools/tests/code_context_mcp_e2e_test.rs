//! End-to-end MCP tool tests with real filesystem discovery.
//!
//! These tests verify that the code_context MCP tool works with actual file discovery
//! from isolated temporary projects. Critical fix: get_status now triggers startup_cleanup
//! so the index is populated on first access.

use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;

/// Helper: Create a test context with a specific working directory
fn make_context(working_dir: PathBuf) -> ToolContext {
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(working_dir);
    ctx
}

/// Helper: Create a test project with source files
fn create_test_project() -> TempDir {
    let tmp = TempDir::new().expect("Failed to create temp dir");
    let root = tmp.path();

    // Create Cargo.toml
    std::fs::write(
        root.join("Cargo.toml"),
        r#"[package]
name = "test-project"
version = "0.1.0"
edition = "2021"
"#,
    )
    .unwrap();

    // Create src/main.rs with multiple functions
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/main.rs"),
        r#"fn main() {
    process_data();
}

fn process_data() {
    let result = compute(42);
    println!("Result: {}", result);
}

fn compute(x: i32) -> i32 {
    x * 2 + 1
}
"#,
    )
    .unwrap();

    // Create src/lib.rs with struct and impl
    std::fs::write(
        root.join("src/lib.rs"),
        r#"pub struct Config {
    pub value: i32,
}

impl Config {
    pub fn new(value: i32) -> Self {
        Self { value }
    }

    pub fn is_valid(&self) -> bool {
        self.value > 0
    }
}
"#,
    )
    .unwrap();

    tmp
}

// ---------------------------------------------------------------------------
// Test 1: get_status discovers files via startup_cleanup
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mcp_get_status_discovers_files() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get status"));

    let result = tool
        .execute(args, &context)
        .await
        .expect("get status failed");
    assert_eq!(result.is_error, Some(false));

    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(text).expect("Failed to parse response");

    // After startup_cleanup in get_status, should discover at least 3 files:
    // Cargo.toml, main.rs, lib.rs
    let total_files = response["total_files"]
        .as_u64()
        .expect("total_files field missing");
    assert!(
        total_files >= 3,
        "Expected at least 3 files discovered, got {}",
        total_files
    );
}

// ---------------------------------------------------------------------------
// Test 2: Multiple invocations are idempotent
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mcp_get_status_is_idempotent() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // First call
    let result1 = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse response 1")
    };

    // Second call
    let result2 = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse response 2")
    };

    // Both should report the same file count
    let files1 = result1["total_files"].as_u64().unwrap();
    let files2 = result2["total_files"].as_u64().unwrap();
    assert_eq!(
        files1, files2,
        "File count should be stable across multiple calls"
    );
}

// ---------------------------------------------------------------------------
// Test 3: File addition is detected on re-scan
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mcp_detects_new_files() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // First: get_status discovers initial files
    let initial_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };
    assert!(
        initial_count >= 3,
        "Should discover at least 3 files initially"
    );

    // Add a new file to the project
    std::fs::write(
        project.path().join("src/utils.rs"),
        "pub fn helper() -> String { String::new() }",
    )
    .unwrap();

    // Trigger rebuild_index to mark files for re-scan
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("both"));
        tool.execute(args, &context)
            .await
            .expect("rebuild_index failed");
    }

    // get_status should now discover the new file
    let final_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };

    assert_eq!(
        final_count,
        initial_count + 1,
        "Should detect the new file. Before: {}, After: {}",
        initial_count,
        final_count
    );
}

// ---------------------------------------------------------------------------
// Test 4: File deletion is detected on re-scan
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_mcp_detects_deleted_files() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // First: get_status discovers initial files
    let initial_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };
    assert!(initial_count >= 3);

    // Delete a file
    std::fs::remove_file(project.path().join("src/lib.rs")).unwrap();

    // Trigger rebuild_index
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("both"));
        tool.execute(args, &context)
            .await
            .expect("rebuild_index failed");
    }

    // get_status should detect the deletion
    let final_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text).unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };

    assert_eq!(
        final_count,
        initial_count - 1,
        "Should detect the deleted file. Before: {}, After: {}",
        initial_count,
        final_count
    );
}

// ---------------------------------------------------------------------------
// Test 5: `rebuild index` is synchronous and reports real run stats
// ---------------------------------------------------------------------------

/// `rebuild index` must wait for the indexer to finish before it returns.
///
/// Before the synchronous-rebuild change, the op was a fire-and-forget marker:
/// it flipped `ts_indexed = 0` on every row and returned immediately, leaving
/// the actual indexing for whenever a background worker noticed. The new
/// contract: by the time the MCP response lands, the dirty set has been
/// processed end-to-end and the response carries `files_indexed`,
/// `chunks_written`, and `elapsed_ms` directly — no sleeps, no polling.
///
/// This test asserts that contract: after a single `rebuild index` call,
/// `files_indexed` and `chunks_written` are both non-zero in the response,
/// and `get status` immediately reports `ts_indexed_percent == 100`.
#[tokio::test]
async fn test_rebuild_index_is_synchronous_and_reports_stats() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // 1. Prime the workspace so `indexed_files` is populated. `get status`
    //    calls `startup_cleanup` which discovers the source files written
    //    by `create_test_project` and inserts them with `ts_indexed = 0`.
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        tool.execute(args, &context)
            .await
            .expect("priming get status failed");
    }

    // 2. Call `rebuild index`. After this returns, indexing must be done.
    let rebuild_response = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("treesitter"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("rebuild index failed");
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };
        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse rebuild response")
    };

    // The response carries the new synchronous-run stats. `files_marked`
    // (the pre-existing field) must equal the dirty set we asked to
    // re-index; `files_indexed` and `chunks_written` are what the indexer
    // actually produced. We can't pin exact numbers (tree-sitter chunking
    // depends on the source layout), but each must be > 0 for a project
    // with two real Rust source files.
    let files_marked = rebuild_response["files_marked"]
        .as_u64()
        .expect("files_marked missing");
    let files_indexed = rebuild_response["files_indexed"]
        .as_u64()
        .expect("files_indexed missing");
    let chunks_written = rebuild_response["chunks_written"]
        .as_u64()
        .expect("chunks_written missing");
    assert!(
        rebuild_response.get("elapsed_ms").is_some(),
        "elapsed_ms field must be present on the rebuild response"
    );

    assert!(
        files_marked >= 2,
        "expected at least 2 files marked dirty (main.rs + lib.rs), got {}",
        files_marked
    );
    assert!(
        files_indexed >= 2,
        "expected the synchronous indexer to process at least 2 files, got {} — \
         rebuild index returned before the indexer finished",
        files_indexed
    );
    assert!(
        chunks_written > 0,
        "expected the synchronous indexer to write at least one chunk, got 0 — \
         rebuild index returned before the indexer finished"
    );

    // 3. Without any sleep or polling, `get status` must report 100%
    //    tree-sitter indexed. This is the regression check called out in
    //    the kanban task description.
    let status_response = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("post-rebuild get status failed");
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };
        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse status response")
    };

    let ts_percent = status_response["ts_indexed_percent"]
        .as_f64()
        .expect("ts_indexed_percent missing");
    assert!(
        (ts_percent - 100.0).abs() < 0.01,
        "after a synchronous rebuild, ts_indexed_percent must be 100.0 with no sleep, got {}",
        ts_percent
    );
}

// ---------------------------------------------------------------------------
// Test 6: `rebuild index` with layer=both reports real tree-sitter stats
// ---------------------------------------------------------------------------

/// `rebuild index` with `layer=both` (the default) must still synchronously
/// drive the tree-sitter indexer to completion. The LSP portion stays
/// background-driven, but the tree-sitter counters in the response must
/// reflect what the run actually produced — non-zero `files_indexed`,
/// non-zero `chunks_written`, and `ts_indexed_percent == 100` immediately
/// after the call.
///
/// This locks down the default-layer behavior. Without it, a regression that
/// only kept `layer=treesitter` synchronous would go uncaught.
#[tokio::test]
async fn test_rebuild_index_layer_both_reports_real_stats() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // Prime the workspace so `indexed_files` is populated.
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        tool.execute(args, &context)
            .await
            .expect("priming get status failed");
    }

    // Call `rebuild index` with the default layer (`both`). We explicitly
    // pass it rather than relying on the default so the test is unambiguous
    // about which contract it is locking down.
    let rebuild_response = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("both"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("rebuild index failed");
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };
        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse rebuild response")
    };

    let layer = rebuild_response["layer"]
        .as_str()
        .expect("layer field missing");
    assert_eq!(
        layer, "both",
        "rebuild response must echo the requested layer"
    );

    let files_indexed = rebuild_response["files_indexed"]
        .as_u64()
        .expect("files_indexed missing");
    let chunks_written = rebuild_response["chunks_written"]
        .as_u64()
        .expect("chunks_written missing");
    assert!(
        files_indexed >= 2,
        "expected the synchronous tree-sitter indexer to process at least 2 files \
         under layer=both, got {} — the default layer must still honour the \
         synchronous tree-sitter contract",
        files_indexed
    );
    assert!(
        chunks_written > 0,
        "expected the synchronous tree-sitter indexer to write at least one \
         chunk under layer=both, got 0"
    );

    // Layers that include LSP must carry a `note` describing the
    // asynchronous LSP contract so callers aren't misled by zeroes in the
    // LSP-side counters down the road.
    let note = rebuild_response
        .get("note")
        .and_then(|v| v.as_str())
        .expect("note field must be present for layers that include LSP");
    assert!(
        note.to_lowercase().contains("lsp"),
        "note must call out LSP, got: {}",
        note
    );
    assert!(
        note.to_lowercase().contains("async") || note.to_lowercase().contains("background"),
        "note must describe the asynchronous LSP contract, got: {}",
        note
    );

    // Immediately after the call, `ts_indexed_percent` must be 100 with no
    // polling — the tree-sitter portion of `layer=both` is synchronous.
    let status_response = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("post-rebuild get status failed");
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };
        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse status response")
    };
    let ts_percent = status_response["ts_indexed_percent"]
        .as_f64()
        .expect("ts_indexed_percent missing");
    assert!(
        (ts_percent - 100.0).abs() < 0.01,
        "after a layer=both rebuild, ts_indexed_percent must be 100.0 with no sleep, got {}",
        ts_percent
    );
}

// ---------------------------------------------------------------------------
// Test 7: `rebuild index` with layer=lsp returns zero stats + a contract note
// ---------------------------------------------------------------------------

/// `rebuild index` with `layer=lsp` flips `lsp_indexed = 0` and returns. The
/// synchronous indexer wired into this op is tree-sitter only — it queries
/// `WHERE ts_indexed = 0`, sees an empty dirty set, and exits in
/// milliseconds. The response therefore reports `files_indexed=0,
/// chunks_written=0` and must carry a `note` explaining that the LSP rebuild
/// remains an asynchronous background activity.
///
/// This is the contract called out in the task description and is the
/// behavior callers must be able to rely on until full LSP-side synchronous
/// rebuild lands.
#[tokio::test]
async fn test_rebuild_index_layer_lsp_returns_zero_stats_with_note() {
    let project = create_test_project();
    let context = make_context(project.path().to_path_buf());

    let tool = CodeContextTool::new();

    // Prime the workspace so `indexed_files` is populated, then drive the
    // tree-sitter indexer to completion. After this, `ts_indexed = 1` on
    // every row, so the tree-sitter dirty set is empty — the layer=lsp
    // call below cannot pick up any tree-sitter work.
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        tool.execute(args, &context)
            .await
            .expect("priming get status failed");
    }
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("treesitter"));
        tool.execute(args, &context)
            .await
            .expect("priming tree-sitter rebuild failed");
    }

    let rebuild_response = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("rebuild index"));
        args.insert("layer".to_string(), serde_json::json!("lsp"));
        let result = tool
            .execute(args, &context)
            .await
            .expect("rebuild index failed");
        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };
        serde_json::from_str::<serde_json::Value>(text).expect("Failed to parse rebuild response")
    };

    let layer = rebuild_response["layer"]
        .as_str()
        .expect("layer field missing");
    assert_eq!(
        layer, "lsp",
        "rebuild response must echo the requested layer"
    );

    let files_indexed = rebuild_response["files_indexed"]
        .as_u64()
        .expect("files_indexed missing");
    let chunks_written = rebuild_response["chunks_written"]
        .as_u64()
        .expect("chunks_written missing");
    assert_eq!(
        files_indexed, 0,
        "layer=lsp must not drive the tree-sitter indexer; expected \
         files_indexed=0, got {}",
        files_indexed
    );
    assert_eq!(
        chunks_written, 0,
        "layer=lsp must not produce tree-sitter chunks; expected \
         chunks_written=0, got {}",
        chunks_written
    );

    // The response must carry a `note` explaining that the synchronous
    // counters are tree-sitter only and the LSP rebuild remains asynchronous.
    let note = rebuild_response
        .get("note")
        .and_then(|v| v.as_str())
        .expect("note field must be present for layer=lsp");
    assert!(
        note.to_lowercase().contains("lsp"),
        "note must call out LSP, got: {}",
        note
    );
    assert!(
        note.to_lowercase().contains("async") || note.to_lowercase().contains("background"),
        "note must describe the asynchronous LSP contract, got: {}",
        note
    );
}
