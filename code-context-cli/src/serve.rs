//! MCP server that exposes code-context tools over stdio.
//!
//! Creates an rmcp server hosting the code-context tool suite, suitable
//! for AI coding agents that need structural code intelligence.

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
use swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool;
use tokio::sync::Mutex;

/// Minimal MCP server that exposes code-context tools.
///
/// Wraps `CodeContextTool` and implements `rmcp::ServerHandler` so it can be
/// served directly over stdio using `rmcp::serve_server`.
#[derive(Clone)]
pub struct CodeContextServer {
    tool: CodeContextTool,
    context: ToolContext,
}

impl CodeContextServer {
    /// Create a new `CodeContextServer` with a fresh tool context.
    pub fn new() -> Self {
        let context = ToolContext::new(
            Arc::new(ToolHandlers::new()),
            Arc::new(Mutex::new(None)),
            Arc::new(ModelConfig::default()),
        );
        Self {
            tool: CodeContextTool::new(),
            context,
        }
    }
}

impl Default for CodeContextServer {
    fn default() -> Self {
        Self::new()
    }
}

impl ServerHandler for CodeContextServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(ServerCapabilities::builder().enable_tools().build()).with_server_info(
            Implementation::new("code-context", env!("CARGO_PKG_VERSION")),
        )
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

/// Run the MCP code-context server over stdio until EOF.
///
/// Starts the rmcp stdio server with the code-context tool and blocks until the
/// MCP client disconnects or an error occurs. Intended to be called from
/// the `serve` subcommand handler.
///
/// # Errors
///
/// Returns an error if the server fails to start or encounters a fatal error.
/// The error chain preserves the original cause for debugging.
pub async fn run_serve() -> anyhow::Result<()> {
    use anyhow::Context;
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;

    let server = CodeContextServer::new();
    let running = serve_server(server, stdio())
        .await
        .context("starting MCP stdio server")?;
    running
        .waiting()
        .await
        .context("MCP server terminated unexpectedly")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::ServerHandler;

    #[test]
    fn test_new() {
        // Construction must not panic.
        let _server = CodeContextServer::new();
    }

    #[test]
    fn test_get_info_has_server_name() {
        let server = CodeContextServer::new();
        let info = server.get_info();
        assert_eq!(info.server_info.name, "code-context");
    }
}
