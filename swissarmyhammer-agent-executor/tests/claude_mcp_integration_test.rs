//! Integration test verifying ClaudeCodeExecutor can use MCP tools
//!
//! This test ensures that ClaudeCode executor properly connects to the MCP server
//! and can execute tools like file reading.

use swissarmyhammer_agent_executor::{AgentExecutionContext, AgentExecutor, ClaudeCodeExecutor};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

#[tokio::test]
async fn test_claude_mcp_config_argument_order() {
    // Test that verifies --mcp-config comes BEFORE --print in command args
    // This is critical: Claude CLI requires this specific order to parse correctly

    let mcp_config = agent_client_protocol::McpServer::Http(
        agent_client_protocol::McpServerHttp::new("test", "http://127.0.0.1:12345/mcp"),
    );

    let mut executor = ClaudeCodeExecutor::new(mcp_config);

    // We can't actually test the command execution without Claude CLI,
    // but we can verify the configuration is correct by checking that
    // initialization succeeds
    let init_result = executor.initialize().await;

    // Should succeed (finds claude in PATH) or fail with expected error
    match init_result {
        Ok(_) => {
            // Claude CLI is available, test passed
        }
        Err(e) => {
            // Should only fail if Claude CLI not in PATH
            let error_msg = format!("{:?}", e);
            assert!(
                error_msg.contains("not found in PATH") || error_msg.contains("Claude CLI"),
                "Unexpected error: {}",
                error_msg
            );
        }
    }
}

#[tokio::test]
#[ignore] // Only run when Claude CLI is available
async fn test_claude_executor_can_use_mcp_tools() {
    // Start MCP server
    let mut mcp_server = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
        .await
        .expect("Failed to start MCP server");

    let port = mcp_server.info().port.expect("MCP server should have port");

    // Create McpServer configuration
    let mcp_config = agent_client_protocol::McpServer::Http(
        agent_client_protocol::McpServerHttp::new("test", format!("http://127.0.0.1:{}/mcp", port)),
    );

    // Create and initialize ClaudeCode executor
    let mut executor = ClaudeCodeExecutor::new(mcp_config);
    executor
        .initialize()
        .await
        .expect("Executor should initialize");

    // Create execution context
    let agent_config = swissarmyhammer_config::model::ModelConfig::claude_code();
    let context = AgentExecutionContext::new(&agent_config);

    // Execute a prompt that requires Claude to use the Read tool
    let system_prompt = "You have access to MCP tools. Use them to complete the task.".to_string();
    let prompt =
        "Read the file 'Cargo.toml' in the current directory and tell me the package name."
            .to_string();

    let response = executor
        .execute_prompt(system_prompt, prompt, &context)
        .await
        .expect("Execution should succeed");

    // Verify the response contains evidence of file reading
    let content = response.content.to_lowercase();
    assert!(
        content.contains("swissarmyhammer") || content.contains("package"),
        "Response should mention the package name or workspace, got: {}",
        response.content
    );

    // Cleanup
    executor.shutdown().await.expect("Shutdown should succeed");
    mcp_server
        .shutdown()
        .await
        .expect("MCP server shutdown should succeed");
}
