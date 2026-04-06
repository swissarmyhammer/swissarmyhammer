//! Integration tests for FilteringMcpProxy ServerHandler methods.
//!
//! These tests verify the core proxy behaviors by serving FilteringMcpProxy
//! over in-memory transports (tokio::io::duplex) and exercising each
//! ServerHandler method through the MCP protocol.

use rmcp::ServiceExt;
use swissarmyhammer_mcp_proxy::{FilteringMcpProxy, ToolFilter};
use swissarmyhammer_tools::mcp::unified_server::{
    start_mcp_server, McpServerHandle, McpServerMode,
};

/// A minimal MCP client handler used in tests.
#[derive(Debug, Clone, Default)]
struct TestClientHandler;

impl rmcp::ClientHandler for TestClientHandler {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        rmcp::model::ClientInfo::default()
    }
}

/// Start the real SwissArmyHammer upstream and return its URL and handle.
///
/// Waits until the upstream health endpoint responds before returning so
/// that tests can immediately issue MCP requests without timing issues.
async fn start_upstream() -> (String, McpServerHandle) {
    let handle = start_mcp_server(McpServerMode::Http { port: None }, None, None, None)
        .await
        .expect("failed to start upstream MCP server");
    let port = handle.info().port.expect("upstream has no port");
    let url = format!("http://127.0.0.1:{}/mcp", port);
    let health_url = format!("http://127.0.0.1:{}/health", port);

    // Wait until the upstream is ready (up to 5 seconds)
    let client = reqwest::Client::new();
    for _ in 0..50 {
        if client.get(&health_url).send().await.is_ok() {
            break;
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    (url, handle)
}

/// Build a FilteringMcpProxy pointed at the given upstream URL with the given filter
/// and serve it over the given transport in the background.
///
/// Returns the RunningService for the client side — callers must keep this alive
/// for the duration of the test so the underlying service does not stop.
async fn make_client(
    upstream_url: String,
    filter: ToolFilter,
) -> rmcp::service::RunningService<rmcp::RoleClient, TestClientHandler> {
    let proxy = FilteringMcpProxy::new(upstream_url, filter);

    let (server_transport, client_transport) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        if let Ok(service) = proxy.serve(server_transport).await {
            let _ = service.waiting().await;
        }
    });

    TestClientHandler
        .serve(client_transport)
        .await
        .expect("client connection failed")
}

// ---------------------------------------------------------------------------
// initialize
// ---------------------------------------------------------------------------

/// initialize() does not call the upstream — it returns the proxy's own
/// capabilities.  This test verifies the response contains the proxy's
/// implementation name and expected capabilities.
#[tokio::test]
async fn test_initialize_returns_proxy_capabilities() {
    let (upstream_url, _upstream) = start_upstream().await;

    let filter = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let proxy = FilteringMcpProxy::new(upstream_url, filter);

    let (server_transport, client_transport) = tokio::io::duplex(65536);
    tokio::spawn(async move {
        if let Ok(service) = proxy.serve(server_transport).await {
            let _ = service.waiting().await;
        }
    });

    // Connect a test client — the client performs the initialize handshake
    let running = TestClientHandler
        .serve(client_transport)
        .await
        .expect("client connection failed");

    // peer_info() returns the ServerInfo sent during initialize
    let server_info = running
        .peer_info()
        .expect("server should have provided initialize result");
    assert_eq!(
        server_info.server_info.name, "swissarmyhammer-filtering-proxy",
        "initialize should return the proxy's own implementation name"
    );
    assert!(
        server_info.capabilities.tools.is_some(),
        "initialize should advertise tool capabilities"
    );
    assert!(
        server_info.capabilities.prompts.is_some(),
        "initialize should advertise prompt capabilities"
    );
}

// ---------------------------------------------------------------------------
// list_tools (with filtering)
// ---------------------------------------------------------------------------

/// When no allow patterns are set, all upstream tools should pass through.
#[tokio::test]
async fn test_list_tools_no_filter_returns_all_tools() {
    let (upstream_url, _upstream) = start_upstream().await;

    let filter = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer = make_client(upstream_url, filter).await;

    let result = peer
        .list_tools(None)
        .await
        .expect("list_tools should succeed");

    assert!(
        result.tools.len() > 1,
        "unfiltered list should return multiple tools, got {}",
        result.tools.len()
    );
}

