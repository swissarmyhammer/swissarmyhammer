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

use once_cell::sync::Lazy;
use rmcp::{
    model::{ClientCapabilities, ClientInfo, Implementation},
    service::RunningService,
    transport::streamable_http_client::StreamableHttpClientTransport,
    ServiceExt,
};

/// Process-wide HTTP client for the RMCP test transport, with system-proxy
/// detection disabled.
///
/// A fresh `reqwest::Client::default()` resolves the OS proxy configuration on
/// its first request. On macOS that lookup goes through the single-threaded
/// `configd` daemon and serializes across processes — even for a literal
/// `127.0.0.1` URL — adding seconds per first contact and blowing the nextest
/// timeout when many in-process server tests handshake concurrently. The
/// loopback MCP server never needs a proxy, so `.no_proxy()` removes that cost,
/// and sharing one client process-wide pays connection setup once instead of
/// per test.
static TEST_HTTP_CLIENT: Lazy<reqwest::Client> = Lazy::new(|| {
    reqwest::Client::builder()
        .no_proxy()
        .build()
        .expect("Failed to build no-proxy reqwest client for MCP test transport")
});

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
#[allow(clippy::field_reassign_with_default)] // field init syntax breaks with #[non_exhaustive] in newer rmcp
pub async fn create_test_client(server_url: &str) -> RunningService<rmcp::RoleClient, ClientInfo> {
    let transport = StreamableHttpClientTransport::with_client(TEST_HTTP_CLIENT.clone(), {
        let mut config =
            rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig::default();
        config.uri = server_url.into();
        config
    });

    let client_info = ClientInfo::new(
        ClientCapabilities::default(),
        Implementation::new("test-client", "1.0.0"),
    );

    client_info
        .serve(transport)
        .await
        .expect("Failed to create RMCP client")
}

#[cfg(test)]
mod tests {
    use super::create_test_client;
    use crate::mcp::unified_server::{start_mcp_server_with_options, McpServerMode};
    use rmcp::model::CallToolRequestParams;

    /// Regression guard for the in-process MCP-server handshake speed.
    ///
    /// Two independent issues used to make the test-side RMCP client handshake
    /// take seconds (and time out under nextest's full-workspace parallelism):
    ///
    /// 1. A fresh `reqwest::Client::default()` triggered macOS system-proxy
    ///    resolution via the single-threaded `configd` daemon on first contact,
    ///    even for `127.0.0.1`. That lookup serialized across processes, so
    ///    ~10 concurrent server tests queued behind one OS daemon. Fixed by the
    ///    shared `no_proxy` [`super::TEST_HTTP_CLIENT`].
    /// 2. A current-thread tokio runtime (`#[tokio::test]`) cannot make progress
    ///    on the in-process server's response task while the same thread is
    ///    blocked awaiting the client SSE handshake — the handshake stalled for
    ///    multiple seconds until a scheduler tick. Fixed by running these tests
    ///    on a `multi_thread` runtime.
    ///
    /// With both fixes the handshake completes in single-digit milliseconds.
    /// A 1s ceiling catches a regression of either issue (each reintroduces a
    /// multi-second handshake) without being flaky on a busy CI machine.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_client_handshake_is_fast() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut server = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            None,
            Some(temp.path().to_path_buf()),
            true,
        )
        .await
        .unwrap();

        let start = std::time::Instant::now();
        let client = create_test_client(server.url()).await;
        let elapsed = start.elapsed();
        assert!(
            elapsed < std::time::Duration::from_secs(1),
            "loopback MCP client handshake took {elapsed:?}, expected < 1s; a multi-second \
             handshake means proxy detection or current-thread runtime starvation has regressed"
        );

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }

    // In-process MCP server tests must run on a `multi_thread` runtime: the test
    // both hosts the server and drives the RMCP client, and a current-thread
    // runtime cannot advance the server's SSE response task while blocked on the
    // client handshake — see `test_client_handshake_is_fast` for the full story.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_client_list_tools() {
        // Use agent_mode=true since this test checks for agent tools (files).
        // Pass a tempdir as working_dir so the server doesn't bind to the host
        // monorepo — prevents `startup_cleanup` from walking/hashing it and lets
        // multiple server tests run in parallel without a CWD serial guard.
        let temp = tempfile::TempDir::new().unwrap();
        let mut server = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            None,
            Some(temp.path().to_path_buf()),
            true,
        )
        .await
        .unwrap();

        let client = create_test_client(server.url()).await;

        let tools = client.list_tools(Default::default()).await.unwrap();
        assert!(!tools.tools.is_empty());

        let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
        assert!(tool_names.contains(&"files".to_string()));

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }

    // multi_thread required — see `test_client_handshake_is_fast`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_client_list_prompts() {
        let temp = tempfile::TempDir::new().unwrap();
        let mut server = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            None,
            Some(temp.path().to_path_buf()),
            false,
        )
        .await
        .unwrap();

        let client = create_test_client(server.url()).await;

        let _prompts = client.list_prompts(Default::default()).await.unwrap();

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }

    // multi_thread required — see `test_client_handshake_is_fast`.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_client_call_tool() {
        // Use agent_mode=true since this test calls files (an agent tool).
        let temp = tempfile::TempDir::new().unwrap();
        let mut server = start_mcp_server_with_options(
            McpServerMode::Http { port: None },
            None,
            None,
            Some(temp.path().to_path_buf()),
            true,
        )
        .await
        .unwrap();

        let client = create_test_client(server.url()).await;

        let result = client
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
            .unwrap();

        assert!(!result.content.is_empty());

        client.cancel().await.unwrap();
        server.shutdown().await.unwrap();
    }
}
