use std::process;
mod cli;
mod completions;
mod config;
mod doctor;
mod error;
mod exit_codes;
mod flow;
mod issue;
mod list;
mod logging;
mod mcp_integration;
mod memo;
// prompt_loader module removed - using SDK's PromptResolver directly
mod prompt;
mod search;
mod signal_handler;
mod test;
mod validate;

use clap::CommandFactory;
use cli::{Cli, Commands};
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use logging::FileWriterGuard;
use swissarmyhammer::SwissArmyHammerError;

#[tokio::main]
async fn main() {
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
    let create_filter = || {
        if cli.debug {
            // When --debug is used, all crates including ORT get debug level
            EnvFilter::new(format!("{log_level}"))
        } else {
            // Otherwise, set ORT to WARN and everything else to the requested level
            EnvFilter::new(format!("ort=warn,{log_level}"))
        }
    };

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
            tracing::info!("Starting MCP server");
            run_server().await
        }
        Some(Commands::Doctor) => {
            tracing::info!("Running diagnostics");
            run_doctor()
        }
        Some(Commands::Prompt { subcommand }) => {
            tracing::info!("Running prompt command");
            run_prompt(subcommand).await
        }
        Some(Commands::Completion { shell }) => {
            tracing::info!("Generating completion for {:?}", shell);
            run_completions(shell)
        }
        Some(Commands::Flow { subcommand }) => {
            tracing::info!("Running flow command");
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
        Some(Commands::Memo { subcommand }) => {
            tracing::info!("Running memo command");
            run_memo(subcommand).await
        }
        Some(Commands::Search { subcommand }) => {
            tracing::info!("Running search command");
            run_search(subcommand).await
        }
        Some(Commands::Config { subcommand }) => {
            tracing::info!("Running config command");
            run_config(subcommand).await
        }
        Some(Commands::Plan { plan_filename }) => {
            tracing::info!("Running plan command for file: {}", plan_filename);
            run_plan(plan_filename).await
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

fn run_doctor() -> i32 {
    use doctor::Doctor;

    let mut doctor = Doctor::new();
    match doctor.run_diagnostics() {
        Ok(exit_code) => exit_code,
        Err(e) => {
            tracing::error!("Doctor error: {}", e);
            EXIT_ERROR
        }
    }
}

async fn run_prompt(subcommand: cli::PromptSubcommand) -> i32 {
    use error::handle_cli_result;
    use prompt;

    handle_cli_result(prompt::run_prompt_command(subcommand).await)
}

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

async fn run_issue(subcommand: cli::IssueCommands) -> i32 {
    use issue;

    match issue::handle_issue_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Issue error: {}", e);
            EXIT_WARNING
        }
    }
}

async fn run_memo(subcommand: cli::MemoCommands) -> i32 {
    use memo;

    match memo::handle_memo_command(subcommand).await {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Memo error: {}", e);
            EXIT_WARNING
        }
    }
}

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

async fn run_plan(plan_filename: String) -> i32 {
    println!("Plan command called with file: {}", plan_filename);
    println!("Plan functionality will be implemented in a future step.");
    EXIT_SUCCESS
}
