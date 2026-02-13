use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
mod cli;
mod commands;
mod context;
mod dynamic_cli;
mod error;
mod exit_codes;
mod logging;
mod mcp_integration;
mod schema_conversion;
mod schema_validation;
mod signal_handler;
mod validate;
use crate::context::CliContext;
use dynamic_cli::CliBuilder;
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use logging::FileWriterGuard;
use mcp_integration::CliToolContext;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;
use swissarmyhammer_js::JsState;

/// Track if we've already performed shutdown to prevent double-shutdown
static SHUTDOWN_PERFORMED: AtomicBool = AtomicBool::new(false);

/// Initialize global JS variables used by workflows
///
/// This ensures that common workflow variables exist before any workflows
/// are loaded or executed, preventing "undeclared reference" errors.
async fn initialize_global_js_variables() {
    let js_state = JsState::global();

    // Initialize are_tests_passing to false by default
    // This variable is used by the test workflow to track test status
    let _ = js_state.set("are_tests_passing", "false").await;
}

/// Perform graceful shutdown before process exit
///
/// This ensures the global LlamaAgent executor is properly shut down before
/// process termination, preventing Metal device cleanup assertion failures on macOS.
///
/// This function safely handles both cases: when called from within a tokio runtime
/// (e.g., during tests) and when called from outside a runtime (e.g., normal execution).
fn shutdown_before_exit() {
    // Mark shutdown as performed (idempotent via atomic swap)
    let _ = SHUTDOWN_PERFORMED.swap(true, Ordering::SeqCst);

    // Shutdown is now handled automatically by the agent lifecycle.
    // The swissarmyhammer-agent crate manages cleanup internally.
}

/// Global flags extracted from command-line arguments
struct GlobalFlags {
    verbose: bool,
    debug: bool,
    quiet: bool,
    validate_tools: bool,
    format: cli::OutputFormat,
    format_option: Option<cli::OutputFormat>,
}

/// Extract global flags from command-line arguments
///
/// This function centralizes the extraction of global flags to reduce nesting
/// in the main command handler.
fn extract_global_flags(matches: &clap::ArgMatches) -> GlobalFlags {
    use crate::cli::OutputFormat;
    use std::str::FromStr;

    let verbose = matches.get_flag("verbose");
    let debug = matches.get_flag("debug");
    let quiet = matches.get_flag("quiet");
    let validate_tools = matches.get_flag("validate-tools");

    let format_option = matches
        .try_get_one::<String>("format")
        .unwrap_or(None)
        .map(|s| OutputFormat::from_str(s).unwrap_or(OutputFormat::Table));
    let format = format_option.unwrap_or(OutputFormat::Table);

    GlobalFlags {
        verbose,
        debug,
        quiet,
        validate_tools,
        format,
        format_option,
    }
}

/// Load configuration for CLI usage with graceful error handling
///
/// This function loads configuration from all standard sources (global, project, environment)
/// and handles errors gracefully to ensure the CLI remains functional even with invalid config.
fn load_cli_configuration() -> TemplateContext {
    match swissarmyhammer_config::load_configuration_for_cli() {
        Ok(context) => {
            tracing::debug!("Loaded configuration with {} variables", context.len());
            context
        }
        Err(e) => {
            // Log the error but don't fail the CLI - configuration is optional for many operations
            tracing::warn!("Failed to load configuration: {}", e);
            eprintln!("Warning: Configuration loading failed: {}", e);
            eprintln!("Continuing with default configuration...");
            TemplateContext::new()
        }
    }
}

/// Extract a vector of strings from clap matches
///
/// This helper function encapsulates the common pattern of extracting string vectors
/// from clap argument matches with a default empty vector if the argument is not present.
/// This is the standard pattern used throughout the CLI for handling multi-value arguments.
///
/// # Usage Pattern
/// This function provides consistent extraction of string vectors and is used in multiple
/// locations to avoid code duplication:
/// - Extracting workflow positional arguments
/// - Extracting parameter lists
/// - Extracting file patterns and paths
///
/// # Arguments
/// * `matches` - The clap ArgMatches to extract from
/// * `key` - The argument key to extract
///
/// # Returns
/// A vector of strings, or an empty vector if the argument is not present
fn extract_string_vec(matches: &clap::ArgMatches, key: &str) -> Vec<String> {
    matches
        .try_get_many::<String>(key)
        .ok()
        .flatten()
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default()
}

/// Display a numbered list of items with optional truncation
///
/// This generic function handles displaying any list of displayable items with
/// consistent formatting and truncation support.
///
/// # Arguments
/// * `items` - The list of items to display (must implement Display)
/// * `verbose` - Whether to show all items or just a summary
/// * `max_display` - Maximum number of items to display when not in verbose mode
/// * `item_type` - Description of the items for the truncation message (e.g., "warnings", "errors")
fn display_numbered_items<T: std::fmt::Display>(
    items: &[T],
    verbose: bool,
    max_display: usize,
    item_type: &str,
) {
    if items.is_empty() {
        return;
    }

    if verbose {
        for (i, item) in items.iter().enumerate() {
            eprintln!("  {}. {}", i + 1, item);
        }
    } else {
        for (i, item) in items.iter().enumerate().take(max_display) {
            eprintln!("  {}. {}", i + 1, item);
        }
        if items.len() > max_display {
            eprintln!("  ... and {} more {}", items.len() - max_display, item_type);
            eprintln!("  Use --verbose for complete validation report");
        }
    }
}

