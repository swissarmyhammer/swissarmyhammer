//! Model Context Protocol (MCP) server support
//!
//! This module provides MCP server functionality for serving prompts, workflows,
//! and various tools through the Model Context Protocol.

// Module declarations
pub mod error_handling;
pub mod file_watcher;
pub mod http_server;
pub mod memo_types;
pub mod notify_types;
pub mod responses;
pub mod search_types;
pub mod server;
pub mod shared_utils;
pub mod tool_descriptions;
pub mod tool_handlers;
pub mod tool_registry;
pub mod tools;
pub mod types;
pub mod utils;

#[cfg(test)]
mod tests;

// Re-export commonly used items from submodules
pub use http_server::{start_http_server, start_in_process_mcp_server, McpServerHandle};
pub use server::McpServer;
pub use tool_handlers::ToolHandlers;
pub use tool_registry::{
    register_abort_tools, register_file_tools, register_issue_tools, register_memo_tools,
    register_notify_tools, register_outline_tools, register_search_tools, register_shell_tools,
    register_todo_tools, register_web_fetch_tools, register_web_search_tools, ToolContext,
    ToolRegistry,
};
pub use types::{GetPromptRequest, ListPromptsRequest};

pub use types::{
    AllCompleteRequest, CreateIssueRequest, IssueName, MarkCompleteRequest, MergeIssueRequest,
    UpdateIssueRequest, WorkIssueRequest,
};
pub use utils::validate_issue_name;

use std::sync::Arc;
use swissarmyhammer_config::McpServerConfig;
use tokio::sync::OnceCell;

/// Global singleton for MCP server
/// This ensures the MCP server is started only once per process
static GLOBAL_MCP_SERVER: OnceCell<Arc<McpServerHandle>> = OnceCell::const_new();

/// Get or initialize the global MCP server
///
/// This function implements a singleton pattern to ensure the MCP server
/// is started only once per process. Subsequent calls will return the
/// same server handle.
pub async fn get_or_init_global_mcp_server() -> swissarmyhammer::Result<Arc<McpServerHandle>> {
    GLOBAL_MCP_SERVER
        .get_or_try_init(|| async {
            tracing::info!("Initializing global MCP server");

            // Use default configuration with random port
            let config = McpServerConfig {
                port: 0, // Let the OS assign a port
                timeout_seconds: 60,
            };

            let server_handle = start_in_process_mcp_server(&config).await?;

            tracing::info!(
                "Global MCP server initialized on port {}",
                server_handle.port()
            );

            Ok(Arc::new(server_handle))
        })
        .await
        .cloned()
}
