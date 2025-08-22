//! Dynamic CLI builder for MCP tools
//!
//! This module implements dynamic CLI command generation from MCP tool definitions,
//! eliminating the need for redundant CLI command enums and ensuring consistency
//! between MCP and CLI interfaces.

use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};

/// Dynamic CLI builder that generates commands from MCP tool registry
pub struct CliBuilder<'a> {
    tool_registry: &'a ToolRegistry,
}

impl<'a> CliBuilder<'a> {
    /// Create a new CLI builder with the given tool registry
    pub fn new(tool_registry: &'a ToolRegistry) -> Self {
        Self { tool_registry }
    }

    /// Build the complete CLI with both static and dynamic commands
    pub fn build_cli(&self) -> Command {
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("An MCP server for managing prompts as markdown files")
            .long_about("
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

This CLI includes both static commands and dynamic commands generated from MCP tools.
")
            // Add verbose/debug/quiet flags from parent CLI
            .arg(Arg::new("verbose")
                .short('v')
                .long("verbose")
                .help("Enable verbose logging")
                .action(ArgAction::SetTrue))
            .arg(Arg::new("debug")
                .short('d')
                .long("debug")
                .help("Enable debug logging")
                .action(ArgAction::SetTrue))
            .arg(Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress all output except errors")
                .action(ArgAction::SetTrue));

        // Add dynamic MCP tool commands
        let tool_categories = self.tool_registry.get_cli_categories();
        for category in tool_categories {
            cli = cli.subcommand(self.build_category_command(&category));
        }

        cli
    }

    /// Build a command for a specific tool category
    fn build_category_command(&self, category: &str) -> Command {
        let about_text = format!("{} management commands", category.to_uppercase());
        let mut cmd = Command::new(category)
            .about(about_text);

        let tools = self.tool_registry.get_tools_for_category(category);
        for tool in tools {
            if !tool.hidden_from_cli() {
                cmd = cmd.subcommand(self.build_tool_command(tool));
            }
        }

        cmd
    }

    /// Build a command for a specific MCP tool
    fn build_tool_command(&self, tool: &dyn McpTool) -> Command {
        let schema = tool.schema();
        let mut cmd = Command::new(tool.cli_name());

        // Set about text from tool
        if let Some(about) = tool.cli_about() {
            cmd = cmd.about(about);
        }

        // Set long about from full description
        cmd = cmd.long_about(tool.description());

        // Convert JSON schema to clap arguments
        cmd = self.schema_to_clap_args(cmd, &schema);

        cmd
    }

    /// Convert JSON schema properties to clap arguments
    fn schema_to_clap_args(&self, mut cmd: Command, schema: &Value) -> Command {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            // Determine required fields
            let required_fields: std::collections::HashSet<String> = schema
                .get("required")
                .and_then(|r| r.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default();

            for (prop_name, prop_schema) in properties {
                let arg = self.json_schema_to_clap_arg(
                    prop_name,
                    prop_schema,
                    required_fields.contains(prop_name),
                );
                cmd = cmd.arg(arg);
            }
        }

        cmd
    }

    /// Convert a single JSON schema property to a clap argument
    fn json_schema_to_clap_arg(
        &self,
        name: &str,
        schema: &Value,
        is_required: bool,
    ) -> Arg {
        let mut arg = Arg::new(name).long(name);

        // Set help text from description
        if let Some(desc) = schema.get("description").and_then(|d| d.as_str()) {
            arg = arg.help(desc);
        }

        // Set as required if specified
        if is_required {
            arg = arg.required(true);
        }

        // Configure based on type
        match schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => {
                arg = arg.action(ArgAction::SetTrue);
            }
            Some("integer") => {
                arg = arg.value_parser(clap::value_parser!(i64));
                if !is_required {
                    arg = arg.value_name("NUMBER");
                }
            }
            Some("number") => {
                arg = arg.value_parser(clap::value_parser!(f64));
                if !is_required {
                    arg = arg.value_name("NUMBER");
                }
            }
            Some("array") => {
                arg = arg.action(ArgAction::Append);
                if !is_required {
                    arg = arg.value_name("VALUE");
                }
            }
            _ => {
                // Default to string
                if !is_required {
                    arg = arg.value_name("TEXT");
                }
            }
        }

        // Handle enum values
        if let Some(enum_values) = schema.get("enum").and_then(|e| e.as_array()) {
            let string_values: Vec<&str> = enum_values
                .iter()
                .filter_map(|v| v.as_str())
                .collect();
            if !string_values.is_empty() {
                arg = arg.value_parser(clap::builder::PossibleValuesParser::new(string_values));
            }
        }

        // Handle default values
        if let Some(default_val) = schema.get("default") {
            if let Some(default_str) = default_val.as_str() {
                arg = arg.default_value(default_str);
            } else if let Some(default_bool) = default_val.as_bool() {
                if default_bool {
                    // For boolean defaults that are true, we need special handling
                    arg = arg.action(ArgAction::SetFalse);
                }
            }
        }

        arg
    }
}