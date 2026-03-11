//! Test the code_context tool in a realistic MCP scenario
//! This mimics what happens when Claude Code invokes the tool through MCP

use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;
use tokio::sync::Mutex as TokioMutex;

/// Test calling code_context tool with the workspace root project.
/// Uses CARGO_MANIFEST_DIR to find the project, so it works on CI too.
#[tokio::test]
#[ignore]
async fn test_code_context_on_real_project() {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));

    // Set up the context as it would be in real MCP usage
    let git_ops = Arc::new(TokioMutex::new(None));
    let tool_handlers = Arc::new(ToolHandlers::new());
    let agent_config = Arc::new(ModelConfig::default());
    let mut ctx = ToolContext::new(tool_handlers, git_ops, agent_config);
    ctx.working_dir = Some(project_root.clone());

    let tool = CodeContextTool::new();
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), serde_json::json!("get status"));

    let result = tool
        .execute(args, &ctx)
        .await
        .expect("Tool execution failed");
    assert_eq!(result.is_error, Some(false), "Tool should not error");

    let text = match &result.content[0].raw {
        rmcp::model::RawContent::Text(t) => &t.text,
        _ => panic!("Expected text content"),
    };

    let response: serde_json::Value = serde_json::from_str(text).expect("Failed to parse response");

    // The critical assertion: files should be discovered
    let total_files = response["total_files"]
        .as_u64()
        .expect("total_files field missing");

    println!("Files discovered: {}", total_files);
    println!(
        "Full response:\n{}",
        serde_json::to_string_pretty(&response).unwrap()
    );

    // On the swissarmyhammer-tools project, we should discover at least some files
    assert!(
        total_files > 0,
        "Expected to discover files from swissarmyhammer-tools project, got {}",
        total_files
    );
}
