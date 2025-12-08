//! Dynamic CLI builder for MCP tools
//!
//! This module implements dynamic CLI command generation from MCP tool definitions,
//! eliminating the need for redundant CLI command enums and ensuring consistency
//! between MCP and CLI interfaces.

use crate::schema_validation::{SchemaValidator, ValidationError};
use clap::{Arg, ArgAction, Command};
use once_cell::sync::Lazy;
use owo_colors::OwoColorize;
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};
use swissarmyhammer_workflow::WorkflowStorage;
use tokio::sync::RwLock;

/// Global string cache to prevent memory leaks from Box::leak
/// Strings are interned once and reused, satisfying clap's 'static lifetime requirement
static STRING_CACHE: Lazy<Mutex<HashSet<&'static str>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Check if a JSON schema contains a specific type name
/// Handles both string and array type specifications
fn schema_has_type(schema: &Value, type_name: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(t)) => t == type_name,
        Some(Value::Array(types)) => types.iter().any(|t| t.as_str() == Some(type_name)),
        _ => false,
    }
}

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
    NullableBoolean,
    Array,
}

/// Configuration for building a command with documentation
struct CommandConfig {
    name: &'static str,
    about: &'static str,
    long_about: &'static str,
}

// Documentation constants for commands
const BASE_CLI_LONG_ABOUT: &str = "
SwissArmyHammer - The only coding assistant you'll ever need

Commands are organized into three types:
- Static commands (serve, doctor, validate, model, prompt, rule, flow)
- Workflow shortcuts (do, plan, review, etc.) - use 'sah flow list' to see all
- Tool commands (file, issue, memo, search, shell, web-search)

Examples:
  sah serve                    Run as MCP server
  sah doctor                   Diagnose configuration
  sah flow list                List all workflows
  sah do                       Execute do workflow (shortcut)
  sah plan spec.md             Execute plan workflow (shortcut)
  sah file read path.txt       Read a file via MCP tool
";

const SERVE_COMMAND_LONG_ABOUT: &str = "
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
";

const SERVE_HTTP_LONG_ABOUT: &str = "
Starts an HTTP MCP server for web clients, debugging, and LlamaAgent integration.
The server exposes MCP tools through HTTP endpoints and provides:

- RESTful MCP protocol implementation
- Health check endpoint at /health
- Support for random port allocation (use port 0)
- Graceful shutdown with Ctrl+C

Example:
  swissarmyhammer serve http --port 8080 --host 127.0.0.1
  swissarmyhammer serve http --port 0  # Random port
";

const DOCTOR_COMMAND_LONG_ABOUT: &str = "
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
";

const VALIDATE_COMMAND_LONG_ABOUT: &str = "
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
";

const PROMPT_COMMAND_LONG_ABOUT: &str = "
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
";

const PROMPT_LIST_LONG_ABOUT: &str = "
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
";

const PROMPT_TEST_LONG_ABOUT: &str = "
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
";

const FLOW_COMMAND_LONG_ABOUT: &str = "Execute workflows or list available workflows.

Usage:
  sah flow list                List all workflows
  sah flow <workflow> [args]   Execute a workflow

Special case: 'list' shows all available workflows
All other names execute the named workflow.

Examples:
  sah flow list --verbose
  sah flow do
  sah flow plan spec.md
";

const MODEL_COMMAND_LONG_ABOUT: &str = "
Manage and interact with models in the SwissArmyHammer system.
Models provide specialized functionality through dedicated workflows
and tools for specific use cases.

The model system provides three main commands:
• show - Display current model use case assignments (default)
• list - Display all available models from all sources
• use - Apply or execute a specific model

Use global arguments to control output:
  --verbose         Show detailed information
  --format FORMAT   Output format: table, json, yaml
  --debug           Enable debug mode
  --quiet           Suppress output except errors

Examples:
  sah model                                # Show use case assignments
  sah model show                           # Same as above
  sah model list                           # List all models
  sah --verbose model list                 # Show detailed information
  sah --format=json model list             # Output as JSON
  sah model use code-reviewer              # Apply code-reviewer model
  sah --debug model use planner            # Use model with debug output
";

const MODEL_USE_LONG_ABOUT: &str = "
Apply a specific model configuration to the project for a use case.

Usage patterns:
  sah model use <MODEL>              # Set root model (backward compatible)
  sah model use <USE_CASE> <MODEL>   # Set model for specific use case

