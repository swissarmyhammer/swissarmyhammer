//! Integration test: the ACP agent must honor an HTTP MCP server passed in
//! `newSession.mcpServers`.
//!
//! The kanban-app's TypeScript ACP client gives the agent the full
//! SwissArmyHammer toolset by putting an HTTP `McpServer::Http` entry — pointing
//! at the board's in-process SAH toolset URL — into the ACP `session/new`
//! request's `mcpServers` array. This test pins the contract that
//! `claude-agent` actually connects such a server and exposes its tools.
//!
//! It starts a minimal in-process HTTP MCP server implementing the Streamable
//! HTTP transport handshake (`initialize`, `initialized`, `tools/list`,
//! `prompts/list`, `tools/call`), creates a session with that server as the
//! sole `mcpServers` entry, and asserts both that the agent lists the server's
//! tool and that it can invoke it.

use agent_client_protocol::schema::{HttpHeader, McpServer, McpServerHttp, NewSessionRequest};
use claude_agent::config::AgentConfig;
use claude_agent::tools::InternalToolRequest;
use claude_agent::ClaudeAgent;
use serde_json::{json, Value};
use std::net::SocketAddr;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

/// Name advertised by the test MCP server. The agent prefixes discovered tool
/// names with this, so the agent-visible tool name is `test-http-mcp:echo`.
const TEST_SERVER_NAME: &str = "test-http-mcp";

/// The single tool the test MCP server exposes.
const TEST_TOOL_NAME: &str = "echo";

/// A minimal HTTP MCP server speaking the Streamable HTTP transport.
///
/// It binds an ephemeral port, answers a single POST endpoint with JSON-RPC
/// responses, and shuts down when the returned handle is dropped. Only the
/// methods `claude-agent` exercises during connection and tool invocation are
/// implemented; everything else is answered with a JSON-RPC method-not-found
/// error.
struct TestHttpMcpServer {
    addr: SocketAddr,
    shutdown: Option<tokio::sync::oneshot::Sender<()>>,
}

impl TestHttpMcpServer {
    /// Bind an ephemeral port and start serving in a background task.
    async fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let (shutdown_tx, mut shutdown_rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = &mut shutdown_rx => break,
                    accepted = listener.accept() => {
                        if let Ok((stream, _)) = accepted {
                            tokio::spawn(handle_connection(stream));
                        }
                    }
                }
            }
        });

        Self {
            addr,
            shutdown: Some(shutdown_tx),
        }
    }

    /// The MCP endpoint URL the agent should connect to.
    fn url(&self) -> String {
        format!("http://{}/mcp", self.addr)
    }
}

impl Drop for TestHttpMcpServer {
    fn drop(&mut self) {
        if let Some(tx) = self.shutdown.take() {
            let _ = tx.send(());
        }
    }
}

/// Serve a single HTTP/1.1 connection, handling one request at a time.
///
/// The agent's `reqwest` client may reuse a connection for the `initialize`,
/// `initialized`, `tools/list`, `prompts/list`, and `tools/call` requests, so
/// the loop keeps reading until the peer closes the socket.
async fn handle_connection(mut stream: tokio::net::TcpStream) {
    loop {
        let Some(body) = read_http_request_body(&mut stream).await else {
            break;
        };

        let request: Value = match serde_json::from_slice(&body) {
            Ok(value) => value,
            Err(_) => break,
        };

        let response = mcp_response(&request);
        if write_http_response(&mut stream, response).await.is_err() {
            break;
        }
    }
}

/// Read one HTTP request and return its body bytes, or `None` on EOF / error.
async fn read_http_request_body(stream: &mut tokio::net::TcpStream) -> Option<Vec<u8>> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];

    // Read until headers are complete.
    let header_end = loop {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
        if let Some(pos) = find_subslice(&buffer, b"\r\n\r\n") {
            break pos + 4;
        }
    };

    let headers = String::from_utf8_lossy(&buffer[..header_end]).to_lowercase();
    let content_length = headers
        .lines()
        .find_map(|line| line.strip_prefix("content-length:"))
        .and_then(|value| value.trim().parse::<usize>().ok())
        .unwrap_or(0);

    // Read until the full body has arrived.
    while buffer.len() < header_end + content_length {
        let read = stream.read(&mut chunk).await.ok()?;
        if read == 0 {
            return None;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }

    Some(buffer[header_end..header_end + content_length].to_vec())
}

