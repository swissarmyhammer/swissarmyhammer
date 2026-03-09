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

    let result = tool.execute(args, &context).await.expect("get status failed");
    assert_eq!(result.is_error, Some(false));

    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value =
        serde_json::from_str(text).expect("Failed to parse response");

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
        let result = tool.execute(args, &context).await.expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .expect("Failed to parse response 1")
    };

    // Second call
    let result2 = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool.execute(args, &context).await.expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .expect("Failed to parse response 2")
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
        let result = tool.execute(args, &context).await.expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };
    assert!(initial_count >= 3, "Should discover at least 3 files initially");

    // Add a new file to the project
    std::fs::write(
        project.path().join("src/utils.rs"),
        "pub fn helper() -> String { String::new() }",
    )
    .unwrap();

    // Trigger build_status to mark files for re-scan
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("build status"));
        args.insert("layer".to_string(), serde_json::json!("both"));
        tool.execute(args, &context)
            .await
            .expect("build_status failed");
    }

    // get_status should now discover the new file
    let final_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool.execute(args, &context).await.expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };

    assert_eq!(
        final_count, initial_count + 1,
        "Should detect the new file. Before: {}, After: {}",
        initial_count, final_count
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
        let result = tool.execute(args, &context).await.expect("get status 1 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };
    assert!(initial_count >= 3);

    // Delete a file
    std::fs::remove_file(project.path().join("src/lib.rs")).unwrap();

    // Trigger build_status
    {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("build status"));
        args.insert("layer".to_string(), serde_json::json!("both"));
        tool.execute(args, &context)
            .await
            .expect("build_status failed");
    }

    // get_status should detect the deletion
    let final_count = {
        let mut args = serde_json::Map::new();
        args.insert("op".to_string(), serde_json::json!("get status"));
        let result = tool.execute(args, &context).await.expect("get status 2 failed");

        let text = match &result.content[0].raw {
            rmcp::model::RawContent::Text(t) => &t.text,
            _ => panic!("Expected text content"),
        };

        serde_json::from_str::<serde_json::Value>(text)
            .unwrap()["total_files"]
            .as_u64()
            .unwrap()
    };

    assert_eq!(
        final_count, initial_count - 1,
        "Should detect the deleted file. Before: {}, After: {}",
        initial_count, final_count
    );
}
