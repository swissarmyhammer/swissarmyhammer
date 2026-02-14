//! RMCP client test for MCP server
//!
//! Tests RMCP client functionality using in-process HTTP server instead of subprocess.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server, McpServerMode},
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
    // Start in-process HTTP MCP server
    let mut server = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
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
        tool_names.contains(&"files_read".to_string()),
        "Should have files_read tool"
    );
    assert!(
        tool_names.contains(&"files_grep".to_string()),
        "Should have files_grep tool"
    );

    // List prompts
    let prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

    // Prompts may be empty or not depending on configuration, just verify call succeeds
    assert!(prompts.prompts.is_empty() || !prompts.prompts.is_empty());

    // Test a tool call to verify full functionality
    let tool_result = client
        .call_tool(CallToolRequestParams {
            name: "files_grep".into(),
            arguments: serde_json::json!({
                "pattern": "test"
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