/// Report validation issues for CLI tools
///
/// This function displays validation statistics and warnings with appropriate formatting.
/// It provides a consistent reporting experience across different parts of the CLI.
///
/// # Arguments
/// * `cli_builder` - The CLI builder containing validation state
/// * `verbose` - Whether to show all warnings or just a summary
/// * `max_warnings` - Maximum number of warnings to display when not in verbose mode
fn report_validation_issues(cli_builder: &CliBuilder, verbose: bool, max_warnings: usize) {
    let validation_stats = cli_builder.get_validation_stats();

    if validation_stats.is_all_valid() {
        return;
    }

    // Always show validation summary for issues
    eprintln!("⚠️  CLI Validation Issues: {}", validation_stats.summary());

    let warnings = cli_builder.get_validation_warnings();
    if !warnings.is_empty() {
        eprintln!("Validation warnings ({} issues):", warnings.len());
        display_numbered_items(&warnings, verbose, max_warnings, "warnings");
    }
    eprintln!(); // Add blank line for readability
}

/// Display detailed validation report in verbose mode
///
/// This function shows a detailed validation report when verbose mode is enabled,
/// reducing complexity in the main handler.
///
/// # Arguments
/// * `cli_tool_context` - The tool context for accessing registry
/// * `is_serve_command` - Whether this is a serve command (skip reporting)
async fn display_verbose_validation_report(
    cli_tool_context: &Arc<CliToolContext>,
    is_serve_command: bool,
) {
    if is_serve_command {
        return;
    }

    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry);
    let validation_stats = cli_builder.get_validation_stats();

    eprintln!("🔍 CLI Tool Validation Report:");
    eprintln!("   {}", validation_stats.summary());

    if !validation_stats.is_all_valid() {
        eprintln!("   Tools with issues:");
        let warnings = cli_builder.get_validation_warnings();
        for (i, warning) in warnings.iter().enumerate() {
            eprintln!("     {}. {}", i + 1, warning);
        }
    }
    eprintln!(); // Add blank line
}

/// Ensure the .swissarmyhammer directory exists
///
/// This function creates the .swissarmyhammer directory if it doesn't exist,
/// providing a consistent way to handle directory creation across the CLI.
/// It also ensures a .gitignore file is created to exclude temporary files.
///
/// # Returns
/// The path to the .swissarmyhammer directory or an error if creation fails
fn ensure_swissarmyhammer_dir() -> Result<PathBuf, std::io::Error> {
    use swissarmyhammer_common::SwissarmyhammerDirectory;

    // Try to create from git root first, fall back to current directory
    let sah_dir = SwissarmyhammerDirectory::from_git_root()
        .or_else(|_| {
            // If not in a git repo, create in current directory
            SwissarmyhammerDirectory::from_custom_root(std::env::current_dir()?)
        })
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(sah_dir.root().to_path_buf())
}

/// Report an error and return EXIT_ERROR code
///
/// This helper function provides a consistent way to report errors and return
/// the appropriate exit code across the CLI.
///
/// # Arguments
/// * `error` - The error to display
///
/// # Returns
/// EXIT_ERROR constant
fn report_error_and_exit(error: impl std::fmt::Display) -> i32 {
    eprintln!("{}", error);
    EXIT_ERROR
}

/// Unwrap a Result or exit with an error message
///
/// This generic helper function handles the common pattern of printing an error
/// message and exiting the process with EXIT_ERROR if a Result is an error.
///
/// # Arguments
/// * `result` - The Result to unwrap
/// * `message` - Prefix message to display before the error
///
/// # Returns
/// The unwrapped value if the Result is Ok
///
/// # Usage Notes
/// This function directly exits the process on error. For contexts that need to
/// return an exit code instead of exiting immediately, convert the Result to
/// Result<T, i32> by mapping errors through `report_error_and_exit`.
fn unwrap_or_exit<T, E: std::fmt::Display>(result: Result<T, E>, message: &str) -> T {
    match result {
        Ok(value) => value,
        Err(e) => {
            eprintln!("{}: {}", message, e);
            shutdown_before_exit();
            process::exit(EXIT_ERROR);
        }
    }
}

/// Handle --cwd flag to change working directory
///
/// This function checks for the --cwd flag and changes the directory if specified.
fn handle_cwd_flag(args: &[String]) {
    if let Some(cwd_index) = args.iter().position(|arg| arg == "--cwd") {
        if let Some(cwd_path) = args.get(cwd_index + 1) {
            unwrap_or_exit(
                std::env::set_current_dir(cwd_path),
                &format!("Failed to change directory to '{}'", cwd_path),
            );
        } else {
            eprintln!("--cwd requires a path argument");
            shutdown_before_exit();
            process::exit(EXIT_ERROR);
        }
    }
}

/// Extract --model flag from command-line arguments
///
/// This function checks for the --model flag and extracts its value.
/// Returns None if the flag is not present.
fn extract_model_flag(args: &[String]) -> Option<String> {
    args.iter()
        .position(|arg| arg == "--model")
        .and_then(|model_index| {
            args.get(model_index + 1)
                .map(|model_name| model_name.to_string())
        })
}

/// Initialize tool context and registry
///
/// This function initializes the tool context required for MCP tool execution.
///
/// # Arguments
///
/// * `model_override` - Optional model name to use for all use cases
///
/// # Returns
///
/// Arc to the initialized CliToolContext
async fn initialize_tool_context(model_override: Option<&str>) -> Arc<CliToolContext> {
    let current_dir = unwrap_or_exit(std::env::current_dir(), "Failed to get current directory");
    let context = unwrap_or_exit(
        CliToolContext::new_with_config(&current_dir, model_override).await,
        "Failed to initialize tool context",
    );
    Arc::new(context)
}

/// Initialize workflow storage for shortcuts
///
/// This function initializes the workflow storage used for generating shortcuts.
fn initialize_workflow_storage() -> Option<swissarmyhammer_workflow::WorkflowStorage> {
    match swissarmyhammer_workflow::WorkflowStorage::file_system() {
        Ok(storage) => Some(storage),
        Err(e) => {
            tracing::warn!("Failed to initialize workflow storage: {}", e);
            None
        }
    }
}

