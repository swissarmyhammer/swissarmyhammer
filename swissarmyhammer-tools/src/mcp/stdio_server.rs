//! Stdio MCP server implementation using rmcp transport
//!
//! This module provides stdio transport for the existing MCP server using the
//! proper rmcp stdio transport instead of reimplementing MCP protocol.
//! 
//! NOTE: This module is DEPRECATED in favor of unified_server.rs
//! It's kept for backward compatibility with existing AgentExecutor integration.

use std::sync::Arc;
use swissarmyhammer_common::Result;
use tokio::sync::Mutex;

/// Handle for managing stdio MCP server lifecycle and providing connection information
///
/// DEPRECATED: Use unified_server::McpServerHandle instead.
/// This is kept for backward compatibility with existing AgentExecutor integration.
#[derive(Debug, Clone)]
pub struct McpServerHandle {
    /// Connection URL (always "stdio" for stdio transport)
    url: String,
    /// Shutdown sender for graceful shutdown (dummy for stdio)
    shutdown_tx: Arc<Mutex<Option<tokio::sync::oneshot::Sender<()>>>>,
}

impl McpServerHandle {
    /// Create a new stdio MCP server handle
    fn new(shutdown_tx: tokio::sync::oneshot::Sender<()>) -> Self {
        Self {
            url: "stdio".to_string(),
            shutdown_tx: Arc::new(Mutex::new(Some(shutdown_tx))),
        }
    }

    /// Get the connection URL (always "stdio" for stdio transport)
    pub fn url(&self) -> &str {
        &self.url
    }

    /// Get the port (None for stdio transport)
    pub fn port(&self) -> Option<u16> {
        None
    }

    /// Get the host (None for stdio transport)
    pub fn host(&self) -> Option<&str> {
        None
    }

    /// Shutdown the server gracefully
    /// 
    /// Note: For stdio mode, this is mostly a no-op since stdio servers
    /// typically run until the client disconnects or the process terminates.
    pub async fn shutdown(&self) -> Result<()> {
        let mut guard = self.shutdown_tx.lock().await;
        if let Some(tx) = guard.take() {
            if tx.send(()).is_err() {
                tracing::warn!("Server shutdown signal receiver already dropped");
            }
        }
        Ok(())
    }
}

/// Start standalone stdio MCP server (for CLI usage)
///
/// DEPRECATED: Use unified_server::start_mcp_server instead.
/// This is kept for backward compatibility with existing AgentExecutor integration.
pub async fn start_stdio_server() -> Result<McpServerHandle> {
    use super::unified_server::{start_mcp_server, McpServerMode};
    
    tracing::warn!("Using deprecated stdio_server::start_stdio_server - consider migrating to unified_server::start_mcp_server");
    
    let mode = McpServerMode::Stdio;
    
    // Use the unified server and wrap the result in the legacy handle format
    let _unified_handle = start_mcp_server(mode, None).await?;
    
    // Create legacy handle format for backward compatibility
    let (shutdown_tx, _) = tokio::sync::oneshot::channel();
    Ok(McpServerHandle::new(shutdown_tx))
}

/// Start in-process stdio MCP server and return handle
///
/// DEPRECATED: Use unified_server::start_mcp_server instead.
/// This is kept for backward compatibility with existing AgentExecutor integration.
/// 
/// Note: "In-process" stdio server is somewhat of a misnomer since stdio
/// inherently involves external process communication. This function exists
/// for API consistency with the HTTP server.
pub async fn start_in_process_stdio_server() -> Result<McpServerHandle> {
    start_stdio_server().await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_stdio_server_handle_creation() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let handle = McpServerHandle::new(tx);
        
        assert_eq!(handle.url(), "stdio");
        assert_eq!(handle.port(), None);
        assert_eq!(handle.host(), None);
    }

    #[tokio::test]
    async fn test_stdio_server_shutdown() {
        let (tx, _rx) = tokio::sync::oneshot::channel();
        let handle = McpServerHandle::new(tx);
        
        // Should not panic
        handle.shutdown().await.unwrap();
        
        // Second shutdown should also work (idempotent)
        handle.shutdown().await.unwrap();
    }
}