use crate::schema_conversion::SchemaConverter;
use anyhow::Result;
use clap::Command;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::ToolRegistry;

/// Builder for creating dynamic CLI commands from MCP tool registry
///
/// The CliBuilder generates a complete Clap CLI structure by combining:
/// - Static CLI-only commands (serve, doctor, prompt, flow, etc.)
/// - Dynamic commands generated from MCP tool registry
///
/// # Design Principles
///
/// - **Backward Compatibility**: All existing static commands preserved exactly
/// - **Dynamic Generation**: MCP tools automatically become CLI commands
/// - **Category Organization**: Tools grouped by category become subcommands
/// - **Schema Integration**: JSON schemas drive argument generation
/// - **Help Generation**: Tool descriptions become CLI help text
///
/// # Architecture
///
/// ```text
/// CLI Structure:
/// sah
/// ├── serve                    # Static command
/// ├── doctor                   # Static command  
/// ├── prompt                   # Static command with subcommands
/// ├── flow                     # Static command with subcommands
/// ├── issue                    # Dynamic category from MCP tools
/// │   ├── create              # Generated from issue_create MCP tool
/// │   ├── list                # Generated from issue_list MCP tool
/// │   └── ...
/// ├── memo                     # Dynamic category from MCP tools
/// │   ├── create              # Generated from memo_create MCP tool
/// │   └── ...
/// └── search                   # Dynamic root-level tool (no category)
/// ```
pub struct CliBuilder {
    /// Reference to the MCP tool registry
    tool_registry: Arc<ToolRegistry>,
}

