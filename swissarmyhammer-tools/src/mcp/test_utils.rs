//! Test utilities for MCP server testing
//!
//! Provides reusable helper functions for testing MCP servers without subprocess overhead.
//!
//! This module provides the `create_test_client()` helper function that creates RMCP clients
//! with StreamableHttpClientTransport for testing HTTP MCP servers. Tests using this helper
//! avoid code duplication and ensure consistent test client configuration across all tests.
//!
//! The module also includes example tests demonstrating proper usage patterns for:
//! - Listing tools and verifying tool availability
//! - Listing prompts
//! - Calling tools with arguments
//!
//! Tests using these utilities avoid subprocess overhead and build lock contention that
//! occurs with `cargo run` based tests.

use rmcp::{
    model::{ClientCapabilities, ClientInfo, Implementation},
    service::RunningService,
    transport::streamable_http_client::StreamableHttpClientTransport,
    ServiceExt,
};

/// Creates a test RMCP client connected to the specified server URL
///
/// This helper function creates an RMCP client with StreamableHttpClientTransport,
/// which is the appropriate transport for the MCP server's SSE-based HTTP endpoints.
///
/// # Arguments
/// * `server_url` - The base URL of the MCP HTTP server
///
/// # Returns
/// An initialized RMCP client ready to make MCP protocol calls
pub async fn create_test_client(server_url: &str) -> RunningService<rmcp::RoleClient, ClientInfo> {
    let transport = StreamableHttpClientTransport::with_client(
        reqwest::Client::default(),
        rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig {
            uri: server_url.into(),
            auth_header: None,
            ..Default::default()
        },
    );

    let client_info = ClientInfo {
        protocol_version: Default::default(),
        capabilities: ClientCapabilities::default(),
        client_info: Implementation {
            name: "test-client".to_string(),
            title: None,
            version: "1.0.0".to_string(),
            description: None,
            website_url: None,
            icons: None,
        },
        meta: None,
    };

    client_info
        .serve(transport)
        .await
        .expect("Failed to create RMCP client")
}

#[cfg(test)]
mod tests {
    use super::create_test_client;
    use crate::mcp::unified_server::{start_mcp_server, McpServerMode};
    use rmcp::model::CallToolRequestParams;

    #[tokio::test]
    async fn test_client_list_tools() {
        let mut server = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
            .await
            .unwrap();

        let client = create_test_client(server.url()).await;

        let tools = client.list_tools(Default::default()).await.unwrap();
        assert!(!tools.tools.is_empty());

        let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
        assert!(tool_names.contains(&"files_read".to_string()));

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_client_list_prompts() {
        let mut server = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
            .await
            .unwrap();

        let client = create_test_client(server.url()).await;

        let _prompts = client.list_prompts(Default::default()).await.unwrap();

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }

    #[tokio::test]
    async fn test_client_call_tool() {
        let mut server = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
            .await
            .unwrap();

        let client = create_test_client(server.url()).await;

        let result = client
            .call_tool(CallToolRequestParams {
                name: "files_glob".into(),
                arguments: serde_json::json!({
                    "pattern": "*.md"
                })
                .as_object()
                .cloned(),
                meta: None,
                task: None,
            })
            .await
            .unwrap();

        assert!(!result.content.is_empty());

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }
}
