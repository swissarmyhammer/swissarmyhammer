#![cfg_attr(feature = "dynamic-cli", allow(dead_code))]

use std::process;
mod cli;
#[cfg(not(feature = "dynamic-cli"))]
mod completions;
pub mod config;
#[cfg(not(feature = "dynamic-cli"))]
mod doctor;
#[cfg(feature = "dynamic-cli")]
mod dynamic_cli;
mod error;
mod exit_codes;
#[cfg(not(feature = "dynamic-cli"))]
mod file;
#[cfg(not(feature = "dynamic-cli"))]
mod flow;
#[cfg(not(feature = "dynamic-cli"))]
mod issue;
#[cfg(not(feature = "dynamic-cli"))]
mod list;
mod logging;
mod mcp_integration;
#[cfg(not(feature = "dynamic-cli"))]
mod migrate;
#[cfg(not(feature = "dynamic-cli"))]
mod parameter_cli;
// prompt_loader module removed - using SDK's PromptResolver directly
#[cfg(not(feature = "dynamic-cli"))]
mod prompt;
#[cfg(not(feature = "dynamic-cli"))]
mod search;
#[cfg(not(feature = "dynamic-cli"))]
mod shell;
mod signal_handler;
#[cfg(not(feature = "dynamic-cli"))]
mod test;
#[cfg(not(feature = "dynamic-cli"))]
mod validate;
#[cfg(not(feature = "dynamic-cli"))]
mod web_search;

#[cfg(not(feature = "dynamic-cli"))]
use clap::CommandFactory;
#[cfg(not(feature = "dynamic-cli"))]
use cli::{Cli, Commands};
#[cfg(not(feature = "dynamic-cli"))]
use exit_codes::EXIT_WARNING;
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
#[cfg(not(feature = "dynamic-cli"))]
use logging::FileWriterGuard;
#[cfg(not(feature = "dynamic-cli"))]
use swissarmyhammer::SwissArmyHammerError;

#[cfg(feature = "dynamic-cli")]
use dynamic_cli::CliBuilder;
#[cfg(feature = "dynamic-cli")]
use logging::FileWriterGuard;
#[cfg(feature = "dynamic-cli")]
use mcp_integration::CliToolContext;
#[cfg(feature = "dynamic-cli")]
use std::sync::Arc;

#[tokio::main]
async fn main() {
    #[cfg(feature = "dynamic-cli")]
    {
        run_with_dynamic_cli().await;
    }

    #[cfg(not(feature = "dynamic-cli"))]
    {
        run_with_static_cli().await;
    }
}

#[cfg(feature = "dynamic-cli")]
async fn run_with_dynamic_cli() {
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
    let dynamic_cli = cli_builder.build_cli();

    // Parse arguments with dynamic CLI
    let matches = match dynamic_cli.try_get_matches() {
        Ok(matches) => matches,
        Err(e) => {
            eprintln!("{}", e);
            process::exit(EXIT_ERROR);
        }
    };

    // Handle dynamic command dispatch
    let exit_code = handle_dynamic_matches(matches, cli_tool_context).await;
    process::exit(exit_code);
}

#[cfg(feature = "dynamic-cli")]
async fn handle_dynamic_matches(
    matches: clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    // Handle global verbose/debug/quiet flags
    let verbose = matches.get_flag("verbose");
    let debug = matches.get_flag("debug");
    let quiet = matches.get_flag("quiet");

    // Initialize logging similar to static CLI
    configure_logging(verbose, debug, quiet, false).await;

    // Handle subcommands
    match matches.subcommand() {
        Some((category, sub_matches)) => match sub_matches.subcommand() {
            Some((tool_name, tool_matches)) => {
                handle_dynamic_tool_command(category, tool_name, tool_matches, cli_tool_context)
                    .await
            }
            None => {
                eprintln!("No command specified for category '{}'", category);
                EXIT_ERROR
            }
        },
        None => {
            eprintln!("No command specified. Use --help for usage information.");
            EXIT_ERROR
        }
    }
}