impl CliBuilder {
    /// Create a new CLI builder
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        Self { tool_registry }
    }

    /// Build the complete CLI with static and dynamic commands
    pub fn build_cli(&self) -> Result<Command> {
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("An MCP server for managing prompts, workflows, and development tasks")
            .long_about(Self::get_long_about())
            .subcommand_required(false)
            .arg_required_else_help(true);

        // Add static CLI-only commands (unchanged)
        cli = self.add_static_commands(cli);

        // Add dynamic MCP-based commands
        cli = self.add_dynamic_commands(cli)?;

        Ok(cli)
    }

    /// Add static commands that have no MCP equivalent
    ///
    /// These commands are CLI-specific and don't have corresponding MCP tools.
    /// They are preserved exactly as they were in the original CLI definition.
    fn add_static_commands(&self, mut cli: Command) -> Command {
        cli = cli
            .subcommand(Command::new("serve").about("Run as MCP server"))
            .subcommand(Command::new("doctor").about("Diagnose configuration and setup issues"))
            .subcommand(
                Command::new("prompt")
                    .subcommand_required(true)
                    .subcommand(Command::new("list").about("List available prompts"))
                    .subcommand(
                        Command::new("test")
                            .about("Test prompt rendering")
                            .arg(clap::Arg::new("name").required(true).help("Prompt name")),
                    ),
            )
            .subcommand(
                Command::new("flow").subcommand_required(true).subcommand(
                    Command::new("run").about("Execute workflow").arg(
                        clap::Arg::new("workflow")
                            .required(true)
                            .help("Workflow name"),
                    ),
                ),
            )
            .subcommand(
                Command::new("completion")
                    .about("Generate shell completions")
                    .arg(
                        clap::Arg::new("shell")
                            .required(true)
                            .value_parser(clap::value_parser!(clap_complete::Shell)),
                    ),
            )
            .subcommand(Command::new("validate").about("Validate prompt files and workflows"))
            .subcommand(
                Command::new("plan")
                    .about("Plan a specific specification file")
                    .arg(
                        clap::Arg::new("plan_filename")
                            .required(true)
                            .help("Path to plan file"),
                    ),
            )
            .subcommand(Command::new("implement").about("Execute implement workflow"));

        cli
    }

    /// Add dynamic commands generated from MCP tools
    ///
    /// Creates CLI commands for all registered MCP tools that are not hidden from CLI.
    /// Tools are organized by category, with root-level tools appearing at the top level.
    fn add_dynamic_commands(&self, mut cli: Command) -> Result<Command> {
        let categories = self.tool_registry.get_cli_categories();

        // Add category-based commands
        for category in categories {
            let category_cmd = self.build_category_command(&category)?;
            cli = cli.subcommand(category_cmd);
        }

        // Add root-level tools (tools without category)
        let root_tools = self.tool_registry.get_root_cli_tools();
        for tool in root_tools {
            let tool_cmd = self.build_tool_command(tool)?;
            cli = cli.subcommand(tool_cmd);
        }

        Ok(cli)
    }

    /// Build command for a specific category of tools
    ///
    /// Creates a subcommand for the category with nested subcommands for each tool.
    /// For example, "issue" category contains "create", "list", etc.
    fn build_category_command(&self, category: &str) -> Result<Command> {
        // Create leaked 'static strings for clap
        let category_static: &'static str = Box::leak(category.to_string().into_boxed_str());
        let about_text = format!("{} management commands", Self::capitalize_first(category));
        let about_static: &'static str = Box::leak(about_text.into_boxed_str());

        let mut cmd = Command::new(category_static)
            .about(about_static)
            .subcommand_required(true);

        let tools = self.tool_registry.get_tools_for_category(category);

        for tool in tools {
            if tool.hidden_from_cli() {
                continue;
            }

            let tool_cmd = self.build_tool_command(tool)?;
            cmd = cmd.subcommand(tool_cmd);
        }

        Ok(cmd)
    }

    /// Build command for individual MCP tool
    ///
    /// Converts an MCP tool definition into a Clap Command by:
    /// - Using CLI-specific name and help text
    /// - Converting JSON schema to Clap arguments
    /// - Preserving all validation and description information
    fn build_tool_command(
        &self,
        tool: &dyn swissarmyhammer_tools::mcp::tool_registry::McpTool,
    ) -> Result<Command> {
        // Create leaked 'static strings for clap
        let cli_name_static: &'static str = Box::leak(tool.cli_name().to_string().into_boxed_str());
        let mut cmd = Command::new(cli_name_static);

        // Use CLI-specific about text or fall back to description
        let about = tool
            .cli_about()
            .unwrap_or_else(|| tool.description())
            .lines()
            .next() // Use first line for short about
            .unwrap_or(tool.cli_name());

        let about_static: &'static str = Box::leak(about.to_string().into_boxed_str());
        cmd = cmd.about(about_static);

        // Add long description if available
        if let Some(long_about) = tool.cli_about().or_else(|| Some(tool.description())) {
            let long_about_static: &'static str =
                Box::leak(long_about.to_string().into_boxed_str());
            cmd = cmd.long_about(long_about_static);
        }

        // Convert tool schema to clap arguments
        let schema = tool.schema();
        let args = SchemaConverter::schema_to_clap_args(&schema)?;

        for arg in args {
            cmd = cmd.arg(arg);
        }

        Ok(cmd)
    }

    /// Get application long about text
    fn get_long_about() -> &'static str {
        "swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts, workflows, issues, memos, and development tools. It supports file watching, 
template substitution, and seamless integration with Claude Code.

Example usage:
  swissarmyhammer serve     # Run as MCP server
  swissarmyhammer doctor    # Check configuration and setup
  swissarmyhammer issue create \"Bug fix\"    # Create new issue
  swissarmyhammer memo list                   # List all memos"
    }

    /// Capitalize first letter of string
    fn capitalize_first(s: &str) -> String {
        let mut chars = s.chars();
        match chars.next() {
            None => String::new(),
            Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
        }
    }
}

/// Information about a dynamic command extracted from CLI matches
///
/// Used to identify which MCP tool should handle a parsed command and
/// provide context for tool execution.
#[derive(Debug, PartialEq, Eq)]
pub struct DynamicCommandInfo {
    /// Category of the command (e.g., "issue", "memo"), None for root tools
    pub category: Option<String>,

    /// CLI command name (e.g., "create", "list")
    pub tool_name: String,

    /// MCP tool name for registry lookup (e.g., "issue_create", "memo_list")
    pub mcp_tool_name: String,
}