/// Handle CLI parse errors and exit appropriately
///
/// This function handles different types of clap parsing errors,
/// exiting with appropriate status codes for each error kind.
///
/// # Arguments
/// * `error` - The clap error to handle
///
/// # Returns
/// Never returns - always exits the process
fn handle_cli_parse_error(error: clap::Error) -> ! {
    use clap::error::ErrorKind;
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
            print!("{}", error);
            shutdown_before_exit();
            process::exit(EXIT_SUCCESS);
        }
        _ => {
            eprintln!("{}", error);
            shutdown_before_exit();
            process::exit(EXIT_ERROR);
        }
    }
}

/// Build and parse CLI with dynamic tool registration
///
/// This function builds the CLI with dynamic tools and parses command-line arguments.
fn build_and_parse_cli(
    cli_builder: CliBuilder,
    workflow_storage: Option<&swissarmyhammer_workflow::WorkflowStorage>,
) -> clap::ArgMatches {
    // Check for validation issues and report them
    report_validation_issues(&cli_builder, false, 5);

    // Build CLI with warnings for validation issues (graceful degradation)
    let dynamic_cli = cli_builder.build_cli_with_warnings(workflow_storage);

    // Parse arguments with dynamic CLI
    match dynamic_cli.try_get_matches() {
        Ok(matches) => matches,
        Err(e) => handle_cli_parse_error(e),
    }
}

#[tokio::main]
async fn main() {
    // Parse CLI early to check for --cwd and --model flags BEFORE doing anything else
    let args: Vec<String> = std::env::args().collect();

    // Check for --cwd flag and change directory FIRST
    handle_cwd_flag(&args);

    // Initialize global JS variables used by workflows
    initialize_global_js_variables().await;

    // Extract --model flag for global override
    let model_override = extract_model_flag(&args);

    // Load configuration early for CLI operations
    let template_context = load_cli_configuration();

    // Initialize tool context and registry for dynamic CLI with model override
    let cli_tool_context = initialize_tool_context(model_override.as_deref()).await;

    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry);

    // Initialize workflow storage for generating shortcuts
    let workflow_storage = initialize_workflow_storage();

    // Build CLI and parse arguments
    let matches = build_and_parse_cli(cli_builder, workflow_storage.as_ref());

    // Handle dynamic command dispatch
    let exit_code = handle_dynamic_matches(matches, cli_tool_context, template_context).await;

    // Shutdown global LlamaAgent executor before exit to prevent Metal cleanup crashes
    // Note: Agent shutdown is now handled automatically by the agent lifecycle
    // The swissarmyhammer-agent crate manages cleanup internally
    let _ = SHUTDOWN_PERFORMED.swap(true, Ordering::SeqCst);

    process::exit(exit_code);
}

/// Display validation summary with consistent formatting
///
/// This function provides a reusable way to display validation summaries
/// across different parts of the CLI.
///
/// # Arguments
/// * `validation_stats` - Statistics from validation
fn display_validation_summary(validation_stats: &dynamic_cli::CliValidationStats) {
    println!("📊 Validation Summary:");
    println!("   {}", validation_stats.summary());
    println!();
}

/// Details to display after validation summary
///
/// This enum encapsulates what additional information to display after
/// showing the validation summary, reducing duplication between success
/// and error reporting functions.
enum ValidationDetails<'a> {
    Success {
        registry:
            &'a Arc<tokio::sync::RwLock<swissarmyhammer_tools::mcp::tool_registry::ToolRegistry>>,
    },
    Errors {
        errors: &'a [schema_validation::ValidationError],
        cli_builder: &'a CliBuilder,
    },
}

/// Display validation report with summary and optional details
///
/// This function provides a unified way to display validation results,
/// reducing duplication between success and error reporting.
///
/// # Arguments
/// * `validation_stats` - Statistics from validation
/// * `details` - What details to display (success or errors)
/// * `verbose` - Whether to show detailed information
async fn display_validation_report(
    validation_stats: &dynamic_cli::CliValidationStats,
    details: ValidationDetails<'_>,
    verbose: bool,
) {
    display_validation_summary(validation_stats);

    match details {
        ValidationDetails::Success { registry } => {
            println!("{} All tools passed validation!", "✓".green());

            if verbose {
                let registry_guard = registry.read().await;
                let categories = registry_guard.get_cli_categories();
                println!("\n📋 Validated CLI categories ({}):", categories.len());
                for category in categories {
                    let tools = registry_guard.get_tools_for_category(&category);
                    println!("   {} - {} tools", category, tools.len());
                    for tool in tools {
                        println!("     ├── {} ({})", tool.cli_name(), <dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(tool));
                    }
                }
            }
        }
        ValidationDetails::Errors {
            errors,
            cli_builder,
        } => {
            println!("✗ Validation Issues Found:");

            if verbose {
                for (i, error) in errors.iter().enumerate() {
                    println!("{}. {}", i + 1, error);
                    if let Some(suggestion) = error.suggestion() {
                        println!("   💡 {}", suggestion);
                    }
                    println!();
                }
            } else {
                let warnings = cli_builder.get_validation_warnings();
                display_numbered_items(&warnings, false, 10, "errors");
            }
        }
    }
}

/// Display fix suggestions for validation issues
///
/// This function shows actionable suggestions for fixing validation problems.
fn display_fix_suggestions() {
    println!("🔧 To fix these issues:");
    println!("   • Review tool schema definitions");
    println!("   • Ensure all CLI tools have proper categories");
    println!("   • Use supported parameter types (string, integer, number, boolean, array)");
    println!("   • Add required schema fields like 'properties'");
}

