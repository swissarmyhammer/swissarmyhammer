use std::process;
mod cli;
mod cli_builder;
mod completions;
mod config;
mod doctor;
mod dynamic_execution;
mod error;
mod exit_codes;
mod flow;
mod list;
mod logging;
mod mcp_integration;
mod parameter_cli;
// prompt_loader module removed - using SDK's PromptResolver directly
mod prompt;
mod response_formatting;
mod schema_conversion;
mod search;
mod signal_handler;
mod test;
mod validate;

use clap::CommandFactory;
use cli::Cli;
use cli_builder::CliBuilder;
use dynamic_execution::{is_dynamic_command, DynamicCommandExecutor};
use exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};
use std::sync::Arc;
use swissarmyhammer::SwissArmyHammerError;
use swissarmyhammer_tools::{ToolContext, ToolRegistry};

/// Initialize MCP infrastructure (tool registry and context)
///
/// This function uses resilient initialization to ensure the dynamic CLI system
/// works even when some components fail to initialize. It provides fallbacks
/// and default implementations to maximize availability.
async fn create_mcp_infrastructure(
) -> Result<(Arc<ToolRegistry>, Arc<ToolContext>), Box<dyn std::error::Error>> {
    use swissarmyhammer::common::rate_limiter::get_rate_limiter;
    use swissarmyhammer::git::GitOperations;
    use swissarmyhammer::issues::{FileSystemIssueStorage, IssueStorage};
    use swissarmyhammer::memoranda::{
        mock_storage::MockMemoStorage, MarkdownMemoStorage, MemoStorage,
    };
    use swissarmyhammer_tools::mcp::tool_handlers::ToolHandlers;
    use swissarmyhammer_tools::{
        register_file_tools, register_issue_tools, register_memo_tools, register_notify_tools,
        register_search_tools, register_shell_tools, register_todo_tools, register_web_fetch_tools,
        register_web_search_tools,
    };
    use tokio::sync::{Mutex, RwLock};

    // Get current working directory - use current dir or temp as fallback
    let work_dir = std::env::current_dir().unwrap_or_else(|_| {
        tracing::warn!("Failed to get current directory, using /tmp as fallback");
        std::path::PathBuf::from("/tmp")
    });

    // Initialize issue storage with resilient fallback
    let issue_storage: Box<dyn IssueStorage> = {
        let issues_dir = work_dir.join("issues");
        match FileSystemIssueStorage::new(issues_dir.clone()) {
            Ok(fs_storage) => {
                tracing::debug!("Using filesystem issue storage");
                Box::new(fs_storage)
            }
            Err(e) => {
                tracing::debug!(
                    "Filesystem issue storage failed ({}), creating directory and retrying",
                    e
                );
                // Try to create the directory and retry
                if let Err(mkdir_err) = std::fs::create_dir_all(&issues_dir) {
                    tracing::warn!("Failed to create issues directory: {}", mkdir_err);
                }

                match FileSystemIssueStorage::new(issues_dir) {
                    Ok(fs_storage) => {
                        tracing::debug!("Successfully created filesystem issue storage after directory creation");
                        Box::new(fs_storage)
                    }
                    Err(retry_err) => {
                        tracing::warn!(
                            "Issue storage still failed, using temp directory fallback: {}",
                            retry_err
                        );
                        // Try temp directory as final fallback
                        let temp_issues_dir = std::env::temp_dir().join("sah-issues");
                        if let Err(temp_err) = std::fs::create_dir_all(&temp_issues_dir) {
                            tracing::error!("Failed to create temp issues directory: {}", temp_err);
                            return Err(format!(
                                "Failed to initialize any issue storage: {retry_err}"
                            )
                            .into());
                        }

                        match FileSystemIssueStorage::new(temp_issues_dir) {
                            Ok(temp_storage) => {
                                tracing::debug!("Using temporary directory issue storage");
                                Box::new(temp_storage)
                            }
                            Err(temp_storage_err) => {
                                tracing::error!("Even temp storage failed: {}", temp_storage_err);
                                return Err(format!(
                                    "Failed to initialize any issue storage: {retry_err}"
                                )
                                .into());
                            }
                        }
                    }
                }
            }
        }
    };
    let issue_storage = Arc::new(RwLock::new(issue_storage));

    // Initialize memo storage with resilient fallback
    let memo_storage: Box<dyn MemoStorage> = {
        match MarkdownMemoStorage::new_default() {
            Ok(fs_storage) => {
                tracing::debug!("Using filesystem memo storage");
                Box::new(fs_storage)
            }
            Err(e) => {
                tracing::debug!("Filesystem memo storage failed ({}), using mock storage", e);
                Box::new(MockMemoStorage::new())
            }
        }
    };
    let memo_storage = Arc::new(RwLock::new(memo_storage));

    // Initialize git operations (optional for CLI - None is acceptable)
    let git_ops = GitOperations::with_work_dir(work_dir.clone())
        .map_err(|e| tracing::debug!("Git operations not available: {}", e))
        .ok();
    let git_ops = Arc::new(Mutex::new(git_ops));

    // Create tool handlers
    let tool_handlers = ToolHandlers::new(memo_storage.clone());

    // Initialize tool registry and context
    let mut tool_registry = ToolRegistry::new();
    let tool_context = Arc::new(ToolContext::new(
        Arc::new(tool_handlers),
        issue_storage,
        git_ops,
        memo_storage,
        get_rate_limiter().clone(),
    ));

    // Register all tools - continue even if individual tools fail
    let tools_to_register = [
        ("file", register_file_tools as fn(&mut ToolRegistry)),
        ("issue", register_issue_tools),
        ("memo", register_memo_tools),
        ("notify", register_notify_tools),
        ("search", register_search_tools),
        ("shell", register_shell_tools),
        ("todo", register_todo_tools),
        ("web_fetch", register_web_fetch_tools),
        ("web_search", register_web_search_tools),
    ];

    for (name, register_fn) in tools_to_register {
        match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            register_fn(&mut tool_registry);
        })) {
            Ok(()) => {
                tracing::debug!("Registered {} tools successfully", name);
            }
            Err(e) => {
                tracing::warn!("Failed to register {} tools: {:?}", name, e);
                // Continue with other tools
            }
        }
    }

    // Always return success - partial functionality is better than no functionality
    tracing::debug!("MCP infrastructure initialized successfully");

    Ok((Arc::new(tool_registry), tool_context))
}

