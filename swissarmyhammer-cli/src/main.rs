use std::process;
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
use std::sync::Arc;
use swissarmyhammer_config::TemplateContext;

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

#[tokio::main]
async fn main() {
    // Parse CLI early to check for --cwd flag BEFORE doing anything else
    // We need to do a minimal parse just to extract --cwd
    let args: Vec<String> = std::env::args().collect();

    // Check for --cwd flag and change directory FIRST
    if let Some(cwd_index) = args.iter().position(|arg| arg == "--cwd") {
        if let Some(cwd_path) = args.get(cwd_index + 1) {
            if let Err(e) = std::env::set_current_dir(cwd_path) {
                eprintln!("Failed to change directory to '{}': {}", cwd_path, e);
                process::exit(EXIT_ERROR);
            }
        } else {
            eprintln!("--cwd requires a path argument");
            process::exit(EXIT_ERROR);
        }
    }

    // Load configuration early for CLI operations
    let template_context = load_cli_configuration();

    // Initialize tool context and registry for dynamic CLI
    let cli_tool_context = match CliToolContext::new().await {
        Ok(context) => Arc::new(context),
        Err(e) => {
            eprintln!("Failed to initialize tool context: {}", e);
            process::exit(EXIT_ERROR);
        }
    };

    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry);

    // Initialize workflow storage for generating shortcuts
    // This is done early before CLI parsing to enable workflow shortcuts
    let workflow_storage = match swissarmyhammer_workflow::WorkflowStorage::file_system() {
        Ok(storage) => Some(storage),
        Err(e) => {
            tracing::warn!("Failed to initialize workflow storage: {}", e);
            None
        }
    };

    // Get validation statistics for startup reporting (only for non-serve commands)
    let validation_stats = cli_builder.get_validation_stats();

    // Check for validation issues and report them
    if !validation_stats.is_all_valid() {
        // Always show validation summary for issues (not just in verbose mode)
        eprintln!("⚠️  CLI Validation Issues: {}", validation_stats.summary());

        // Show detailed warnings if there are validation problems
        let warnings = cli_builder.get_validation_warnings();
        if !warnings.is_empty() {
            eprintln!("Validation warnings ({} issues):", warnings.len());
            for (i, warning) in warnings.iter().enumerate().take(5) {
                eprintln!("  {}. {}", i + 1, warning);
            }
            if warnings.len() > 5 {
                eprintln!("  ... and {} more warnings", warnings.len() - 5);
                eprintln!("  Use --verbose for complete validation report");
            }
        }
        eprintln!(); // Add blank line for readability
    }

    // Build CLI with warnings for validation issues (graceful degradation)
    // This will skip problematic tools but continue building the CLI
    // Pass workflow storage to enable dynamic shortcut generation
    let dynamic_cli = cli_builder.build_cli_with_warnings(workflow_storage.as_ref());

    // Parse arguments with dynamic CLI
    let matches = match dynamic_cli.try_get_matches() {
        Ok(matches) => matches,
        Err(e) => {
            // Check if this is a help or version request (which are normal exits)
            use clap::error::ErrorKind;
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    // Print the help/version output
                    print!("{}", e);
                    process::exit(EXIT_SUCCESS);
                }
                _ => {
                    eprintln!("{}", e);
                    process::exit(EXIT_ERROR);
                }
            }
        }
    };

    // Handle dynamic command dispatch
    let exit_code = handle_dynamic_matches(matches, cli_tool_context, template_context).await;
    process::exit(exit_code);
}

