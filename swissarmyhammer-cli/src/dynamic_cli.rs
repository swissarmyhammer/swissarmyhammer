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
use swissarmyhammer_operations::Operation;
use swissarmyhammer_tools::mcp::tool_registry::{McpTool, ToolRegistry};
use swissarmyhammer_workflow::WorkflowStorage;
use tokio::sync::RwLock;

/// Multiplier for converting decimal ratios to percentage values (0.0-1.0 → 0.0-100.0)
const PERCENTAGE_MULTIPLIER: f64 = 100.0;

/// Default HTTP port used when no environment variable is set
const DEFAULT_HTTP_PORT: &str = "8000";

/// Default HTTP host used when no environment variable is set
const DEFAULT_HTTP_HOST: &str = "127.0.0.1";

/// Get default configuration from environment variables with fallback chain
///
/// Checks both SAH_* and SWISSARMYHAMMER_* prefixed environment variables,
/// falling back to the provided default if neither is set.
///
/// # Arguments
///
/// * `primary_env` - Primary environment variable name (e.g., "SAH_HTTP_PORT")
/// * `fallback_env` - Fallback environment variable name (e.g., "SWISSARMYHAMMER_HTTP_PORT")
/// * `default` - Default value if neither environment variable is set
fn get_default_config(primary_env: &str, fallback_env: &str, default: &str) -> String {
    std::env::var(primary_env)
        .or_else(|_| std::env::var(fallback_env))
        .unwrap_or_else(|_| default.to_string())
}

/// Get the default HTTP port from environment or use fallback
///
/// Checks SAH_HTTP_PORT and SWISSARMYHAMMER_HTTP_PORT environment variables,
/// falling back to DEFAULT_HTTP_PORT if not set.
fn get_default_http_port() -> String {
    get_default_config(
        "SAH_HTTP_PORT",
        "SWISSARMYHAMMER_HTTP_PORT",
        DEFAULT_HTTP_PORT,
    )
}

/// Get the default HTTP host from environment or use fallback
///
/// Checks SAH_HTTP_HOST and SWISSARMYHAMMER_HTTP_HOST environment variables,
/// falling back to DEFAULT_HTTP_HOST if not set.
fn get_default_http_host() -> String {
    get_default_config(
        "SAH_HTTP_HOST",
        "SWISSARMYHAMMER_HTTP_HOST",
        DEFAULT_HTTP_HOST,
    )
}

/// Global string cache for interning strings to satisfy clap's 'static lifetime requirement.
///
/// # Design Trade-off
///
/// This uses `Box::leak` to create 'static string references, which intentionally leaks memory.
/// This is an acceptable trade-off because:
///
/// 1. **Clap Requirement**: Clap requires 'static lifetimes for command/arg names and help text
/// 2. **Bounded Growth**: The cache only grows with unique CLI commands/args, not unbounded
/// 3. **One-time Cost**: Strings are interned once at CLI build time, not per-invocation
/// 4. **Deduplication**: The HashSet ensures each unique string is only leaked once
///
/// Alternative approaches considered:
/// - `Arc<str>`: Cannot satisfy 'static lifetime requirement without unsafe transmutation
/// - `string-interner` crate: Adds dependency and still requires similar memory management
/// - Regenerating on each CLI build: Would require complex lifetime management across registry
///
/// The memory footprint is negligible (typically <1KB for all CLI metadata) and acceptable
/// for a CLI application that runs once per invocation.
static STRING_CACHE: Lazy<Mutex<HashSet<&'static str>>> = Lazy::new(|| Mutex::new(HashSet::new()));

/// Check if a JSON schema contains a specific type name.
///
/// Handles both string type specifications (e.g., `"type": "string"`)
/// and array type specifications (e.g., `"type": ["string", "null"]`).
///
/// # Arguments
///
/// * `schema` - The JSON schema to check
/// * `type_name` - The type name to search for (e.g., "string", "boolean")
///
/// # Returns
///
/// `true` if the schema contains the specified type, `false` otherwise
pub fn schema_has_type(schema: &Value, type_name: &str) -> bool {
    match schema.get("type") {
        Some(Value::String(t)) => t.as_str() == type_name,
        Some(Value::Array(types)) => types.iter().any(|t| t.as_str() == Some(type_name)),
        _ => false,
    }
}

/// Intern a string into the global cache, returning a 'static reference.
///
/// This ensures each unique string is only leaked once, preventing unbounded
/// memory growth while satisfying clap's requirement for 'static string lifetimes.
///
/// # Arguments
///
/// * `s` - The string to intern
///
/// # Returns
///
/// A 'static reference to the interned string
///
/// # Thread Safety
///
/// This function is thread-safe and uses a mutex-protected cache
pub fn intern_string(s: String) -> &'static str {
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
    /// Subcommands for operation-based tools (verb-noun operations)
    subcommands: Vec<CommandData>,
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

/// Specification for building an argument declaratively
#[derive(Clone)]
struct ArgSpec {
    name: &'static str,
    long: Option<&'static str>,
    short: Option<char>,
    help: &'static str,
    action: ArgSpecAction,
    value_name: Option<&'static str>,
    default_value: Option<String>,
    value_parser: Option<ArgSpecValueParser>,
    hide: bool,
    required: bool,
}

/// Action type for argument specification
#[derive(Clone)]
enum ArgSpecAction {
    Set,
    SetTrue,
    Append,
}

/// Value parser type for argument specification
#[derive(Clone)]
enum ArgSpecValueParser {
    Strings(Vec<&'static str>),
    U16,
    U64,
    Usize,
}

impl ArgSpec {
    /// Create a new argument specification with minimal required fields
    fn new(name: &'static str, help: &'static str) -> Self {
        Self {
            name,
            long: None,
            short: None,
            help,
            action: ArgSpecAction::Set,
            value_name: None,
            default_value: None,
            value_parser: None,
            hide: false,
            required: false,
        }
    }

