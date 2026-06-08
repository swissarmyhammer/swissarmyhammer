//! Command modules for code-context CLI.
//!
//! Each command is organized in its own module:
//! - `serve`: MCP server over stdio
//! - `doctor`: Diagnostic checks for setup and configuration
//! - `registry`: Init/deinit component registration
//! - `skill`: Skill resolution and deployment
//! - `ops`: CLI-to-MCP operation dispatch

pub mod doctor;
pub mod ops;
pub mod registry;
pub mod serve;
pub mod skill;

/// Build the FULL `CodeContextTool` schema for tests.
///
/// The runtime command-tree generator consumes the FULL schema in-process
/// (per-op `x-operation-schemas` + flat properties), so tests that build the
/// tree use `schema_full()`. Shared here so `main.rs` and `commands::ops` do
/// not each redeclare the same one-liner.
#[cfg(test)]
pub(crate) fn test_schema_full() -> serde_json::Value {
    use swissarmyhammer_tools::mcp::tool_registry::McpTool;
    swissarmyhammer_tools::mcp::tools::code_context::CodeContextTool::new().schema_full()
}
