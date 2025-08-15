//! Shell command execution tools for MCP operations
//!
//! This module provides shell command execution capabilities through the MCP protocol,
//! enabling LLMs to interact with the system through controlled shell commands.
//!
//! ## Overview
//!
//! The shell tools follow the SwissArmyHammer tool organization pattern with noun/verb
//! structure. Commands are executed in isolated processes with proper timeout controls,
//! output capture, and security validation.
//!
//! ## Architecture
//!
//! Shell commands are executed using Rust's `std::process::Command` with async timeout
//! management via `tokio::time::timeout`. The implementation provides:
//!
//! - **Secure Execution**: Process isolation and command validation
//! - **Timeout Management**: Configurable timeouts with process cleanup
//! - **Output Handling**: Structured capture of stdout, stderr, and exit codes
//! - **Environment Control**: Optional working directory and environment variables
//! - **Error Handling**: Comprehensive error reporting with context
//!
//! ## Security Considerations
//!
//! Shell command execution inherently carries security risks. The implementation includes:
//!
//! - Input validation to prevent injection attacks
//! - Configurable command filtering and restrictions
//! - Process isolation using system-level controls
//! - Comprehensive audit logging of all executed commands
//! - Rate limiting to prevent denial of service attacks
//! - Resource monitoring to prevent system exhaustion
//!
//! ## Tool Implementation Pattern
//!
//! Each shell tool follows the standard MCP pattern:
//! ```rust,ignore
//! use async_trait::async_trait;
//! use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
//! use swissarmyhammer_tools::mcp::tool_descriptions;
//!
//! #[derive(Default)]
//! pub struct ShellExecuteTool;
//!
//! impl ShellExecuteTool {
//!     pub fn new() -> Self { Self }
//! }
//!
//! #[async_trait]
//! impl McpTool for ShellExecuteTool {
//!     fn name(&self) -> &'static str {
//!         "shell_execute"
//!     }
//!     
//!     fn description(&self) -> &'static str {
//!         tool_descriptions::get_tool_description("shell", "execute")
//!             .unwrap_or("Tool description not available")
//!     }
//!     
//!     fn schema(&self) -> serde_json::Value {
//!         // JSON schema for command execution parameters
//!     }
//!     
//!     async fn execute(
//!         &self,
//!         arguments: serde_json::Map<String, serde_json::Value>,
//!         context: &ToolContext,
//!     ) -> std::result::Result<rmcp::model::CallToolResult, rmcp::Error> {
//!         // Shell command execution logic
//!     }
//! }
//! ```
//!
//! ## Available Tools
//!
//! - **execute**: Execute shell commands with timeout and environment control
//!
//! ## Usage Examples
//!
//! ### Basic Command Execution
//! ```json
//! {
//!   "command": "ls -la",
//!   "timeout": 30
//! }
//! ```
//!
//! ### Development Workflow
//! ```json
//! {
//!   "command": "cargo test",
//!   "working_directory": "/project/path",
//!   "timeout": 600,
//!   "environment": {
//!     "RUST_LOG": "debug"
//!   }
//! }
//! ```
//!
//! ## Configuration
//!
//! Shell tools can be configured through the SwissArmyHammer configuration system:
//!
//! ```toml
//! [shell_tool]
//! default_timeout = 300      # seconds
//! max_timeout = 1800         # seconds  
//! max_output_size = "10MB"   # maximum output size
//! allowed_directories = ["/project", "/tmp"]  # directory restrictions
//! blocked_commands = ["rm -rf", "format"]     # command blacklist
//! log_commands = true        # audit logging
//! ```
//!
//! ## Integration with Workflows
//!
//! Shell tools integrate seamlessly with SwissArmyHammer workflows:
//!
//! - Commands can be part of larger automation workflows
//! - Output can be passed to subsequent workflow steps
//! - Conditional execution based on command exit codes
//! - Integration with abort mechanisms for failure handling
//!
//! ## Performance Considerations
//!
//! - Commands execute in separate processes for isolation
//! - Timeout management prevents hung processes
//! - Output buffering prevents memory exhaustion
//! - Rate limiting prevents system overload
//! - Process cleanup ensures no orphaned processes
//!
//! ## Error Handling
//!
//! The shell tools provide comprehensive error handling:
//!
//! - **Command Failures**: Detailed exit code and stderr reporting
//! - **Timeouts**: Graceful process termination with partial output
//! - **Permission Errors**: Clear error messages with context
//! - **Resource Exhaustion**: Controlled failure with diagnostics
//! - **Invalid Parameters**: Comprehensive validation error messages
//!
//! ## Testing Strategy
//!
//! Shell tools include comprehensive test coverage:
//!
//! - Unit tests for parameter validation and error handling
//! - Integration tests with actual command execution
//! - Security tests for injection prevention
//! - Performance tests for timeout and resource management
//! - Cross-platform compatibility tests

pub mod execute;

use crate::mcp::tool_registry::ToolRegistry;

/// Register all shell-related tools with the registry
///
/// This function registers all shell command execution tools following the
/// SwissArmyHammer tool registry pattern. Currently includes:
///
/// - `shell_execute`: Execute shell commands with timeout and environment control
///
/// # Arguments
///
/// * `registry` - The tool registry to register shell tools with
///
/// # Example
///
/// ```rust,ignore
/// use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
/// use swissarmyhammer_tools::mcp::tools::shell::register_shell_tools;
///
/// let mut registry = ToolRegistry::new();
/// register_shell_tools(&mut registry);
/// ```
pub fn register_shell_tools(registry: &mut ToolRegistry) {
    registry.register(execute::ShellExecuteTool::new());
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolRegistry;

    #[test]
    fn test_register_shell_tools() {
        let mut registry = ToolRegistry::new();
        register_shell_tools(&mut registry);

        // Verify shell_execute tool is registered
        assert!(registry.get_tool("shell_execute").is_some());
        assert_eq!(registry.len(), 1);
    }

    #[test]
    fn test_shell_tools_properties() {
        let mut registry = ToolRegistry::new();
        register_shell_tools(&mut registry);

        let tools = registry.list_tools();
        assert_eq!(tools.len(), 1);

        let shell_execute_tool = tools
            .iter()
            .find(|tool| tool.name == "shell_execute")
            .expect("shell_execute tool should be registered");

        assert_eq!(shell_execute_tool.name, "shell_execute");
        assert!(shell_execute_tool.description.is_some());
        assert!(!shell_execute_tool.input_schema.is_empty());
    }

    #[test]
    fn test_multiple_registrations() {
        let mut registry = ToolRegistry::new();

        // Register twice to ensure no conflicts
        register_shell_tools(&mut registry);
        register_shell_tools(&mut registry);

        // Should have only one tool (second registration overwrites)
        assert_eq!(registry.len(), 1);
        assert!(registry.get_tool("shell_execute").is_some());
    }

    #[test]
    fn test_shell_tool_name_uniqueness() {
        let mut registry = ToolRegistry::new();
        register_shell_tools(&mut registry);

        let tool_names = registry.list_tool_names();
        let unique_names: std::collections::HashSet<_> = tool_names.iter().collect();

        // All tool names should be unique
        assert_eq!(tool_names.len(), unique_names.len());
    }
}