async fn handle_tool_validation(cli_tool_context: Arc<CliToolContext>, verbose: bool) -> i32 {
    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry.clone());

    println!("🔍 Validating MCP tool schemas for CLI compatibility...\n");

    let validation_stats = cli_builder.get_validation_stats();
    let validation_errors = cli_builder.validate_all_tools();

    if validation_stats.is_all_valid() {
        display_validation_report(
            &validation_stats,
            ValidationDetails::Success {
                registry: &tool_registry,
            },
            verbose,
        )
        .await;
        return EXIT_SUCCESS;
    }

    display_validation_report(
        &validation_stats,
        ValidationDetails::Errors {
            errors: &validation_errors,
            cli_builder: &cli_builder,
        },
        verbose,
    )
    .await;
    display_fix_suggestions();

    EXIT_WARNING
}

/// Route subcommands to appropriate handlers
///
/// This function centralizes subcommand routing to reduce nesting in the main handler.
///
/// # Arguments
/// * `context` - The CLI context containing matches and configuration
/// * `cli_tool_context` - The tool context for MCP tool execution
///
/// # Returns
/// Exit code from the handler
async fn route_subcommand(context: &CliContext, cli_tool_context: Arc<CliToolContext>) -> i32 {
    match context.matches.subcommand() {
        Some(("serve", sub_matches)) => commands::serve::handle_command(sub_matches, context).await,
        Some(("init", sub_matches)) => handle_init_command(sub_matches),
        Some(("deinit", sub_matches)) => handle_deinit_command(sub_matches),
        Some(("doctor", _)) => handle_doctor_command(context).await,
        Some(("prompt", sub_matches)) => handle_prompt_command(sub_matches, context).await,
        // "rule" command is now dynamically generated from MCP tools
        // Keeping this comment for now to track the migration
        Some(("flow", sub_matches)) => {
            handle_flow_command(sub_matches, context, cli_tool_context.clone()).await
        }
        Some(("validate", sub_matches)) => handle_validate_command(sub_matches, context).await,
        Some(("model", sub_matches)) => handle_model_command(sub_matches, context).await,
        Some(("agent", sub_matches)) => handle_agent_command(sub_matches, context).await,
        Some((category, sub_matches)) => {
            route_category_command(category, sub_matches, context, cli_tool_context).await
        }
        None => report_error_and_exit("No command specified. Use --help for usage information."),
    }
}

/// Route MCP tool commands
///
/// Handles MCP tool commands with the pattern: `sah <category> <tool_name> [args...]`
///
/// # Arguments
/// * `category` - The tool category (e.g., "files")
/// * `tool_name` - The specific tool within that category (e.g., "read")
/// * `tool_matches` - The tool's specific arguments
/// * `cli_tool_context` - The tool context for MCP tool execution
///
/// # Returns
/// Exit code from the handler
async fn route_mcp_tool_command(
    category: &str,
    tool_name: &str,
    tool_matches: &clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    handle_dynamic_tool_command(category, tool_name, tool_matches, cli_tool_context).await
}

/// Route workflow shortcut commands
///
/// Handles workflow shortcut commands with the pattern: `sah <workflow_name> [positional_args...] [flags...]`
///
/// # Arguments
/// * `workflow_name` - The workflow name (e.g., "plan")
/// * `sub_matches` - The workflow's positional arguments and flags
/// * `context` - The CLI context
/// * `cli_tool_context` - The tool context for MCP tool execution
///
/// # Returns
/// Exit code from the handler
async fn route_workflow_shortcut_command(
    workflow_name: &str,
    sub_matches: &clap::ArgMatches,
    context: &CliContext,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    handle_workflow_shortcut(workflow_name, sub_matches, context, cli_tool_context).await
}

/// Route category commands (MCP tools or workflow shortcuts)
///
/// This function determines whether a category command is an MCP tool or workflow shortcut
/// based on the presence of a subcommand.
///
/// # Command Routing Logic
///
/// There are two distinct routing paths handled by this function:
///
/// 1. **MCP Tool Path** (with subcommand):
///    - Pattern: `sah <category> <tool_name> [args...]`
///    - Example: `sah files read --path foo.txt`
///    - When `sub_matches.subcommand()` returns `Some((tool_name, tool_matches))`,
///      this indicates an MCP tool command where:
///      - `category` is the tool category (e.g., "files")
///      - `tool_name` is the specific tool within that category (e.g., "read")
///      - `tool_matches` contains the tool's specific arguments
///    - Routes to: `route_mcp_tool_command()`
///
/// 2. **Workflow Shortcut Path** (without subcommand):
///    - Pattern: `sah <workflow_name> [positional_args...] [flags...]`
///    - Example: `sah plan spec.md --interactive`
///    - When `sub_matches.subcommand()` returns `None`, this indicates
///      a workflow shortcut where:
///      - `category` is actually the workflow name (e.g., "plan")
///      - `sub_matches` contains the workflow's positional arguments and flags
///    - Routes to: `route_workflow_shortcut_command()`
///
/// # Arguments
/// * `category` - The category name (for MCP tools) or workflow name (for shortcuts)
/// * `sub_matches` - The subcommand matches
/// * `context` - The CLI context
/// * `cli_tool_context` - The tool context for MCP tool execution
///
/// # Returns
/// Exit code from the handler
async fn route_category_command(
    category: &str,
    sub_matches: &clap::ArgMatches,
    context: &CliContext,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    match sub_matches.subcommand() {
        Some((tool_name, tool_matches)) => {
            route_mcp_tool_command(category, tool_name, tool_matches, cli_tool_context).await
        }
        None => {
            route_workflow_shortcut_command(category, sub_matches, context, cli_tool_context).await
        }
    }
}

