//! Dynamic CLI builder for MCP tools
//!
//! This module implements dynamic CLI command generation from MCP tool definitions,
//! eliminating the need for redundant CLI command enums and ensuring consistency
//! between MCP and CLI interfaces.

use swissarmyhammer_cli::schema_validation::{SchemaValidator, ValidationError};
use clap::{Arg, ArgAction, Command};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};

/// Pre-computed command data with owned strings for clap's 'static requirements
#[derive(Debug, Clone)]
struct CommandData {
    name: String,
    about: Option<String>,
    long_about: Option<String>,
    args: Vec<ArgData>,
}

/// Pre-computed argument data with owned strings
#[derive(Debug, Clone)]
struct ArgData {
    name: String,
    help: Option<String>,
    is_required: bool,
    arg_type: ArgType,
    default_value: Option<String>,
    possible_values: Option<Vec<String>>,
}

/// Argument type for proper clap configuration
#[derive(Debug, Clone)]
enum ArgType {
    String,
    Integer,
    Float,
    Boolean,
    Array,
}

/// Statistics about CLI tool validation
#[derive(Debug, Clone)]
pub struct CliValidationStats {
    pub total_tools: usize,
    pub valid_tools: usize,
    pub invalid_tools: usize,
    pub validation_errors: usize,
}

impl CliValidationStats {
    pub fn new() -> Self {
        Self {
            total_tools: 0,
            valid_tools: 0,
            invalid_tools: 0,
            validation_errors: 0,
        }
    }

    pub fn is_all_valid(&self) -> bool {
        self.invalid_tools == 0 && self.validation_errors == 0
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_tools == 0 {
            100.0
        } else {
            (self.valid_tools as f64 / self.total_tools as f64) * 100.0
        }
    }

    pub fn summary(&self) -> String {
        if self.is_all_valid() {
            format!("✅ All {} CLI tools are valid", self.total_tools)
        } else {
            format!(
                "⚠️  {} of {} CLI tools are valid ({:.1}% success rate, {} validation errors)",
                self.valid_tools,
                self.total_tools,
                self.success_rate(),
                self.validation_errors
            )
        }
    }
}

/// Dynamic CLI builder that generates commands from MCP tool registry
pub struct CliBuilder {
    #[allow(dead_code)] // Used during initialization, kept for future functionality
    tool_registry: Arc<ToolRegistry>,
    // Pre-computed command data with owned strings
    category_commands: HashMap<String, CommandData>,
    tool_commands: HashMap<String, HashMap<String, CommandData>>,
}

impl CliBuilder {
    /// Create a new CLI builder with the given tool registry
    pub fn new(tool_registry: Arc<ToolRegistry>) -> Self {
        let mut category_commands = HashMap::new();
        let mut tool_commands = HashMap::new();

        // Pre-compute all command data
        let categories = tool_registry.get_cli_categories();
        for category in categories {
            let category_name = category.to_string();

            // Create category command data
            let category_cmd_data = CommandData {
                name: category_name.clone(),
                about: Some(format!("{} management commands", category.to_uppercase())),
                long_about: None,
                args: Vec::new(),
            };
            category_commands.insert(category_name.clone(), category_cmd_data);

            // Create tool commands for this category
            let mut tools_in_category = HashMap::new();
            let tools = tool_registry.get_tools_for_category(&category);

            for tool in tools {
                if !tool.hidden_from_cli() {
                    // Only add tools that pass validation
                    if let Some(tool_cmd_data) = Self::precompute_tool_command(tool) {
                        tools_in_category.insert(tool.cli_name().to_string(), tool_cmd_data);
                    }
                }
            }

            tool_commands.insert(category_name, tools_in_category);
        }

        Self {
            tool_registry,
            category_commands,
            tool_commands,
        }
    }

    /// Pre-compute command data for a tool with validation
    fn precompute_tool_command(tool: &dyn McpTool) -> Option<CommandData> {
        let schema = tool.schema();

        // Validate schema before processing
        if let Err(validation_error) = SchemaValidator::validate_schema(&schema) {
            tracing::warn!(
                "Skipping tool '{}' from CLI due to schema validation error: {}",
                tool.name(),
                validation_error
            );
            return None;
        }

        Some(CommandData {
            name: tool.cli_name().to_string(),
            about: tool.cli_about().map(|s| s.to_string()),
            long_about: Some(tool.description().to_string()),
            args: Self::precompute_args(&schema),
        })
    }

