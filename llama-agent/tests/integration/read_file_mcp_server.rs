//! In-process MCP server fixture exposing a single `read_file` tool.
//!
//! This server is used by the multi-turn tool-use integration tests to verify
//! that `llama-agent`'s agentic loop dispatches tool calls, feeds results
//! back to the model, and continues generation in a single `prompt()` /
//! `generate()` call.
//!
//! The server is intentionally minimal:
//! - One tool: `read_file(path: string)` that returns the file contents
//! - HTTP transport via `StreamableHttpService` for use with
//!   `MCPServerConfig::Http`
//! - Listens on an OS-assigned port (`127.0.0.1:0`) so multiple tests can
//!   run in parallel without colliding
//!
//! Pattern follows `agent-client-protocol-extras::test_mcp_server` — we
//! don't reuse that module directly because its `list-files` /
//! `create-plan` tools don't exercise real filesystem reads.

use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use serde_json::{json, Map};
use std::sync::Arc;
use tokio::net::TcpListener;

/// Minimal MCP server with a single `read_file` tool that performs a real
/// filesystem read. Used as a fixture for multi-turn tool-use tests.
#[derive(Clone)]
pub struct ReadFileMcpServer {
    name: String,
    version: String,
}

impl ReadFileMcpServer {
    /// Construct a new server instance. Server name and version are fixed.
    pub fn new() -> Self {
        Self {
            name: "read-file-test-server".to_string(),
            version: "1.0.0".to_string(),
        }
    }

    /// Build the canonical `read_file` tool descriptor advertised via MCP.
    ///
    /// Schema is intentionally simple — a single required `path` string —
    /// so the model has an unambiguous shape to invoke.
    fn build_tools() -> Vec<Tool> {
        vec![Tool::new(
            "read_file",
            "Read a file from the filesystem and return its contents as text.",
            Arc::new({
                let mut map = Map::new();
                map.insert("type".to_string(), json!("object"));
                map.insert(
                    "properties".to_string(),
                    json!({
                        "path": {
                            "type": "string",
                            "description": "Absolute path to the file to read"
                        }
                    }),
                );
                map.insert("required".to_string(), json!(["path"]));
                map
            }),
        )]
    }
}

/// Start the `ReadFileMcpServer` as an in-process HTTP server bound to an
/// OS-assigned port on localhost.
///
/// Returns the URL the rmcp client should connect to (e.g.
/// `http://127.0.0.1:54321/mcp`). The server runs in a background tokio
/// task and is implicitly cleaned up when the test process exits.
pub async fn start_read_file_mcp_server() -> Result<String, Box<dyn std::error::Error>> {
    let server = Arc::new(ReadFileMcpServer::new());
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}/mcp", addr);

    tracing::info!("ReadFileMcpServer starting on {}", url);

    let http_service = StreamableHttpService::new(
        move || Ok((*server).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", http_service);

    tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("ReadFileMcpServer error: {}", e);
        }
    });

    // Brief delay so the listener is fully ready before tests connect.
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    tracing::info!("ReadFileMcpServer running at {}", url);
    Ok(url)
}

impl ServerHandler for ReadFileMcpServer {
    async fn initialize(
        &self,
        request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::debug!(
            "ReadFileMcpServer: Client connecting: {} v{}",
            request.client_info.name,
            request.client_info.version
        );

        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        Ok(InitializeResult::new(caps)
            .with_server_info(Implementation::new(self.name.clone(), self.version.clone()))
            .with_instructions(
                "Test MCP server providing a single `read_file` tool for multi-turn tool-use \
                 round-trip testing.",
            ))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        Ok(ListToolsResult {
            tools: Self::build_tools(),
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!("ReadFileMcpServer: call_tool: {}", request.name);

        match request.name.as_ref() {
            "read_file" => {
                let arguments = request.arguments.unwrap_or_default();
                let path = arguments
                    .get("path")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params(
                            "read_file requires a `path` string argument".to_string(),
                            None,
                        )
                    })?;

                tracing::debug!("ReadFileMcpServer: reading file: {}", path);

                match std::fs::read_to_string(path) {
                    Ok(contents) => Ok(CallToolResult::success(vec![Content::text(contents)])),
                    Err(e) => {
                        let msg = format!("Failed to read file '{}': {}", path, e);
                        tracing::warn!("ReadFileMcpServer: {}", msg);
                        Err(McpError::invalid_request(msg, None))
                    }
                }
            }
            other => Err(McpError::invalid_request(
                format!("Unknown tool: {}", other),
                None,
            )),
        }
    }

    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });

        ServerInfo::new(caps)
            .with_server_info(Implementation::new(self.name.clone(), self.version.clone()))
            .with_instructions(
                "Test MCP server providing a single `read_file` tool for multi-turn tool-use \
                 round-trip testing.",
            )
    }
}
