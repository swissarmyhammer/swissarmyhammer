//! Integration tests using rmcp client to test our MCP servers
//!
//! These tests use the rmcp client library to connect to our servers
//! and verify they implement the MCP protocol correctly.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::test_utils::start_test_server_and_client;

/// Test MCP server with RMCP client (Fast In-Process)
///
/// Tests MCP server tool functionality without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests tool listing and tool calls
/// - Fast execution (<1s instead of 20-30s)
#[tokio::test]
async fn test_mcp_server_with_rmcp_client() {
    // The shared helper starts the in-process HTTP server bound to a fresh temp
    // dir (so `initialize_code_context` skips the host-monorepo walk) and
    // connects a client. `_temp` holds the working-dir guard alive.
    let (mut server, client, _temp) = start_test_server_and_client().await;

    // List tools to verify our server provides the expected tools
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    assert!(!tools.tools.is_empty(), "Server should provide tools");

    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    // `tools/list` composes per connecting client. The default `test-client`
    // name is an unknown host, served `Shared` tools only — neither the
    // `Agent`-category `files` tool nor the `Replacement`-category `shell` tool
    // (which is reserved for Claude). Both remain *callable*; composition gates
    // only what is advertised.
    assert!(
        !tool_names.contains(&"files".to_string()),
        "unknown host must not be advertised the Agent-category files tool"
    );
    assert!(
        !tool_names.contains(&"shell".to_string()),
        "unknown host must not be advertised the Replacement-category shell tool"
    );

    // Test a simple tool call - files with glob op should work
    let tool_result = client
        .call_tool(
            CallToolRequestParams::new("files").with_arguments(
                serde_json::json!({
                    "op": "glob files",
                    "pattern": "*.md"
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
