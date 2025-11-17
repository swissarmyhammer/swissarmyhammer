//! Dynamic CLI builder for MCP tools
//!
//! This module implements dynamic CLI command generation from MCP tool definitions,
//! eliminating the need for redundant CLI command enums and ensuring consistency
//! between MCP and CLI interfaces.

use crate::schema_validation::{SchemaValidator, ValidationError};
use clap::{Arg, ArgAction, Command};
use once_cell::sync::Lazy;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};
use swissarmyhammer_workflow::WorkflowStorage;
use tokio::sync::RwLock;

/// Global string cache to prevent memory leaks from Box::leak
/// Strings are interned once and reused, satisfying clap's 'static lifetime requirement
static STRING_CACHE: Lazy<Mutex<HashSet<&'static str>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Intern a string into the global cache, returning a 'static reference
/// This ensures each unique string is only leaked once, preventing unbounded memory growth
fn intern_string(s: String) -> &'static str {
    let mut cache = STRING_CACHE.lock().unwrap();

    // Check if we already have this string cached
    if let Some(&cached) = cache.get(s.as_str()) {
        return cached;
    }

    // Leak the string and cache the reference
    let leaked: &'static str = Box::leak(s.into_boxed_str());
    cache.insert(leaked);
    leaked
}

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
#[derive(Debug, Clone, Default)]
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
    tool_registry: Arc<RwLock<ToolRegistry>>,
    // Pre-computed command data with owned strings
    category_commands: HashMap<String, CommandData>,
    tool_commands: HashMap<String, HashMap<String, CommandData>>,
}

impl CliBuilder {
    /// Create a new CLI builder with the given tool registry
    pub fn new(tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        let (category_commands, tool_commands) = {
            let registry = tool_registry
                .try_read()
                .expect("ToolRegistry should not be locked");
            let category_commands = Self::precompute_category_commands(&registry);
            let tool_commands = Self::precompute_tool_commands(&registry);
            (category_commands, tool_commands)
        }; // Drop registry guard

        Self {
            tool_registry,
            category_commands,
            tool_commands,
        }
    }

    /// Pre-compute category command data
    fn precompute_category_commands(registry: &ToolRegistry) -> HashMap<String, CommandData> {
        let mut category_commands = HashMap::new();
        let categories = registry.get_cli_categories();

        for category in categories {
            let category_name = category.to_string();
            let category_cmd_data = CommandData {
                name: category_name.clone(),
                about: Some(format!(
                    "{} management commands (MCP Tool)",
                    category.to_uppercase()
                )),
                long_about: None,
                args: Vec::new(),
            };
            category_commands.insert(category_name, category_cmd_data);
        }

        category_commands
    }

