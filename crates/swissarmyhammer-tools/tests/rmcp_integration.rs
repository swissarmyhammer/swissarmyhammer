//! Integration tests using rmcp client to test our MCP servers
//!
//! These tests use the rmcp client library to connect to our servers
//! and verify they implement the MCP protocol correctly.

use rmcp::model::CallToolRequestParams;
use swissarmyhammer_tools::mcp::test_utils::start_test_server_and_client;

/// Bytes in the old log preview cap. A result logged past this cap proves the
/// completion record is not truncated.
const OLD_LOG_PREVIEW_CAP_BYTES: usize = 256;

/// Length of the filler in the large-result marker. It sits well past
/// [`OLD_LOG_PREVIEW_CAP_BYTES`].
const LARGE_TOKEN_FILL_BYTES: usize = 600;

/// Turn a JSON object literal into the argument map a tool call takes.
fn arguments(value: serde_json::Value) -> serde_json::Map<String, serde_json::Value> {
    value.as_object().cloned().unwrap_or_default()
}

/// The `tool_call complete` record among the captured log lines.
fn find_complete_line<'a>(lines: &[&'a str]) -> Result<&'a str, String> {
    lines
        .iter()
        .find(|l| l.contains("tool_call complete"))
        .copied()
        .ok_or_else(|| "expected a tool_call complete log line".to_string())
}

/// Test MCP server with RMCP client (Fast In-Process)
///
/// Tests MCP server tool functionality without subprocess overhead:
/// - Uses in-process HTTP MCP server
/// - No cargo build/run overhead
/// - Tests tool listing and tool calls
/// - Fast execution (<1s instead of 20-30s)
#[tokio::test]
async fn test_mcp_server_with_rmcp_client() {
    // The shared helper starts the in-process HTTP server bound to a fresh temp
    // dir (so `initialize_code_context` skips the host-monorepo walk) and
    // connects a client. `_temp` holds the working-dir guard alive.
    let (mut server, client, _temp) = start_test_server_and_client().await;

    // List tools to verify our server provides the expected tools
    let tools = client
        .list_tools(Default::default())
        .await
        .expect("Failed to list tools");

    assert!(!tools.tools.is_empty(), "Server should provide tools");

    let tool_names: Vec<String> = tools.tools.iter().map(|t| t.name.to_string()).collect();
    // `tools/list` composes per connecting client. The default `test-client`
    // name is an unknown host, served `Shared` tools only — not the
    // `Agent`-category `web` tool. It remains *callable*; composition gates
    // only what is advertised.
    assert!(
        !tool_names.contains(&"web".to_string()),
        "unknown host must not be advertised the Agent-category web tool"
    );

    // Test a simple tool call - kanban with the init board op should work
    let tool_result = client
        .call_tool(
            CallToolRequestParams::new("kanban").with_arguments(arguments(serde_json::json!({
                "op": "init board",
                "name": "Rmcp Test"
            }))),
        )
        .await
        .expect("Tool call should work");

    assert_ne!(
        tool_result.is_error,
        Some(true),
        "kanban init board must succeed: {tool_result:?}"
    );
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
/// asserts the complete result token surfaces in the `result` field of the
/// emitted completion record. Under the old `truncate_utf8_for_log(_, 256)`
/// path the tail was dropped and this assertion failed.
///
/// The result is an error message that echoes the unknown `op`: error text is
/// a single line, so the whole record sits on one captured log line. (A
/// multi-line result wraps onto continuation lines that the `traced_test`
/// scope filter drops, so it cannot be asserted through `logs_assert`.)
///
/// Runs on the default current-thread tokio runtime so the in-process server's
/// spawned tasks are polled on the same OS thread the `traced_test` subscriber
/// is installed on — otherwise the server's logs would not be captured.
#[tokio::test]
#[tracing_test::traced_test]
async fn tool_call_complete_log_emits_full_result_untruncated() {
    // The shared helper starts the in-process HTTP server bound to a fresh temp
    // dir (so `initialize_code_context` skips the host-monorepo walk) and
    // connects a client. `_temp` holds the working-dir guard alive.
    let (mut server, client, _temp) = start_test_server_and_client().await;

    // A single unique token, far past the old 256-byte preview cap. The `git`
    // tool rejects an unknown `op` with an error that echoes the op, so the
    // token rides in the tool result.
    let token = format!(
        "FULLRESULTMARKER_{}_TAIL",
        "x".repeat(LARGE_TOKEN_FILL_BYTES)
    );

    client
        .call_tool(
            CallToolRequestParams::new("git")
                .with_arguments(arguments(serde_json::json!({ "op": token }))),
        )
        .await
        .expect_err("an unknown git op must be rejected with an error naming the op");

    // The `tool_call complete` record must carry the complete token in its
    // `result` field. The same token also rides on the span's `op` field before
    // `result=`, so the result check reads only what follows `result=`. The
    // record also reports `result_bytes=<N>` with N at least the token length —
    // the full result was measured, not a 256-byte preview. The `tool_call`
    // span must carry the dispatched `op` so per-op aggregation over the logs
    // (token accounting, latency by op) does not depend on joining the args
    // line by adjacency — which breaks under concurrent calls. The span prefix
    // renders on every line inside the call, including the completion line.
    logs_assert(move |lines: &[&str]| {
        let complete_line = find_complete_line(lines)?;

        let reported_bytes = complete_line
            .split("result_bytes=")
            .nth(1)
            .and_then(|rest| {
                rest.chars()
                    .take_while(char::is_ascii_digit)
                    .collect::<String>()
                    .parse::<usize>()
                    .ok()
            })
            .ok_or_else(|| format!("no result_bytes on the record: {complete_line}"))?;
        if reported_bytes < token.len() {
            return Err(format!(
                "tool_call complete must report the FULL result length (at least {} \
                 bytes), proving it was not truncated to a {OLD_LOG_PREVIEW_CAP_BYTES}-byte \
                 preview; got: {complete_line}",
                token.len()
            ));
        }

        let result_field = complete_line
            .split("result=")
            .nth(1)
            .ok_or_else(|| format!("no result field on the record: {complete_line}"))?;
        if !result_field.contains(&token) {
            return Err(format!(
                "the full result token (past byte {OLD_LOG_PREVIEW_CAP_BYTES}) is missing \
                 from the result field — result appears truncated; got: {result_field}"
            ));
        }

        if complete_line.contains(r#"op="FULLRESULTMARKER_"#) {
            Ok(())
        } else {
            Err(format!(
                "tool_call span must carry the dispatched op; got: {complete_line}"
            ))
        }
    });

    client.cancel().await.expect("Failed to cancel client");
    server.shutdown().await.expect("Failed to shutdown server");
}
