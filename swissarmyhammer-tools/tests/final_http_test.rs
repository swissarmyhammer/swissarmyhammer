//! Final HTTP MCP test

use rmcp::{
    model::{ClientCapabilities, ClientInfo, Implementation},
    transport::streamable_http_client::StreamableHttpClientTransport,
    ServiceExt,
};
use swissarmyhammer_tools::mcp::unified_server::{start_mcp_server, McpServerMode};

#[tokio::test]
#[test_log::test]
async fn test_http_mcp_server_rmcp_client_final() {
    // Start HTTP MCP server
    let mode = McpServerMode::Http { port: None };
    let mut server = start_mcp_server(mode, None, None).await.unwrap();

    let server_url = server.url();

    // Use the same pattern as from_uri implementation
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
            name: "test http client".to_string(),
            title: None,
            version: "0.0.1".to_string(),
            website_url: None,
            icons: None,
        },
    };

    let client = client_info.serve(transport).await.unwrap();

    // Test MCP functionality
    let tools = client.list_tools(Default::default()).await.unwrap();
    assert!(!tools.tools.is_empty(), "Should have tools");

    // Clean up
    client.cancel().await.unwrap();
    server.shutdown().await.unwrap();
}