Use cases:
  root      - Default model for general operations
  rules     - Model for rule checking operations
  workflows - Model for workflow execution

Examples:
  sah model use claude-code               # Set root model
  sah model use rules qwen-coder          # Use Qwen for rules
  sah model use workflows claude-code     # Use Claude for workflows
";

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

    /// Check if there are no tools to validate
    fn has_no_tools(&self) -> bool {
        self.total_tools == 0
    }

    /// Check if there are any validation issues
    pub fn is_all_valid(&self) -> bool {
        self.invalid_tools == 0 && self.validation_errors == 0
    }

    /// Calculate success rate percentage
    pub fn success_rate(&self) -> f64 {
        if self.has_no_tools() {
            100.0
        } else {
            (self.valid_tools as f64 / self.total_tools as f64) * 100.0
        }
    }

    pub fn summary(&self) -> String {
        if self.is_all_valid() {
            format!(
                "{} All {} CLI tools are valid",
                "✓".green(),
                self.total_tools
            )
        } else {
            format!(
                "{} {} of {} CLI tools are valid ({:.1}% success rate, {} validation errors)",
                "⚠".yellow(),
                self.valid_tools,
                self.total_tools,
                self.success_rate(),
                self.validation_errors
            )
        }
    }
}

/// Type of iteration to perform over the tool registry
enum RegistryIterType {
    AllTools,
    ToolsInCategory(String),
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
        let (category_commands, tool_commands) = Self::precompute_all_commands(&tool_registry);

