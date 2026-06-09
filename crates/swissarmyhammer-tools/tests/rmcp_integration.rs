//! Integration tests using rmcp client to test our MCP servers
//!
//! These tests use the rmcp client library to connect to our servers
//! and verify they implement the MCP protocol correctly.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::{
    test_utils::create_test_client,
    unified_server::{start_mcp_server_with_options, McpServerMode},
};

/// Test MCP server with RMCP client (Fast In-Process)
///
/// Tests MCP server tool/prompt functionality without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests tool listing, prompt listing, and tool calls
/// - Fast execution (<1s instead of 20-30s)
#[tokio::test]
async fn test_mcp_server_with_rmcp_client() {
    // Start in-process HTTP MCP server. The full tool union (including agent
    // tools) is always registered. Run against an isolated temp dir so
    // `initialize_code_context` skips the synchronous monorepo walk that would
    // otherwise run when MCP Initialize fires.
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");
    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("Failed to start in-process MCP server");

    // Create RMCP client
    let client = create_test_client(server.url()).await;

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

    // List prompts to verify prompt functionality
    let _prompts = client
        .list_prompts(Default::default())
        .await
        .expect("Failed to list prompts");

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

/// The MCP server's `tool_call complete` INFO log must emit the FULL result
/// text — never a truncated preview.
///
/// The user has repeatedly forbidden truncating log messages. This drives a
/// real tool call whose result far exceeds the old 256-byte preview cap and
/// asserts the complete result token (including its tail, well past byte 256)
/// surfaces in the emitted log. Under the old `truncate_utf8_for_log(_, 256)`
/// path the tail was dropped and this assertion failed.
///
/// Runs on the default current-thread tokio runtime so the in-process server's
/// spawned tasks are polled on the same OS thread the `traced_test` subscriber
/// is installed on — otherwise the server's logs would not be captured.
#[tokio::test]
#[tracing_test::traced_test]
async fn tool_call_complete_log_emits_full_result_untruncated() {
    let temp = tempfile::TempDir::new().expect("Failed to create temp dir");

    // A single unique token, 600 bytes long — comfortably past the old 256-byte
    // preview cap. The tail (`...TAIL_MARKER`) only lands in the log if the full
    // result is logged without truncation.
    let token = format!("FULLRESULTMARKER_{}_TAIL", "x".repeat(600));
    let read_target = temp.path().join("big_result.txt");
    std::fs::write(&read_target, &token).expect("write big result fixture");

    let mut server = start_mcp_server_with_options(
        McpServerMode::Http { port: None },
        None,
        None,
        Some(temp.path().to_path_buf()),
    )
    .await
    .expect("Failed to start in-process MCP server");

    let client = create_test_client(server.url()).await;

    let tool_result = client
        .call_tool(
            CallToolRequestParams::new("files").with_arguments(
                serde_json::json!({
                    "op": "read file",
                    "path": read_target.to_string_lossy(),
                })
                .as_object()
                .cloned()
                .unwrap_or_default(),
            ),
        )
        .await
        .expect("read_file tool call should work");
    assert!(
        !tool_result.content.is_empty(),
        "read_file should return content"
    );

    // The full token — including the `_TAIL` suffix past byte 256 — must appear
    // on the emitted `tool_call complete` line itself. We scope the assertion to
    // that exact line (not the whole log buffer) so a separate full-payload TRACE
    // line cannot mask truncation on the INFO line. Truncation at 256 bytes would
    // drop the tail and fail this assertion.
    logs_assert(|lines: &[&str]| {
        let complete_line = lines
            .iter()
            .find(|l| l.contains("tool_call complete"))
            .ok_or_else(|| "expected a tool_call complete log line".to_string())?;
        if complete_line.contains(&token) {
            Ok(())
        } else {
            Err(format!(
                "tool_call complete line must contain the FULL result text \
                 (including the tail past byte 256), proving it was not \
                 truncated; got: {complete_line}"
            ))
        }
    });

    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}