/// Find the first index of `needle` within `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// One MCP response: either a JSON-RPC body (with status 200) or, for the
/// `initialized` notification, a bodiless `202 Accepted`.
enum McpResponse {
    /// JSON-RPC result body, served as `200 OK`.
    Json(Value),
    /// Bodiless `202 Accepted`, used for the `initialized` notification.
    Accepted,
}

/// Build the MCP response for one JSON-RPC request.
fn mcp_response(request: &Value) -> McpResponse {
    let method = request.get("method").and_then(Value::as_str).unwrap_or("");
    let id = request.get("id").cloned().unwrap_or(Value::Null);

    match method {
        "initialize" => McpResponse::Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": "2024-11-05",
                "capabilities": { "tools": {} },
                "serverInfo": { "name": TEST_SERVER_NAME, "version": "0.0.0" }
            }
        })),
        "initialized" => McpResponse::Accepted,
        "tools/list" => McpResponse::Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "tools": [{
                    "name": TEST_TOOL_NAME,
                    "description": "Echo the provided text back to the caller.",
                    "inputSchema": {
                        "type": "object",
                        "properties": {
                            "text": { "type": "string" }
                        },
                        "required": ["text"]
                    }
                }]
            }
        })),
        "prompts/list" => McpResponse::Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": { "prompts": [] }
        })),
        "tools/call" => {
            let text = request
                .get("params")
                .and_then(|params| params.get("arguments"))
                .and_then(|arguments| arguments.get("text"))
                .and_then(Value::as_str)
                .unwrap_or("");
            McpResponse::Json(json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "content": [{ "type": "text", "text": format!("echo: {text}") }]
                }
            }))
        }
        _ => McpResponse::Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": { "code": -32601, "message": "Method not found" }
        })),
    }
}

/// Write an MCP response to the socket as an HTTP/1.1 reply.
async fn write_http_response(
    stream: &mut tokio::net::TcpStream,
    response: McpResponse,
) -> std::io::Result<()> {
    let raw = match response {
        McpResponse::Accepted => "HTTP/1.1 202 Accepted\r\nContent-Length: 0\r\n\r\n".to_string(),
        McpResponse::Json(body) => {
            let serialized = body.to_string();
            format!(
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                serialized.len(),
                serialized
            )
        }
    };
    stream.write_all(raw.as_bytes()).await?;
    stream.flush().await
}

/// An HTTP MCP server passed in `newSession.mcpServers` is connected by the
/// agent and its tools are exposed to the agent's tool handler.
#[tokio::test]
async fn http_mcp_server_in_new_session_is_connected_and_tools_exposed() {
    let mcp_server = TestHttpMcpServer::start().await;

    // Skip spawning the real `claude` CLI: this test only exercises the
    // MCP-connection side of `new_session`, which runs before the spawn.
    let config = AgentConfig {
        spawn_claude_on_new_session: false,
        ..Default::default()
    };
    let (agent, _notifications) = ClaudeAgent::new(config)
        .await
        .expect("agent construction should succeed");

    // The TypeScript ACP client shape: an HTTP MCP server in `mcpServers`.
    let request = NewSessionRequest::new(std::env::current_dir().unwrap()).mcp_servers(vec![
        McpServer::Http(
            McpServerHttp::new(TEST_SERVER_NAME, mcp_server.url())
                .headers(vec![HttpHeader::new("X-Test", "1")]),
        ),
    ]);

    agent
        .new_session(request)
        .await
        .expect("session creation with an HTTP MCP server should succeed");

    // The tool from the HTTP MCP server must be visible to the agent. The
    // agent prefixes MCP tool names with the server name.
    let expected_tool = format!("{TEST_SERVER_NAME}:{TEST_TOOL_NAME}");
    let tools = {
        let handler = agent.tool_handler();
        let handler = handler.read().await;
        handler.list_all_available_tools().await
    };
    assert!(
        tools.contains(&expected_tool),
        "agent should expose the HTTP MCP server's tool `{expected_tool}`; got {tools:?}"
    );

    // The agent must also be able to invoke the connected server's tool.
    let mcp_manager = agent
        .mcp_manager()
        .expect("agent should hold an MCP manager");
    let result = mcp_manager
        .execute_tool_call(
            TEST_SERVER_NAME,
            &InternalToolRequest {
                id: "test-call-1".to_string(),
                name: expected_tool.clone(),
                arguments: json!({ "text": "hello" }),
            },
        )
        .await
        .expect("invoking the HTTP MCP server's tool should succeed");
    assert!(
        result.contains("echo: hello"),
        "tool call should round-trip through the HTTP MCP server; got `{result}`"
    );
}