async fn handle_dynamic_matches(
    matches: clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
    template_context: TemplateContext,
) -> i32 {
    // Extract global flags
    let flags = extract_global_flags(&matches);

    // Check if this is a serve command for MCP mode logging
    let is_serve_command = matches
        .subcommand()
        .is_some_and(|(name, _)| name == "serve");

    // Initialize logging similar to static CLI
    configure_logging(flags.verbose, flags.debug, flags.quiet, is_serve_command).await;

    // Handle --validate-tools flag
    if flags.validate_tools {
        return handle_tool_validation(cli_tool_context, flags.verbose).await;
    }

    // Show detailed validation report in verbose mode (but not during serve mode)
    if flags.verbose {
        display_verbose_validation_report(&cli_tool_context, is_serve_command).await;
    }

    // Create shared CLI context
    let context = unwrap_or_exit(
        CliContext::new(
            template_context.clone(),
            flags.format,
            flags.format_option,
            flags.verbose,
            flags.debug,
            flags.quiet,
            matches,
        )
        .await,
        "Failed to initialize CLI context",
    );

    // Route to appropriate subcommand handler
    route_subcommand(&context, cli_tool_context).await
}

/// Format an error message for a tool not found in a category
///
/// This function creates a consistent error message format for tool lookup failures.
///
/// # Arguments
/// * `tool_name` - The tool name that was not found
/// * `category` - The category name
/// * `available_tools` - List of available tools in the category
///
/// # Returns
/// A formatted error message
fn format_tool_not_found_error(
    tool_name: &str,
    category: &str,
    available_tools: &[String],
) -> String {
    format!(
        "Tool '{}' not found in category '{}'. Available tools in this category: [{}]",
        tool_name,
        category,
        available_tools.join(", ")
    )
}

/// Lookup a tool by CLI name and category
///
/// This function looks up a tool in the registry and provides helpful error messages.
///
/// # Arguments
/// * `registry` - The tool registry
/// * `category` - The category name
/// * `tool_name` - The CLI tool name
///
/// # Returns
/// Result with the full tool name or an error message
async fn lookup_tool_by_cli_name(
    cli_tool_context: &Arc<CliToolContext>,
    category: &str,
    tool_name: &str,
) -> Result<String, String> {
    let registry_arc = cli_tool_context.get_tool_registry_arc();
    let registry = registry_arc.read().await;

    // For the unified "tool" category, tool_name is already the full MCP tool name
    if category == "tool" {
        match registry.get_tool(tool_name) {
            Some(tool) => Ok(<dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(tool).to_string()),
            None => {
                // List all available tools across all categories
                let mut available_tools: Vec<String> = Vec::new();
                for cat in registry.get_cli_categories() {
                    for t in registry.get_tools_for_category(&cat) {
                        if !t.hidden_from_cli() {
                            available_tools.push(<dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(t).to_string());
                        }
                    }
                }
                Err(format_tool_not_found_error(
                    tool_name,
                    category,
                    &available_tools,
                ))
            }
        }
    } else {
        // Legacy category-based lookup
        match registry.get_tool_by_cli_name(category, tool_name) {
            Some(tool) => Ok(<dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(tool).to_string()),
            None => {
                let available_tools: Vec<String> = registry
                    .get_tools_for_category(category)
                    .iter()
                    .map(|t| format!("{} -> {}", t.cli_name(), <dyn swissarmyhammer_tools::mcp::tool_registry::McpTool as swissarmyhammer_tools::mcp::tool_registry::McpTool>::name(*t)))
                    .collect();
                Err(format_tool_not_found_error(
                    tool_name,
                    category,
                    &available_tools,
                ))
            }
        }
    }
}

async fn handle_dynamic_tool_command(
    category: &str,
    tool_name: &str,
    matches: &clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    // Look up the tool by category and CLI name
    let full_tool_name = match lookup_tool_by_cli_name(&cli_tool_context, category, tool_name).await
    {
        Ok(name) => name,
        Err(e) => return report_error_and_exit(e),
    };

    // Check if tool has operations (subcommands)
    let registry_arc = cli_tool_context.get_tool_registry_arc();
    let registry = registry_arc.read().await;
    let tool = match registry.get_tool(&full_tool_name) {
        Some(t) => t,
        None => return report_error_and_exit(format!("Tool not found: {}", full_tool_name)),
    };

    let operations = tool.operations();
    let schema = tool.schema();
    drop(registry); // Release lock before executing

    // Convert clap matches to JSON arguments
    let arguments = if !operations.is_empty() {
        // Operation-based tool with noun-grouped structure
        // Pattern: tool -> noun -> verb (e.g., kanban -> board -> init)
        match matches.subcommand() {
            Some((noun, noun_matches)) => {
                // Look for verb subcommand within the noun
                match noun_matches.subcommand() {
                    Some((verb, verb_matches)) => {
                        // Construct "verb noun" for the op parameter (e.g., "init board")
                        let op_string = format!("{} {}", verb, noun);
                        match convert_operation_matches_to_arguments(
                            verb_matches,
                            &op_string,
                            &schema,
                        ) {
                            Ok(args) => args,
                            Err(e) => {
                                return report_error_and_exit(format!(
                                    "Error processing arguments: {}",
                                    e
                                ))
                            }
                        }
                    }
                    None => {
                        // No verb subcommand - show help for noun
                        return report_error_and_exit(format!(
                            "No verb specified for '{}'. Use --help to see available operations for '{}'.",
                            noun, noun
                        ));
                    }
                }
            }
            None => {
                // No noun subcommand - show help
                return report_error_and_exit(format!(
                    "No noun specified for '{}'. Use --help to see available nouns.",
                    tool_name
                ));
            }
        }
    } else {
        // Schema-based tool
        match convert_matches_to_arguments(matches, &full_tool_name, &cli_tool_context).await {
            Ok(args) => args,
            Err(e) => return report_error_and_exit(format!("Error processing arguments: {}", e)),
        }
    };

    // Execute the MCP tool
    match cli_tool_context
        .execute_tool(&full_tool_name, arguments)
        .await
    {
        Ok(result) => {
            // Format and display the result
            if result.is_error.unwrap_or(false) {
                eprintln!(
                    "{}",
                    mcp_integration::response_formatting::format_error_response(&result)
                );
                EXIT_ERROR
            } else {
                println!(
                    "{}",
                    mcp_integration::response_formatting::format_success_response(&result)
                );
                EXIT_SUCCESS
            }
        }
        Err(e) => report_error_and_exit(format!("Tool execution error: {}", e)),
    }
}