        Self {
            tool_registry,
            category_commands,
            tool_commands,
        }
    }

    /// Precompute all command data from the registry
    fn precompute_all_commands(
        tool_registry: &Arc<RwLock<ToolRegistry>>,
    ) -> (
        HashMap<String, CommandData>,
        HashMap<String, HashMap<String, CommandData>>,
    ) {
        let registry = tool_registry
            .try_read()
            .expect("ToolRegistry should not be locked");
        let category_commands = Self::precompute_category_commands(&registry);
        let tool_commands = Self::precompute_tool_commands(&registry);
        (category_commands, tool_commands)
    }

    /// Generic iteration helper over the tool registry
    ///
    /// Consolidates all iteration patterns into a single helper function
    fn iter_registry<F>(registry: &ToolRegistry, iter_type: RegistryIterType, f: F)
    where
        F: FnMut(&dyn McpTool),
    {
        match iter_type {
            RegistryIterType::AllTools => Self::iter_all_categories(registry, f),
            RegistryIterType::ToolsInCategory(category) => {
                Self::iter_category_tools(registry, &category, f)
            }
        }
    }

    /// Iterate through all categories and their tools
    fn iter_all_categories<F>(registry: &ToolRegistry, mut f: F)
    where
        F: FnMut(&dyn McpTool),
    {
        for category in registry.get_cli_categories() {
            Self::iter_category_tools(registry, &category, &mut f);
        }
    }

    /// Iterate through tools in a specific category
    fn iter_category_tools<F>(registry: &ToolRegistry, category: &str, mut f: F)
    where
        F: FnMut(&dyn McpTool),
    {
        for tool in registry.get_tools_for_category(category) {
            f(tool);
        }
    }

    /// Iterate through all tools in the registry, applying a function to each
    fn iter_all_tools<F>(registry: &ToolRegistry, f: F)
    where
        F: FnMut(&dyn McpTool),
    {
        Self::iter_registry(registry, RegistryIterType::AllTools, f)
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

    /// Iterate through categories, applying a function to each
    fn iter_categories<F>(registry: &ToolRegistry, mut f: F)
    where
        F: FnMut(&str),
    {
        for category in registry.get_cli_categories() {
            f(&category);
        }
    }

    /// Pre-compute tool command data
    fn precompute_tool_commands(
        registry: &ToolRegistry,
    ) -> HashMap<String, HashMap<String, CommandData>> {
        let mut tool_commands = HashMap::new();

        Self::iter_categories(registry, |category| {
            let tools_in_category = Self::precompute_category_tool_commands(registry, category);
            tool_commands.insert(category.to_string(), tools_in_category);
        });

        tool_commands
    }

    /// Pre-compute tool commands for a specific category
    fn precompute_category_tool_commands(
        registry: &ToolRegistry,
        category: &str,
    ) -> HashMap<String, CommandData> {
        let mut tools_in_category = HashMap::new();

        Self::iter_tools_in_category(registry, category, |tool| {
            if !tool.hidden_from_cli() {
                if let Some(tool_cmd_data) = Self::precompute_tool_command(tool) {
                    tools_in_category.insert(tool.cli_name().to_string(), tool_cmd_data);
                }
            }
        });

        tools_in_category
    }

    /// Iterate through tools in a category, applying a function to each
    fn iter_tools_in_category<F>(registry: &ToolRegistry, category: &str, f: F)
    where
        F: FnMut(&dyn McpTool),
    {
        Self::iter_registry(
            registry,
            RegistryIterType::ToolsInCategory(category.to_string()),
            f,
        )
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
        let properties = match schema.get("properties").and_then(|p| p.as_object()) {
            Some(props) => props,
            None => return Vec::new(),
        };

        let required_fields = Self::extract_required_fields(schema);
        Self::extract_property_args(properties, &required_fields)
    }

    /// Extract argument data from properties object
    fn extract_property_args(
        properties: &serde_json::Map<String, Value>,
        required_fields: &std::collections::HashSet<String>,
    ) -> Vec<ArgData> {
        properties
            .iter()
            .map(|(prop_name, prop_schema)| {
                Self::precompute_arg_data(
                    prop_name,
                    prop_schema,
                    required_fields.contains(prop_name),
                )
            })
            .collect()
    }

    /// Extract required field names from JSON schema
    fn extract_required_fields(schema: &Value) -> std::collections::HashSet<String> {
        schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Extract a string field from a JSON schema
    fn extract_schema_string(schema: &Value, field: &str) -> Option<String> {
        schema.get(field).and_then(|v| v.as_str()).map(String::from)
    }

    /// Extract enum values from a JSON schema
    fn extract_enum_values(schema: &Value) -> Option<Vec<String>> {
        schema.get("enum").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    }

    /// Pre-compute data for a single argument
    fn precompute_arg_data(name: &str, schema: &Value, is_required: bool) -> ArgData {
        let arg_type = Self::determine_arg_type(schema);

        ArgData {
            name: name.to_string(),
            help: Self::extract_schema_string(schema, "description"),
            is_required,
            arg_type,
            default_value: Self::extract_schema_string(schema, "default"),
            possible_values: Self::extract_enum_values(schema),
        }
    }

    /// Determine the argument type from a JSON schema
    fn determine_arg_type(schema: &Value) -> ArgType {
        // Check for nullable boolean first (both types must be present)
        if Self::is_nullable_boolean(schema) {
            return ArgType::NullableBoolean;
        }

        // Match on primary type
        match Self::get_primary_type(schema) {
            Some("boolean") => ArgType::Boolean,
            Some("integer") => ArgType::Integer,
            Some("number") => ArgType::Float,
            Some("array") => ArgType::Array,
            Some("string") | _ => ArgType::String,
        }
    }

    /// Check if schema represents a nullable boolean
    fn is_nullable_boolean(schema: &Value) -> bool {
        schema_has_type(schema, "boolean") && schema_has_type(schema, "null")
    }

    /// Get the primary type from a schema
    fn get_primary_type(schema: &Value) -> Option<&str> {
        match schema.get("type") {
            Some(Value::String(t)) => Some(t.as_str()),
            Some(Value::Array(types)) => types
                .iter()
                .find_map(|t| t.as_str().filter(|s| *s != "null")),
            _ => None,
        }
    }

    /// Build the complete CLI with dynamic commands generated from MCP tools
    ///
    /// # Parameters
    ///
    /// * `workflow_storage` - Optional workflow storage for generating shortcut commands.
    ///   If None, shortcuts will not be generated.
    pub fn build_cli(&self, workflow_storage: Option<&WorkflowStorage>) -> Command {
        let mut cli = Self::build_base_cli();

        // Add static commands (serve, doctor, prompt, flow, validate, model)
        cli = Self::add_static_commands(cli);

        // Add workflow shortcuts if storage is provided
        cli = self.add_workflow_shortcuts_to_cli(cli, workflow_storage);

        // Add dynamic MCP tool commands using pre-computed data
        cli = self.add_tool_category_commands(cli);

        cli
    }

    /// Build the base CLI command with global arguments
    fn build_base_cli() -> Command {
        let cmd = Command::new("swissarmyhammer")
            .version(env!("CARGO_PKG_VERSION"))
            .about("The only coding assistant you'll ever need")
            .long_about(BASE_CLI_LONG_ABOUT);

        Self::add_global_arguments(cmd)
    }

    /// Add global arguments to the base CLI command
    fn add_global_arguments(cmd: Command) -> Command {
        cmd.arg(Self::create_flag_arg(
            "verbose",
            "verbose",
            Some('v'),
            "Enable verbose logging",
        ))
        .arg(Self::create_flag_arg(
            "debug",
            "debug",
            Some('d'),
            "Enable debug logging",
        ))
        .arg(
            Arg::new("cwd")
                .long("cwd")
                .help("Set working directory before executing command")
                .value_name("PATH")
                .global(true)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
        .arg(Self::create_flag_arg(
            "quiet",
            "quiet",
            Some('q'),
            "Suppress all output except errors",
        ))
        .arg(Self::create_flag_arg(
            "validate-tools",
            "validate-tools",
            None,
            "Validate all tool schemas and exit",
        ))
        .arg(
            Arg::new("format")
                .long("format")
                .help("Global output format")
                .value_parser(["table", "json", "yaml"]),
        )
        .arg(
            Arg::new("model")
                .long("model")
                .help("Override model for all use cases (runtime only, doesn't modify config)")
                .value_name("MODEL")
                .global(true),
        )
    }

    /// Add workflow shortcuts to the CLI if storage is provided
    fn add_workflow_shortcuts_to_cli(
        &self,
        mut cli: Command,
        workflow_storage: Option<&WorkflowStorage>,
    ) -> Command {
        if let Some(storage) = workflow_storage {
            let shortcuts = Self::get_sorted_workflow_shortcuts(storage);
            for shortcut in shortcuts {
                cli = cli.subcommand(shortcut);
            }
        }
        cli
    }

    /// Get sorted workflow shortcuts
    fn get_sorted_workflow_shortcuts(storage: &WorkflowStorage) -> Vec<Command> {
        let mut shortcuts = Self::build_workflow_shortcuts(storage);
        shortcuts.sort_by(|a, b| a.get_name().cmp(b.get_name()));
        shortcuts
    }

    /// Add dynamic MCP tool category commands to the CLI
    fn add_tool_category_commands(&self, mut cli: Command) -> Command {
        let category_names = self.get_sorted_category_names();

        for category_name in category_names.iter() {
            if let Some(category_data) = self.category_commands.get(category_name) {
                let cmd = self.build_category_command_from_data(category_name, category_data);
                cli = cli.subcommand(cmd);
            }
        }

        cli
    }

    /// Get sorted category names for consistent ordering
    fn get_sorted_category_names(&self) -> Vec<String> {
        let mut category_names: Vec<String> = self.category_commands.keys().cloned().collect();
        category_names.sort();
        category_names
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
        self.fold_validation_results(Vec::new(), |mut errors, result| {
            if let Err(tool_errors) = result {
                errors.extend(tool_errors);
            }
            errors
        })
    }

    /// Process all tool validation results with a fold function
    ///
    /// This provides a single source of truth for tool validation iteration
    /// and processing using a fold pattern that avoids intermediate allocations.
    ///
    /// # Type Parameters
    ///
    /// * `T` - Accumulator type
    /// * `F` - Fold function type
    ///
    /// # Parameters
    ///
    /// * `init` - Initial accumulator value
    /// * `folder` - Function to fold each validation result into the accumulator
    ///
    /// # Returns
    ///
    /// Final accumulated result
    fn fold_validation_results<T, F>(&self, init: T, folder: F) -> T
    where
        F: FnMut(T, Result<(), Vec<ValidationError>>) -> T,
    {
        let registry = self
            .tool_registry
            .try_read()
            .expect("ToolRegistry should not be locked");

        Self::collect_and_fold_validation_results(&registry, self, init, folder)
    }

    /// Collect validation results and fold them into an accumulator
    fn collect_and_fold_validation_results<T, F>(
        registry: &ToolRegistry,
        builder: &CliBuilder,
        init: T,
        folder: F,
    ) -> T
    where
        F: FnMut(T, Result<(), Vec<ValidationError>>) -> T,
    {
        let mut validation_results = Vec::new();

        Self::iter_all_tools(registry, |tool| {
            validation_results.push(builder.validate_single_tool(tool));
        });

        validation_results.into_iter().fold(init, folder)
    }

    /// Validate a single tool for CLI compatibility
    fn validate_single_tool(&self, tool: &dyn McpTool) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        // Validate schema
        Self::validate_tool_schema(tool, &mut errors);

        // Validate CLI integration requirements
        if !tool.hidden_from_cli() {
            Self::validate_tool_cli_requirements(tool, &mut errors);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate tool schema
    fn validate_tool_schema(tool: &dyn McpTool, errors: &mut Vec<ValidationError>) {
        if let Err(schema_error) = SchemaValidator::validate_schema(&tool.schema()) {
            errors.push(schema_error);
        }
    }

    /// Validate CLI integration requirements for a tool
    fn validate_tool_cli_requirements(tool: &dyn McpTool, errors: &mut Vec<ValidationError>) {
        // Check for CLI category
        if tool.cli_category().is_none() {
            errors.push(ValidationError::MissingSchemaField {
                field: format!("CLI category for tool {}", tool.name()),
            });
        }

        // Validate CLI name
        Self::validate_tool_cli_name(tool, errors);
    }

    /// Validate tool CLI name
    fn validate_tool_cli_name(tool: &dyn McpTool, errors: &mut Vec<ValidationError>) {
        let cli_name = tool.cli_name();
        if cli_name.is_empty() {
            errors.push(ValidationError::InvalidParameterName {
                parameter: tool.name().to_string(),
                reason: "CLI name cannot be empty".to_string(),
            });
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
        self.fold_validation_results(CliValidationStats::new(), |mut stats, result| {
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

    /// Helper to apply optional argument configuration
    ///
    /// Reduces duplication in conditional argument chaining
    fn apply_optional_arg_config<T>(
        arg: Arg,
        value: &Option<T>,
        applier: impl FnOnce(Arg, &T) -> Arg,
    ) -> Arg {
        if let Some(val) = value {
            applier(arg, val)
        } else {
            arg
        }
    }

    /// Build a clap argument from pre-computed data
    fn build_arg_from_data(&self, arg_data: &ArgData) -> Arg {
        let name_static = intern_string(arg_data.name.clone());
        let arg = Arg::new(name_static).long(name_static);

        // Set as required if specified
        let arg = if arg_data.is_required {
            arg.required(true)
        } else {
            arg
        };

        // Configure based on type using strategy pattern
        let arg = Self::build_typed_arg(arg, arg_data);

        // Apply optional configurations (help, enum values, defaults)
        Self::apply_arg_metadata(arg, arg_data)
    }

    /// Build typed argument by dispatching to type-specific builders
    fn build_typed_arg(arg: Arg, arg_data: &ArgData) -> Arg {
        match arg_data.arg_type {
            ArgType::Boolean => Self::build_boolean_arg(arg),
            ArgType::NullableBoolean => Self::build_nullable_boolean_arg(arg),
            ArgType::Integer => Self::build_integer_arg(arg, arg_data.is_required),
            ArgType::Float => Self::build_float_arg(arg, arg_data.is_required),
            ArgType::Array => Self::build_array_arg(arg, arg_data.is_required),
            ArgType::String => Self::build_string_arg(arg, arg_data.is_required),
        }
    }

    /// Apply metadata configuration (help text, enum values, default values)
    fn apply_arg_metadata(mut arg: Arg, arg_data: &ArgData) -> Arg {
        // Set help text
        arg = Self::apply_optional_arg_config(arg, &arg_data.help, |a, help| {
            a.help(intern_string(help.clone()))
        });

        // Handle enum values
        arg = Self::apply_optional_arg_config(arg, &arg_data.possible_values, |a, values| {
            let str_values: Vec<&'static str> =
                values.iter().map(|s| intern_string(s.clone())).collect();
            a.value_parser(clap::builder::PossibleValuesParser::new(str_values))
        });

        // Handle default values
        arg = Self::apply_optional_arg_config(arg, &arg_data.default_value, |a, default| {
            a.default_value(intern_string(default.clone()))
        });

        arg
    }

    /// Build a boolean argument
    fn build_boolean_arg(arg: Arg) -> Arg {
        arg.action(ArgAction::SetTrue)
    }

    /// Build a nullable boolean argument
    fn build_nullable_boolean_arg(arg: Arg) -> Arg {
        arg.value_parser(clap::builder::PossibleValuesParser::new(["true", "false"]))
            .value_name("BOOL")
    }

    /// Build an integer argument
    fn build_integer_arg(arg: Arg, is_required: bool) -> Arg {
        Self::build_numeric_arg(arg, is_required, clap::value_parser!(i64))
    }

    /// Build a float argument
    fn build_float_arg(arg: Arg, is_required: bool) -> Arg {
        Self::build_numeric_arg(arg, is_required, clap::value_parser!(f64))
    }

    /// Build a numeric argument with a specific value parser
    fn build_numeric_arg(
        arg: Arg,
        is_required: bool,
        parser: impl clap::builder::IntoResettable<clap::builder::ValueParser>,
    ) -> Arg {
        let arg = arg.value_parser(parser);
        Self::add_optional_value_name(arg, is_required, "NUMBER")
    }

    /// Build an array argument
    fn build_array_arg(arg: Arg, is_required: bool) -> Arg {
        let arg = arg.action(ArgAction::Append);
        Self::add_optional_value_name(arg, is_required, "VALUE")
    }

    /// Build a string argument
    fn build_string_arg(arg: Arg, is_required: bool) -> Arg {
        Self::add_optional_value_name(arg, is_required, "TEXT")
    }

    /// Add value_name to optional arguments
    fn add_optional_value_name(arg: Arg, is_required: bool, name: &'static str) -> Arg {
        if !is_required {
            arg.value_name(name)
        } else {
            arg
        }
    }

    /// Create a flag argument (boolean with SetTrue action)
    ///
    /// Consolidates the common pattern of creating boolean flag arguments with
    /// optional short form. This eliminates duplication in build_base_cli and
    /// add_workflow_execution_flags.
    fn create_flag_arg(
        name: &'static str,
        long: &'static str,
        short: Option<char>,
        help: &'static str,
    ) -> Arg {
        let mut arg = Arg::new(name)
            .long(long)
            .help(help)
            .action(ArgAction::SetTrue);

        if let Some(short_char) = short {
            arg = arg.short(short_char);
        }

        arg
    }

    /// Build a command with standard documentation pattern
    fn build_command_with_docs(config: CommandConfig) -> Command {
        let data = CommandData {
            name: config.name.to_string(),
            about: Some(config.about.to_string()),
            long_about: Some(config.long_about.to_string()),
            args: Vec::new(),
        };
        Self::build_command_base(&data)
    }

    /// Build a command with documentation and arguments
    ///
    /// Consolidates the common pattern of creating a command with build_command_with_docs
    /// and then chaining .arg() calls.
    fn build_command_with_args(config: CommandConfig, args: Vec<Arg>) -> Command {
        let mut cmd = Self::build_command_with_docs(config);
        for arg in args {
            cmd = cmd.arg(arg);
        }
        cmd
    }

    /// Build the HTTP subcommand for serve
    fn build_serve_http_subcommand() -> Command {
        Self::build_command_with_args(
            CommandConfig {
                name: "http",
                about: "Start HTTP MCP server",
                long_about: SERVE_HTTP_LONG_ABOUT,
            },
            vec![
                Arg::new("port")
                    .long("port")
                    .short('p')
                    .help("Port to bind to (use 0 for random port)")
                    .default_value("8000")
                    .value_parser(clap::value_parser!(u16)),
                Arg::new("host")
                    .long("host")
                    .short('H')
                    .help("Host to bind to")
                    .default_value("127.0.0.1"),
            ],
        )
    }

    /// Build the serve command
    fn build_serve_command() -> Command {
        Self::build_command_with_docs(CommandConfig {
            name: "serve",
            about: "Run as MCP server (default when invoked via stdio)",
            long_about: SERVE_COMMAND_LONG_ABOUT,
        })
        .subcommand(Self::build_serve_http_subcommand())
    }

    /// Build the doctor command
    fn build_doctor_command() -> Command {
        Self::build_command_with_docs(CommandConfig {
            name: "doctor",
            about: "Diagnose configuration and setup issues",
            long_about: DOCTOR_COMMAND_LONG_ABOUT,
        })
    }

    /// Build the validate command
    fn build_validate_command() -> Command {
        Self::build_command_with_args(
            CommandConfig {
                name: "validate",
                about: "Validate prompt files and workflows for syntax and best practices",
                long_about: VALIDATE_COMMAND_LONG_ABOUT,
            },
            Self::create_validate_command_args(),
        )
    }

    /// Create arguments for the validate command
    fn create_validate_command_args() -> Vec<Arg> {
        vec![
            Arg::new("quiet")
                .short('q')
                .long("quiet")
                .help("Suppress all output except errors")
                .action(ArgAction::SetTrue),
            Arg::new("format")
                .long("format")
                .help("Output format")
                .value_parser(["text", "json"])
                .default_value("text"),
            Arg::new("workflow-dirs")
                .long("workflow-dir")
                .help("[DEPRECATED] This parameter is ignored. Workflows are now only loaded from standard locations.")
                .action(ArgAction::Append)
                .hide(true),
            Arg::new("validate-tools")
                .long("validate-tools")
                .help("Validate MCP tool schemas for CLI compatibility")
                .action(ArgAction::SetTrue),
        ]
    }

    /// Add static commands to the CLI (serve, doctor, prompt, flow, validate, model)
    fn add_static_commands(mut cli: Command) -> Command {
        // Add serve command
        cli = cli.subcommand(Self::build_serve_command());

        // Add doctor command
        cli = cli.subcommand(Self::build_doctor_command());

        // Add prompt command with subcommands
        cli = cli.subcommand(Self::build_prompt_command());

        // Add flow command with subcommands
        cli = cli.subcommand(Self::build_flow_command());

        // Add validate command
        cli = cli.subcommand(Self::build_validate_command());

        // Add model command with subcommands
        cli = cli.subcommand(Self::build_model_command());

        // Add rule command with subcommands
        // Rule command is now dynamically generated from rules_check MCP tool
        // cli = cli.subcommand(Self::build_rule_command());

        cli
    }

    /// Build the prompt command with all its subcommands
    fn build_prompt_command() -> Command {
        Self::build_command_with_docs(CommandConfig {
            name: "prompt",
            about: "Manage and test prompts",
            long_about: PROMPT_COMMAND_LONG_ABOUT,
        })
        .subcommand(Self::build_prompt_list_subcommand())
        .subcommand(Self::build_prompt_test_subcommand())
        .subcommand(Self::build_prompt_validate_subcommand())
    }

    /// Build the prompt list subcommand
    fn build_prompt_list_subcommand() -> Command {
        Command::new("list")
            .about("Display all available prompts from all sources")
            .long_about(PROMPT_LIST_LONG_ABOUT)
    }

    /// Build the prompt test subcommand
    fn build_prompt_test_subcommand() -> Command {
        Command::new("test")
            .about("Test prompts interactively with sample arguments")
            .long_about(PROMPT_TEST_LONG_ABOUT)
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
            )
    }

    /// Build the prompt validate subcommand
    fn build_prompt_validate_subcommand() -> Command {
        Command::new("validate")
            .about("Validate prompt files and workflows")
            .arg(
                Arg::new("verbose")
                    .short('v')
                    .long("verbose")
                    .help("Show verbose validation output")
                    .action(ArgAction::SetTrue),
            )
    }

    /// Build the flow command with all its subcommands
    fn build_flow_command() -> Command {
        Self::build_command_with_docs(CommandConfig {
            name: "flow",
            about: "Execute or list workflows",
            long_about: FLOW_COMMAND_LONG_ABOUT,
        })
        .trailing_var_arg(true)
        .allow_external_subcommands(true)
        .arg(
            Arg::new("args")
                .num_args(0..)
                .help("Workflow name (or 'list') followed by arguments"),
        )
    }

    /// Build the model command with all its subcommands
    pub fn build_model_command() -> Command {
        Self::build_command_with_docs(CommandConfig {
            name: "model",
            about: "Manage and interact with models",
            long_about: MODEL_COMMAND_LONG_ABOUT,
        })
        .subcommand(Self::build_model_show_subcommand())
        .subcommand(Self::build_model_list_subcommand())
        .subcommand(Self::build_model_use_subcommand())
    }

    /// Build the model show subcommand
    fn build_model_show_subcommand() -> Command {
        Command::new("show")
            .about("Show current model use case assignments")
            .arg(
                Arg::new("format")
                    .long("format")
                    .help("Output format")
                    .value_parser(["table", "json", "yaml"])
                    .default_value("table"),
            )
    }

    /// Build the model list subcommand
    fn build_model_list_subcommand() -> Command {
        Command::new("list").about("List available models").arg(
            Arg::new("format")
                .long("format")
                .help("Output format")
                .value_parser(["table", "json", "yaml"])
                .default_value("table"),
        )
    }

    /// Build the model use subcommand
    fn build_model_use_subcommand() -> Command {
        Command::new("use")
            .about("Use a specific model for a use case")
            .long_about(MODEL_USE_LONG_ABOUT)
            .arg(
                Arg::new("first")
                    .help("Model name OR use case (root, rules, workflows)")
                    .value_name("FIRST")
                    .required(true),
            )
            .arg(
                Arg::new("second")
                    .help("Model name (required when first argument is a use case)")
                    .value_name("SECOND")
                    .required(false),
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
    /// - Reserved: serve, doctor, prompt, rule, flow, model, validate, plan, implement, list
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
        let workflows = Self::load_available_workflows(workflow_storage);
        Self::create_shortcuts_from_workflows(workflows)
    }

    /// Load available workflows from storage
    fn load_available_workflows(
        workflow_storage: &WorkflowStorage,
    ) -> Vec<swissarmyhammer_workflow::Workflow> {
        match workflow_storage.list_workflows() {
            Ok(workflows) => workflows,
            Err(e) => {
                tracing::warn!("Failed to load workflows for shortcuts: {}", e);
                Vec::new()
            }
        }
    }

    /// Create shortcut commands from workflows
    fn create_shortcuts_from_workflows(
        workflows: Vec<swissarmyhammer_workflow::Workflow>,
    ) -> Vec<Command> {
        workflows
            .into_iter()
            .map(Self::create_workflow_shortcut_command)
            .collect()
    }

    /// Reserved command names that would conflict with top-level commands
    const RESERVED_COMMAND_NAMES: &'static [&'static str] = &[
        "serve", "doctor", "prompt", "rule", "flow", "model", "validate",
        "list", // Special: flow subcommand that should not conflict
    ];

    /// Resolve command name conflicts by prefixing with underscore if reserved
    fn resolve_command_name_conflict(workflow_name: &str) -> String {
        if Self::RESERVED_COMMAND_NAMES.contains(&workflow_name) {
            format!("_{}", workflow_name)
        } else {
            workflow_name.to_string()
        }
    }

    /// Create a single workflow shortcut command
    fn create_workflow_shortcut_command(workflow: swissarmyhammer_workflow::Workflow) -> Command {
        let workflow_name = workflow.name.to_string();
        let command_name = Self::resolve_command_name_conflict(&workflow_name);

        Self::build_shortcut_command(command_name, &workflow_name, &workflow)
    }

    /// Standard workflow execution flag definitions
    /// Format: (name, long, short, help)
    const WORKFLOW_FLAGS: &'static [(&'static str, &'static str, Option<char>, &'static str)] = &[
        (
            "interactive",
            "interactive",
            Some('i'),
            "Interactive mode - prompt at each state",
        ),
        (
            "dry_run",
            "dry-run",
            None,
            "Dry run - show execution plan without running",
        ),
        ("quiet", "quiet", Some('q'), "Quiet mode - only show errors"),
    ];

    /// Add standard workflow execution flags to a command
    ///
    /// Adds the three standard workflow flags: interactive, dry-run, and quiet
    fn add_workflow_execution_flags(cmd: Command) -> Command {
        Self::WORKFLOW_FLAGS
            .iter()
            .fold(cmd, |cmd, (name, long, short, help)| {
                cmd.arg(Self::create_flag_arg(name, long, *short, help))
            })
    }

    /// Create a workflow parameter argument
    ///
    /// Returns a pre-configured parameter argument for workflow commands
    fn create_workflow_param_arg() -> Arg {
        Arg::new("param")
            .long("param")
            .short('p')
            .action(ArgAction::Append)
            .value_name("KEY=VALUE")
            .help("Optional workflow parameter")
    }

    /// Add positional arguments for required workflow parameters
    ///
    /// Helper to extract the logic of adding required parameter positional args
    fn add_required_params_as_positional(
        cmd: Command,
        workflow: &swissarmyhammer_workflow::Workflow,
    ) -> Command {
        let required_params: Vec<_> = workflow.parameters.iter().filter(|p| p.required).collect();

        if required_params.is_empty() {
            return cmd;
        }

        let value_names: Vec<&'static str> = required_params
            .iter()
            .map(|p| intern_string(p.name.clone()))
            .collect();

        cmd.arg(
            Arg::new("positional")
                .num_args(required_params.len())
                .value_names(value_names)
                .required(true)
                .help("Required workflow parameters"),
        )
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

        // Add positional arguments for required parameters
        cmd = Self::add_required_params_as_positional(cmd, workflow);

        // Add --param flag for optional parameters
        cmd = cmd.arg(Self::create_workflow_param_arg());

        // Add standard workflow execution flags
        cmd = Self::add_workflow_execution_flags(cmd);

        cmd
    }
}

#[cfg(test)]
#[path = "dynamic_cli_tests.rs"]
mod tests;
