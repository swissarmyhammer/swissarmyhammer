//! MCP Tools Registry Module
//!
//! This module organizes MCP tools using the modular registry pattern.
//! Each tool category has its own submodule with dedicated implementations.
//!
//! ## Architecture Overview
//!
//! The tool registry pattern provides a clean, modular approach to organizing MCP tools:
//!
//! ### Tool Structure
//! Each tool follows a consistent pattern:
//! - Individual module directory (e.g., `issues/create/`)
//! - `mod.rs` containing the tool implementation with `McpTool` trait
//! - `description.md` containing comprehensive tool documentation
//! - Registration function that adds tools to the global registry
//!
//! ### Registration Workflow
//! 1. Tools are organized by category (issues, memoranda, etc.)
//! 2. Each category module exports a `register_*_tools(registry)` function
//! 3. The main `tool_registry.rs` calls these registration functions
//! 4. Tools are stored in a centralized `ToolRegistry` for MCP operations
//!
//! ### MCP Integration
//! Tools implement the `McpTool` trait which provides:
//! - `name()`: Unique tool identifier for MCP protocol
//! - `description()`: Human-readable documentation from `description.md`
//! - `schema()`: JSON schema defining input parameters
//! - `execute()`: Async implementation handling tool execution
//!
//! ## Architectural Benefits
//!
//! - **Modularity**: Each tool is self-contained with its own module
//! - **Consistency**: All tools follow the same implementation pattern
//! - **Maintainability**: Easy to add, modify, or remove individual tools
//! - **Documentation**: Comprehensive descriptions co-located with implementation
//! - **Type Safety**: Strong typing through schema validation and Rust's type system

pub mod agent;
pub mod code_context;
pub mod files;

pub mod git;
pub mod kanban;
pub mod questions;
pub mod ralph;
pub mod review;
pub mod shell;
pub mod skill;
pub mod web;

use crate::mcp::tool_registry::ToolRegistry;

/// Compose the **validator profile** registry.
///
/// This is the single, data-driven definition of the locked-down tool surface
/// served to AVP validators on the `/mcp/validator` endpoint. It is the
/// validator analogue of [`files::register_file_tools`] and friends: one place
/// that names exactly the tools a validator may reach, interpreted by one code
/// path ([`crate::mcp::server::McpServer::create_validator_server`]).
///
/// The profile is exactly:
///
/// - `code_context` — indexed code intelligence (search/grep/symbols/callgraph)
/// - `read_file`, `glob_files`, `grep_files` — the **split** read-only file
///   tools, exposed under their natural names (not the unified `op`-dispatched
///   `files` tool) so Hermes-trained validator models can call them by name.
///
/// The split read-only file tools are [`ToolCategory::Agent`](crate::mcp::tool_registry::ToolCategory::Agent),
/// so the main per-client serve path correctly does **not** advertise them
/// (off-the-shelf agents provide file access natively). This profile is the
/// one path that hands them to validators — the validator server serves this
/// registry verbatim rather than re-filtering by host/category.
///
/// This composition replaces the former per-tool `is_validator_tool()` boolean:
/// the membership of the validator surface is declared here as data, not
/// scattered across each tool module as a flag the server must scan.
///
/// # Arguments
///
/// * `registry` - The tool registry to populate with the validator profile.
pub fn register_validator_tools(registry: &mut ToolRegistry) {
    registry.register(code_context::CodeContextTool::new());
    files::register_validator_file_tools(registry);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    /// The validator profile is exactly the locked-down AVP subset: the
    /// `code_context` tool plus the three split read-only file tools. This pins
    /// the data-driven membership at its source of truth; the served-set audit
    /// in `mcp::server::tests::test_validator_server_serves_exactly_the_profile`
    /// then proves the validator server serves precisely this set.
    #[test]
    fn test_validator_profile_membership() {
        let mut registry = ToolRegistry::new();
        register_validator_tools(&mut registry);

        let names: BTreeSet<&str> = registry
            .iter_tools()
            .map(crate::mcp::tool_registry::McpTool::name)
            .collect();
        let expected: BTreeSet<&str> = ["code_context", "read_file", "glob_files", "grep_files"]
            .into_iter()
            .collect();

        assert_eq!(names, expected);
    }
}