/// Try to parse and execute command using dynamic CLI builder
/// Returns Some(exit_code) if handled, None if should fallback to static CLI
async fn try_dynamic_cli(args: &[String]) -> Option<i32> {
    use tokio::time::{timeout, Duration};

    tracing::debug!("Starting try_dynamic_cli with args: {:?}", args);

    // Enable dynamic CLI for E2E tests (subprocess calls with SWISSARMYHAMMER_TEST_MODE set)
    let is_e2e_test = std::env::var("SWISSARMYHAMMER_TEST_MODE").is_ok();
    if is_e2e_test {
        tracing::debug!("E2E test mode detected, ensuring dynamic CLI is enabled");
    }

    // Disable dynamic CLI during unit tests to prevent hanging, but allow it for E2E tests
    if cfg!(test) && !is_e2e_test {
        tracing::debug!("Unit test environment detected, disabling dynamic CLI");
        return None;
    }

    // Only disable dynamic CLI when explicitly requested
    if std::env::var("SAH_DISABLE_DYNAMIC_CLI").is_ok() {
        tracing::debug!("Dynamic CLI explicitly disabled via SAH_DISABLE_DYNAMIC_CLI, falling back to static CLI");
        return None;
    }

    // Attempt to create MCP infrastructure with timeout
    // Use longer timeout in test/CI environments
    let is_ci = std::env::var("CI").is_ok();
    let has_cargo_target = std::env::var("CARGO_TARGET_DIR").is_ok();
    let has_test_in_args = args.iter().any(|arg| arg.contains("test"));
    // Additional test environment detection - CLI subprocess tests set this flag
    let is_cli_test = is_e2e_test;

    // Check for explicit timeout override first
    let timeout_secs = if let Ok(timeout_str) = std::env::var("SAH_MCP_TIMEOUT") {
        timeout_str.parse::<u64>().unwrap_or(300)
    } else if is_ci || has_cargo_target || has_test_in_args || is_cli_test {
        tracing::debug!("Detected test/CI environment (CI={}, CARGO_TARGET_DIR={}, test_in_args={}, cli_test={}), using 300s timeout", is_ci, has_cargo_target, has_test_in_args, is_cli_test);
        300 // 5 minutes for CI/test environments - tests can be very slow due to cold start and compilation
    } else {
        tracing::debug!("Normal CLI usage detected, using 10s timeout");
        10 // 10 seconds for normal CLI usage
    };

    let infrastructure_future = create_mcp_infrastructure();
    let (tool_registry, tool_context) =
        match timeout(Duration::from_secs(timeout_secs), infrastructure_future).await {
            Ok(Ok((registry, context))) => {
                tracing::debug!("MCP infrastructure initialization successful");
                (registry, context)
            }
            Ok(Err(e)) => {
                tracing::debug!(
                    "MCP infrastructure initialization failed: {}, falling back to static CLI",
                    e
                );
                // In test environments, also warn so we can see what's failing
                if is_ci || has_cargo_target || has_test_in_args || is_cli_test {
                    tracing::warn!(
                        "MCP infrastructure initialization failed in test environment: {}",
                        e
                    );
                }
                return None;
            }
            Err(_) => {
                tracing::debug!(
                "MCP infrastructure initialization timed out after {}s, falling back to static CLI",
                timeout_secs
            );
                // In test environments, also warn about timeouts
                if is_ci || has_cargo_target || has_test_in_args || is_cli_test {
                    tracing::warn!(
                        "MCP infrastructure initialization timed out after {}s in test environment",
                        timeout_secs
                    );
                }
                return None;
            }
        };

    // Create dynamic CLI builder
    let cli_builder = CliBuilder::new(tool_registry.clone());
    let categories = tool_registry.get_cli_categories();
    tracing::debug!(
        "About to build dynamic CLI with {} categories: {:?}",
        categories.len(),
        categories
    );
    let cli = match cli_builder.build_cli() {
        Ok(cli) => {
            tracing::debug!("Dynamic CLI built successfully");
            cli
        }
        Err(e) => {
            tracing::debug!(
                "Dynamic CLI build failed: {}, falling back to static CLI",
                e
            );
            return None;
        }
    };

    // Try to parse arguments with dynamic CLI
    tracing::debug!("Trying to parse args with dynamic CLI: {:?}", args);

    let matches = match cli.try_get_matches_from(args) {
        Ok(matches) => {
            tracing::debug!("Dynamic CLI parsing successful");
            tracing::debug!("Subcommand name: {:?}", matches.subcommand_name());
            matches
        }
        Err(e) => {
            tracing::debug!("Dynamic CLI parsing failed: {}", e);

            // Check if this is just a help display or version request
            use clap::error::ErrorKind;
            tracing::debug!("Error kind: {:?}", e.kind());
            match e.kind() {
                ErrorKind::DisplayHelp => {
                    // Print help to stdout (where clap normally sends --help)
                    println!("{e}");
                    tracing::debug!("Help displayed, exiting successfully");
                    return Some(EXIT_SUCCESS);
                }
                ErrorKind::DisplayVersion => {
                    // Print version to stdout (where clap normally sends --version)
                    println!("{e}");
                    tracing::debug!("Version displayed, exiting successfully");
                    return Some(EXIT_SUCCESS);
                }
                _ => {
                    // Check if this is a validation error for a command we recognize
                    // If so, show the validation error instead of falling back to static CLI
                    if let Some(subcommand) = args.get(1) {
                        let categories = tool_registry.get_cli_categories();
                        if categories.contains(subcommand) {
                            // This is a dynamic command with validation error - show the error
                            eprintln!("{e}");
                            tracing::debug!(
                                "Dynamic command validation error shown, exiting with failure"
                            );
                            return Some(EXIT_ERROR);
                        }
                    }

                    // Real parsing error for unknown command, fall back to static CLI
                    tracing::debug!(
                        "Real parsing error, falling back to static CLI: {:?}",
                        e.kind()
                    );
                    return None;
                }
            }
        }
    };

    tracing::debug!("About to check if command is dynamic");

    // Check if this is a dynamic command
    let is_dynamic = is_dynamic_command(&matches, &cli_builder);
    tracing::debug!("Checking if command is dynamic: {}", is_dynamic);

    if is_dynamic {
        tracing::debug!("Command recognized as dynamic, extracting command info");
        let command_info = match cli_builder.extract_command_info(&matches) {
            Some(info) => {
                tracing::debug!("Successfully extracted command info: {:?}", info);
                info
            }
            None => {
                tracing::error!("Failed to extract command info for dynamic command");
                return Some(EXIT_ERROR);
            }
        };

        tracing::debug!("About to execute dynamic command with executor");
        // Execute dynamic command
        let executor = DynamicCommandExecutor::new(tool_registry, tool_context);
        match executor.execute_command(command_info, &matches).await {
            Ok(()) => {
                tracing::debug!("Dynamic command execution completed successfully");
                Some(EXIT_SUCCESS)
            }
            Err(e) => {
                // Print user-friendly error message to stderr
                eprintln!("Error: {e}");
                // Also log for debugging
                tracing::error!("Dynamic command execution failed: {}", e);
                Some(EXIT_ERROR)
            }
        }
    } else {
        tracing::debug!("Command not recognized as dynamic, falling back to static CLI");
        // This is a static command, let static CLI handle it
        None
    }
}

