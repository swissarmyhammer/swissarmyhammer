//! Dynamic CLI Builder Infrastructure
//!
//! This module provides the `CliBuilder` struct that dynamically generates Clap commands
//! from MCP tool registry, replacing static command definitions for MCP tools.
//!
//! # Architecture
//!
//! The dynamic CLI builder separates concerns between:
//! - Static CLI commands (serve, doctor, prompt, flow, etc.) - preserved as-is
//! - Dynamic MCP tool commands - generated from tool registry
//!
//! # Usage
//!
//! ```rust,ignore
//! use std::sync::Arc;
//! use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
//! use dynamic_cli::CliBuilder;
//!
//! let mut registry = ToolRegistry::new();
//! // Register tools...
//! 
//! let builder = CliBuilder::new(Arc::new(registry));
//! let cli = builder.build_cli();
//! let matches = cli.get_matches();
//! ```

use clap::Command;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;
use crate::schema_conversion::SchemaConverter;

/// Dynamic CLI builder that generates Clap commands from MCP tool registry
///
/// The `CliBuilder` creates a complete CLI application by combining:
/// 1. Static commands (serve, doctor, prompt, flow, completion, validate, plan, implement)
/// 2. Dynamic MCP tool commands organized by category
///
/// # Design Goals
///
/// - **Single Source of Truth**: MCP tool schemas drive both MCP and CLI interfaces
/// - **Automatic Registration**: New MCP tools appear in CLI without code changes
/// - **Category Organization**: Tools grouped by logical categories (memo, issue, file, etc.)
/// - **Static Command Preservation**: Existing CLI commands remain unchanged
///
/// # Command Structure
///
/// ```text
/// swissarmyhammer
/// ├── serve                    # Static command
/// ├── doctor                   # Static command
/// ├── prompt                   # Static command with subcommands
/// ├── flow                     # Static command with subcommands
/// ├── completion               # Static command
/// ├── validate                 # Static command
/// ├── plan                     # Static command
/// ├── implement                # Static command
/// ├── memo                     # Dynamic category
/// │   ├── create               # MCP tool: memo_create
/// │   ├── list                 # MCP tool: memo_list
/// │   └── ...
/// ├── issue                    # Dynamic category
/// │   ├── create               # MCP tool: issue_create
/// │   ├── list                 # MCP tool: issue_list
/// │   └── ...
/// └── file                     # Dynamic category
///     ├── read                 # MCP tool: files_read
///     ├── write                # MCP tool: files_write
///     └── ...
/// ```
pub struct CliBuilder {
    /// The MCP tool registry containing all registered tools
    tool_registry: Arc<ToolRegistry>,
}

