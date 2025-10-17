//! Notification tools for MCP operations
//!
//! This module provides tools for sending messages from LLMs to users through the logging system.
//! The notify tool enables LLMs to communicate important information, status updates, and contextual
//! feedback during workflow execution.
//!
//! ## Communication Channel
//!
//! The notify system uses the tracing framework to log messages with structured data:
//!
//! - **Target**: Uses "llm_notify" as the logging target for filtering
//! - **Levels**: Supports info, warn, and error notification levels
//! - **Context**: Optional structured JSON data can be included
//! - **Real-time**: Messages appear immediately in the CLI output stream
//!
//! ## Use Cases
//!
//! The notify tool addresses several important communication needs:
//!
//! - **Status Updates**: Inform users of progress during long-running operations
//! - **Discovery Notifications**: Surface important findings during code analysis
//! - **Decision Communication**: Explain automated choices and recommendations
//! - **Warning Messages**: Alert users to potential issues or required attention
//! - **Workflow Visibility**: Provide transparency into LLM reasoning and actions
//!
//! ## Tool Implementation Pattern
//!
//! Notify tools follow the standard MCP pattern with tracing integration:
//! ```rust,no_run
//! use tracing;
//!
//! fn example_notification() {
//!     let message = "Processing large codebase - this may take a few minutes";
//!     let level = "info";
//!     let context = serde_json::json!({"stage": "analysis"});
//!     
//!     match level {
//!         "info" => tracing::info!(target: "llm_notify", context = %context, "{}", message),
//!         "warn" => tracing::warn!(target: "llm_notify", context = %context, "{}", message),
//!         "error" => tracing::error!(target: "llm_notify", context = %context, "{}", message),
//!         _ => tracing::info!(target: "llm_notify", context = %context, "{}", message),
//!     }
//! }
//! ```
//!
use crate::mcp::tool_registry::ToolRegistry;

/// Register all notification-related tools with the registry
///
/// Note: Notification tools have been removed in favor of native MCP progress notifications.
/// This function is kept for backward compatibility but does not register any tools.
pub fn register_notify_tools(_registry: &mut ToolRegistry) {
    // No tools to register - notification functionality replaced by MCP progress notifications
}