async fn convert_matches_to_arguments(
    matches: &clap::ArgMatches,
    tool_name: &str,
    cli_tool_context: &CliToolContext,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut arguments = serde_json::Map::new();

    // Get the tool to access its schema
    let registry_arc = cli_tool_context.get_tool_registry_arc();
    let registry = registry_arc.read().await;
    let tool = registry
        .get_tool(tool_name)
        .ok_or_else(|| format!("Tool not found: {}", tool_name))?;

    let schema = tool.schema();

    // Extract properties from schema
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            if let Some(value) = extract_clap_value(matches, prop_name, prop_schema) {
                arguments.insert(prop_name.clone(), value);
            }
        }
    }

    Ok(arguments)
}

/// Convert operation subcommand matches to JSON arguments for operation-based tools
///
/// This extracts arguments from a subcommand and adds the "op" parameter.
/// Uses the tool's schema for argument extraction since that's what the MCP tool expects.
fn convert_operation_matches_to_arguments(
    matches: &clap::ArgMatches,
    op_string: &str,
    schema: &serde_json::Value,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut arguments = serde_json::Map::new();

    // Set the op parameter
    arguments.insert(
        "op".to_string(),
        serde_json::Value::String(op_string.to_string()),
    );

    // Extract arguments from schema properties (same as schema-based tools)
    if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
        for (prop_name, prop_schema) in properties {
            // Skip "op" since we already set it
            if prop_name == "op" {
                continue;
            }
            if let Some(value) = extract_clap_value(matches, prop_name, prop_schema) {
                arguments.insert(prop_name.clone(), value);
            }
        }
    }

    Ok(arguments)
}

/// Check if a JSON schema type contains a specific type name
///
/// Handles both string types and array types (for nullable types).
///
/// # Arguments
/// * `prop_schema` - The property schema to check
/// * `type_name` - The type name to look for
///
/// # Returns
/// True if the type is present
fn has_type(prop_schema: &serde_json::Value, type_name: &str) -> bool {
    match prop_schema.get("type") {
        Some(serde_json::Value::String(t)) => t == type_name,
        Some(serde_json::Value::Array(types)) => {
            types.iter().any(|t| t.as_str() == Some(type_name))
        }
        _ => false,
    }
}

/// Check if a JSON schema type is nullable
///
/// A type is nullable if it's an array containing "null" as one of the types.
///
/// # Arguments
/// * `prop_schema` - The property schema to check
///
/// # Returns
/// True if the type is nullable
fn is_nullable(prop_schema: &serde_json::Value) -> bool {
    has_type(prop_schema, "null")
}

/// Extract a nullable boolean value from clap matches
///
/// This helper function handles the extraction of nullable boolean values,
/// which have special handling compared to regular boolean flags.
///
/// # Arguments
/// * `matches` - The clap ArgMatches
/// * `prop_name` - The property name to extract
///
/// # Returns
/// The extracted JSON boolean value or None if not present
fn extract_nullable_boolean(
    matches: &clap::ArgMatches,
    prop_name: &str,
) -> Option<serde_json::Value> {
    matches
        .get_one::<String>(prop_name)
        .and_then(|s| match s.as_str() {
            "true" => Some(serde_json::Value::Bool(true)),
            "false" => Some(serde_json::Value::Bool(false)),
            _ => None,
        })
}

/// Value extraction strategies for different JSON schema types
///
/// This enum provides a unified approach to extracting values from clap matches
/// based on JSON schema types, eliminating duplication across extraction functions.
enum ValueExtractor {
    Boolean { nullable: bool },
    Integer,
    Number,
    Array,
    String,
}

impl ValueExtractor {
    /// Create an extractor based on JSON schema type information
    ///
    /// # Arguments
    /// * `prop_schema` - The JSON schema for the property
    ///
    /// # Returns
    /// The appropriate ValueExtractor for the schema type
    fn from_schema(prop_schema: &serde_json::Value) -> Self {
        if has_type(prop_schema, "boolean") {
            Self::Boolean {
                nullable: is_nullable(prop_schema),
            }
        } else if has_type(prop_schema, "integer") {
            Self::Integer
        } else if has_type(prop_schema, "number") {
            Self::Number
        } else if has_type(prop_schema, "array") {
            Self::Array
        } else {
            Self::String
        }
    }

    /// Extract a value from clap matches using this extraction strategy
    ///
    /// # Arguments
    /// * `matches` - The clap ArgMatches
    /// * `prop_name` - The property name to extract
    ///
    /// # Returns
    /// The extracted JSON value or None if not present
    fn extract(&self, matches: &clap::ArgMatches, prop_name: &str) -> Option<serde_json::Value> {
        match self {
            Self::Boolean { nullable } => {
                if *nullable {
                    extract_nullable_boolean(matches, prop_name)
                } else if matches.get_flag(prop_name) {
                    Some(serde_json::Value::Bool(true))
                } else {
                    None
                }
            }
            Self::Integer => matches
                .get_one::<i64>(prop_name)
                .map(|v| serde_json::Value::Number(serde_json::Number::from(*v))),
            Self::Number => matches
                .get_one::<f64>(prop_name)
                .and_then(|v| serde_json::Number::from_f64(*v))
                .map(serde_json::Value::Number),
            Self::Array => {
                let values = extract_string_vec(matches, prop_name);
                if values.is_empty() {
                    None
                } else {
                    Some(serde_json::Value::Array(
                        values.into_iter().map(serde_json::Value::String).collect(),
                    ))
                }
            }
            Self::String => matches
                .get_one::<String>(prop_name)
                .map(|s| serde_json::Value::String(s.clone())),
        }
    }
}