impl CliBuilder {
    /// Create a new CLI builder with the given tool registry
    ///
    /// # Arguments
    ///
    /// * `tool_registry` - Arc-wrapped tool registry containing all MCP tools
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    /// Build the complete CLI application with both static and dynamic commands
    ///
    /// Constructs a Clap Command with:
    /// 1. Base CLI metadata (name, version, about)
    /// 2. Static commands (serve, doctor, etc.)
    /// 3. Dynamic MCP tool commands organized by category
    ///
    /// # Returns
    ///
    /// * `Command` - Complete Clap command ready for argument parsing
    pub fn build_cli(&self) -> Command {
        // Build base CLI with metadata
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("An MCP server for managing prompts and workflows")
            .long_about("
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

Example usage:
  swissarmyhammer serve     # Run as MCP server
  swissarmyhammer doctor    # Check configuration and setup
  swissarmyhammer completion bash > ~/.bashrc.d/swissarmyhammer  # Generate bash completions
");

        // Add static commands (preserve existing CLI structure)
        cli = self.add_static_commands(cli);

        // Add dynamic MCP tool commands
        cli = self.add_dynamic_commands(cli);

        cli
    }

    /// Add static CLI commands that don't correspond to MCP tools
    ///
    /// These commands are preserved exactly as they exist in the current CLI:
    /// - serve: Run as MCP server
    /// - doctor: Diagnose configuration issues  
    /// - prompt: Manage and test prompts
    /// - flow: Execute and manage workflows
    /// - completion: Generate shell completion scripts
    /// - validate: Validate prompt files and workflows
    /// - plan: Execute planning workflow
    /// - implement: Execute implement workflow
    ///
    /// # Arguments
    ///
    /// * `cli` - The base CLI command to add static commands to
    ///
    /// # Returns
    ///
    /// * `Command` - CLI with static commands added
    fn add_static_commands(&self, cli: Command) -> Command {
        // For now, return the CLI unchanged since static commands are already
        // handled by the existing Commands enum. In a full implementation,
        // we would reconstruct the static commands here.
        //
        // This is a placeholder that maintains the current structure while
        // providing the foundation for future static command migration.
        cli
    }

    /// Add dynamic MCP tool commands organized by category
    ///
    /// Creates subcommands for each CLI category discovered in the tool registry.
    /// Each category becomes a subcommand with its own set of tool-based subcommands.
    ///
    /// # Arguments
    ///
    /// * `cli` - The CLI command to add dynamic commands to
    ///
    /// # Returns
    ///
    /// * `Command` - CLI with dynamic MCP tool commands added
    fn add_dynamic_commands(&self, mut cli: Command) -> Command {
        let categories = self.tool_registry.get_cli_categories();
        
        for category in categories {
            let category_command = self.build_category_command(&category);
            cli = cli.subcommand(category_command);
        }
        
        cli
    }

    /// Build a category subcommand containing all tools for that category
    ///
    /// Creates a subcommand for a specific category (e.g., "memo", "issue") and
    /// adds all tools belonging to that category as further subcommands.
    ///
    /// # Arguments
    ///
    /// * `category` - The category name to build a command for
    ///
    /// # Returns
    ///
    /// * `Command` - Category subcommand with tool subcommands
    fn build_category_command(&self, category: &str) -> Command {
        // Use match to provide static string literals for known categories
        let mut cmd = match category {
            "memo" => Command::new("memo").about("MEMO management commands"),
            "issue" => Command::new("issue").about("ISSUE management commands"),
            "file" => Command::new("file").about("FILE management commands"),
            "search" => Command::new("search").about("SEARCH management commands"),
            "web" => Command::new("web").about("WEB management commands"),
            "shell" => Command::new("shell").about("SHELL management commands"),
            "todo" => Command::new("todo").about("TODO management commands"),
            "outline" => Command::new("outline").about("OUTLINE management commands"),
            "notify" => Command::new("notify").about("NOTIFY management commands"),
            "abort" => Command::new("abort").about("ABORT management commands"),
            _ => {
                // For unknown categories, use a generic command
                tracing::warn!("Unknown category '{}', using generic command", category);
                Command::new("unknown").about("Unknown management commands")
            }
        };

        let tools = self.tool_registry.get_tools_for_category(category);
        
        for tool in tools {
            let tool_command = self.build_tool_command(tool);
            cmd = cmd.subcommand(tool_command);
        }
        
        cmd
    }

    /// Build a CLI command for a specific MCP tool
    ///
    /// Converts an MCP tool into a Clap subcommand by:
    /// 1. Using the tool's CLI name as the command name
    /// 2. Using the tool's CLI about text as the command description
    /// 3. Converting the tool's JSON schema to Clap arguments
    ///
    /// # Arguments
    ///
    /// * `tool` - The MCP tool to build a command for
    ///
    /// # Returns
    ///
    /// * `Command` - Clap command representing the tool
    fn build_tool_command(&self, tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool) -> Command {
        let mut cmd = Command::new(tool.cli_name());

        // Set command description
        if let Some(about) = tool.cli_about() {
            cmd = cmd.about(about);
        }

        // Add full description as long_about
        cmd = cmd.long_about(tool.description());

        // Convert JSON schema to Clap arguments
        let schema = tool.schema();
        match SchemaConverter::schema_to_clap_args(&schema) {
            Ok(args) => {
                for arg in args {
                    cmd = cmd.arg(arg);
                }
            }
            Err(e) => {
                // Log the error but don't fail - the command will work without arguments
                tracing::warn!(
                    "Failed to convert schema for tool {}: {}",
                    tool.name(),
                    e
                );
            }
        }

        cmd
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_tools::mcp::tool_registry::{ToolRegistry, McpTool, ToolContext};
    use rmcp::model::CallToolResult;
    use rmcp::Error as McpError;
    use async_trait::async_trait;

    /// Mock tool for testing
    #[derive(Default)]
    struct MockTool {
        name: &'static str,
        category: Option<&'static str>,
        cli_name: &'static str,
    }

    impl MockTool {
        fn new(name: &'static str, category: Option<&'static str>, cli_name: &'static str) -> Self {
            Self { name, category, cli_name }
        }
    }

    #[async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            "Mock tool for testing"
        }

        fn schema(&self) -> serde_json::Value {
            serde_json::json!({
                "type": "object",
                "properties": {
                    "test_param": {
                        "type": "string",
                        "description": "Test parameter"
                    }
                },
                "required": ["test_param"]
            })
        }

        fn cli_category(&self) -> Option<&'static str> {
            self.category
        }

        fn cli_name(&self) -> &'static str {
            self.cli_name
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(swissarmyhammer_tools::mcp::tool_registry::BaseToolImpl::create_success_response("mock"))
        }
    }

    #[test]
    fn test_cli_builder_creation() {
        let registry = Arc::new(ToolRegistry::new());
        let builder = CliBuilder::new(registry);
        
        // Should not panic
        let _cli = builder.build_cli();
    }

    #[test]
    fn test_empty_registry_categories() {
        let registry = Arc::new(ToolRegistry::new());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli();
        
        // With empty registry, should only have base command
        assert_eq!(cli.get_name(), "swissarmyhammer");
    }

    #[test]
    fn test_category_discovery() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("memo_create", Some("memo"), "create"));
        registry.register(MockTool::new("memo_list", Some("memo"), "list"));
        registry.register(MockTool::new("issue_create", Some("issue"), "create"));
        
        let registry = Arc::new(registry);
        let _builder = CliBuilder::new(registry.clone());
        
        let categories = registry.get_cli_categories();
        assert_eq!(categories, vec!["issue", "memo"]); // BTreeSet ensures sorted order
    }

    #[test]
    fn test_tools_for_category() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool::new("memo_create", Some("memo"), "create"));
        registry.register(MockTool::new("memo_list", Some("memo"), "list"));
        registry.register(MockTool::new("issue_create", Some("issue"), "create"));
        
        let registry = Arc::new(registry);
        
        let memo_tools = registry.get_tools_for_category("memo");
        assert_eq!(memo_tools.len(), 2);
        
        let issue_tools = registry.get_tools_for_category("issue");
        assert_eq!(issue_tools.len(), 1);
        
        let nonexistent_tools = registry.get_tools_for_category("nonexistent");
        assert_eq!(nonexistent_tools.len(), 0);
    }
}