impl CliBuilder {
    /// Extract dynamic command information from matches
    ///
    /// Analyzes parsed command line arguments to determine which MCP tool
    /// should handle the command. Returns information needed for tool execution.
    ///
    /// # Arguments
    ///
    /// * `matches` - Parsed command line arguments from Clap
    ///
    /// # Returns
    ///
    /// * `Some(DynamicCommandInfo)` - Information about the matched dynamic command
    /// * `None` - Command is static or not found
    ///
    /// # Examples
    ///
    /// ```ignore
    /// // For command: sah issue create --title "Bug fix"
    /// let info = builder.extract_command_info(&matches)?;
    /// assert_eq!(info.category, Some("issue"));
    /// assert_eq!(info.tool_name, "create");
    /// assert_eq!(info.mcp_tool_name, "issue_create");
    /// ```
    pub fn extract_command_info(&self, matches: &clap::ArgMatches) -> Option<DynamicCommandInfo> {
        // Check each category for matches
        for category in self.tool_registry.get_cli_categories() {
            if let Some((category_name, sub_matches)) = matches.subcommand() {
                if category_name == category {
                    if let Some((tool_name, _)) = sub_matches.subcommand() {
                        // Find the MCP tool name
                        let tools = self.tool_registry.get_tools_for_category(&category);
                        for tool in tools {
                            if tool.cli_name() == tool_name {
                                return Some(DynamicCommandInfo {
                                    category: Some(category),
                                    tool_name: tool_name.to_string(),
                                    mcp_tool_name: tool.name().to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }

        // Check root-level tools
        if let Some((command_name, _)) = matches.subcommand() {
            let root_tools = self.tool_registry.get_root_cli_tools();
            for tool in root_tools {
                if tool.cli_name() == command_name {
                    return Some(DynamicCommandInfo {
                        category: None,
                        tool_name: command_name.to_string(),
                        mcp_tool_name: tool.name().to_string(),
                    });
                }
            }
        }

        None
    }

    /// Get the appropriate ArgMatches for a dynamic command
    ///
    /// Navigates the command hierarchy to get the ArgMatches that contain
    /// the tool-specific arguments for the identified dynamic command.
    ///
    /// # Arguments
    ///
    /// * `matches` - Top-level ArgMatches from command parsing
    /// * `info` - Dynamic command information from extract_command_info
    ///
    /// # Returns
    ///
    /// * `Some(&ArgMatches)` - ArgMatches containing tool arguments
    /// * `None` - Command structure doesn't match expected pattern
    pub fn get_tool_matches<'a>(
        &self,
        matches: &'a clap::ArgMatches,
        info: &DynamicCommandInfo,
    ) -> Option<&'a clap::ArgMatches> {
        if let Some(category) = &info.category {
            // Categorized tool: navigate category -> tool
            if let Some((category_name, category_matches)) = matches.subcommand() {
                if category_name == category {
                    if let Some((tool_name, tool_matches)) = category_matches.subcommand() {
                        if tool_name == info.tool_name {
                            return Some(tool_matches);
                        }
                    }
                }
            }
        } else {
            // Root-level tool: navigate directly to tool
            if let Some((tool_name, tool_matches)) = matches.subcommand() {
                if tool_name == info.tool_name {
                    return Some(tool_matches);
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rmcp::{model::CallToolResult, Error as McpError};
    use serde_json::json;
    use swissarmyhammer_tools::mcp::tool_registry::{BaseToolImpl, McpTool, ToolRegistry};

    /// Mock tool for testing
    #[derive(Default)]
    struct MockTool {
        name: &'static str,
        description: &'static str,
        category: Option<&'static str>,
        cli_name: &'static str,
        hidden: bool,
    }

    #[async_trait::async_trait]
    impl McpTool for MockTool {
        fn name(&self) -> &'static str {
            self.name
        }

        fn description(&self) -> &'static str {
            self.description
        }

        fn schema(&self) -> serde_json::Value {
            json!({
                "type": "object",
                "properties": {
                    "test_param": {
                        "type": "string",
                        "description": "A test parameter"
                    }
                },
                "required": []
            })
        }

        fn cli_category(&self) -> Option<&'static str> {
            self.category
        }

        fn cli_name(&self) -> &'static str {
            self.cli_name
        }

        fn hidden_from_cli(&self) -> bool {
            self.hidden
        }

        async fn execute(
            &self,
            _arguments: serde_json::Map<String, serde_json::Value>,
            _context: &swissarmyhammer_tools::mcp::tool_registry::ToolContext,
        ) -> std::result::Result<CallToolResult, McpError> {
            Ok(BaseToolImpl::create_success_response("Mock executed"))
        }
    }

    fn create_test_registry() -> ToolRegistry {
        let mut registry = ToolRegistry::new();

        // Add categorized tools
        registry.register(MockTool {
            name: "issue_create",
            description: "Create a new issue",
            category: Some("issue"),
            cli_name: "create",
            hidden: false,
        });

        registry.register(MockTool {
            name: "issue_list",
            description: "List all issues",
            category: Some("issue"),
            cli_name: "list",
            hidden: false,
        });

        registry.register(MockTool {
            name: "memo_create",
            description: "Create a new memo",
            category: Some("memo"),
            cli_name: "create",
            hidden: false,
        });

        // Add root tool
        registry.register(MockTool {
            name: "search_files",
            description: "Search through files",
            category: None,
            cli_name: "search",
            hidden: false,
        });

        // Add hidden tool
        registry.register(MockTool {
            name: "internal_tool",
            description: "Internal tool",
            category: Some("internal"),
            cli_name: "internal",
            hidden: true,
        });

        registry
    }

    #[test]
    fn test_static_commands_preserved() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);

        let cli = builder.build_cli().unwrap();

        // Verify static commands are present
        assert!(cli.find_subcommand("serve").is_some());
        assert!(cli.find_subcommand("doctor").is_some());
        assert!(cli.find_subcommand("prompt").is_some());
        assert!(cli.find_subcommand("flow").is_some());
        assert!(cli.find_subcommand("completion").is_some());
        assert!(cli.find_subcommand("validate").is_some());
        assert!(cli.find_subcommand("plan").is_some());
        assert!(cli.find_subcommand("implement").is_some());
    }

    #[test]
    fn test_category_commands_generated() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);

        let cli = builder.build_cli().unwrap();

        // Verify category commands are generated
        assert!(cli.find_subcommand("issue").is_some());
        assert!(cli.find_subcommand("memo").is_some());

        // Verify hidden category is not generated
        assert!(cli.find_subcommand("internal").is_none());

        // Verify nested commands within categories
        let issue_cmd = cli.find_subcommand("issue").unwrap();
        assert!(issue_cmd.find_subcommand("create").is_some());
        assert!(issue_cmd.find_subcommand("list").is_some());
    }

    #[test]
    fn test_root_level_tools() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);