    /// Set the long flag name
    fn long(mut self, long: &'static str) -> Self {
        self.long = Some(long);
        self
    }

    /// Set the short flag character
    fn short(mut self, short: char) -> Self {
        self.short = Some(short);
        self
    }

    /// Set the action type
    fn action(mut self, action: ArgSpecAction) -> Self {
        self.action = action;
        self
    }

    /// Set the value name
    fn value_name(mut self, value_name: &'static str) -> Self {
        self.value_name = Some(value_name);
        self
    }

    /// Set the default value
    fn default_value(mut self, default_value: String) -> Self {
        self.default_value = Some(default_value);
        self
    }

    /// Set the value parser
    fn value_parser(mut self, parser: ArgSpecValueParser) -> Self {
        self.value_parser = Some(parser);
        self
    }

    /// Set whether to hide the argument
    fn hide(mut self, hide: bool) -> Self {
        self.hide = hide;
        self
    }

    /// Set whether the argument is required
    fn required(mut self, required: bool) -> Self {
        self.required = required;
        self
    }

    // Note: Builder methods intentionally use explicit implementations rather than macros
    // to maintain clarity and allow for future per-method customization without complexity.

    /// Build a clap Arg from this specification
    fn build(self) -> Arg {
        let mut arg = Arg::new(self.name).help(self.help);

        if let Some(long) = self.long {
            arg = arg.long(long);
        }

        if let Some(short) = self.short {
            arg = arg.short(short);
        }

        arg = match self.action {
            ArgSpecAction::Set => arg,
            ArgSpecAction::SetTrue => arg.action(ArgAction::SetTrue),
            ArgSpecAction::Append => arg.action(ArgAction::Append),
        };

        if let Some(value_name) = self.value_name {
            arg = arg.value_name(value_name);
        }

        if let Some(default_value) = self.default_value {
            arg = arg.default_value(intern_string(default_value));
        }

        if let Some(parser) = self.value_parser {
            arg = match parser {
                ArgSpecValueParser::Strings(values) => {
                    arg.value_parser(clap::builder::PossibleValuesParser::new(values))
                }
                ArgSpecValueParser::U16 => arg.value_parser(clap::value_parser!(u16)),
                ArgSpecValueParser::U64 => arg.value_parser(clap::value_parser!(u64)),
                ArgSpecValueParser::Usize => arg.value_parser(clap::value_parser!(usize)),
            };
        }

        if self.hide {
            arg = arg.hide(true);
        }

        if self.required {
            arg = arg.required(true);
        }

        arg
    }
}

/// Specification for building a subcommand declaratively
struct SubcommandSpec {
    name: &'static str,
    about: &'static str,
    long_about: Option<&'static str>,
    args: Vec<ArgSpec>,
}

impl SubcommandSpec {
    /// Create a new subcommand specification
    fn new(name: &'static str, about: &'static str) -> Self {
        Self {
            name,
            about,
            long_about: None,
            args: Vec::new(),
        }
    }

    /// Set the long about text
    fn long_about(mut self, long_about: &'static str) -> Self {
        self.long_about = Some(long_about);
        self
    }

    /// Set the argument specifications
    fn args(mut self, args: Vec<ArgSpec>) -> Self {
        self.args = args;
        self
    }

    /// Build a clap Command from this specification
    fn build(self) -> Command {
        let mut cmd = Command::new(self.name).about(self.about);

        if let Some(long_about) = self.long_about {
            cmd = cmd.long_about(long_about);
        }

        for arg_spec in self.args {
            cmd = cmd.arg(arg_spec.build());
        }

        cmd
    }
}

/// Schema parser for extracting fields from JSON schemas
///
/// Encapsulates schema field extraction logic to reduce cognitive complexity
/// in argument preprocessing functions.
struct SchemaParser;

impl SchemaParser {
    /// Extract a string field from a JSON schema
    fn parse_string(schema: &Value, field: &str) -> Option<String> {
        schema.get(field).and_then(|v| v.as_str()).map(String::from)
    }

    /// Extract enum values from a JSON schema
    fn parse_enum(schema: &Value) -> Option<Vec<String>> {
        schema.get("enum").and_then(|v| v.as_array()).map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
    }

    /// Extract the description from a schema
    fn parse_description(schema: &Value) -> Option<String> {
        Self::parse_string(schema, "description")
    }

    /// Extract the default value from a schema
    fn parse_default(schema: &Value) -> Option<String> {
        Self::parse_string(schema, "default")
    }

    /// Parse argument data from a schema
    fn parse_arg_data(name: &str, schema: &Value, is_required: bool) -> ArgData {
        ArgData {
            name: name.to_string(),
            help: Self::parse_description(schema),
            is_required,
            arg_type: Self::parse_type(schema),
            default_value: Self::parse_default(schema),
            possible_values: Self::parse_enum(schema),
        }
    }

    /// Determine the argument type from a JSON schema
    fn parse_type(schema: &Value) -> ArgType {
        match (
            Self::is_nullable_boolean(schema),
            Self::get_primary_type(schema),
        ) {
            (true, _) => ArgType::NullableBoolean,
            (false, Some("boolean")) => ArgType::Boolean,
            (false, Some("integer")) => ArgType::Integer,
            (false, Some("number")) => ArgType::Float,
            (false, Some("array")) => ArgType::Array,
            _ => ArgType::String,
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
}

/// Tool validator for checking CLI compatibility
///
/// Encapsulates validation logic with clean error propagation using the ? operator.
struct ToolValidator<'a> {
    tool: &'a dyn McpTool,
}

impl<'a> ToolValidator<'a> {
    fn new(tool: &'a dyn McpTool) -> Self {
        Self { tool }
    }

