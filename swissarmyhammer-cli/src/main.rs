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

/// Configuration constants for the plan command  
struct PlanConfig {
    workflow_name: &'static str,
    filename_var: &'static str,
    template_vars_key: &'static str,
}

impl Default for PlanConfig {
    fn default() -> Self {
        Self {
            workflow_name: "plan",
            filename_var: "plan_filename",
            template_vars_key: "_template_vars",
        }
    }
}

/// Helper function to handle common error patterns
fn log_error_and_exit(message: &str, error: impl std::fmt::Display) -> i32 {
    tracing::error!("{}: {}", message, error);
    EXIT_ERROR
}

/// Validate a plan file path for existence, type, and readability
/// 
/// This function provides comprehensive validation of plan file paths including:
/// - File existence checking
/// - Directory vs file validation
/// - Permission checking for readability
/// - Support for both relative and absolute paths
/// - Clear error messages with actionable suggestions
fn validate_plan_file(plan_filename: &str) -> Result<std::path::PathBuf, crate::error::CliError> {
    use crate::error::CliError;
    use crate::exit_codes::EXIT_ERROR;
    
    let path = std::path::Path::new(plan_filename);
    
    // Check if path exists
    if !path.exists() {
        return Err(CliError::new(
            format!(
                "Plan file not found: {plan_filename}\nSuggestion: Check the file path and ensure the file exists"
            ),
            EXIT_ERROR,
        ));
    }
    
    // Check if it's a file (not directory)
    if !path.is_file() {
        return Err(CliError::new(
            format!(
                "Path is not a file: {plan_filename}\nSuggestion: Path must point to a markdown file, not a directory"
            ),
            EXIT_ERROR,
        ));
    }
    
    // Check readability by attempting to open the file
    match std::fs::File::open(path) {
        Ok(_) => Ok(path.to_path_buf()),
        Err(e) => Err(CliError::new(
            format!(
                "Permission denied accessing file: {plan_filename}\nError: {e}\nSuggestion: Check file permissions and ensure you can read the file"
            ),
            EXIT_ERROR,
        )),
    }
}

