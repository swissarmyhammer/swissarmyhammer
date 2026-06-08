use swissarmyhammer_tools::mcp::test_utils::start_test_server_and_client;

/// Test MCP server basic functionality (Fast In-Process)
///
/// Brings up the in-process HTTP MCP server, completes the RMCP handshake, and
/// lists tools. The prompt protocol surface (prompts/list, prompts/get) was
/// removed, so this exercises the tool surface instead.
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests initialization and tool listing
/// - Fast execution (<1s instead of 20-30s)
///
/// Runs on a `multi_thread` runtime: the test hosts the in-process server and
/// drives the RMCP client on the same runtime, and a current-thread runtime
/// cannot advance the server's SSE response task while blocked awaiting the
/// client handshake — that starvation made the handshake stall for seconds. See
/// `test_client_handshake_is_fast` in `swissarmyhammer-tools` for the analysis.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn test_mcp_server_basic_functionality() {
    // The shared helper binds the server to a fresh temp dir (so startup doesn't
    // walk/hash the host monorepo) and completes the RMCP handshake. `_temp`
    // holds the working-dir guard alive for the test's duration.
    let (mut server, client, _temp) = start_test_server_and_client().await;

    // List tools to verify the handshake completed and the server is serving.
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");
    assert!(!tools.tools.is_empty(), "Server should provide tools");

    // Clean shutdown
    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}

// Removed slow subprocess E2E tests - they are replaced by the fast in-process tests above
// The subprocess tests caused build lock deadlocks and took 20-30s each
// The in-process tests provide equivalent coverage in <1s each
