//! # SwissArmyHammer Tools
//!
//! MCP (Model Context Protocol) tools and server implementation for SwissArmyHammer.
//!
//! This crate provides the MCP server functionality and tools that integrate with
//! the SwissArmyHammer prompt management library. It includes tools for:
//!
//! - Issue management and tracking
//! - Memoranda (memo/note) management
//! - Semantic search across codebases  
//! - Code outline generation
//!
//! ## Features
//!
//! - **MCP Server**: Full Model Context Protocol server implementation
//! - **Tool Registry**: Extensible tool registration system
//! - **Issue Tools**: Create, manage, and track work items
//! - **Memo Tools**: Note-taking and knowledge management
//! - **Search Tools**: Semantic code search and indexing
//! - **Outline Tools**: Code structure analysis and extraction
//!
//! ## Usage
//!
//! Basic usage example:
//! ```rust,ignore
//! use swissarmyhammer_tools::McpServer;
//! use swissarmyhammer_prompts::PromptLibrary;
//! 
//! let library = PromptLibrary::new();
//! let server = McpServer::new(library)?;
//! ```

#![warn(missing_docs)]

/// Model Context Protocol (MCP) server and tools
pub mod mcp;

/// Test utilities
#[cfg(test)]
pub mod test_utils;

// Re-export key types for convenience
pub use mcp::McpServer;
pub use mcp::{
    register_file_tools, register_issue_tools, register_memo_tools, register_notify_tools,
    register_search_tools, register_shell_tools, register_todo_tools, register_web_fetch_tools,
    register_web_search_tools,
};
pub use mcp::{ToolContext, ToolRegistry};

/// Version of this crate
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