async fn handle_tool_validation(cli_tool_context: Arc<CliToolContext>, verbose: bool) -> i32 {
    let tool_registry = cli_tool_context.get_tool_registry_arc();
    let cli_builder = CliBuilder::new(tool_registry.clone());

    println!("🔍 Validating MCP tool schemas for CLI compatibility...\n");

    let validation_stats = cli_builder.get_validation_stats();
    let validation_errors = cli_builder.validate_all_tools();

    // Always show validation summary
    println!("📊 Validation Summary:");
    println!("   {}", validation_stats.summary());
    println!();

    if validation_stats.is_all_valid() {
        println!("✅ All tools passed validation!");
        if verbose {
            let categories = tool_registry.get_cli_categories();
            println!("\n📋 Validated CLI categories ({}):", categories.len());
            for category in categories {
                let tools = tool_registry.get_tools_for_category(&category);
                println!("   {} - {} tools", category, tools.len());
                if verbose {
                    for tool in tools {
                        println!("     ├── {} ({})", tool.cli_name(), tool.name());
                    }
                }
            }
        }
        return EXIT_SUCCESS;
    }

    // Show validation errors
    println!("❌ Validation Issues Found:");

    if verbose {
        for (i, error) in validation_errors.iter().enumerate() {
            println!("{}. {}", i + 1, error);
            if let Some(suggestion) = error.suggestion() {
                println!("   💡 {}", suggestion);
            }
            println!();
        }
    } else {
        let warnings = cli_builder.get_validation_warnings();
        for (i, warning) in warnings.iter().enumerate().take(10) {
            println!("{}. {}", i + 1, warning);
        }
        if warnings.len() > 10 {
            println!("   ... and {} more issues", warnings.len() - 10);
            println!("   Use --verbose for complete details");
        }
    }

    println!("🔧 To fix these issues:");
    println!("   • Review tool schema definitions");
    println!("   • Ensure all CLI tools have proper categories");
    println!("   • Use supported parameter types (string, integer, number, boolean, array)");
    println!("   • Add required schema fields like 'properties'");

    EXIT_WARNING
}

async fn handle_dynamic_matches(
    matches: clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
    template_context: TemplateContext,
) -> i32 {
    // Handle global verbose/debug/quiet flags
    let verbose = matches.get_flag("verbose");
    let debug = matches.get_flag("debug");
    let quiet = matches.get_flag("quiet");
    let validate_tools = matches.get_flag("validate-tools");

    // Handle global format flag
    use crate::cli::OutputFormat;
    let format_option = matches
        .try_get_one::<String>("format")
        .unwrap_or(None)
        .map(|s| match s.as_str() {
            "json" => OutputFormat::Json,
            "yaml" => OutputFormat::Yaml,
            "table" => OutputFormat::Table,
            _ => OutputFormat::Table,
        });
    let format = format_option.unwrap_or(OutputFormat::Table);

    // Check if this is a serve command for MCP mode logging
    let is_serve_command = matches
        .subcommand()
        .is_some_and(|(name, _)| name == "serve");

    // Initialize logging similar to static CLI
    configure_logging(verbose, debug, quiet, is_serve_command).await;

    // Handle --validate-tools flag
    if validate_tools {
        return handle_tool_validation(cli_tool_context, verbose).await;
    }

    // Show detailed validation report in verbose mode (but not during serve mode)
    if verbose && !is_serve_command {
        let tool_registry = cli_tool_context.get_tool_registry_arc();
        let cli_builder = CliBuilder::new(tool_registry);
        let validation_stats = cli_builder.get_validation_stats();

        if verbose {
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
    }

    // Create shared CLI context
    let context = match CliContext::new(
        template_context.clone(),
        format,
        format_option,
        verbose,
        debug,
        quiet,
        matches,
    )
    .await
    {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to initialize CLI context: {}", e);
            process::exit(EXIT_ERROR);
        }
    };

    // Handle subcommands
    match context.matches.subcommand() {
        Some(("serve", sub_matches)) => {
            commands::serve::handle_command(sub_matches, &context).await
        }
        Some(("doctor", _)) => handle_doctor_command(&context).await,
        Some(("prompt", sub_matches)) => handle_prompt_command(sub_matches, &context).await,
        // "rule" command is now dynamically generated from MCP tools
        // Keeping this comment for now to track the migration
        Some(("flow", sub_matches)) => handle_flow_command(sub_matches, &context).await,
        Some(("validate", sub_matches)) => handle_validate_command(sub_matches, &context).await,
        Some(("agent", sub_matches)) => handle_agent_command(sub_matches, &context).await,
        Some((category, sub_matches)) => {
            // Check if this is a workflow shortcut or an MCP tool command
            // Workflow shortcuts are top-level commands with no subcommands
            // MCP tool commands have the pattern: category -> tool_name
            match sub_matches.subcommand() {
                Some((tool_name, tool_matches)) => {
                    // This is an MCP tool command
                    handle_dynamic_tool_command(category, tool_name, tool_matches, cli_tool_context)
                        .await
                }
                None => {
                    // This might be a workflow shortcut (no subcommand)
                    // Try to handle as a workflow shortcut
                    handle_workflow_shortcut(category, sub_matches, &context).await
                }
            }
        }
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            EXIT_ERROR
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
    let tool = match cli_tool_context
        .get_tool_registry()
        .get_tool_by_cli_name(category, tool_name)
    {
        Some(tool) => tool,
        None => {
            let available_tools: Vec<String> = cli_tool_context
                .get_tool_registry()
                .get_tools_for_category(category)
                .iter()
                .map(|t| format!("{} -> {}", t.cli_name(), t.name()))
                .collect();
            eprintln!(
                "Tool '{}' not found in category '{}'. Available tools in this category: [{}]",
                tool_name,
                category,
                available_tools.join(", ")
            );
            return EXIT_ERROR;
        }
    };

    let full_tool_name = tool.name();

    // Convert clap matches to JSON arguments
    let arguments =
        match convert_matches_to_arguments(matches, full_tool_name, &cli_tool_context).await {
            Ok(args) => args,
            Err(e) => {
                eprintln!("Error processing arguments: {}", e);
                return EXIT_ERROR;
            }
        };

    // Execute the MCP tool
    match cli_tool_context
        .execute_tool(full_tool_name, arguments)
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
        Err(e) => {
            eprintln!("Tool execution error: {}", e);
            EXIT_ERROR
        }
    }
}

