//! Integration tests using rmcp client to test our MCP servers
//!
//! These tests use the rmcp client library to connect to our servers
//! and verify they implement the MCP protocol correctly.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server_with_options, McpServerMode},
};

/// Test MCP server with RMCP client (Fast In-Process)
///
/// Tests MCP server tool/prompt functionality without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests tool listing, prompt listing, and tool calls
/// - Fast execution (<1s instead of 20-30s)
#[tokio::test]
async fn test_mcp_server_with_rmcp_client() {
    // Start in-process HTTP MCP server with agent_mode=true since we test agent tools
    let mut server =
        start_mcp_server_with_options(McpServerMode::Http { port: None }, None, None, None, true)
            .await
            .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

    // List tools to verify our server provides the expected tools
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
    assert!(
        tool_names.contains(&"shell".to_string()),
        "Should have shell tool"
    );

    // List prompts to verify prompt functionality
    let _prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Test a simple tool call - files with glob op should work
    let tool_result = client
        .call_tool(CallToolRequestParams {
            name: "files".into(),
            arguments: serde_json::json!({
                "op": "glob files",
                "pattern": "*.md"
            })
            .as_object()
            .cloned(),
            meta: None,
            task: None,
        })
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