#[cfg(feature = "dynamic-cli")]
async fn handle_dynamic_tool_command(
    category: &str,
    tool_name: &str,
    matches: &clap::ArgMatches,
    cli_tool_context: Arc<CliToolContext>,
) -> i32 {
    // Construct full tool name (category_tool_name)
    let full_tool_name = format!("{}_{}", category, tool_name);

    // Convert clap matches to JSON arguments
    let arguments =
        match convert_matches_to_arguments(matches, &full_tool_name, &cli_tool_context).await {
            Ok(args) => args,
            Err(e) => {
                eprintln!("Error processing arguments: {}", e);
                return EXIT_ERROR;
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
        Err(e) => {
            eprintln!("Tool execution error: {}", e);
            EXIT_ERROR
        }
    }
}

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
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

#[cfg(feature = "dynamic-cli")]
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
        // In MCP mode, write logs to .swissarmyhammer/log for debugging
        use std::fs;
        use std::path::PathBuf;

        let log_dir = PathBuf::from(".swissarmyhammer");
        if let Err(e) = fs::create_dir_all(&log_dir) {
            eprintln!("Warning: Could not create log directory: {}", e);
        }

        let log_file_path = log_dir.join("log");
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

#[cfg(not(feature = "dynamic-cli"))]
async fn run_with_static_cli() {
    let cli = Cli::parse_args();

    // Fast path for help - avoid expensive initialization
    if cli.command.is_none() {
        Cli::command().print_help().expect("Failed to print help");
        process::exit(EXIT_SUCCESS);
    }

    // Only initialize heavy dependencies when actually needed
    use tracing::Level;
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    // Configure logging based on verbosity flags and MCP mode detection
    use is_terminal::IsTerminal;
    let is_mcp_mode =
        matches!(cli.command, Some(Commands::Serve)) && !std::io::stdin().is_terminal();

    let log_level = if is_mcp_mode {
        Level::DEBUG // More verbose for MCP mode to help with debugging
    } else if cli.quiet {
        Level::ERROR
    } else if cli.debug {
        Level::DEBUG
    } else if cli.verbose {
        Level::TRACE
    } else {
        Level::INFO
    };

    // Helper function to create EnvFilter since it doesn't implement Clone
    let create_filter = || EnvFilter::new(format!("ort=warn,rmcp=warn,{log_level}"));

    if is_mcp_mode {
        // In MCP mode, write logs to .swissarmyhammer/log for debugging
        use std::fs;
        use std::path::PathBuf;

        let log_dir = PathBuf::from(".swissarmyhammer");

        // Ensure the directory exists
        if let Err(e) = fs::create_dir_all(&log_dir) {
            tracing::warn!("Failed to create log directory: {}", e);
        }

        let log_filename =
            std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
        let log_file = log_dir.join(log_filename);

        // Try to open the log file - use unbuffered writing for immediate flushing
        match std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file)
        {
            Ok(file) => {
                // Use Arc<Mutex<File>> for thread-safe, unbuffered writing
                use std::sync::{Arc, Mutex};
                let shared_file = Arc::new(Mutex::new(file));

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
                // Fallback to stderr if file logging fails
                tracing::warn!("Failed to open log file, using stderr: {}", e);
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

    let exit_code = match cli.command {
        Some(Commands::Serve) => {
            tracing::debug!("Starting MCP server");
            run_server().await
        }
        Some(Commands::Doctor { migration }) => {
            tracing::debug!("Running diagnostics with migration={}", migration);
            run_doctor_with_options(migration)
        }
        Some(Commands::Prompt { subcommand }) => {
            tracing::debug!("Running prompt command");
            run_prompt(subcommand).await
        }
        Some(Commands::Completion { shell }) => {
            tracing::debug!("Generating completion for {:?}", shell);
            run_completions(shell)
        }
        Some(Commands::Flow { subcommand }) => {
            tracing::debug!("Running flow command");
            run_flow(subcommand).await
        }
        Some(Commands::Validate {
            quiet,
            format,
            workflow_dirs,
        }) => {
            tracing::info!("Running validate command");
            run_validate(quiet, format, workflow_dirs)
        }
        Some(Commands::Issue { subcommand }) => {
            tracing::info!("Running issue command");
            run_issue(subcommand).await
        }

        Some(Commands::File { subcommand }) => {
            tracing::info!("Running file command");
            run_file(subcommand).await
        }
        Some(Commands::Search { subcommand }) => {
            tracing::info!("Running search command");
            run_search(subcommand).await
        }
        Some(Commands::Plan { plan_filename }) => {
            tracing::info!("Running plan command");
            run_plan(plan_filename).await
        }
        Some(Commands::WebSearch { subcommand }) => {
            tracing::info!("Running web search command");
            run_web_search(subcommand).await
        }
        Some(Commands::Config { subcommand }) => {
            tracing::info!("Running config command");
            run_config(subcommand).await
        }
        Some(Commands::Implement) => {
            tracing::info!("Running implement command");
            run_implement().await
        }
        Some(Commands::Shell { subcommand }) => {
            tracing::info!("Running shell command");
            run_shell(subcommand).await
        }
        Some(Commands::Migrate { subcommand }) => {
            tracing::info!("Running migrate command");
            run_migrate(subcommand).await
        }
        None => {
            // This case is handled early above for performance
            unreachable!()
        }
    };

    // Ensure all logs are flushed before process exit
    if is_mcp_mode {
        // Give tracing sufficient time to flush any pending logs
        std::thread::sleep(std::time::Duration::from_millis(200));
    }

    process::exit(exit_code);
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_server() -> i32 {
    use rmcp::serve_server;
    use rmcp::transport::io::stdio;
    use swissarmyhammer::PromptLibrary;
    use swissarmyhammer_tools::McpServer;

    // Create library and server
    let library = PromptLibrary::new();
    let server = match McpServer::new(library) {
        Ok(server) => server,
        Err(e) => {
            tracing::error!("Failed to create MCP server: {}", e);
            return EXIT_WARNING;
        }
    };

    // Initialize prompts (this will load user and local prompts)
    if let Err(e) = server.initialize().await {
        tracing::error!("Failed to initialize MCP server: {}", e);
        return EXIT_WARNING;
    }

    // Don't start file watching here - it will be started when MCP client connects
    // File watching is started in the ServerHandler::initialize method
    tracing::info!("MCP server initialized, file watching will start when client connects");

    // Start the rmcp SDK server with stdio transport
    let running_service = match serve_server(server, stdio()).await {
        Ok(service) => {
            tracing::info!("MCP server started successfully");
            service
        }
        Err(e) => {
            tracing::error!("MCP server error: {}", e);
            return EXIT_WARNING;
        }
    };

    // Wait for the service to complete - this will return when:
    // - The client disconnects (transport closed)
    // - The server is cancelled
    // - A serious error occurs
    match running_service.waiting().await {
        Ok(quit_reason) => {
            // The QuitReason enum is not exported by rmcp, so we'll just log it
            tracing::info!("MCP server stopped: {:?}", quit_reason);
        }
        Err(e) => {
            tracing::error!("MCP server task error: {}", e);
            return EXIT_WARNING;
        }
    }

    tracing::info!("MCP server shutting down gracefully");
    EXIT_SUCCESS
}

#[cfg(not(feature = "dynamic-cli"))]
fn run_doctor_with_options(migration: bool) -> i32 {
    use doctor::Doctor;

    let mut doctor = Doctor::new();
    match doctor.run_diagnostics_with_options(migration) {
        Ok(exit_code) => exit_code,
        Err(e) => {
            tracing::error!("Doctor error: {}", e);
            EXIT_ERROR
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_prompt(subcommand: cli::PromptSubcommand) -> i32 {
    use error::handle_cli_result;
    use prompt;

    handle_cli_result(prompt::run_prompt_command(subcommand).await)
}

#[cfg(not(feature = "dynamic-cli"))]
fn run_completions(shell: clap_complete::Shell) -> i32 {
    use completions;

    match completions::print_completion(shell) {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Completion error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_flow(subcommand: cli::FlowSubcommand) -> i32 {
    use flow;

    match flow::run_flow_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            // Check if this is an abort error (file-based detection)
            if let SwissArmyHammerError::ExecutorError(
                swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
            ) = &e
            {
                tracing::error!("Workflow aborted: {}", abort_reason);
                return EXIT_ERROR;
            }
            tracing::error!("Flow error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_issue(subcommand: cli::IssueCommands) -> i32 {
    use error::CliError;
    use issue;

    match issue::handle_issue_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            // Check if this is a CliError and preserve exit code
            if let Some(cli_error) = e.downcast_ref::<CliError>() {
                tracing::error!("Issue error: {}", cli_error);
                cli_error.exit_code
            } else {
                tracing::error!("Issue error: {}", e);
                EXIT_WARNING
            }
        }
    }
}



#[cfg(not(feature = "dynamic-cli"))]
async fn run_file(subcommand: cli::FileCommands) -> i32 {
    use file;

    match file::handle_file_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("File error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_search(subcommand: cli::SearchCommands) -> i32 {
    use search;

    search::run_search(subcommand).await
}

/// Runs the validate command to check prompt files and workflows for syntax and best practices.
///
/// This function validates:
/// - All prompt files from builtin, user, and local directories
/// - YAML front matter syntax (skipped for .liquid files with {% partial %} marker)
/// - Required fields (title, description)
/// - Template variables match arguments
/// - Liquid template syntax
/// - Workflow structure and connectivity in .mermaid files
///
/// # Arguments
///
/// * `quiet` - Only show errors, no warnings or info
/// * `format` - Output format (text or json)
/// * `workflow_dirs` - \[DEPRECATED\] This parameter is ignored
///
/// # Returns
///
/// Exit code:
/// - 0: Success (no errors or warnings)
/// - 1: Warnings found
/// - 2: Errors found
#[cfg(not(feature = "dynamic-cli"))]
fn run_validate(quiet: bool, format: cli::ValidateFormat, workflow_dirs: Vec<String>) -> i32 {
    use validate;

    match validate::run_validate_command_with_dirs(quiet, format, workflow_dirs) {
        Ok(exit_code) => exit_code,
        Err(e) => {
            tracing::error!("Validate error: {}", e);
            EXIT_ERROR
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_plan(plan_filename: String) -> i32 {
    use cli::FlowSubcommand;
    use flow;
    use swissarmyhammer::error::{ErrorSeverity, PlanCommandError};
    use swissarmyhammer::plan_utils::{
        validate_issues_directory, validate_plan_file_comprehensive,
    };

    // Comprehensive plan file validation
    let validated_file = match validate_plan_file_comprehensive(&plan_filename, None) {
        Ok(file) => file,
        Err(e) => {
            // Display user-friendly error with color support
            let use_color = cli::Cli::should_use_color();
            eprintln!("{}", e.display_to_user(use_color));

            // Log the error for debugging
            e.log_error();

            // Return appropriate exit code based on severity
            return match e.severity() {
                ErrorSeverity::Warning => EXIT_WARNING,
                ErrorSeverity::Error => EXIT_ERROR,
                ErrorSeverity::Critical => EXIT_ERROR,
            };
        }
    };

    // Validate issues directory
    match validate_issues_directory() {
        Ok(_) => {
            tracing::debug!("Issues directory validation successful");
        }
        Err(e) => {
            // Display user-friendly error
            let use_color = cli::Cli::should_use_color();
            eprintln!("{}", e.display_to_user(use_color));

            // Log the error for debugging
            e.log_error();

            return EXIT_ERROR;
        }
    }

    // Create a FlowSubcommand::Run with the validated plan_filename variable
    let plan_var = format!("plan_filename={}", validated_file.path.display());

    let subcommand = FlowSubcommand::Run {
        workflow: "plan".to_string(),
        vars: vec![plan_var],
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };

    tracing::info!(
        "Executing plan workflow for file: {}",
        validated_file.path.display()
    );
    tracing::debug!("Plan file size: {} bytes", validated_file.size);

    match flow::run_flow_command(subcommand).await {
        Ok(_) => {
            tracing::info!("Plan workflow completed successfully");
            EXIT_SUCCESS
        }
        Err(e) => {
            // Check if this is an abort error (file-based detection)
            if let SwissArmyHammerError::ExecutorError(
                swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
            ) = &e
            {
                // Create and display a PlanCommandError for workflow failures
                let plan_error = PlanCommandError::WorkflowExecutionFailed {
                    plan_filename: plan_filename.clone(),
                    source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                        reason: abort_reason.clone(),
                    },
                };

                let use_color = cli::Cli::should_use_color();
                eprintln!("{}", plan_error.display_to_user(use_color));
                plan_error.log_error();

                return EXIT_ERROR;
            }

            // For other workflow errors, also wrap them
            let plan_error = PlanCommandError::WorkflowExecutionFailed {
                plan_filename: plan_filename.clone(),
                source: swissarmyhammer::error::WorkflowError::ExecutionFailed {
                    reason: e.to_string(),
                },
            };

            let use_color = cli::Cli::should_use_color();
            eprintln!("{}", plan_error.display_to_user(use_color));
            plan_error.log_error();

            EXIT_ERROR
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_config(subcommand: cli::ConfigCommands) -> i32 {
    use config;

    match config::handle_config_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Config error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_implement() -> i32 {
    use cli::FlowSubcommand;
    use flow;

    // Create a FlowSubcommand::Run equivalent to 'sah flow run implement'
    let subcommand = FlowSubcommand::Run {
        workflow: "implement".to_string(),
        vars: Vec::new(),
        interactive: false,
        dry_run: false,
        test: false,
        timeout: None,
        quiet: false,
    };

    tracing::info!("Executing implement workflow");

    match flow::run_flow_command(subcommand).await {
        Ok(_) => {
            tracing::info!("Implement workflow completed successfully");
            EXIT_SUCCESS
        }
        Err(e) => {
            // Check if this is an abort error (file-based detection)
            if let SwissArmyHammerError::ExecutorError(
                swissarmyhammer::workflow::ExecutorError::Abort(abort_reason),
            ) = &e
            {
                tracing::error!("Implement workflow aborted: {}", abort_reason);
                return EXIT_ERROR;
            }
            tracing::error!("Implement workflow error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_shell(subcommand: cli::ShellCommands) -> i32 {
    use shell;

    match shell::handle_shell_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Shell error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_web_search(subcommand: cli::WebSearchCommands) -> i32 {
    use web_search;

    match web_search::handle_web_search_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Web search error: {}", e);
            EXIT_WARNING
        }
    }
}

#[cfg(not(feature = "dynamic-cli"))]
async fn run_migrate(subcommand: cli::MigrateCommands) -> i32 {
    match migrate::handle_migrate_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Migration error: {}", e);
            EXIT_ERROR
        }
    }
}
