//! MCP server that exposes ShellExecuteTool over stdio.
//!
//! Creates a minimal rmcp server hosting only the shell tool, suitable
//! for AI coding agents that need a persistent shell with history and search.

use std::sync::Arc;

use rmcp::model::{
    CallToolRequestParams, CallToolResult, Implementation, ListToolsResult, PaginatedRequestParams,
    ServerCapabilities, ServerInfo, Tool,
};
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use swissarmyhammer_config::model::ModelConfig;
use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolContext};
use swissarmyhammer_tools::mcp::tools::shell::ShellExecuteTool;
use tokio::sync::Mutex;

/// Minimal MCP server that exposes only the shell tool.
///
/// Wraps `ShellExecuteTool` and implements `rmcp::ServerHandler` so it can be
/// served directly over stdio using `rmcp::serve_server`.
#[derive(Clone)]
pub struct ShellToolServer {
    tool: ShellExecuteTool,
    context: ToolContext,
}

impl ShellToolServer {
    /// Create a new `ShellToolServer` with a fresh shell state.
    pub fn new() -> Self {
        let context = ToolContext::new(
            Arc::new(ToolHandlers::new()),
            Arc::new(Mutex::new(None)),
            Arc::new(ModelConfig::default()),
        );
        Self {
            tool: ShellExecuteTool::new(),
            context,
        }
    }
}

impl Default for ShellToolServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for ShellToolServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new("shelltool", env!("CARGO_PKG_VERSION")))
    }

    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let schema = self.tool.schema();
        let schema_map = match schema {
            serde_json::Value::Object(map) => map,
            _ => serde_json::Map::new(),
        };
        let tool = Tool::new(self.tool.name(), self.tool.description(), schema_map)
            .with_title(self.tool.name());

        Ok(ListToolsResult {
            tools: vec![tool],
            next_cursor: None,
            meta: None,
        })
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        if request.name != self.tool.name() {
            return Err(McpError::invalid_request(
                format!("Unknown tool: {}", request.name),
                None,
            ));
        }

        let arguments = request.arguments.unwrap_or_default();
        self.tool.execute(arguments, &self.context).await
    }
}

/// Run the MCP shell server over stdio until EOF.
///
/// Starts the rmcp stdio server with the shell tool and blocks until the
/// MCP client disconnects or an error occurs. Intended to be called from
/// the `serve` subcommand handler.
///
/// # Errors
///
/// Returns an error string if the server fails to start or encounters a fatal error.
pub async fn run_serve() -> Result<(), String> {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;

    let server = ShellToolServer::new();
    let running = serve_server(server, stdio())
        .await
        .map_err(|e| e.to_string())?;
    running.waiting().await.map_err(|e| e.to_string())?;
    Ok(())
}