async fn convert_matches_to_arguments(
    matches: &clap::ArgMatches,
    tool_name: &str,
    cli_tool_context: &CliToolContext,
) -> Result<serde_json::Map<String, serde_json::Value>, Box<dyn std::error::Error>> {
    let mut arguments = serde_json::Map::new();

    // Get the tool to access its schema
    let tool = cli_tool_context
        .get_tool_registry()
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

fn extract_clap_value(
    matches: &clap::ArgMatches,
    prop_name: &str,
    prop_schema: &serde_json::Value,
) -> Option<serde_json::Value> {
    match prop_schema.get("type").and_then(|t| t.as_str()) {
        Some("boolean") => {
            if matches.get_flag(prop_name) {
                Some(serde_json::Value::Bool(true))
            } else {
                None
            }
        }
        Some("integer") => matches
            .get_one::<i64>(prop_name)
            .map(|v| serde_json::Value::Number(serde_json::Number::from(*v))),
        Some("number") => matches
            .get_one::<f64>(prop_name)
            .and_then(|v| serde_json::Number::from_f64(*v))
            .map(serde_json::Value::Number),
        Some("array") => {
            let values: Vec<String> = matches
                .get_many::<String>(prop_name)
                .map(|vals| vals.cloned().collect())
                .unwrap_or_default();
            if values.is_empty() {
                None
            } else {
                Some(serde_json::Value::Array(
                    values.into_iter().map(serde_json::Value::String).collect(),
                ))
            }
        }
        _ => {
            // Default to string
            matches
                .get_one::<String>(prop_name)
                .map(|s| serde_json::Value::String(s.clone()))
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
) -> i32 {
    use crate::cli::FlowSubcommand;

    // Remove underscore prefix if present (from conflict resolution)
    let actual_workflow_name = if let Some(stripped) = workflow_name.strip_prefix('_') {
        stripped
    } else {
        workflow_name
    };

    // Extract positional arguments (may not exist if workflow has no required params)
    let positional_args: Vec<String> = matches
        .try_get_many::<String>("positional")
        .ok()
        .flatten()
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

    // Extract --param arguments
    let params: Vec<String> = matches
        .get_many::<String>("param")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();

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
    commands::flow::handle_command(subcommand, context).await
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
async fn handle_flow_command(sub_matches: &clap::ArgMatches, context: &CliContext) -> i32 {
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
            return EXIT_ERROR;
        }
    };

    commands::flow::handle_command(subcommand, context).await
}

async fn handle_validate_command(matches: &clap::ArgMatches, cli_context: &CliContext) -> i32 {
    let workflow_dirs = matches
        .get_many::<String>("workflow-dirs")
        .map(|vals| vals.cloned().collect())
        .unwrap_or_default();
    let validate_tools = matches.get_flag("validate-tools");

    commands::validate::handle_command(workflow_dirs, validate_tools, cli_context).await
}

async fn handle_agent_command(matches: &clap::ArgMatches, context: &CliContext) -> i32 {
    use crate::cli::{AgentSubcommand, OutputFormat};

    let subcommand = match matches.subcommand() {
        Some(("list", sub_matches)) => {
            // Since we have default_value="table" and non-optional format,
            // clap should always provide a value
            let format = sub_matches
                .get_one::<String>("format")
                .map(|s| match s.as_str() {
                    "json" => OutputFormat::Json,
                    "yaml" => OutputFormat::Yaml,
                    "table" => OutputFormat::Table,
                    _ => OutputFormat::Table,
                })
                .unwrap_or(OutputFormat::Table);

            AgentSubcommand::List { format }
        }
        Some(("use", sub_matches)) => {
            let agent_name = sub_matches
                .get_one::<String>("agent_name")
                .cloned()
                .unwrap();
            AgentSubcommand::Use { agent_name }
        }
        _ => {
            eprintln!("No agent subcommand specified");
            return EXIT_ERROR;
        }
    };

    commands::agent::handle_command(subcommand, context).await
}

async fn configure_logging(verbose: bool, debug: bool, quiet: bool, is_mcp_mode: bool) {
    use tracing::Level;
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    let log_level = if is_mcp_mode {
        Level::DEBUG // More verbose for MCP mode to help with debugging
    } else if quiet {
        Level::ERROR
    } else if debug {
        Level::DEBUG
    } else if verbose {
        Level::TRACE
    } else {
        Level::INFO
    };

    let create_filter = || EnvFilter::new(format!("ort=warn,rmcp=warn,{log_level}"));

    if is_mcp_mode {
        // Set flag to prevent unified server from also configuring logging
        std::env::set_var("SAH_CLI_MODE", "1");

        // In MCP mode, write logs to .swissarmyhammer/mcp.log for debugging
        use std::fs;
        use std::path::PathBuf;

        let log_dir = PathBuf::from(".swissarmyhammer");
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Warning: Could not create log directory: {}", e);
        }

        let log_file_name =
            std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
        let log_file_path = log_dir.join(log_file_name);
        match fs::File::create(&log_file_path) {
            Ok(file) => {
                let shared_file = Arc::new(std::sync::Mutex::new(file));
                registry()
                    .with(create_filter())
                    .with(
                        fmt::layer()
                            .with_writer(move || {
                                let file = shared_file.clone();
                                Box::new(FileWriterGuard::new(file)) as Box<dyn std::io::Write>
                            })
                            .with_ansi(false), // No color codes in file
                    )
                    .init();
            }
            Err(e) => {
                eprintln!(
                    "Warning: Could not create log file: {}. Falling back to stderr.",
                    e
                );
                registry()
                    .with(create_filter())
                    .with(fmt::layer().with_writer(std::io::stderr))
                    .init();
            }
        }
    } else {
        registry()
            .with(create_filter())
            .with(fmt::layer().with_writer(std::io::stderr))
            .init();
    }
}