    /// Pre-compute tool command data
    fn precompute_tool_commands(
        registry: &ToolRegistry,
    ) -> HashMap<String, HashMap<String, CommandData>> {
        let mut tool_commands = HashMap::new();
        let categories = registry.get_cli_categories();

        for category in categories {
            let category_name = category.to_string();
            let mut tools_in_category = HashMap::new();
            let tools = registry.get_tools_for_category(&category);

            for tool in tools {
                if !tool.hidden_from_cli() {
                    if let Some(tool_cmd_data) = Self::precompute_tool_command(tool) {
                        tools_in_category.insert(tool.cli_name().to_string(), tool_cmd_data);
                    }
                }
            }

            tool_commands.insert(category_name, tools_in_category);
        }

        tool_commands
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

    /// Build the complete CLI with dynamic commands generated from MCP tools
    ///
    /// # Parameters
    ///
    /// * `workflow_storage` - Optional workflow storage for generating shortcut commands.
    ///   If None, shortcuts will not be generated.
    pub fn build_cli(&self, workflow_storage: Option<&WorkflowStorage>) -> Command {
        let mut cli = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("The only coding assistant you'll ever need")
            .long_about(
                "
SwissArmyHammer - The only coding assistant you'll ever need

Commands are organized into three types:
- Static commands (serve, doctor, validate, agent, prompt, rule, flow)
- Workflow shortcuts (implement, plan, etc.) - use 'sah flow list' to see all
- Tool commands (file, issue, memo, search, shell, web-search)

Examples:
  sah serve                    Run as MCP server
  sah doctor                   Diagnose configuration  
  sah flow list                List all workflows
  sah implement                Execute implement workflow (shortcut)
  sah plan spec.md             Execute plan workflow (shortcut)
  sah file read path.txt       Read a file via MCP tool
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
                Arg::new("cwd")
                    .long("cwd")
                    .help("Set working directory before executing command")
                    .value_name("PATH")
                    .global(true)
                    .value_parser(clap::value_parser!(std::path::PathBuf)),
            )
            .arg(
                Arg::new("quiet")
                    .short('q')
                    .long("quiet")
                    .help("Suppress all output except errors")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("validate-tools")
                    .long("validate-tools")
                    .help("Validate all tool schemas and exit")
                    .action(ArgAction::SetTrue),
            )
            .arg(
                Arg::new("format")
                    .long("format")
                    .help("Global output format")
                    .value_parser(["table", "json", "yaml"]),
            );

        // === STATIC COMMANDS (Default heading) ===
        // Add core serve command (non-MCP command)
        cli = cli.subcommand(
            Command::new("serve")
                .about("Run as MCP server (default when invoked via stdio)")
                .long_about(
                    "
Runs swissarmyhammer as an MCP server. This is the default mode when
invoked via stdio (e.g., by Claude Code). The server will:

- Load all prompts from builtin, user, and local directories
- Watch for file changes and reload prompts automatically
- Expose prompts via the MCP protocol
- Support template substitution with {{variables}}

Example:
  swissarmyhammer serve
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  # Or configure in Claude Code's MCP settings
                ",
                )
                .subcommand(
                    Command::new("http")
                        .about("Start HTTP MCP server")
                        .long_about(
                            "
Starts an HTTP MCP server for web clients, debugging, and LlamaAgent integration.
The server exposes MCP tools through HTTP endpoints and provides:

- RESTful MCP protocol implementation
- Health check endpoint at /health
- Support for random port allocation (use port 0)
- Graceful shutdown with Ctrl+C

Example:
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  swissarmyhammer serve http --port 0  # Random port
                            ",
                        )
                        .arg(
                            Arg::new("port")
                                .long("port")
                                .short('p')
                                .help("Port to bind to (use 0 for random port)")
                                .default_value("8000")
                                .value_parser(clap::value_parser!(u16)),
                        )
                        .arg(
                            Arg::new("host")
                                .long("host")
                                .short('H')
                                .help("Host to bind to")
                                .default_value("127.0.0.1"),
                        ),
                ),
        );

        // Add other static commands (doctor, prompt, flow, validate, plan, implement, agent, rule)
        cli = Self::add_static_commands(cli);

        // Add workflow shortcuts if storage is provided
        if let Some(storage) = workflow_storage {
            let mut shortcuts = Self::build_workflow_shortcuts(storage);
            // Sort alphabetically for easier scanning
            shortcuts.sort_by(|a, b| a.get_name().cmp(b.get_name()));

            for shortcut in shortcuts {
                cli = cli.subcommand(shortcut);
            }
        }

        // Add dynamic MCP tool commands using pre-computed data
        // Get sorted category names for consistent ordering
        let mut category_names: Vec<String> = self.category_commands.keys().cloned().collect();
        category_names.sort();

        for category_name in category_names.iter() {
            if let Some(category_data) = self.category_commands.get(category_name) {
                let cmd = self.build_category_command_from_data(category_name, category_data);
                cli = cli.subcommand(cmd);
            }
        }

        cli
    }

    /// Build CLI with warnings for validation issues (graceful degradation)
    ///
    /// This method builds the CLI but logs warnings for any validation issues,
    /// skipping problematic tools rather than failing completely.
    ///
    /// # Parameters
    ///
    /// * `workflow_storage` - Optional workflow storage for generating shortcut commands
    ///
    /// # Returns
    ///
    /// Always returns a `Command`, but may skip invalid tools with warnings
    pub fn build_cli_with_warnings(&self, workflow_storage: Option<&WorkflowStorage>) -> Command {
        let warnings = self.get_validation_warnings();

        if !warnings.is_empty() {
            tracing::warn!(
                "Found {} tool validation warnings during CLI build:",
                warnings.len()
            );
            for (i, warning) in warnings.iter().enumerate() {
                tracing::warn!("  {}. {}", i + 1, warning);
            }
        }

        // The build_cli method already includes graceful degradation
        // by skipping tools that fail validation
        self.build_cli(workflow_storage)
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
        self.collect_validation_results(|result| result.err())
            .into_iter()
            .flatten()
            .flatten()
            .collect()
    }

    /// Collect validation results with a mapper function
    ///
    /// This provides a single source of truth for tool validation iteration
    /// and processing. The mapper function transforms each validation result
    /// into the desired output type.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Output type from the mapper function
    /// * `F` - Mapper function type
    ///
    /// # Parameters
    ///
    /// * `mapper` - Function to transform validation results
    ///
    /// # Returns
    ///
    /// Vector of mapped results
    fn collect_validation_results<T, F>(&self, mapper: F) -> Vec<T>
    where
        F: Fn(Result<(), Vec<ValidationError>>) -> T,
    {
        let registry = self
            .tool_registry
            .try_read()
            .expect("ToolRegistry should not be locked");
        let categories = registry.get_cli_categories();

        let mut results = Vec::new();
        for category in categories {
            let tools = registry.get_tools_for_category(&category);
            for tool in tools {
                let validation_result = self.validate_single_tool(tool);
                results.push(mapper(validation_result));
            }
        }

        results
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

        errors
            .into_iter()
            .map(|error| format!("Tool validation warning: {}", error))
            .collect()
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
        self.collect_validation_results(|result| result)
            .into_iter()
            .fold(CliValidationStats::new(), |mut stats, result| {
                stats.total_tools += 1;
                match result {
                    Ok(()) => {
                        stats.valid_tools += 1;
                    }
                    Err(errors) => {
                        stats.invalid_tools += 1;
                        stats.validation_errors += errors.len();
                    }
                }
                stats
            })
    }

    /// Build base command from command data (shared between category and tool commands)
    fn build_command_base(data: &CommandData) -> Command {
        let mut cmd = Command::new(intern_string(data.name.clone()));

        if let Some(about) = &data.about {
            cmd = cmd.about(intern_string(about.clone()));
        }

        if let Some(long_about) = &data.long_about {
            cmd = cmd.long_about(intern_string(long_about.clone()));
        }

        cmd
    }

    /// Build a command for a specific tool category from pre-computed data
    fn build_category_command_from_data(
        &self,
        category_name: &str,
        category_data: &CommandData,
    ) -> Command {
        let mut cmd = Self::build_command_base(category_data);
        cmd = cmd.subcommand_help_heading("Tools");

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
        let mut cmd = Self::build_command_base(tool_data);

        // Add arguments from pre-computed data
        for arg_data in &tool_data.args {
            cmd = cmd.arg(self.build_arg_from_data(arg_data));
        }

        cmd
    }

    /// Build a clap argument from pre-computed data
    fn build_arg_from_data(&self, arg_data: &ArgData) -> Arg {
        let name_static = intern_string(arg_data.name.clone());
        let mut arg = Arg::new(name_static).long(name_static);

        // Set help text
        if let Some(help) = &arg_data.help {
            arg = arg.help(intern_string(help.clone()));
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
                .map(|s| intern_string(s.clone()))
                .collect();
            arg = arg.value_parser(clap::builder::PossibleValuesParser::new(str_values));
        }

        // Handle default values
        if let Some(default_value) = &arg_data.default_value {
            arg = arg.default_value(intern_string(default_value.clone()));
        }

        arg
    }

    /// Add static commands to the CLI (doctor, prompt, flow, validate, plan, implement)
    fn add_static_commands(mut cli: Command) -> Command {
        // Add doctor command
        cli = cli.subcommand(
            Command::new("doctor")
                .about("Diagnose configuration and setup issues")
                .long_about(
                    "
Runs comprehensive diagnostics to help troubleshoot setup issues.
The doctor command will check:

- If swissarmyhammer is in your PATH
- Claude Code MCP configuration
- Prompt directories and permissions
- YAML syntax in prompt files
- File watching capabilities

Exit codes:
  0 - All checks passed
  1 - Warnings found
  2 - Errors found

Example:
  swissarmyhammer doctor
  swissarmyhammer doctor               # Check system health and configuration
                ",
                ),
        );

        // Add prompt command with subcommands
        cli = cli.subcommand(Self::build_prompt_command());

        // Add flow command with subcommands
        cli = cli.subcommand(Self::build_flow_command());

        // Add validate command
        cli = cli.subcommand(
            Command::new("validate")
                .about("Validate prompt files and workflows for syntax and best practices")
                .long_about(
                    "
Validates BOTH prompt files AND workflows for syntax errors and best practices.

This command comprehensively validates:
- All prompt files from builtin, user, and local directories
- All workflow files from standard locations (builtin, user, local)

Validation checks:
- YAML front matter syntax (skipped for .liquid files with {% partial %} marker)
- Required fields (title, description)
- Template variables match arguments
- Liquid template syntax
- Workflow structure and connectivity
- Best practice recommendations

Examples:
  swissarmyhammer validate                 # Validate all prompts and workflows
  swissarmyhammer validate --quiet         # CI/CD mode - only shows errors, hides warnings
  swissarmyhammer validate --format json   # JSON output for tooling
                ",
                )
                .arg(
                    Arg::new("quiet")
                        .short('q')
                        .long("quiet")
                        .help("Suppress all output except errors")
                        .action(ArgAction::SetTrue),
                )
                .arg(
                    Arg::new("format")
                        .long("format")
                        .help("Output format")
                        .value_parser(["text", "json"])
                        .default_value("text"),
                )
                .arg(
                    Arg::new("workflow-dirs")
                        .long("workflow-dir")
                        .help("[DEPRECATED] This parameter is ignored. Workflows are now only loaded from standard locations.")
                        .action(ArgAction::Append)
                        .hide(true),
                )
                .arg(
                    Arg::new("validate-tools")
                        .long("validate-tools")
                        .help("Validate MCP tool schemas for CLI compatibility")
                        .action(ArgAction::SetTrue),
                ),
        );

        // Add agent command with subcommands
        cli = cli.subcommand(Self::build_agent_command());

        // Add rule command with subcommands
        // Rule command is now dynamically generated from rules_check MCP tool
        // cli = cli.subcommand(Self::build_rule_command());

        cli
    }

    /// Build the prompt command with all its subcommands
    fn build_prompt_command() -> Command {
        Command::new("prompt")
            .about("Manage and test prompts")
            .long_about(
                "
Manage and test prompts with a clean, simplified interface.

The prompt system provides two main commands:
• list - Display all available prompts from all sources  
• test - Test prompts interactively with sample data

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah prompt list                           # List all prompts
  sah --verbose prompt list                 # Show detailed information
  sah --format=json prompt list             # Output as JSON
  sah prompt test code-review               # Interactive testing
  sah prompt test help --var topic=git      # Test with parameters  
  sah --debug prompt test plan              # Test with debug output
",
            )
            .subcommand(
                Command::new("list")
                    .about("Display all available prompts from all sources")
                    .long_about(
                        "
Display all available prompts from all sources (built-in, user, local).

## Global Options

Control output using global arguments:

  sah --verbose prompt list           # Show detailed information including descriptions
  sah --format=json prompt list       # Output as JSON for scripting
  sah --format=yaml prompt list       # Output as YAML for scripting  

## Output

### Standard Output (default)
Shows prompt names and titles in a clean table format.

### Verbose Output (--verbose)
Shows additional information including:
- Full descriptions
- Source information (builtin, user, local)
- Categories and tags
- Parameter counts

### Structured Output (--format=json|yaml)
Machine-readable output suitable for scripting and automation.

## Examples

  # Basic list
  sah prompt list

  # Detailed information  
  sah --verbose prompt list

  # JSON output for scripts
  sah --format=json prompt list | jq '.[] | .name'

  # Save YAML output
  sah --format=yaml prompt list > prompts.yaml

## Notes

- Partial templates (internal templates used by other prompts) are automatically filtered out
- All available prompt sources are included automatically
- Use global --quiet to suppress output except errors
",
                    ),
            )
            .subcommand(
                Command::new("test")
                    .about("Test prompts interactively with sample arguments")
                    .long_about(
                        "
Test prompts interactively to see how they render with different arguments.
Perfect for debugging template issues and previewing prompt output.

## Usage
  sah prompt test <PROMPT_NAME> [OPTIONS]
  sah prompt test --file <FILE> [OPTIONS]

## Arguments

- <PROMPT_NAME> - Name of the prompt to test
- --file <FILE> - Path to a local prompt file to test

## Options

- --var <KEY=VALUE> - Set template variables (can be used multiple times)
- --raw - Output raw prompt without additional formatting
- --copy - Copy rendered prompt to clipboard (if supported)
- --save <FILE> - Save rendered prompt to file
- --debug - Show debug information during processing

## Global Options

- --verbose - Show detailed execution information
- --debug - Enable comprehensive debug output
- --quiet - Suppress all output except the rendered prompt

## Interactive Mode

When variables are not provided via --var, the command prompts interactively:

- Shows parameter descriptions and default values
- Validates input according to parameter types
- Supports boolean (true/false, yes/no, 1/0), numbers, choices
- Detects non-interactive environments (CI/CD) and uses defaults

## Examples

### Basic Testing
  # Interactive mode - prompts for all parameters
  sah prompt test code-review

  # Non-interactive with all parameters provided  
  sah prompt test help --var topic=git --var format=markdown

  # Test from file
  sah prompt test --file ./my-prompt.md --var name=John

### Advanced Usage
  # Verbose output with debug information
  sah --verbose --debug prompt test plan --var project=myapp

  # Save output to file
  sah prompt test help --var topic=testing --save help-output.md

  # Raw output (no extra formatting)
  sah prompt test summary --var title=\"Project Status\" --raw

  # Multiple variables
  sah prompt test code-review \\
    --var author=Jane \\
    --var version=2.1 \\
    --var language=Python \\
    --var files=src/main.py,tests/test_main.py
",
                    )
                    .arg(
                        Arg::new("prompt_name")
                            .help("Prompt name to test")
                            .value_name("PROMPT_NAME"),
                    )
                    .arg(
                        Arg::new("file")
                            .short('f')
                            .long("file")
                            .help("Path to prompt file to test")
                            .value_name("FILE"),
                    )
                    .arg(
                        Arg::new("vars")
                            .long("var")
                            .help("Variables as key=value pairs")
                            .value_name("KEY=VALUE")
                            .action(ArgAction::Append),
                    )
                    .arg(
                        Arg::new("raw")
                            .long("raw")
                            .help("Show raw output without formatting")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("copy")
                            .long("copy")
                            .help("Copy rendered prompt to clipboard")
                            .action(ArgAction::SetTrue),
                    )
                    .arg(
                        Arg::new("save")
                            .long("save")
                            .help("Save rendered prompt to file")
                            .value_name("FILE"),
                    )
                    .arg(
                        Arg::new("debug")
                            .long("debug")
                            .help("Show debug information")
                            .action(ArgAction::SetTrue),
                    ),
            )
            .subcommand(
                Command::new("validate")
                    .about("Validate prompt files and workflows")
                    .arg(
                        Arg::new("verbose")
                            .short('v')
                            .long("verbose")
                            .help("Show verbose validation output")
                            .action(ArgAction::SetTrue),
                    ),
            )
    }

    /// Build the flow command with all its subcommands
    fn build_flow_command() -> Command {
        Command::new("flow")
            .about("Execute or list workflows")
            .long_about(
                "Execute workflows or list available workflows.

Usage:
  sah flow list                List all workflows
  sah flow <workflow> [args]   Execute a workflow

Special case: 'list' shows all available workflows
All other names execute the named workflow.

Examples:
  sah flow list --verbose
  sah flow implement
  sah flow plan spec.md
",
            )
            .trailing_var_arg(true)
            .allow_external_subcommands(true)
            .arg(
                Arg::new("args")
                    .num_args(0..)
                    .help("Workflow name (or 'list') followed by arguments"),
            )
    }

    /// Build the agent command with all its subcommands
    pub fn build_agent_command() -> Command {
        Command::new("agent")
            .about("Manage and interact with agents")
            .long_about(
                "
Manage and interact with agents in the SwissArmyHammer system.
Agents provide specialized functionality through dedicated workflows
and tools for specific use cases.

The agent system provides two main commands:
• list - Display all available agents from all sources
• use - Apply or execute a specific agent

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah agent list                           # List all agents
  sah --verbose agent list                 # Show detailed information
  sah --format=json agent list             # Output as JSON
  sah agent use code-reviewer              # Apply code-reviewer agent
  sah --debug agent use planner            # Use agent with debug output
                ",
            )
            .subcommand(
                Command::new("list").about("List available agents").arg(
                    Arg::new("format")
                        .long("format")
                        .help("Output format")
                        .value_parser(["table", "json", "yaml"])
                        .default_value("table"),
                ),
            )
            .subcommand(
                Command::new("use").about("Use a specific agent").arg(
                    Arg::new("agent_name")
                        .help("Name of the agent to use")
                        .value_name("AGENT_NAME")
                        .required(true),
                ),
            )
    }

    /// Generate workflow shortcut commands dynamically
    ///
    /// Creates top-level CLI commands for each workflow, enabling direct access like
    /// `sah plan spec.md` instead of `sah flow plan spec.md`.
    ///
    /// # Conflict Resolution
    ///
    /// Workflows that conflict with reserved command names get an underscore prefix:
    /// - Reserved: serve, doctor, prompt, rule, flow, agent, validate, plan, implement, list
    /// - Example: A workflow named "list" becomes "_list"
    ///
    /// # Parameters
    ///
    /// * `workflow_storage` - Storage instance to load available workflows
    ///
    /// # Returns
    ///
    /// Vector of clap Commands, one for each workflow with proper argument handling
    pub fn build_workflow_shortcuts(workflow_storage: &WorkflowStorage) -> Vec<Command> {
        use std::collections::HashSet;

        // Reserved command names that would conflict with top-level commands
        const RESERVED_NAMES: &[&str] = &[
            "serve", "doctor", "prompt", "rule", "flow", "agent", "validate",
            "list", // Special: flow subcommand that should not conflict
        ];

        let mut shortcuts = Vec::new();
        let reserved: HashSet<&str> = RESERVED_NAMES.iter().copied().collect();

        // Load workflows from storage
        let workflows = match workflow_storage.list_workflows() {
            Ok(workflows) => workflows,
            Err(e) => {
                tracing::warn!("Failed to load workflows for shortcuts: {}", e);
                return shortcuts;
            }
        };

        for workflow in workflows {
            let workflow_name = workflow.name.to_string();

            // Apply conflict resolution - prefix with underscore if reserved
            let command_name = if reserved.contains(workflow_name.as_str()) {
                format!("_{}", workflow_name)
            } else {
                workflow_name.clone()
            };

            // Build the shortcut command
            let cmd = Self::build_shortcut_command(command_name, &workflow_name, &workflow);
            shortcuts.push(cmd);
        }

        shortcuts
    }

    /// Build a single workflow shortcut command
    ///
    /// Creates a clap Command for a workflow shortcut with:
    /// - Positional arguments for required parameters
    /// - --param flag for optional parameters
    /// - Standard flow execution flags (--interactive, --dry-run, --quiet)
    ///
    /// # Parameters
    ///
    /// * `command_name` - CLI command name (may have underscore prefix for conflicts)
    /// * `workflow_name` - Original workflow name
    /// * `workflow` - Workflow definition with parameters
    fn build_shortcut_command(
        command_name: String,
        workflow_name: &str,
        workflow: &swissarmyhammer_workflow::Workflow,
    ) -> Command {
        let mut cmd = Command::new(intern_string(command_name.clone()));

        // Set description indicating this is a shortcut
        let about_text = format!(
            "{} (shortcut for 'flow {}')",
            workflow.description, workflow_name
        );
        cmd = cmd
            .about(intern_string(about_text))
            .subcommand_help_heading("Workflows");

        // Collect required parameters
        let required_params: Vec<_> = workflow.parameters.iter().filter(|p| p.required).collect();

        // Add positional arguments for required parameters ONLY if there are any
        if !required_params.is_empty() {
            let value_names: Vec<&'static str> = required_params
                .iter()
                .map(|p| intern_string(p.name.clone()))
                .collect();

            cmd = cmd.arg(
                Arg::new("positional")
                    .num_args(required_params.len())
                    .value_names(value_names)
                    .required(true)
                    .help("Required workflow parameters"),
            );
        }

        // Add --param flag for optional parameters
        cmd = cmd.arg(
            Arg::new("param")
                .long("param")
                .short('p')
                .action(ArgAction::Append)
                .value_name("KEY=VALUE")
                .help("Optional workflow parameter"),
        );

        // Add standard workflow execution flags
        cmd = cmd
            .arg(
                Arg::new("interactive")
                    .long("interactive")
                    .short('i')
                    .action(ArgAction::SetTrue)
                    .help("Interactive mode - prompt at each state"),
            )
            .arg(
                Arg::new("dry_run")
                    .long("dry-run")
                    .action(ArgAction::SetTrue)
                    .help("Dry run - show execution plan without running"),
            )
            .arg(
                Arg::new("quiet")
                    .long("quiet")
                    .short('q')
                    .action(ArgAction::SetTrue)
                    .help("Quiet mode - only show errors"),
            );

        cmd
    }
}

#[cfg(test)]
#[path = "dynamic_cli_tests.rs"]
mod tests;
