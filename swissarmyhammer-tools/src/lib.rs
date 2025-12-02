//! # SwissArmyHammer Tools
//!
//! MCP (Model Context Protocol) tools and server implementation for SwissArmyHammer.
//!
//! This crate provides the MCP server functionality and tools that integrate with
//! the SwissArmyHammer prompt management library. It includes tools for:
//!
//! - File operations
//! - Shell command execution
//! - Todo list management
//!
//! ## Features
//!
//! - **MCP Server**: Full Model Context Protocol server implementation
//! - **Tool Registry**: Extensible tool registration system
//!
//! ## Usage
//!
//! ### Basic Server Setup
//!
//! ```rust
//! use swissarmyhammer_tools::{McpServer, ToolRegistry, ToolContext};
//! use swissarmyhammer_prompts::PromptLibrary;
//! use std::sync::Arc;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Initialize the prompt library
//! let library = PromptLibrary::new();
//!
//! // Create the MCP server
//! let server = McpServer::new(library, None).await?;
//!
//! // Initialize to register all tools
//! server.initialize().await?;
//!
//! // The server is now ready to handle MCP requests
//! # Ok(())
//! # }
//! ```
//!
//! ### Registering Custom Tools
//!
//! ```rust
//! use swissarmyhammer_tools::ToolRegistry;
//!
//! # fn example() {
//! let mut registry = ToolRegistry::new();
//!
//! // Register individual tool categories
//! swissarmyhammer_tools::register_file_tools(&mut registry);
//! swissarmyhammer_tools::register_shell_tools(&mut registry);
//!
//! // Access registered tools
//! let tool_names: Vec<_> = registry.list_tools().iter()
//!     .map(|t| t.name())
//!     .collect();
//! println!("Registered {} tools", tool_names.len());
//! # }
//! ```

/// Model Context Protocol (MCP) server and tools
pub mod mcp;

/// Test utilities
#[cfg(test)]
pub mod test_utils;

// Re-export key types for convenience
pub use mcp::McpServer;
pub use mcp::{
    register_file_tools, register_flow_tools, register_git_tools, register_rules_tools,
    register_shell_tools, register_todo_tools, register_web_fetch_tools,
    register_web_search_tools,
};
pub use mcp::{ToolContext, ToolRegistry};

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