/// Handle original static CLI commands
async fn handle_original_command(cli: Cli) -> i32 {
    use cli::Commands;

    // Set up logging based on CLI flags
    if !cli.quiet {
        setup_logging(cli.verbose, cli.debug).unwrap_or_else(|_| {
            eprintln!("Warning: Failed to initialize logging");
        });
    }

    match cli.command {
        None | Some(Commands::Serve) => run_server().await,
        Some(Commands::Doctor) => run_doctor(),
        Some(Commands::Prompt { subcommand }) => run_prompt(subcommand).await,
        Some(Commands::Completion { shell }) => run_completions(shell),
        Some(Commands::Flow { subcommand }) => run_flow(subcommand).await,
        Some(Commands::Validate {
            format,
            quiet,
            workflow_dirs,
        }) => run_validate(quiet, format, workflow_dirs),
        Some(Commands::Plan { plan_filename }) => run_plan(plan_filename).await,
        Some(Commands::Config { subcommand }) => run_config(subcommand).await,
        Some(Commands::Implement) => run_implement().await,
        // Note: Dynamic commands are handled in try_dynamic_cli, not here
    }
}

/// Set up logging based on CLI arguments
fn setup_logging(verbose: bool, debug: bool) -> Result<(), Box<dyn std::error::Error>> {
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    let log_level = if debug {
        "debug"
    } else if verbose {
        "info"
    } else {
        "warn"
    };

    let env_filter = std::env::var("RUST_LOG")
        .map(|env_log| format!("ort=warn,rmcp=warn,{env_log}"))
        .unwrap_or_else(|_| format!("ort=warn,rmcp=warn,{log_level}"));

    let _ = registry()
        .with(EnvFilter::new(env_filter))
        .with(fmt::layer().with_writer(std::io::stderr))
        .try_init();

    Ok(())
}

