//! Integration tests using rmcp client to test our MCP servers
//!
//! These tests use the rmcp client library to connect to our servers
//! and verify they implement the MCP protocol correctly.

use rmcp::{
    model::CallToolRequestParam,
    service::ServiceExt,
    transport::{ConfigureCommandExt, TokioChildProcess},
};
use tokio::process::Command;

#[tokio::test]
async fn test_stdio_server_with_rmcp_client() {
    // Use rmcp client to connect to our stdio server running as subprocess
    let service = ()
        .serve(
            TokioChildProcess::new(Command::new("cargo").configure(|cmd| {
                cmd.args(&[
                    "run",
                    "--package",
                    "swissarmyhammer-cli",
                    "--bin",
                    "sah",
                    "--",
                    "serve",
                ]);
            }))
            .expect("Failed to configure subprocess"),
        )
        .await
        .expect("Failed to start service");

    // Initialize and get server info
    let server_info = service.peer_info();
    println!("Connected to stdio server: {:#?}", server_info);

    // List tools to verify our server provides the expected tools
    let tools = service
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");
    println!("Available tools: {:#?}", tools);

    // Verify we have expected tools
    assert!(!tools.tools.is_empty(), "Server should provide tools");

    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    assert!(
        tool_names.contains(&"files_read".to_string()),
        "Should have files_read tool"
    );
    assert!(
        tool_names.contains(&"shell_execute".to_string()),
        "Should have shell_execute tool"
    );

    // List prompts to verify prompt functionality
    let prompts = service
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");
    println!("Available prompts: {} prompts found", prompts.prompts.len());

    // Test a simple tool call - files_glob should work
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "files_glob".into(),
            arguments: serde_json::json!({
                "pattern": "*.md"
            })
            .as_object()
            .cloned(),
        })
        .await;

    match tool_result {
        Ok(result) => println!("Tool call successful: {:#?}", result),
        Err(e) => println!("Tool call failed: {}", e),
    }

    // Clean shutdown
    service.cancel().await.expect("Failed to cancel service");

    println!("Stdio MCP server test with rmcp client PASSED!");
}