        let cli = builder.build_cli().unwrap();

        // Verify root-level tool is generated
        assert!(cli.find_subcommand("search").is_some());
    }

    #[test]
    fn test_command_info_extraction_categorized() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        // Debug: print the command structure
        println!("CLI debug info:");
        let issue_cmd = cli.find_subcommand("issue").unwrap();
        let create_cmd = issue_cmd.find_subcommand("create").unwrap();
        for arg in create_cmd.get_arguments() {
            println!("  Argument: {} (long: {:?})", arg.get_id(), arg.get_long());
        }

        // Parse a categorized command: issue create
        let matches = cli
            .try_get_matches_from(vec!["sah", "issue", "create"])
            .unwrap();
        let info = builder.extract_command_info(&matches).unwrap();

        assert_eq!(info.category, Some("issue".to_string()));
        assert_eq!(info.tool_name, "create");
        assert_eq!(info.mcp_tool_name, "issue_create");
    }

    #[test]
    fn test_command_info_extraction_root() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        // Parse a root command: search
        let matches = cli.try_get_matches_from(vec!["sah", "search"]).unwrap();
        let info = builder.extract_command_info(&matches).unwrap();

        assert_eq!(info.category, None);
        assert_eq!(info.tool_name, "search");
        assert_eq!(info.mcp_tool_name, "search_files");
    }

    #[test]
    fn test_command_info_extraction_static_command() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        // Parse a static command: serve
        let matches = cli.try_get_matches_from(vec!["sah", "serve"]).unwrap();
        let info = builder.extract_command_info(&matches);

        // Static commands should return None
        assert!(info.is_none());
    }

    #[test]
    fn test_get_tool_matches_categorized() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        let matches = cli
            .try_get_matches_from(vec!["sah", "issue", "create"])
            .unwrap();
        let info = builder.extract_command_info(&matches).unwrap();
        let tool_matches = builder.get_tool_matches(&matches, &info).unwrap();

        // Verify we get the correct nested matches structure
        // (we're not testing actual argument values, just the command structure)
        assert!(tool_matches.subcommand().is_none()); // leaf command
    }

    #[test]
    fn test_get_tool_matches_root() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        let matches = cli.try_get_matches_from(vec!["sah", "search"]).unwrap();
        let info = builder.extract_command_info(&matches).unwrap();
        let tool_matches = builder.get_tool_matches(&matches, &info).unwrap();

        // Verify we get the correct matches structure
        // (we're not testing actual argument values, just the command structure)
        assert!(tool_matches.subcommand().is_none()); // leaf command
    }

    #[test]
    fn test_capitalize_first() {
        assert_eq!(CliBuilder::capitalize_first("hello"), "Hello");
        assert_eq!(CliBuilder::capitalize_first("WORLD"), "WORLD");
        assert_eq!(CliBuilder::capitalize_first(""), "");
        assert_eq!(CliBuilder::capitalize_first("a"), "A");
    }

    #[test]
    fn test_help_text_generation() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        let issue_cmd = cli.find_subcommand("issue").unwrap();
        let create_cmd = issue_cmd.find_subcommand("create").unwrap();

        // Verify help text is generated from tool description
        assert!(create_cmd.get_about().is_some());
        assert!(create_cmd
            .get_about()
            .unwrap()
            .to_string()
            .contains("Create a new issue"));
    }

    #[test]
    fn test_argument_generation_from_schema() {
        let registry = Arc::new(create_test_registry());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        let issue_cmd = cli.find_subcommand("issue").unwrap();
        let create_cmd = issue_cmd.find_subcommand("create").unwrap();

        // Verify arguments are generated from schema
        assert!(create_cmd
            .get_arguments()
            .any(|arg| arg.get_id() == "test_param"));
    }

    #[test]
    fn test_dynamic_command_info_equality() {
        let info1 = DynamicCommandInfo {
            category: Some("issue".to_string()),
            tool_name: "create".to_string(),
            mcp_tool_name: "issue_create".to_string(),
        };

        let info2 = DynamicCommandInfo {
            category: Some("issue".to_string()),
            tool_name: "create".to_string(),
            mcp_tool_name: "issue_create".to_string(),
        };

        let info3 = DynamicCommandInfo {
            category: None,
            tool_name: "search".to_string(),
            mcp_tool_name: "search_files".to_string(),
        };

        assert_eq!(info1, info2);
        assert_ne!(info1, info3);
    }

    #[test]
    fn test_empty_registry() {
        let registry = Arc::new(ToolRegistry::new());
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        // Should still have static commands
        assert!(cli.find_subcommand("serve").is_some());
        assert!(cli.find_subcommand("doctor").is_some());

        // Should have no dynamic commands
        assert!(cli.find_subcommand("issue").is_none());
        assert!(cli.find_subcommand("memo").is_none());
        assert!(cli.find_subcommand("search").is_none());
    }

    #[test]
    fn test_only_hidden_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(MockTool {
            name: "hidden1",
            description: "Hidden tool 1",
            category: Some("category"),
            cli_name: "hidden1",
            hidden: true,
        });

        let registry = Arc::new(registry);
        let builder = CliBuilder::new(registry);
        let cli = builder.build_cli().unwrap();

        // Should have static commands but no dynamic categories
        assert!(cli.find_subcommand("serve").is_some());
        assert!(cli.find_subcommand("category").is_none());
    }
}
