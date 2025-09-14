//! Final HTTP MCP test - prove it works with rmcp client

use rmcp::{service::ServiceExt, transport::{SseClientTransport, sse_client::SseClientConfig}};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};
use tokio::time::{timeout, Duration};

#[tokio::test]
async fn test_http_mcp_server_rmcp_client_final() {
    // Start HTTP server
    let mode = McpServerMode::Http { port: Some(18091) };
    let mut server = start_mcp_server(mode, None).await.expect("Failed to start HTTP server");
    
    println!("HTTP server started on port: {}", server.port().unwrap());
    
    // Give server time to start
    tokio::time::sleep(Duration::from_millis(500)).await;
    
    let sse_url = format!("{}/sse", server.url());
    println!("Connecting to: {}", sse_url);
    
    // Test with timeout to avoid hanging
    let test_result = timeout(Duration::from_secs(10), async {
        // Create rmcp SSE client
        let reqwest_client = reqwest::Client::new();
        let config = SseClientConfig {
            sse_endpoint: sse_url.into(),
            retry_policy: std::sync::Arc::new(rmcp::transport::common::client_side_sse::FixedInterval::default()),
            use_message_endpoint: None,
        };
        
        let transport = SseClientTransport::start_with_client(reqwest_client, config)
            .await
            .expect("Failed to create transport");
        
        let client = ().serve(transport).await.expect("Failed to start client");
        
        // List tools
        let tools = client.list_tools(Default::default()).await.expect("Failed to list tools");
        println!("SUCCESS: Listed {} tools", tools.tools.len());
        
        // List prompts  
        let prompts = client.list_prompts(Default::default()).await.expect("Failed to list prompts");
        println!("SUCCESS: Listed {} prompts", prompts.prompts.len());
        
        client.cancel().await.expect("Failed to cancel client");
        
        (tools.tools.len(), prompts.prompts.len())
    }).await;
    
    server.shutdown().await.expect("Failed to shutdown server");
    
    match test_result {
        Ok((tool_count, prompt_count)) => {
            assert!(tool_count > 0, "Should have tools");
            // Prompts count can be 0 or more 
            println!("HTTP MCP server with rmcp client SUCCESS: {} tools, {} prompts", tool_count, prompt_count);
        }
        Err(_) => {
            panic!("Test timed out - rmcp client connection failed");
        }
    }
}