fn extract_clap_value(
    matches: &clap::ArgMatches,
    prop_name: &str,
    prop_schema: &serde_json::Value,
) -> Option<serde_json::Value> {
    let extractor = ValueExtractor::from_schema(prop_schema);
    extractor.extract(matches, prop_name)
}

fn parse_install_target(matches: &clap::ArgMatches) -> cli::InstallTarget {
    matches
        .get_one::<String>("target")
        .map(|s| match s.as_str() {
            "local" => cli::InstallTarget::Local,
            "user" => cli::InstallTarget::User,
            _ => cli::InstallTarget::Project,
        })
        .unwrap_or(cli::InstallTarget::Project)
}

fn handle_init_command(matches: &clap::ArgMatches) -> i32 {
    match commands::install::init::install(parse_install_target(matches)) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

fn handle_deinit_command(matches: &clap::ArgMatches) -> i32 {
    let remove_directory = matches.get_flag("remove-directory");

    match commands::install::deinit::uninstall(parse_install_target(matches), remove_directory) {
        Ok(()) => EXIT_SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            EXIT_ERROR
        }
    }
}

async fn handle_doctor_command(cli_context: &CliContext) -> i32 {
    commands::doctor::handle_command(cli_context).await
}

/// Handle workflow shortcut commands
///
/// Workflow shortcuts are top-level commands that directly execute workflows
/// without needing the `flow` prefix. For example, `sah plan spec.md` instead
/// of `sah flow plan spec.md`.
///
/// # Arguments
/// * `workflow_name` - Name of the workflow (may have underscore prefix for conflicts)
/// * `matches` - Argument matches from clap
/// * `context` - CLI context
///
/// # Returns
/// Exit code (0 for success, non-zero for error)
async fn handle_workflow_shortcut(
    workflow_name: &str,
    matches: &clap::ArgMatches,
    context: &CliContext,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    use crate::cli::FlowSubcommand;

    // Remove underscore prefix if present (from conflict resolution)
    let actual_workflow_name = if let Some(stripped) = workflow_name.strip_prefix('_') {
        stripped
    } else {
        workflow_name
    };

    // Extract positional arguments (may not exist if workflow has no required params)
    let positional_args = extract_string_vec(matches, "positional");

    // Extract --param arguments
    let params = extract_string_vec(matches, "param");

    // Extract flags
    let interactive = matches.get_flag("interactive");
    let dry_run = matches.get_flag("dry_run");
    let quiet = matches.get_flag("quiet");

    // Create FlowSubcommand::Execute
    let subcommand = FlowSubcommand::Execute {
        workflow: actual_workflow_name.to_string(),
        positional_args,
        params,
        vars: vec![], // Shortcuts don't support deprecated --var
        interactive,
        dry_run,
        quiet,
    };

    // Delegate to flow command handler
    commands::flow::handle_command(subcommand, context, cli_tool_context).await
}

/// Handle prompt command routing using the new CliContext-based architecture.
///
/// This function parses prompt subcommands using the new typed CLI system and routes
/// them to appropriate handlers. It supports global arguments like --verbose, --format,
/// and --debug through the CliContext parameter.
///
/// # Arguments
/// * `matches` - Clap argument matches for the prompt subcommand
/// * `context` - CliContext containing global configuration and prompt library access
///
/// # Returns
/// Exit code (0 for success, non-zero for error)
async fn handle_prompt_command(matches: &clap::ArgMatches, context: &CliContext) -> i32 {
    use crate::commands::prompt::cli;

    // Parse using the proper CLI parsing function
    let command = cli::parse_prompt_command(matches);

    // Use the new typed handler
    commands::prompt::handle_command_typed(command, context).await
}

/// Handle the rule subcommand
///
/// # Arguments
/// * `matches` - Clap argument matches for the rule subcommand
/// * `context` - CliContext containing global configuration and rule library access
///
/// # Returns
/// Exit code (0 for success, non-zero for error)
async fn handle_flow_command(
    sub_matches: &clap::ArgMatches,
    context: &CliContext,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    // Get the args vector from the trailing_var_arg
    let args: Vec<String> = sub_matches
        .get_many::<String>("args")
        .map(|vals| vals.map(|s| s.to_string()).collect())
        .unwrap_or_default();

    // Parse the args into a FlowSubcommand using the new parser
    let subcommand = match commands::flow::parse_flow_args(args) {
        Ok(cmd) => cmd,
        Err(e) => {
            // Check if this is the special help message
            if e.to_string().contains("__HELP_DISPLAYED__") {
                return EXIT_SUCCESS;
            }
            eprintln!("Error parsing flow command: {}", e);
            eprintln!("Use 'sah flow list' to see available workflows");
            eprintln!("Use 'sah flow <workflow> --help' for workflow-specific help");
            return report_error_and_exit("Flow command parsing failed");
        }
    };

    commands::flow::handle_command(subcommand, context, cli_tool_context).await
}

async fn handle_validate_command(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    let workflow_dirs = extract_string_vec(matches, "workflow-dirs");
    let validate_tools = matches.get_flag("validate-tools");

    commands::validate::handle_command(workflow_dirs, validate_tools, cli_context).await
}

/// Parse output format from clap matches with fallback
///
/// This helper function extracts the output format from clap matches,
/// providing a consistent fallback to Table format.
///
/// # Arguments
/// * `matches` - The clap ArgMatches to extract from
///
/// # Returns
/// The parsed OutputFormat, defaulting to Table if not found or invalid
fn parse_output_format(matches: &clap::ArgMatches) -> crate::cli::OutputFormat {
    use crate::cli::OutputFormat;
    use std::str::FromStr;

    matches
        .get_one::<String>("format")
        .map(|s| OutputFormat::from_str(s).unwrap_or(OutputFormat::Table))
        .unwrap_or(OutputFormat::Table)
}

