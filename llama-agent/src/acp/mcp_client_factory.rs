//! Factory for creating MCP clients from ACP McpServer configurations

use crate::mcp::{MCPClient, UnifiedMCPClient};
use crate::mcp_client_handler::NotifyingClientHandler;
use crate::types::MCPError;
use agent_client_protocol::McpServer;
use std::sync::Arc;

/// Create a UnifiedMCPClient from an ACP McpServer configuration
///
/// This factory method handles the conversion from ACP protocol types to
/// llama-agent's internal MCP client implementation.
///
/// # Arguments
///
/// * `server` - ACP McpServer configuration (Stdio, Http, or Sse)
/// * `handler` - NotifyingClientHandler for MCP→ACP notification conversion
///
/// # Returns
///
/// A configured and connected UnifiedMCPClient
///
/// # Errors
///
/// Returns MCPError if connection fails
pub async fn create_mcp_client_from_acp(
    server: &McpServer,
    handler: Arc<NotifyingClientHandler>,
) -> Result<Arc<dyn MCPClient>, MCPError> {
    match server {
        McpServer::Stdio(stdio_config) => {
            tracing::info!(
                "Creating stdio MCP client: {} command: {:?}",
                stdio_config.name,
                stdio_config.command
            );

            let client = UnifiedMCPClient::with_spawned_process(
                &stdio_config.command.to_string_lossy(),
                &stdio_config.args,
                None, // Default timeout
            )
            .await?;

            Ok(Arc::new(client))
        }
        McpServer::Http(http_config) => {
            tracing::info!(
                "Creating HTTP MCP client: {} url: {}",
                http_config.name,
                http_config.url
            );

            let client = UnifiedMCPClient::with_streamable_http_and_handler(
                &http_config.url,
                None, // Default timeout
                handler.clone(),
            )
            .await?;

            Ok(Arc::new(client))
        }
        McpServer::Sse(sse_config) => {
            tracing::info!(
                "Creating SSE MCP client: {} url: {}",
                sse_config.name,
                sse_config.url
            );

            // SSE uses same transport as HTTP in rmcp
            let client = UnifiedMCPClient::with_streamable_http_and_handler(
                &sse_config.url,
                None, // Default timeout
                handler,
            )
            .await?;

            Ok(Arc::new(client))
        }
        _ => {
            // Unknown MCP server type (future-proofing)
            tracing::warn!("Unknown MCP server type, cannot create client");
            Err(MCPError::Protocol("Unknown MCP server type".to_string()))
        }
    }
}

#[cfg(test)]
mod tests {
    // Test removed - McpServer construction should be tested at the ACP protocol level
    // This factory just consumes already-constructed McpServer instances
}