/// Set up MCP-specific file logging to .swissarmyhammer/mcp.log
fn setup_mcp_logging() -> Result<(), Box<dyn std::error::Error>> {
    use logging::FileWriterGuard;
    use std::fs::{create_dir_all, File};
    use std::sync::{Arc, Mutex};
    use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};

    // Create .swissarmyhammer directory in current working directory
    let current_dir = std::env::current_dir()?;
    let log_dir = current_dir.join(".swissarmyhammer");
    create_dir_all(&log_dir)?;

    // Get log file name from environment variable or use default
    let log_filename =
        std::env::var("SWISSARMYHAMMER_LOG_FILE").unwrap_or_else(|_| "mcp.log".to_string());
    let log_file_path = log_dir.join(log_filename);

    // Create the log file
    let file = File::create(&log_file_path)?;
    let shared_file = Arc::new(Mutex::new(file));
    let file_writer = FileWriterGuard::new(shared_file);

    // Set up environment filter - default to info level for MCP server
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());
    let env_filter = format!("ort=warn,rmcp=warn,{log_level}");

    // Initialize tracing with file output
    registry()
        .with(EnvFilter::new(env_filter))
        .with(fmt::layer().with_writer(file_writer))
        .try_init()?;

    Ok(())
}

/// Try to handle command using dynamic CLI system
/// Returns Some(exit_code) if command was handled, None if should fall back to static CLI

#[tokio::main]
async fn main() {
    // Try dynamic CLI first, since it includes more commands
    let args: Vec<String> = std::env::args().collect();

    // Fast path for basic help/version without arguments
    if args.len() <= 1 {
        let mut cmd = Cli::command();
        let _ = cmd.write_help(&mut std::io::stdout());
        println!(); // Add newline
        process::exit(EXIT_SUCCESS);
    }

    // Check if this is the serve command and set up appropriate logging
    let is_serve_command = args.len() >= 2 && args[1] == "serve";

    // Initialize logging appropriately
    if is_serve_command {
        // Set up MCP file logging for serve command
        if let Err(e) = setup_mcp_logging() {
            eprintln!("Warning: Failed to initialize MCP logging: {e}");
            // Fallback to stderr logging
            use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};
            let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
            let _ = registry()
                .with(EnvFilter::new(format!("ort=warn,rmcp=warn,{log_level}")))
                .with(fmt::layer().with_writer(std::io::stderr))
                .try_init();
        }
    } else {
        // Initialize basic logging for non-serve commands
        use tracing_subscriber::{fmt, prelude::*, registry, EnvFilter};
        let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "warn".to_string());
        let _ = registry()
            .with(EnvFilter::new(format!("ort=warn,rmcp=warn,{log_level}")))
            .with(fmt::layer().with_writer(std::io::stderr))
            .try_init();
    }

    // Try to parse with dynamic CLI first
    let dynamic_result = try_dynamic_cli(&args).await;
    if let Some(exit_code) = dynamic_result {
        process::exit(exit_code);
    }

    // Fall back to static CLI parsing
    let cli = Cli::parse_args();

    let exit_code = handle_original_command(cli).await;
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