/// list_tools() fetches tools from the upstream and filters them to only
/// return allowed tools.
#[tokio::test]
async fn test_list_tools_filters_to_allowed_tools() {
    let (upstream_url, _upstream) = start_upstream().await;

    // First get the full list of tools to find a real tool name
    let filter_all = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer_all = make_client(upstream_url.clone(), filter_all).await;
    let all = peer_all.list_tools(None).await.expect("list all tools");
    assert!(
        !all.tools.is_empty(),
        "upstream must have at least one tool"
    );

    // Use the first tool name as our filter target
    let target_name = all.tools[0].name.to_string();
    let pattern = format!("^{}$", regex::escape(&target_name));

    let filter = ToolFilter::new(vec![pattern.clone()], vec![]).expect("valid filter");
    let peer = make_client(upstream_url, filter).await;

    let result = peer
        .list_tools(None)
        .await
        .expect("list_tools should succeed");

    // Only the target tool should be visible
    assert_eq!(
        result.tools.len(),
        1,
        "expected exactly 1 tool after filtering with pattern {:?}, got {:?}",
        pattern,
        result.tools.iter().map(|t| &t.name).collect::<Vec<_>>()
    );
    assert_eq!(
        result.tools[0].name.as_ref(),
        target_name.as_str(),
        "the allowed tool should be '{}'",
        target_name
    );
}

/// Deny patterns remove matching tools from the list.
#[tokio::test]
async fn test_list_tools_deny_pattern_removes_tools() {
    let (upstream_url, _upstream) = start_upstream().await;

    // Get the full tool list to know the count and a name to deny
    let filter_all = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer_all = make_client(upstream_url.clone(), filter_all).await;
    let all = peer_all.list_tools(None).await.expect("list all tools");
    let total = all.tools.len();
    assert!(total > 0, "upstream must have at least one tool");

    // Deny the first tool by exact name
    let deny_name = all.tools[0].name.to_string();
    let deny_pattern = format!("^{}$", regex::escape(&deny_name));

    let filter_deny = ToolFilter::new(vec![], vec![deny_pattern]).expect("valid filter");
    let peer_deny = make_client(upstream_url, filter_deny).await;

    let denied_result = peer_deny.list_tools(None).await.expect("list_tools deny");

    assert_eq!(
        denied_result.tools.len(),
        total - 1,
        "denying '{}' should reduce tool count by 1",
        deny_name
    );
    assert!(
        denied_result
            .tools
            .iter()
            .all(|t| t.name.as_ref() != deny_name.as_str()),
        "'{}' tool should be absent after deny filter",
        deny_name
    );
}

// ---------------------------------------------------------------------------
// list_prompts
// ---------------------------------------------------------------------------

/// list_prompts() should forward the request to the upstream and return
/// results without error.
#[tokio::test]
async fn test_list_prompts_forwards_to_upstream() {
    let (upstream_url, _upstream) = start_upstream().await;

    let filter = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer = make_client(upstream_url, filter).await;

    // Should not return an error — even if no prompts are registered
    let result = peer.list_prompts(None).await;
    assert!(
        result.is_ok(),
        "list_prompts should succeed (even with empty result), got: {:?}",
        result.err()
    );
}

// ---------------------------------------------------------------------------
// call_tool
// ---------------------------------------------------------------------------

/// call_tool() should forward the request to the upstream and return results.
///
/// This test verifies forwarding is working by calling a tool that requires no
/// setup. We discover a real tool name from list_tools, then verify that calling
/// a listed tool works (even if the tool itself returns an error for missing args).
#[tokio::test]
async fn test_call_tool_forwards_to_upstream() {
    let (upstream_url, _upstream) = start_upstream().await;

    // Get a real tool name to call
    let filter_all = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer_discover = make_client(upstream_url.clone(), filter_all).await;
    let all = peer_discover
        .list_tools(None)
        .await
        .expect("list_tools should succeed");
    assert!(!all.tools.is_empty(), "upstream must have tools");

    // Use the first tool — we just want to verify the request is forwarded,
    // not that the tool succeeds (it may return an error for missing arguments)
    let tool_name = all.tools[0].name.to_string();

    let filter = ToolFilter::new(vec![], vec![]).expect("valid filter");
    let peer = make_client(upstream_url, filter).await;

    let params = rmcp::model::CallToolRequestParams::new(tool_name.clone());
    let result: Result<rmcp::model::CallToolResult, rmcp::service::ServiceError> =
        peer.call_tool(params).await;

    // The request was forwarded if we got any response (success or tool error).
    // ServiceError::McpError means the upstream replied (even if with an error).
    // Anything other than TransportClosed/TransportSend means forwarding worked.
    match &result {
        Ok(_) => {}                                         // Tool succeeded
        Err(rmcp::service::ServiceError::McpError(_)) => {} // Upstream replied with MCP error — still forwarded
        Err(e) => panic!(
            "call_tool failed with transport error (not forwarded): {:?}",
            e
        ),
    }
}