async fn run_plan(plan_filename: String) -> i32 {
    use std::collections::HashMap;
    use swissarmyhammer::workflow::{WorkflowExecutor, WorkflowName, WorkflowStorage};

    let config = PlanConfig::default();

    // Validate the plan file comprehensively
    let validated_path = match validate_plan_file(&plan_filename) {
        Ok(path) => path,
        Err(e) => {
            tracing::error!("{}", e);
            return e.exit_code;
        }
    };

    // Create workflow storage
    let workflow_storage = match WorkflowStorage::file_system() {
        Ok(storage) => storage,
        Err(e) => return log_error_and_exit("Failed to create workflow storage", e),
    };

    // Load the workflow
    let workflow_name = WorkflowName::new(config.workflow_name);
    let workflow = match workflow_storage.get_workflow(&workflow_name) {
        Ok(workflow) => workflow,
        Err(e) => {
            return log_error_and_exit(
                &format!("Failed to load '{}' workflow", config.workflow_name),
                e,
            )
        }
    };

    // Create executor and start workflow
    let mut executor = WorkflowExecutor::new();
    let mut run = match executor.start_workflow(workflow.clone()) {
        Ok(run) => run,
        Err(e) => return log_error_and_exit("Failed to start workflow", e),
    };

    // Set the plan_filename parameter as a template variable using the validated path
    let mut template_variables = HashMap::new();
    template_variables.insert(
        config.filename_var.to_string(),
        serde_json::Value::String(validated_path.to_string_lossy().to_string()),
    );

    // Store template variables in context for liquid template rendering
    run.context.insert(
        config.template_vars_key.to_string(),
        serde_json::to_value(template_variables)
            .unwrap_or(serde_json::Value::Object(Default::default())),
    );

    // Execute the workflow step by step until completion
    use swissarmyhammer::workflow::WorkflowRunStatus;
    while run.status == WorkflowRunStatus::Running {
        if let Err(e) = executor.execute_state(&mut run).await {
            return log_error_and_exit(
                &format!("Workflow execution failed at state '{}'", run.current_state),
                e,
            );
        }
    }

    // Check final status
    match run.status {
        WorkflowRunStatus::Completed => {
            println!("✅ Plan workflow completed successfully");
            EXIT_SUCCESS
        }
        WorkflowRunStatus::Failed => {
            tracing::error!("Workflow failed");
            EXIT_ERROR
        }
        WorkflowRunStatus::Cancelled => {
            tracing::error!("Workflow was cancelled");
            EXIT_ERROR
        }
        _ => {
            tracing::error!("Workflow ended in unexpected state: {:?}", run.status);
            EXIT_ERROR
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::{tempdir, TempDir};

    /// Helper to create a temporary directory with test files
    struct TestEnvironment {
        _temp_dir: TempDir,
        temp_path: std::path::PathBuf,
    }

    impl TestEnvironment {
        fn new() -> Self {
            let temp_dir = tempdir().expect("Failed to create temp directory");
            let temp_path = temp_dir.path().to_path_buf();
            Self {
                _temp_dir: temp_dir,
                temp_path,
            }
        }

        fn create_file(&self, filename: &str, content: &str) -> std::path::PathBuf {
            let file_path = self.temp_path.join(filename);
            let mut file = File::create(&file_path).expect("Failed to create test file");
            file.write_all(content.as_bytes()).expect("Failed to write test file");
            file_path
        }

        fn create_directory(&self, dirname: &str) -> std::path::PathBuf {
            let dir_path = self.temp_path.join(dirname);
            fs::create_dir(&dir_path).expect("Failed to create test directory");
            dir_path
        }

        fn get_path(&self, name: &str) -> std::path::PathBuf {
            self.temp_path.join(name)
        }
    }

    #[test]
    fn test_validate_plan_file_valid_file() {
        let env = TestEnvironment::new();
        let file_path = env.create_file("valid_plan.md", "# Test Plan\n\nThis is a test plan.");
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok());
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
    }

    #[test]
    fn test_validate_plan_file_nonexistent_file() {
        let env = TestEnvironment::new();
        let nonexistent_path = env.get_path("nonexistent.md");
        
        let result = validate_plan_file(&nonexistent_path.to_string_lossy());
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert_eq!(error.exit_code, EXIT_ERROR);
        assert!(error.message.contains("Plan file not found"));
        assert!(error.message.contains("Check the file path and ensure the file exists"));
    }

    #[test]
    fn test_validate_plan_file_directory_instead_of_file() {
        let env = TestEnvironment::new();
        let dir_path = env.create_directory("test_directory");
        
        let result = validate_plan_file(&dir_path.to_string_lossy());
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert_eq!(error.exit_code, EXIT_ERROR);
        assert!(error.message.contains("Path is not a file"));
        assert!(error.message.contains("Path must point to a markdown file, not a directory"));
    }

    #[test]
    fn test_validate_plan_file_relative_path() {
        let env = TestEnvironment::new();
        let _file_path = env.create_file("relative_plan.md", "# Relative Plan");
        
        // Change to the temp directory to test relative paths
        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(&env.temp_path).expect("Failed to change directory");
        
        let result = validate_plan_file("relative_plan.md");
        
        // Restore original directory
        std::env::set_current_dir(original_dir).expect("Failed to restore directory");
        
        assert!(result.is_ok());
        let validated_path = result.unwrap();
        assert!(validated_path.ends_with("relative_plan.md"));
    }

    #[test]
    fn test_validate_plan_file_absolute_path() {
        let env = TestEnvironment::new();
        let file_path = env.create_file("absolute_plan.md", "# Absolute Plan");
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok());
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
        assert!(validated_path.is_absolute());
    }

    #[test]
    fn test_validate_plan_file_empty_filename() {
        let result = validate_plan_file("");
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        assert_eq!(error.exit_code, EXIT_ERROR);
        assert!(error.message.contains("Plan file not found"));
    }

    #[test]
    fn test_validate_plan_file_various_extensions() {
        let env = TestEnvironment::new();
        
        // Test various file extensions - all should work as we don't enforce .md
        let extensions = vec!["plan.md", "spec.txt", "document.rst", "file"];
        
        for ext in extensions {
            let file_path = env.create_file(ext, &format!("Content for {}", ext));
            let result = validate_plan_file(&file_path.to_string_lossy());
            assert!(result.is_ok(), "Failed for extension: {}", ext);
        }
    }

    #[test]
    fn test_validate_plan_file_with_spaces_in_path() {
        let env = TestEnvironment::new();
        let file_path = env.create_file("plan with spaces.md", "# Plan with spaces");
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok());
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
    }

    #[test]
    fn test_validate_plan_file_with_special_characters() {
        let env = TestEnvironment::new();
        // Test with various special characters that are valid in filenames
        let file_path = env.create_file("plan-file_v2.1.md", "# Special chars plan");
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok());
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
    }

    #[test]
    fn test_validate_plan_file_empty_file() {
        let env = TestEnvironment::new();
        let file_path = env.create_file("empty_plan.md", "");
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok(), "Empty files should be valid");
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
    }

    #[test]
    fn test_validate_plan_file_large_file() {
        let env = TestEnvironment::new();
        let large_content = "x".repeat(10000); // 10KB file
        let file_path = env.create_file("large_plan.md", &large_content);
        
        let result = validate_plan_file(&file_path.to_string_lossy());
        assert!(result.is_ok(), "Large files should be valid");
        
        let validated_path = result.unwrap();
        assert_eq!(validated_path, file_path);
    }

    // Note: Testing permission denied scenarios is complex in unit tests
    // as it requires changing file permissions and may not work consistently
    // across different platforms and CI environments. These would be better
    // tested in integration tests with appropriate setup.

    #[test]
    fn test_error_message_format() {
        let env = TestEnvironment::new();
        let nonexistent_path = env.get_path("missing.md");
        
        let result = validate_plan_file(&nonexistent_path.to_string_lossy());
        assert!(result.is_err());
        
        let error = result.unwrap_err();
        let message = &error.message;
        
        // Check that error message contains expected components
        assert!(message.contains("Plan file not found"));
        assert!(message.contains(&nonexistent_path.to_string_lossy().to_string()));
        assert!(message.contains("Suggestion:"));
    }

    #[test]
    fn test_cli_error_properties() {
        use crate::error::CliError;
        
        let error = CliError::new("Test error message", EXIT_ERROR);
        assert_eq!(error.message, "Test error message");
        assert_eq!(error.exit_code, EXIT_ERROR);
        assert!(error.source.is_none());
        
        // Test Display trait
        assert_eq!(format!("{}", error), "Test error message");
    }

    #[test]
    fn test_plan_config_defaults() {
        let config = PlanConfig::default();
        assert_eq!(config.workflow_name, "plan");
        assert_eq!(config.filename_var, "plan_filename");
        assert_eq!(config.template_vars_key, "_template_vars");
    }
}