    /// Validate all aspects of the tool
    fn validate(&self) -> Result<(), Vec<ValidationError>> {
        let mut errors = Vec::new();

        if let Err(e) = self.check_schema() {
            errors.push(e);
        }

        if !self.tool.hidden_from_cli() {
            if let Err(e) = self.check_cli_category() {
                errors.push(e);
            }
            if let Err(e) = self.check_cli_name() {
                errors.push(e);
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    /// Validate the tool's JSON schema
    fn check_schema(&self) -> Result<(), ValidationError> {
        let schema = self.tool.schema();
        SchemaValidator::validate_schema(&schema)
    }

    /// Check that CLI category is present
    fn check_cli_category(&self) -> Result<(), ValidationError> {
        match self.tool.cli_category() {
            Some(_) => Ok(()),
            None => Err(ValidationError::MissingSchemaField {
                field: format!(
                    "CLI category for tool {}",
                    <dyn McpTool as McpTool>::name(self.tool)
                ),
            }),
        }
    }

    /// Check that CLI name is valid
    fn check_cli_name(&self) -> Result<(), ValidationError> {
        let cli_name = self.tool.cli_name();
        if cli_name.is_empty() {
            Err(ValidationError::InvalidParameterName {
                parameter: <dyn McpTool as McpTool>::name(self.tool).to_string(),
                reason: "CLI name cannot be empty".to_string(),
            })
        } else {
            Ok(())
        }
    }
}

/// Builder for constructing clap arguments from ArgData
///
/// Provides an explicit, testable pipeline for argument construction.
struct ArgBuilder {
    arg: Arg,
}

impl ArgBuilder {
    /// Create a new ArgBuilder from ArgData, fully configured
    fn new(arg_data: &ArgData) -> Self {
        let name_static = intern_string(arg_data.name.clone());
        let mut arg = Arg::new(name_static).long(name_static);

        // Apply required flag
        if arg_data.is_required {
            arg = arg.required(true);
        }

        // Apply help text
        if let Some(help) = &arg_data.help {
            arg = arg.help(intern_string(help.clone()));
        }

        // Apply default value
        if let Some(default) = &arg_data.default_value {
            arg = arg.default_value(intern_string(default.clone()));
        }

        // Apply possible values (enum)
        if let Some(values) = &arg_data.possible_values {
            let str_values: Vec<&'static str> =
                values.iter().map(|s| intern_string(s.clone())).collect();
            arg = arg.value_parser(clap::builder::PossibleValuesParser::new(str_values));
        }

        // Apply type-specific configuration
        arg = match arg_data.arg_type {
            ArgType::Boolean => arg.action(ArgAction::SetTrue),
            ArgType::NullableBoolean => arg
                .value_parser(clap::builder::PossibleValuesParser::new(["true", "false"]))
                .value_name("BOOL"),
            ArgType::Integer => {
                let mut a = arg.value_parser(clap::value_parser!(i64));
                if !arg_data.is_required {
                    a = a.value_name("NUMBER");
                }
                a
            }
            ArgType::Float => {
                let mut a = arg.value_parser(clap::value_parser!(f64));
                if !arg_data.is_required {
                    a = a.value_name("NUMBER");
                }
                a
            }
            ArgType::Array => {
                let mut a = arg.action(ArgAction::Append);
                if !arg_data.is_required {
                    a = a.value_name("VALUE");
                }
                a
            }
            ArgType::String => {
                if !arg_data.is_required {
                    arg.value_name("TEXT")
                } else {
                    arg
                }
            }
        };

        Self { arg }
    }

    /// Build the final Arg
    fn build(self) -> Arg {
        self.arg
    }
}

/// Processor for workflow parameters
///
/// Handles the conversion of workflow parameter definitions into clap arguments.
/// Separates the concern of parameter processing from command construction.
struct WorkflowParameterProcessor;

impl WorkflowParameterProcessor {
    /// Add positional arguments for required workflow parameters
    ///
    /// Extracts required parameters and creates a multi-value positional argument.
    fn add_required_positional_args(
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

    /// Add the --param flag for optional workflow parameters
    ///
    /// Creates a repeatable flag for key=value parameter pairs.
    fn add_optional_param_flag(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("param")
                .long("param")
                .short('p')
                .action(ArgAction::Append)
                .value_name("KEY=VALUE")
                .help("Optional workflow parameter"),
        )
    }

    /// Apply all parameter processing to a command
    ///
    /// Adds both required positional arguments and optional --param flag.
    fn process_parameters(cmd: Command, workflow: &swissarmyhammer_workflow::Workflow) -> Command {
        let cmd = Self::add_required_positional_args(cmd, workflow);
        Self::add_optional_param_flag(cmd)
    }
}

/// Resolver for workflow command name conflicts
///
/// Ensures workflow shortcuts don't conflict with built-in commands by
/// automatically prefixing conflicting names with an underscore.
struct WorkflowCommandNameResolver;

impl WorkflowCommandNameResolver {
    /// Get reserved command names from actual static commands
    ///
    /// Dynamically generates the list of reserved names from the static commands,
    /// ensuring the list stays in sync with the actual CLI structure.
    fn get_reserved_names() -> Vec<String> {
        // Build a temporary CLI with static commands to extract their names
        let temp_cli = CliBuilder::add_static_commands(Command::new("temp"));

        let mut reserved = Vec::new();
        for subcommand in temp_cli.get_subcommands() {
            reserved.push(subcommand.get_name().to_string());
        }

        // Add special flow subcommands that shouldn't conflict
        reserved.push("list".to_string());

        reserved
    }

    /// Resolve command name conflicts by prefixing with underscore if reserved
    ///
    /// # Examples
    ///
    /// - "list" -> "_list" (conflicts with flow list)
    /// - "serve" -> "_serve" (conflicts with serve command)
    /// - "deploy" -> "deploy" (no conflict)
    fn resolve(workflow_name: &str) -> String {
        let reserved_names = Self::get_reserved_names();
        if reserved_names.iter().any(|name| name == workflow_name) {
            format!("_{}", workflow_name)
        } else {
            workflow_name.to_string()
        }
    }
}

impl CliBuilder {
    /// Build a vector of Args from argument specifications
    ///
    /// Consolidates the pattern of creating Vec<Arg> and building each arg
    fn build_args_from_specs(specs: &[ArgSpec]) -> Vec<Arg> {
        specs.iter().map(|spec| spec.clone().build()).collect()
    }

    /// Build a command with subcommands from a specification
    ///
    /// Consolidates the pattern of creating a command with docs and adding multiple subcommands
    fn build_command_with_subcommands(config: CommandConfig, subcommands: Vec<Command>) -> Command {
        let mut cmd = Self::build_command_with_docs(config);
        for subcommand in subcommands {
            cmd = cmd.subcommand(subcommand);
        }
        cmd
    }

    /// Build subcommands from declarative specifications
    ///
    /// Consolidates the pattern of building multiple similar subcommands
    fn build_subcommands_from_specs(specs: &[SubcommandSpec]) -> Vec<Command> {
        specs.iter().map(|spec| spec.clone().build()).collect()
    }
}

impl Clone for SubcommandSpec {
    fn clone(&self) -> Self {
        Self {
            name: self.name,
            about: self.about,
            long_about: self.long_about,
            args: self.args.clone(),
        }
    }
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
  workflows - Model for workflow execution

Examples:
  sah model use claude-code               # Set root model
  sah model use workflows claude-code     # Use Claude for workflows
";

/// Statistics about CLI tool validation results
#[derive(Debug, Clone, Default)]
pub struct CliValidationStats {
    /// Total number of tools checked
    pub total_tools: usize,
    /// Number of tools that passed validation
    pub valid_tools: usize,
    /// Number of tools that failed validation
    pub invalid_tools: usize,
    /// Total number of validation errors found
    pub validation_errors: usize,
}

impl CliValidationStats {
    /// Create a new `CliValidationStats` with all counters initialized to zero
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

    /// Calculate the success rate as a percentage
    ///
    /// # Returns
    ///
    /// The percentage of valid tools (0.0 to 100.0). Returns 100.0 if no tools were checked.
    pub fn success_rate(&self) -> f64 {
        if self.has_no_tools() {
            PERCENTAGE_MULTIPLIER
        } else {
            (self.valid_tools as f64 / self.total_tools as f64) * PERCENTAGE_MULTIPLIER
        }
    }

    /// Generate a human-readable summary of validation results
    ///
    /// # Returns
    ///
    /// A formatted string with validation statistics, using colored output
    /// for success (green ✓) or warnings (yellow ⚠)
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
}

/// Dynamic CLI builder that generates commands from MCP tool registry
///
/// Generates CLI commands dynamically from the tool registry at build time.
pub struct CliBuilder {
    /// Shared reference to the tool registry
    tool_registry: Arc<RwLock<ToolRegistry>>,
}

impl CliBuilder {
    /// Create a new CLI builder with the given tool registry
    pub fn new(tool_registry: Arc<RwLock<ToolRegistry>>) -> Self {
        Self { tool_registry }
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

    /// Pre-compute command data for a tool with validation
    ///
    /// # Validation Flow
    ///
    /// This function is the entry point for converting MCP tools into CLI commands.
    /// The validation and command creation follows this chain:
    ///
    /// 1. `precompute_tool_command` - Entry point, orchestrates validation and creation
    /// 2. `validate_tool_schema_for_cli` - Validates schema structure using SchemaValidator
    /// 3. `create_command_data_from_tool` - Creates CommandData from validated tool
    /// 4. `precompute_args` - Extracts argument data from schema properties
    /// 5. `SchemaParser::parse_arg_data` - Parses individual argument from schema
    ///
    /// Early returns are used throughout to skip invalid tools gracefully rather than
    /// failing the entire CLI build. Invalid tools are logged and skipped.
    fn precompute_tool_command(tool: &dyn McpTool) -> Option<CommandData> {
        let schema = tool.schema();

        // Early return if schema validation fails
        if !Self::validate_tool_schema_for_cli(tool, &schema) {
            return None;
        }

        Self::create_command_data_from_tool(tool, &schema)
    }

    /// Validate tool schema for CLI integration
    ///
    /// Early return on validation failure with warning log.
    fn validate_tool_schema_for_cli(tool: &dyn McpTool, schema: &Value) -> bool {
        if let Err(validation_error) = SchemaValidator::validate_schema(schema) {
            tracing::warn!(
                "Skipping tool '{}' from CLI due to schema validation error: {}",
                <dyn McpTool as McpTool>::name(tool),
                validation_error
            );
            return false;
        }
        true
    }

    /// Create command data from validated tool
    fn create_command_data_from_tool(tool: &dyn McpTool, schema: &Value) -> Option<CommandData> {
        let operations = tool.operations();

        // If tool has operations, create noun-grouped subcommands
        // Structure: tool -> noun -> verb (e.g., kanban -> board -> init)
        if !operations.is_empty() {
            // For operation-based tools, use schema args for each verb subcommand
            let schema_args = Self::precompute_args(schema);

            // Group operations by noun
            let mut noun_groups: HashMap<&str, Vec<&dyn Operation>> = HashMap::new();
            for op in operations {
                noun_groups.entry(op.noun()).or_default().push(*op);
            }

            // Create noun subcommands, each containing verb subcommands
            let mut subcommands: Vec<CommandData> = noun_groups
                .into_iter()
                .map(|(noun, ops)| Self::create_noun_command_data(noun, ops, &schema_args))
                .collect();

            // Sort by noun name for consistent ordering
            subcommands.sort_by(|a, b| a.name.cmp(&b.name));

            Some(CommandData {
                name: tool.cli_name().to_string(),
                about: tool.cli_about().map(|s| s.to_string()),
                long_about: Some(tool.description().to_string()),
                args: Vec::new(), // No direct args - use noun subcommands
                subcommands,
            })
        } else {
            // Non-operation-based tool - use schema for args
            Some(CommandData {
                name: tool.cli_name().to_string(),
                about: tool.cli_about().map(|s| s.to_string()),
                long_about: Some(tool.description().to_string()),
                args: Self::precompute_args(schema),
                subcommands: Vec::new(),
            })
        }
    }

    /// Create command data for a noun grouping (e.g., "board", "task", "column")
    fn create_noun_command_data(
        noun: &str,
        ops: Vec<&dyn Operation>,
        schema_args: &[ArgData],
    ) -> CommandData {
        let mut verb_subcommands: Vec<CommandData> = ops
            .into_iter()
            .map(|op| Self::create_verb_command_data(op, schema_args))
            .collect();

        // Sort by verb name for consistent ordering
        verb_subcommands.sort_by(|a, b| a.name.cmp(&b.name));

        CommandData {
            name: noun.to_string(),
            about: Some(format!("{} operations", noun)),
            long_about: None,
            args: Vec::new(),
            subcommands: verb_subcommands,
        }
    }

    /// Create command data for a verb (e.g., "init", "add", "move")
    fn create_verb_command_data(op: &dyn Operation, schema_args: &[ArgData]) -> CommandData {
        // Use just the verb as the subcommand name
        let verb_name = op.verb().to_string();

        // Use schema args (excluding "op" since that's set by the noun+verb path)
        let args: Vec<ArgData> = schema_args
            .iter()
            .filter(|arg| arg.name != "op")
            .cloned()
            .collect();

        CommandData {
            name: verb_name,
            about: Some(op.description().to_string()),
            long_about: None,
            args,
            subcommands: Vec::new(),
        }
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

    /// Pre-compute data for a single argument
    fn precompute_arg_data(name: &str, schema: &Value, is_required: bool) -> ArgData {
        SchemaParser::parse_arg_data(name, schema, is_required)
    }

    /// Build the complete CLI with dynamic commands generated from MCP tools
    ///
    /// # Parameters
    ///
    /// * `workflow_storage` - Optional workflow storage for generating shortcut commands.
    ///   If None, shortcuts will not be generated.
    pub fn build_cli(&self, workflow_storage: Option<&WorkflowStorage>) -> Command {
        let cli = Self::build_base_cli();
        let cli = Self::add_core_commands(cli);
        let cli = self.build_with_workflow_shortcuts(cli, workflow_storage);
        // All tools are now accessible via unified `sah tool <toolname>` command
        self.add_unified_tool_command(cli)
    }

    /// Build CLI with workflow shortcuts integrated
    ///
    /// Extracts the workflow shortcuts logic to reduce complexity in build_cli.
    /// This method encapsulates the conditional workflow shortcut generation.
    ///
    /// # Parameters
    ///
    /// * `cli` - Base CLI command to extend
    /// * `workflow_storage` - Optional workflow storage for generating shortcut commands
    ///
    /// # Returns
    ///
    /// CLI command with workflow shortcuts added (if storage was provided)
    fn build_with_workflow_shortcuts(
        &self,
        cli: Command,
        workflow_storage: Option<&WorkflowStorage>,
    ) -> Command {
        match workflow_storage {
            Some(storage) => self.add_workflow_shortcuts_from_storage(cli, storage),
            None => cli,
        }
    }

    /// Add workflow shortcuts from storage to CLI
    fn add_workflow_shortcuts_from_storage(
        &self,
        mut cli: Command,
        storage: &WorkflowStorage,
    ) -> Command {
        let shortcuts = Self::get_sorted_workflow_shortcuts(storage);
        for shortcut in shortcuts {
            cli = cli.subcommand(shortcut);
        }
        cli
    }

    /// Add core static commands to the CLI
    fn add_core_commands(cli: Command) -> Command {
        Self::add_static_commands(cli)
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
        let cmd = Self::add_output_control_args(cmd);
        Self::add_runtime_control_args(cmd)
    }

    /// Add output control arguments (logging and formatting)
    fn add_output_control_args(cmd: Command) -> Command {
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
        .arg(Self::create_flag_arg(
            "quiet",
            "quiet",
            Some('q'),
            "Suppress all output except errors",
        ))
        .arg(
            Arg::new("format")
                .long("format")
                .help("Global output format")
                .value_parser(["table", "json", "yaml"]),
        )
    }

    /// Add runtime control arguments (cwd, model, validate-tools)
    fn add_runtime_control_args(cmd: Command) -> Command {
        cmd.arg(
            Arg::new("cwd")
                .long("cwd")
                .help("Set working directory before executing command")
                .value_name("PATH")
                .global(true)
                .value_parser(clap::value_parser!(std::path::PathBuf)),
        )
        .arg(Self::create_flag_arg(
            "validate-tools",
            "validate-tools",
            None,
            "Validate all tool schemas and exit",
        ))
        .arg(
            Arg::new("model")
                .long("model")
                .help("Override model for all use cases (runtime only, doesn't modify config)")
                .value_name("MODEL")
                .global(true),
        )
    }

    /// Get sorted workflow shortcuts
    fn get_sorted_workflow_shortcuts(storage: &WorkflowStorage) -> Vec<Command> {
        let mut shortcuts = Self::build_workflow_shortcuts(storage);
        shortcuts.sort_by(|a, b| a.get_name().cmp(b.get_name()));
        shortcuts
    }

    /// Add unified tool command with all MCP tools as subcommands
    ///
    /// Creates `sah tool <toolname>` command that provides access to all registered MCP tools.
    /// Tools are listed by their full MCP name with underscores converted to hyphens
    /// (e.g., web-search, treesitter-search, files-read).
    fn add_unified_tool_command(&self, cli: Command) -> Command {
        let mut tool_cmd = Command::new("tool")
            .about("Execute any registered MCP tool directly")
            .long_about(
                "Access all registered MCP tools via unified command interface.\n\n\
                 Use 'sah tool --help' to see all available tools.\n\
                 Use 'sah tool <name> --help' for tool-specific help.",
            );

        // Get registry and build subcommands directly from tools
        let registry = self
            .tool_registry
            .try_read()
            .expect("ToolRegistry should not be locked");

        // Collect all tools with their full names
        let mut all_tools: Vec<_> = Vec::new();
        for category in registry.get_cli_categories() {
            for tool in registry.get_tools_for_category(&category) {
                if tool.hidden_from_cli() {
                    continue;
                }
                // Use full tool name as-is (e.g., web_search, treesitter_search)
                let cli_tool_name =
                    <dyn McpTool as McpTool>::name(tool).to_string();
                if let Some(tool_data) = Self::precompute_tool_command(tool) {
                    all_tools.push((cli_tool_name, tool_data));
                }
            }
        }

        // Sort by name for consistent ordering
        all_tools.sort_by(|a, b| a.0.cmp(&b.0));

        for (tool_name, tool_data) in all_tools {
            let subcmd = Self::build_tool_subcommand_from_data(&tool_name, &tool_data);
            tool_cmd = tool_cmd.subcommand(subcmd);
        }

        cli.subcommand(tool_cmd)
    }

    /// Build a tool subcommand from tool data
    fn build_tool_subcommand_from_data(tool_name: &str, tool_data: &CommandData) -> Command {
        let mut cmd = Command::new(intern_string(tool_name.to_string()));

        if let Some(ref about) = tool_data.about {
            cmd = cmd.about(intern_string(about.clone()));
        }

        if let Some(ref long_about) = tool_data.long_about {
            cmd = cmd.long_about(intern_string(long_about.clone()));
        }

        // Recursively add subcommands (handles noun -> verb nesting)
        for subcmd_data in &tool_data.subcommands {
            let subcmd = Self::build_command_from_data(subcmd_data);
            cmd = cmd.subcommand(subcmd);
        }

        // Add direct args (for leaf commands or non-operation tools)
        for arg_data in &tool_data.args {
            cmd = cmd.arg(ArgBuilder::new(arg_data).build());
        }

        cmd
    }

    /// Build a command recursively from CommandData
    fn build_command_from_data(data: &CommandData) -> Command {
        let mut cmd = Command::new(intern_string(data.name.clone()));

        if let Some(ref about) = data.about {
            cmd = cmd.about(intern_string(about.clone()));
        }

        if let Some(ref long_about) = data.long_about {
            cmd = cmd.long_about(intern_string(long_about.clone()));
        }

        // Recursively add subcommands
        for subcmd_data in &data.subcommands {
            cmd = cmd.subcommand(Self::build_command_from_data(subcmd_data));
        }

        // Add args
        for arg_data in &data.args {
            cmd = cmd.arg(ArgBuilder::new(arg_data).build());
        }

        cmd
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
        self.collect_all_errors()
    }

    /// Collect all validation errors from tools
    ///
    /// Concrete helper method that uses fold_validation_results internally
    /// to collect all errors across all tools.
    ///
    /// # Returns
    ///
    /// Vec of all validation errors found
    fn collect_all_errors(&self) -> Vec<ValidationError> {
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

        let validation_results = self.collect_validation_results(&registry);
        validation_results.into_iter().fold(init, folder)
    }

    /// Collect validation results for all tools
    fn collect_validation_results(
        &self,
        registry: &ToolRegistry,
    ) -> Vec<Result<(), Vec<ValidationError>>> {
        let mut results = Vec::new();
        Self::iter_all_tools(registry, |tool| {
            results.push(self.validate_single_tool(tool));
        });
        results
    }

    /// Validate a single tool for CLI compatibility
    fn validate_single_tool(&self, tool: &dyn McpTool) -> Result<(), Vec<ValidationError>> {
        ToolValidator::new(tool).validate()
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
            subcommands: Vec::new(),
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
            Self::create_http_server_args(),
        )
    }

    /// Create HTTP server arguments
    fn create_http_server_args() -> Vec<Arg> {
        let default_port = get_default_http_port();
        let default_host = get_default_http_host();

        Self::build_args_from_specs(&[
            ArgSpec::new(
                "port",
                "Port to bind to (use 0 for random port, configurable via SAH_HTTP_PORT env var)",
            )
            .long("port")
            .short('p')
            .default_value(default_port)
            .value_parser(ArgSpecValueParser::U16),
            ArgSpec::new(
                "host",
                "Host to bind to (configurable via SAH_HTTP_HOST env var)",
            )
            .long("host")
            .short('H')
            .default_value(default_host),
        ])
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
        let mut args = Self::create_validate_output_args();
        args.extend(Self::create_validate_deprecated_args());
        args
    }

    /// Create output control arguments for validate command
    fn create_validate_output_args() -> Vec<Arg> {
        Self::build_args_from_specs(&[
            ArgSpec::new("quiet", "Suppress all output except errors")
                .short('q')
                .long("quiet")
                .action(ArgSpecAction::SetTrue),
            ArgSpec::new("format", "Output format")
                .long("format")
                .value_parser(ArgSpecValueParser::Strings(vec!["text", "json"]))
                .default_value("text".to_string()),
            ArgSpec::new(
                "validate-tools",
                "Validate MCP tool schemas for CLI compatibility",
            )
            .long("validate-tools")
            .action(ArgSpecAction::SetTrue),
        ])
    }

    /// Create deprecated arguments for validate command
    fn create_validate_deprecated_args() -> Vec<Arg> {
        Self::build_args_from_specs(&[
            ArgSpec::new("workflow-dirs", "[DEPRECATED] This parameter is ignored. Workflows are now only loaded from standard locations.")
                .long("workflow-dir")
                .action(ArgSpecAction::Append)
                .hide(true),
        ])
    }

    /// Add static commands to the CLI
    ///
    /// Commands are organized into semantic groups for maintainability:
    /// - Server commands: serve, doctor, validate
    /// - Content commands: prompt, model
    /// - Workflow commands: flow
    fn add_static_commands(cli: Command) -> Command {
        let cli = Self::add_server_commands(cli);
        let cli = Self::add_content_commands(cli);
        Self::add_workflow_commands(cli)
    }

    /// Add server-related commands (serve, doctor, validate)
    fn add_server_commands(cli: Command) -> Command {
        cli.subcommand(Self::build_serve_command())
            .subcommand(Self::build_doctor_command())
            .subcommand(Self::build_validate_command())
    }

    /// Add content management commands (prompt, model, agent)
    fn add_content_commands(cli: Command) -> Command {
        cli.subcommand(Self::build_prompt_command())
            .subcommand(Self::build_model_command())
            .subcommand(Self::build_agent_command())
    }

    /// Add workflow-related commands (flow)
    fn add_workflow_commands(cli: Command) -> Command {
        cli.subcommand(Self::build_flow_command())
    }

    /// Build the prompt command with all its subcommands
    fn build_prompt_command() -> Command {
        Self::build_command_with_subcommands(
            CommandConfig {
                name: "prompt",
                about: "Manage and test prompts",
                long_about: PROMPT_COMMAND_LONG_ABOUT,
            },
            vec![
                Self::build_prompt_list_subcommand(),
                Self::build_prompt_test_subcommand(),
                Self::build_prompt_validate_subcommand(),
            ],
        )
    }

    /// Build the prompt list subcommand
    fn build_prompt_list_subcommand() -> Command {
        Command::new("list")
            .about("Display all available prompts from all sources")
            .long_about(PROMPT_LIST_LONG_ABOUT)
    }

    /// Build the prompt test subcommand
    fn build_prompt_test_subcommand() -> Command {
        let mut cmd = Command::new("test")
            .about("Test prompts interactively with sample arguments")
            .long_about(PROMPT_TEST_LONG_ABOUT);

        for arg in Self::create_test_input_args() {
            cmd = cmd.arg(arg);
        }
        for arg in Self::create_test_output_args() {
            cmd = cmd.arg(arg);
        }
        for arg in Self::create_test_debug_args() {
            cmd = cmd.arg(arg);
        }

        cmd
    }

    /// Create test input arguments (prompt_name, file, vars)
    fn create_test_input_args() -> Vec<Arg> {
        Self::build_args_from_specs(&[
            ArgSpec::new("prompt_name", "Prompt name to test").value_name("PROMPT_NAME"),
            ArgSpec::new("file", "Path to prompt file to test")
                .short('f')
                .long("file")
                .value_name("FILE"),
            ArgSpec::new("vars", "Variables as key=value pairs")
                .long("var")
                .value_name("KEY=VALUE")
                .action(ArgSpecAction::Append),
        ])
    }

    /// Create test output arguments (raw, copy, save)
    fn create_test_output_args() -> Vec<Arg> {
        Self::build_args_from_specs(&[
            ArgSpec::new("raw", "Show raw output without formatting")
                .long("raw")
                .action(ArgSpecAction::SetTrue),
            ArgSpec::new("copy", "Copy rendered prompt to clipboard")
                .long("copy")
                .action(ArgSpecAction::SetTrue),
            ArgSpec::new("save", "Save rendered prompt to file")
                .long("save")
                .value_name("FILE"),
        ])
    }

    /// Create test debug arguments (debug)
    fn create_test_debug_args() -> Vec<Arg> {
        Self::build_args_from_specs(&[ArgSpec::new("debug", "Show debug information")
            .long("debug")
            .action(ArgSpecAction::SetTrue)])
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
    ///
    /// Creates the 'model' command with subcommands for showing, listing,
    /// and using models for different use cases.
    ///
    /// # Returns
    ///
    /// A configured `Command` for model management
    pub fn build_model_command() -> Command {
        let format_arg = ArgSpec::new("format", "Output format")
            .long("format")
            .value_parser(ArgSpecValueParser::Strings(vec!["table", "json", "yaml"]))
            .default_value("table".to_string());

        let subcommand_specs = vec![
            SubcommandSpec::new("show", "Show current model use case assignments")
                .args(vec![format_arg.clone()]),
            SubcommandSpec::new("list", "List available models").args(vec![format_arg]),
            SubcommandSpec::new("use", "Use a specific model for a use case")
                .long_about(MODEL_USE_LONG_ABOUT)
                .args(vec![
                    ArgSpec::new("first", "Model name OR use case (root, workflows)")
                        .value_name("FIRST")
                        .required(true),
                    ArgSpec::new(
                        "second",
                        "Model name (required when first argument is a use case)",
                    )
                    .value_name("SECOND"),
                ]),
        ];

        Self::build_command_with_subcommands(
            CommandConfig {
                name: "model",
                about: "Manage and interact with models",
                long_about: MODEL_COMMAND_LONG_ABOUT,
            },
            Self::build_subcommands_from_specs(&subcommand_specs),
        )
    }

    /// Build the agent command with ACP subcommand
    ///
    /// Creates the agent command for managing Agent Client Protocol (ACP) server integration.
    ///
    /// # Returns
    ///
    /// A configured `Command` for agent management
    pub fn build_agent_command() -> Command {
        let config_arg = ArgSpec::new("config", "Path to ACP configuration file")
            .long("config")
            .short('c')
            .value_name("FILE");

        let permission_policy_arg = ArgSpec::new(
            "permission_policy",
            "Permission policy: always-ask, auto-approve-reads",
        )
        .long("permission-policy")
        .value_name("POLICY");

        let allow_path_arg = ArgSpec::new(
            "allow_path",
            "Allowed filesystem paths (can be specified multiple times)",
        )
        .long("allow-path")
        .value_name("PATH")
        .action(ArgSpecAction::Append);

        let block_path_arg = ArgSpec::new(
            "block_path",
            "Blocked filesystem paths (can be specified multiple times)",
        )
        .long("block-path")
        .value_name("PATH")
        .action(ArgSpecAction::Append);

        let max_file_size_arg = ArgSpec::new(
            "max_file_size",
            "Maximum file size for read operations in bytes",
        )
        .long("max-file-size")
        .value_name("BYTES")
        .value_parser(ArgSpecValueParser::U64);

        let terminal_buffer_size_arg = ArgSpec::new(
            "terminal_buffer_size",
            "Terminal output buffer size in bytes",
        )
        .long("terminal-buffer-size")
        .value_name("BYTES")
        .value_parser(ArgSpecValueParser::Usize);

        let graceful_shutdown_timeout_arg = ArgSpec::new(
            "graceful_shutdown_timeout",
            "Graceful shutdown timeout in seconds",
        )
        .long("graceful-shutdown-timeout")
        .value_name("SECONDS")
        .value_parser(ArgSpecValueParser::U64);

        let subcommand_specs =
            vec![
                SubcommandSpec::new("acp", "Start ACP server over stdio for editor integration")
                    .long_about(
                        "Start Agent Client Protocol (ACP) server for code editor integration.\n\n\
             The ACP server enables SwissArmyHammer to work with ACP-compatible code editors\n\
             like Zed and JetBrains IDEs. The server communicates over stdin/stdout using\n\
             JSON-RPC 2.0 protocol.\n\n\
             Features:\n\
             • Local LLaMA model execution for coding assistance\n\
             • Session management with conversation history\n\
             • File system operations (read/write)\n\
             • Terminal execution\n\
             • Tool integration via MCP servers\n\
             • Permission-based security model\n\n\
             Examples:\n\
               sah agent acp                        # Start with default config\n\
               sah agent acp --config acp.yaml      # Start with custom config\n\
               sah agent acp --permission-policy auto-approve-reads\n\
               sah agent acp --allow-path /home/user/projects --block-path /home/user/.ssh\n\
               sah agent acp --max-file-size 5242880 --terminal-buffer-size 2097152\n\n\
             Configuration:\n\
             Options can be specified via:\n\
             1. Command-line flags (highest priority)\n\
             2. Configuration file (--config)\n\
             3. Default values (lowest priority)\n\n\
             Command-line flags override configuration file settings.\n\n\
             For editor configuration:\n\
             • Zed: Add to agents section in settings\n\
             • JetBrains: Install ACP plugin and configure",
                    )
                    .args(vec![
                        config_arg,
                        permission_policy_arg,
                        allow_path_arg,
                        block_path_arg,
                        max_file_size_arg,
                        terminal_buffer_size_arg,
                        graceful_shutdown_timeout_arg,
                    ]),
            ];

        Self::build_command_with_subcommands(
            CommandConfig {
                name: "agent",
                about: "Manage and interact with Agent Client Protocol server",
                long_about: crate::commands::agent::DESCRIPTION,
            },
            Self::build_subcommands_from_specs(&subcommand_specs),
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

    /// Create a single workflow shortcut command
    fn create_workflow_shortcut_command(workflow: swissarmyhammer_workflow::Workflow) -> Command {
        let workflow_name = workflow.name.to_string();
        let command_name = WorkflowCommandNameResolver::resolve(&workflow_name);

        Self::build_shortcut_command(command_name, &workflow_name, &workflow)
    }

    /// Standard workflow execution flag definitions
    ///
    /// These flags are applied uniformly to all workflows by design, providing consistent
    /// execution control across the workflow system. Each workflow inherits these flags
    /// automatically when registered as a CLI command.
    ///
    /// # Design Rationale
    ///
    /// These flags are intentionally standardized rather than derived from workflow metadata because:
    ///
    /// 1. **Consistent UX**: Users expect the same execution controls across all workflows
    /// 2. **Workflow Engine Contract**: These map to the core execution model defined in
    ///    `RunCommandConfig` and used by all workflow execution paths
    /// 3. **CLI Convention**: These are standard CLI patterns (--interactive, --dry-run, --quiet)
    /// 4. **Composability**: Workflows shouldn't need to redefine common execution modes
    /// 5. **Single Source of Truth**: The workflow engine's execution contract (RunCommandConfig)
    ///    is the authoritative source - these CLI flags must match that contract exactly
    ///
    /// The flags defined here mirror the execution parameters in:
    /// - `swissarmyhammer-cli/src/commands/flow/run.rs::RunCommandConfig`
    /// - MCP flow tool parameters
    /// - Workflow execution context variables (`_quiet`, etc.)
    ///
    /// If a workflow needs custom flags, those should be defined as workflow parameters
    /// in the workflow definition itself, not as execution flags.
    ///
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

        // Add workflow parameters using the processor
        cmd = WorkflowParameterProcessor::process_parameters(cmd, workflow);

        // Add standard workflow execution flags
        cmd = Self::add_workflow_execution_flags(cmd);

        cmd
    }
}

#[cfg(test)]
#[path = "dynamic_cli_tests.rs"]
mod tests;