async fn handle_model_command(matches: &clap::ArgMatches, context: &CliContext) -> i32 {
    use crate::cli::ModelSubcommand;

    let subcommand = match matches.subcommand() {
        Some(("list", sub_matches)) => {
            let format = parse_output_format(sub_matches);
            Some(ModelSubcommand::List { format })
        }
        Some(("show", sub_matches)) => {
            let format = parse_output_format(sub_matches);
            Some(ModelSubcommand::Show { format })
        }
        Some(("use", sub_matches)) => {
            let first = sub_matches.get_one::<String>("first").cloned().unwrap();
            let second = sub_matches.get_one::<String>("second").cloned();
            Some(ModelSubcommand::Use { first, second })
        }
        // Default to show when no subcommand is provided
        None => None,
        _ => return report_error_and_exit("Unknown model subcommand"),
    };

    commands::model::handle_command(subcommand, context).await
}

async fn handle_agent_command(matches: &clap::ArgMatches, context: &CliContext) -> i32 {
    use crate::cli::AgentSubcommand;

    let subcommand = match matches.subcommand() {
        Some(("acp", sub_matches)) => {
            let config = sub_matches.get_one::<std::path::PathBuf>("config").cloned();
            let permission_policy = sub_matches.get_one::<String>("permission_policy").cloned();
            let allow_path = sub_matches
                .get_many::<std::path::PathBuf>("allow_path")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let block_path = sub_matches
                .get_many::<std::path::PathBuf>("block_path")
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            let max_file_size = sub_matches.get_one::<u64>("max_file_size").copied();
            let terminal_buffer_size = sub_matches
                .get_one::<usize>("terminal_buffer_size")
                .copied();
            let graceful_shutdown_timeout = sub_matches
                .get_one::<u64>("graceful_shutdown_timeout")
                .copied();

            Some(AgentSubcommand::Acp {
                config,
                permission_policy,
                allow_path,
                block_path,
                max_file_size,
                terminal_buffer_size,
                graceful_shutdown_timeout,
            })
        }
        None => None,
        _ => return report_error_and_exit("Unknown agent subcommand"),
    };

    commands::agent::handle_command(subcommand, context).await
}

/// Determine the appropriate log level based on configuration flags
///
/// This function centralizes the logic for determining the log level based on
/// verbose, debug, quiet, and MCP mode flags.
///
/// # Arguments
/// * `is_mcp_mode` - Whether MCP mode is active
/// * `verbose` - Whether verbose logging is enabled
/// * `debug` - Whether debug logging is enabled
/// * `quiet` - Whether quiet mode is enabled
///
/// # Returns
/// The appropriate tracing Level
fn determine_log_level(
    is_mcp_mode: bool,
    verbose: bool,
    debug: bool,
    quiet: bool,
) -> tracing::Level {
    use tracing::Level;

    if is_mcp_mode {
        Level::DEBUG // More verbose for MCP mode to help with debugging
    } else if quiet {
        Level::ERROR
    } else if debug {
        Level::DEBUG
    } else if verbose {
        Level::TRACE
    } else {
        Level::INFO
    }
}

/// Create an EnvFilter with the specified log level
///
/// This function centralizes the creation of EnvFilter instances to ensure
/// consistent filter configuration across all logging setups.
///
/// # Arguments
/// * `log_level` - The tracing level to use
///
/// # Returns
/// An EnvFilter configured with the specified log level
fn create_env_filter(log_level: tracing::Level) -> tracing_subscriber::EnvFilter {
    use tracing_subscriber::EnvFilter;
    EnvFilter::new(format!("rmcp=warn,{log_level}"))
}

/// Setup MCP logging configuration with file output
///
/// This function handles the creation of the log directory and file for MCP mode,
/// reducing nesting in the main configure_logging function.
///
/// # Arguments
/// * `log_level` - The tracing level to use
///
/// # Returns
/// Result indicating success or failure
fn setup_mcp_logging(
    log_level: tracing::Level,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Set flag to prevent unified server from also configuring logging
    std::env::set_var("SAH_CLI_MODE", "1");

    // In MCP mode, write logs to .swissarmyhammer/mcp.log for debugging
    let log_dir = ensure_swissarmyhammer_dir()?;

    let log_file_name =
        std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
    let log_file_path = log_dir.join(log_file_name);
    let file = std::fs::File::create(&log_file_path)?;

    let shared_file = Arc::new(std::sync::Mutex::new(file));
    setup_logging_with_writer(log_level, move || {
        let file = shared_file.clone();
        Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
    });

    Ok(())
}

/// Setup logging with the specified log level and writer
///
/// This helper function consolidates the common pattern of creating an EnvFilter
/// and building a tracing registry with the specified writer.
///
/// # Arguments
/// * `log_level` - The tracing level to use
/// * `writer` - The writer to use for log output
fn setup_logging_with_writer<W>(log_level: tracing::Level, writer: W)
where
    W: for<'a> tracing_subscriber::fmt::MakeWriter<'a> + Send + Sync + 'static,
{
    use tracing_subscriber::prelude::*;

    let filter = create_env_filter(log_level);
    tracing_subscriber::registry()
        .with(filter)
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(writer)
                .with_ansi(false),
        )
        .init();
}

/// Setup stderr logging with the specified log level
///
/// This function provides a reusable way to configure stderr logging,
/// ensuring consistent fallback behavior across logging configurations.
///
/// # Arguments
/// * `log_level` - The tracing level to use
fn setup_stderr_logging(log_level: tracing::Level) {
    setup_logging_with_writer(log_level, std::io::stderr);
}

async fn configure_logging(verbose: bool, debug: bool, quiet: bool, is_mcp_mode: bool) {
    let log_level = determine_log_level(is_mcp_mode, verbose, debug, quiet);

    if is_mcp_mode {
        if let Err(e) = setup_mcp_logging(log_level) {
            eprintln!(
                "Warning: Could not setup MCP logging: {}. Falling back to stderr.",
                e
            );
            setup_stderr_logging(log_level);
        }
    } else {
        setup_stderr_logging(log_level);
    }
}
