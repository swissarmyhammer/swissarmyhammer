//! RMCP client test for MCP server
//!
//! Tests RMCP client functionality using in-process HTTP server instead of subprocess.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server_with_options, McpServerMode},
};

/// Test RMCP client lists tools and prompts (Fast In-Process)
///
/// Tests RMCP client functionality without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests tool listing, prompt listing, and tool calls
/// - Fast execution (<1s instead of 20-30s)
#[tokio::test]
async fn test_rmcp_client_lists_tools_and_prompts() {
    // Start in-process HTTP MCP server.
    // Use agent_mode=true since this test checks for agent tools (files).
    // Pass an isolated temp dir as working_dir so that `initialize_code_context`
    // sees no enclosing git repository and skips the synchronous
    // startup_cleanup walk — which would otherwise hash every file in the
    // host workspace on first connection.
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
        true,
    )
    .await
    .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List tools
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    assert!(!tools.tools.is_empty(), "Server should provide tools");

    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();

    assert!(
        tool_names.contains(&"files".to_string()),
        "Should have files tool"
    );

    // List prompts — the `expect` above already verifies the call succeeds;
    // prompt contents depend on host configuration so we don't assert on them.
    let _prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Test a tool call to verify full functionality
    let tool_result = client
        .call_tool(
            CallToolRequestParams::new("files").with_arguments(
                serde_json::json!({
                    "op": "grep files",
                    "pattern": "test"
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            ),
        )
        .await
        .expect("Tool call should work");

    assert!(
        !tool_result.content.is_empty(),
        "Tool should return content"
    );

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}