    /// Pre-compute argument data from JSON schema
    fn precompute_args(schema: &Value) -> Vec<ArgData> {
        let mut args = Vec::new();

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
                let arg_data = Self::precompute_arg_data(
                    prop_name,
                    prop_schema,
                    required_fields.contains(prop_name),
                );
                args.push(arg_data);
            }
        }

        args
    }

    /// Pre-compute data for a single argument
    fn precompute_arg_data(name: &str, schema: &Value, is_required: bool) -> ArgData {
        // Determine argument type
        let arg_type = match schema.get("type").and_then(|t| t.as_str()) {
            Some("boolean") => ArgType::Boolean,
            Some("integer") => ArgType::Integer,
            Some("number") => ArgType::Float,
            Some("array") => ArgType::Array,
            _ => ArgType::String,
        };

        // Extract help text
        let help = schema
            .get("description")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        // Extract default value
        let default_value = schema
            .get("default")
            .and_then(|d| d.as_str())
            .map(|s| s.to_string());

        // Extract possible values for enums
        let possible_values = schema.get("enum").and_then(|e| e.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str())
                .map(|s| s.to_string())
                .collect()
        });

        ArgData {
            name: name.to_string(),
            help,
            is_required,
            arg_type,
            default_value,
            possible_values,
        }
    }

    /// Build the complete CLI with both static and dynamic commands
    pub fn build_cli(&self) -> Command {
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("An MCP server for managing prompts as markdown files")
            .long_about(
                "
swissarmyhammer is an MCP (Model Context Protocol) server that manages
prompts as markdown files. It supports file watching, template substitution,
and seamless integration with Claude Code.

This CLI includes both static commands and dynamic commands generated from MCP tools.
",
            )
            // Add verbose/debug/quiet flags from parent CLI
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Enable verbose logging")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("debug")
                    .short('d')
                    .long("debug")
                    .help("Enable debug logging")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .help("Suppress all output except errors")
                    .action(ArgAction::SetTrue),
            );

        // Add dynamic MCP tool commands using pre-computed data
        for (category_name, category_data) in &self.category_commands {
            cli =
                cli.subcommand(self.build_category_command_from_data(category_name, category_data));
        }

        cli
    }

    /// Build CLI with comprehensive validation and error reporting
    ///
    /// This method performs full validation of all tools before building the CLI
    /// and returns detailed error information if validation fails.
    ///
    /// # Returns
    ///
    /// * `Ok(Command)` - Successfully built CLI with all tools validated
    /// * `Err(Vec<ValidationError>)` - CLI build failed due to validation errors
    pub fn build_cli_with_validation(&self) -> Result<Command, Vec<ValidationError>> {
        // Validate all CLI tools first
        let validation_errors = self.validate_all_tools();
        
        if !validation_errors.is_empty() {
            return Err(validation_errors);
        }

        Ok(self.build_cli())
    }

    /// Build CLI with warnings for validation issues (graceful degradation)
    ///
    /// This method builds the CLI but logs warnings for any validation issues,
    /// skipping problematic tools rather than failing completely.
    ///
    /// # Returns
    ///
    /// Always returns a `Command`, but may skip invalid tools with warnings
    pub fn build_cli_with_warnings(&self) -> Command {
        let warnings = self.get_validation_warnings();
        
        if !warnings.is_empty() {
            tracing::warn!("Found {} tool validation warnings during CLI build:", warnings.len());
            for (i, warning) in warnings.iter().enumerate() {
                tracing::warn!("  {}. {}", i + 1, warning);
            }
        }

        // The build_cli method already includes graceful degradation
        // by skipping tools that fail validation
        self.build_cli()
    }

    /// Validate all tools that should appear in CLI
    ///
    /// Performs comprehensive validation of all CLI-visible tools and collects
    /// all validation errors found.
    ///
    /// # Returns
    ///
    /// Vec of validation errors (empty if all tools are valid)
    pub fn validate_all_tools(&self) -> Vec<ValidationError> {
        let mut errors = Vec::new();

        let categories = self.tool_registry.get_cli_categories();
        for category in categories {
            let tools = self.tool_registry.get_tools_for_category(&category);
            for tool in tools {
                if let Err(tool_errors) = self.validate_single_tool(tool) {
                    errors.extend(tool_errors);
                }
            }
        }

        errors
    }

    /// Validate a single tool for CLI compatibility
    fn validate_single_tool(&self, tool: &dyn McpTool) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate schema
        if let Err(schema_error) = SchemaValidator::validate_schema(&tool.schema()) {
            errors.push(schema_error);
        }

        // Validate CLI integration requirements
        if !tool.hidden_from_cli() {
            if tool.cli_category().is_none() {
                errors.push(ValidationError::MissingSchemaField {
                    field: format!("CLI category for tool {}", tool.name()),
                });
            }

            // Validate CLI name
            let cli_name = tool.cli_name();
            if cli_name.is_empty() {
                errors.push(ValidationError::InvalidParameterName {
                    parameter: tool.name().to_string(),
                    reason: "CLI name cannot be empty".to_string(),
                });
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Get validation warnings for all tools (non-failing validation)
    ///
    /// This method performs validation but returns warnings instead of errors,
    /// suitable for graceful degradation scenarios.
    ///
    /// # Returns
    ///
    /// Vec of user-friendly warning messages
    pub fn get_validation_warnings(&self) -> Vec<String> {
        let errors = self.validate_all_tools();
        
        errors.into_iter().map(|error| {
            format!("Tool validation warning: {}", error)
        }).collect()
    }

    /// Get statistics about CLI tool validation
    ///
    /// Provides summary information about tool validation status for
    /// debugging and monitoring purposes.
    ///
    /// # Returns
    ///
    /// `CliValidationStats` with counts and status information
    pub fn get_validation_stats(&self) -> CliValidationStats {
        let mut stats = CliValidationStats::new();
        
        let categories = self.tool_registry.get_cli_categories();
        for category in categories {
            let tools = self.tool_registry.get_tools_for_category(&category);
            for tool in tools {
                stats.total_tools += 1;
                
                match self.validate_single_tool(tool) {
                    Ok(()) => {
                        stats.valid_tools += 1;
                    }
                    Err(errors) => {
                        stats.invalid_tools += 1;
                        stats.validation_errors += errors.len();
                    }
                }
            }
        }

        stats
    }

    /// Build a command for a specific tool category from pre-computed data
    fn build_category_command_from_data(
        &self,
        category_name: &str,
        category_data: &CommandData,
    ) -> Command {
        let mut cmd =
            Command::new(Box::leak(category_data.name.clone().into_boxed_str()) as &'static str);

        if let Some(about) = &category_data.about {
            cmd = cmd.about(Box::leak(about.clone().into_boxed_str()) as &'static str);
        }

        if let Some(long_about) = &category_data.long_about {
            cmd = cmd.long_about(Box::leak(long_about.clone().into_boxed_str()) as &'static str);
        }

        // Add tool subcommands for this category
        if let Some(tools_in_category) = self.tool_commands.get(category_name) {
            for tool_data in tools_in_category.values() {
                cmd = cmd.subcommand(self.build_tool_command_from_data(tool_data));
            }
        }

        cmd
    }

    /// Build a command for a specific MCP tool from pre-computed data
    fn build_tool_command_from_data(&self, tool_data: &CommandData) -> Command {
        let mut cmd =
            Command::new(Box::leak(tool_data.name.clone().into_boxed_str()) as &'static str);

        if let Some(about) = &tool_data.about {
            cmd = cmd.about(Box::leak(about.clone().into_boxed_str()) as &'static str);
        }

        if let Some(long_about) = &tool_data.long_about {
            cmd = cmd.long_about(Box::leak(long_about.clone().into_boxed_str()) as &'static str);
        }

        // Add arguments from pre-computed data
        for arg_data in &tool_data.args {
            cmd = cmd.arg(self.build_arg_from_data(arg_data));
        }

        cmd
    }

    /// Build a clap argument from pre-computed data
    fn build_arg_from_data(&self, arg_data: &ArgData) -> Arg {
        let name_static = Box::leak(arg_data.name.clone().into_boxed_str()) as &'static str;
        let mut arg = Arg::new(name_static).long(name_static);

        // Set help text
        if let Some(help) = &arg_data.help {
            arg = arg.help(Box::leak(help.clone().into_boxed_str()) as &'static str);
        }

        // Set as required if specified
        if arg_data.is_required {
            arg = arg.required(true);
        }

        // Configure based on type
        match arg_data.arg_type {
            ArgType::Boolean => {
                arg = arg.action(ArgAction::SetTrue);
            }
            ArgType::Integer => {
                arg = arg.value_parser(clap::value_parser!(i64));
                if !arg_data.is_required {
                    arg = arg.value_name("NUMBER");
                }
            }
            ArgType::Float => {
                arg = arg.value_parser(clap::value_parser!(f64));
                if !arg_data.is_required {
                    arg = arg.value_name("NUMBER");
                }
            }
            ArgType::Array => {
                arg = arg.action(ArgAction::Append);
                if !arg_data.is_required {
                    arg = arg.value_name("VALUE");
                }
            }
            ArgType::String => {
                if !arg_data.is_required {
                    arg = arg.value_name("TEXT");
                }
            }
        }

        // Handle enum values
        if let Some(possible_values) = &arg_data.possible_values {
            let str_values: Vec<&'static str> = possible_values
                .iter()
                .map(|s| Box::leak(s.clone().into_boxed_str()) as &'static str)
                .collect();
            arg = arg.value_parser(clap::builder::PossibleValuesParser::new(str_values));
        }

        // Handle default values
        if let Some(default_value) = &arg_data.default_value {
            arg = arg
                .default_value(Box::leak(default_value.clone().into_boxed_str()) as &'static str);
        }

        arg
    }
}
