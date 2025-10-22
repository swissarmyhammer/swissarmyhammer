//! Model Context Protocol (MCP) server support
//!
//! This module provides MCP server functionality for serving prompts, workflows,
//! and various tools through the Model Context Protocol.
//!
//! ## Overview
//!
//! The MCP module implements the core server infrastructure for handling MCP requests.
//! It provides:
//!
//! - **Server Implementation**: [`McpServer`] handles MCP protocol messages
//! - **Tool Registry**: [`ToolRegistry`] manages available tools and their execution
//! - **Tool Context**: [`ToolContext`] provides shared state and storage access
//! - **Progress Notifications**: Real-time progress updates for long-running operations
//! - **File Watching**: Automatic detection of changes to workflows and issues
//!
//! ## Architecture
//!
//! The module follows a layered architecture:
//!
//! 1. **Server Layer**: Handles MCP protocol communication
//! 2. **Registry Layer**: Manages tool registration and dispatch
//! 3. **Tool Layer**: Individual tool implementations
//! 4. **Storage Layer**: Backend storage for issues, memos, and workflows
//!
//! ## Usage
//!
//! ### Starting a Server
//!
//! ```rust
//! use swissarmyhammer_tools::mcp::{McpServer, start_mcp_server, McpServerMode};
//! use swissarmyhammer_prompts::PromptLibrary;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Create and start an MCP server in stdio mode
//! let library = PromptLibrary::new();
//! let handle = start_mcp_server(library, None, McpServerMode::Stdio).await?;
//!
//! // Server is now running and handling requests
//! # Ok(())
//! # }
//! ```
//!
//! ### Registering Tools
//!
//! ```rust
//! use swissarmyhammer_tools::mcp::{ToolRegistry, register_file_tools, register_search_tools};
//!
//! # fn example() {
//! let mut registry = ToolRegistry::new();
//!
//! // Register tool categories
//! register_file_tools(&mut registry);
//! register_search_tools(&mut registry);
//!
//! println!("Registered {} tools", registry.list_tools().len());
//! # }
//! ```
//!
//! ### Sending Progress Notifications
//!
//! ```rust
//! use swissarmyhammer_tools::mcp::{ProgressNotification, generate_progress_token};
//! use swissarmyhammer_tools::mcp::progress_notifications::{start_notification, complete_notification};
//!
//! # async fn example(sender: swissarmyhammer_tools::mcp::ProgressSender) {
//! // Generate a unique progress token
//! let token = generate_progress_token();
//!
//! // Send start notification
//! let _ = start_notification(&sender, &token, "Processing files").await;
//!
//! // Do work...
//!
//! // Send completion notification
//! let _ = complete_notification(&sender, &token, "Completed processing").await;
//! # }
//! ```

// Module declarations
pub mod error_handling;
pub mod file_watcher;
pub mod memo_types;
pub mod notifications;
pub mod notify_types;
pub mod progress_notifications;
pub mod responses;
pub mod search_types;
pub mod server;
pub mod shared_utils;
pub mod tool_descriptions;
pub mod tool_handlers;
pub mod tool_registry;
pub mod tools;
pub mod types;
pub mod unified_server;
pub mod utils;

pub mod test_utils;

#[cfg(test)]
mod tests;

// Re-export commonly used items from submodules
pub use notifications::{
    FlowNotification, FlowNotificationMetadata, NotificationSender, SendError as FlowSendError,
};
pub use progress_notifications::{
    complete_notification, generate_progress_token, start_notification, ProgressNotification,
    ProgressSender, SendError as ProgressSendError,
};
pub use server::McpServer;
pub use tool_handlers::ToolHandlers;
pub use tool_registry::{
    register_abort_tools, register_file_tools, register_git_tools, register_issue_tools,
    register_memo_tools, register_outline_tools, register_rules_tools, register_search_tools,
    register_shell_tools, register_todo_tools, register_web_fetch_tools, register_web_search_tools,
    ToolContext, ToolRegistry,
};
pub use types::{GetPromptRequest, ListPromptsRequest};
pub use unified_server::{
    start_mcp_server, McpServerHandle as UnifiedMcpServerHandle, McpServerInfo, McpServerMode,
};

pub use types::{
    AllCompleteRequest, CreateIssueRequest, IssueName, MarkCompleteRequest, UpdateIssueRequest,
};
pub use utils::validate_issue_name;
