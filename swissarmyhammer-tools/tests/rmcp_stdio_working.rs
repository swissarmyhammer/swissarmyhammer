//! Working rmcp client test for stdio server
//!
//! This test uses the exact rmcp pattern from the documentation

use rmcp::{model::CallToolRequestParam, service::ServiceExt, transport::{TokioChildProcess, ConfigureCommandExt}};
use tokio::process::Command;

#[tokio::test]
async fn test_stdio_rmcp_client_lists_tools_and_prompts() {
    // Use exact rmcp pattern from documentation
    let service = ().serve(TokioChildProcess::new(Command::new("cargo").configure(|cmd| {
        cmd.args(&["run", "--package", "swissarmyhammer-cli", "--bin", "sah", "--", "serve"]);
    })).expect("Failed to configure subprocess")).await.expect("Failed to start service");

    // Initialize and get server info
    let server_info = service.peer_info();
    println!("Connected to stdio server: {:#?}", server_info);

    // List tools - this SHOULD work with rmcp
    let tools = service.list_tools(Default::default()).await.expect("Failed to list tools");
    println!("Successfully listed {} tools", tools.tools.len());
    
    // Verify we have expected tools
    assert!(!tools.tools.is_empty(), "Server should provide tools");
    
    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    println!("Available tools: {:?}", tool_names);
    
    assert!(tool_names.contains(&"files_read".to_string()), "Should have files_read tool");
    assert!(tool_names.contains(&"shell_execute".to_string()), "Should have shell_execute tool");

    // List prompts - this SHOULD work with rmcp  
    let prompts = service.list_prompts(Default::default()).await.expect("Failed to list prompts");
    println!("Successfully listed {} prompts", prompts.prompts.len());

    // Test a tool call to verify full functionality
    let tool_result = service
        .call_tool(CallToolRequestParam {
            name: "files_glob".into(),
            arguments: serde_json::json!({ 
                "pattern": "*.md"
            }).as_object().cloned(),
        })
        .await
        .expect("Tool call should work");
        
    println!("Tool call result: {:#?}", tool_result);

    // Clean shutdown
    service.cancel().await.expect("Failed to cancel service");
    
    println!("SUCCESS: rmcp stdio client can list tools, prompts, and call tools!");
}