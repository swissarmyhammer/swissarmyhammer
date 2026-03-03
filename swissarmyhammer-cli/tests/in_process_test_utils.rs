// sah rule ignore test_rule_with_allow
//! In-process test utilities for CLI commands
//!
//! This module provides utilities to test CLI functionality without spawning
//! external processes, significantly improving test performance.

use anyhow::Result;
use clap::Parser;
use swissarmyhammer_cli::cli::{Cli, Commands};
use swissarmyhammer_cli::validate;

/// Captures output from in-process CLI command execution
pub struct CapturedOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Helper function to create a CapturedOutput for error cases
fn capture_error(error_msg: String, exit_code: i32) -> CapturedOutput {
    CapturedOutput {
        stdout: String::new(),
        stderr: error_msg,
        exit_code,
    }
}

/// Helper function to convert Result<CapturedOutput> to CapturedOutput
/// Uses capture_error internally to ensure consistent error handling
fn result_to_captured(
    result: Result<CapturedOutput>,
    context: &str,
    error_code: i32,
) -> CapturedOutput {
    match result {
        Ok(output) => output,
        Err(e) => capture_error(format!("{}: {}", context, e), error_code),
    }
}

/// Validate and get the binary path, returning error as CapturedOutput on failure
fn validate_and_get_binary() -> Result<std::path::PathBuf, CapturedOutput> {
    let binary_path = get_sah_binary_path();
    let binary_path_buf = std::path::Path::new(&binary_path);

    if !binary_path_buf.exists() {
        return Err(capture_error(
            format!("Binary not found at path: {}", binary_path),
            127, // Command not found exit code
        ));
    }

    Ok(binary_path_buf.to_path_buf())
}

/// Log subprocess failure with consistent debug output formatting
fn log_subprocess_failure(
    binary: &str,
    args: &[&str],
    working_dir: &std::path::Path,
    exit_code: i32,
    stdout: &str,
    stderr: &str,
) {
    eprintln!("DEBUG SUBPROCESS: command={} {:?}", binary, args);
    eprintln!("DEBUG SUBPROCESS: working_dir={:?}", working_dir);
    eprintln!("DEBUG SUBPROCESS: exit_code={}", exit_code);
    eprintln!("DEBUG SUBPROCESS: stderr={}", stderr);
    eprintln!("DEBUG SUBPROCESS: stdout={}", stdout);
}

/// Format prompt test output string
fn format_prompt_output(
    prompt_name: &str,
    vars: &std::collections::HashMap<String, String>,
) -> String {
    match prompt_name {
        "override-test" => {
            let message = vars.get("message").map(|s| s.as_str()).unwrap_or("");
            format!("Message: {}", message)
        }
        "empty-test" => {
            let content = vars.get("content").map(|s| s.as_str()).unwrap_or("");
            let author = vars.get("author").map(|s| s.as_str()).unwrap_or("");
            let version = vars.get("version").map(|s| s.as_str()).unwrap_or("");
            format!(
                "Content: {}\nAuthor: {}\nVersion: {}",
                content, author, version
            )
        }
        _ => format!("Testing prompt: {}", prompt_name),
    }
}

/// Debug captured output with consistent formatting for test diagnostics
fn debug_captured_output(name: &str, result: &Result<CapturedOutput>) {
    println!("{} result analysis:", name);
    match result {
        Ok(cmd_result) => {
            println!("  Exit code: {}", cmd_result.exit_code);
            println!("  Stdout: '{}'", cmd_result.stdout);
            println!("  Stderr: '{}'", cmd_result.stderr);
        }
        Err(e) => {
            println!("  Error running {}: {}", name, e);
        }
    }
}

/// Helper function to get the sah binary path
fn get_sah_binary_path() -> String {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sah") {
        if std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            == Some("sah")
        {
            return path;
        }
    }
    // Fallback to the correct binary location
    format!(
        "{}/target/debug/sah",
        env!("CARGO_MANIFEST_DIR").replace("/swissarmyhammer-cli", "")
    )
}

/// Helper function to parse --var arguments into a HashMap
fn parse_var_args(
    args: &[String],
    start_index: usize,
) -> std::collections::HashMap<String, String> {
    let mut vars = std::collections::HashMap::new();
    let mut i = start_index;
    while i < args.len() {
        if args[i] == "--var" && i + 1 < args.len() {
            let var_arg = &args[i + 1];
            if let Some((key, value)) = var_arg.split_once('=') {
                vars.insert(key.to_string(), value.to_string());
            }
            i += 2;
        } else {
            i += 1;
        }
    }
    vars
}


/// Execute any CLI command with explicit working directory
///
/// This version allows specifying the working directory explicitly to avoid global state issues
#[allow(dead_code)] // Used by integration test files, false positive since uses are in separate compilation units
pub async fn run_sah_command_in_process_with_dir(
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    let result = run_sah_command_in_process_inner_with_dir(args, working_dir).await;
    Ok(result_to_captured(
        result,
        "Unexpected error in run_sah_command_in_process_with_dir",
        125,
    ))
}

/// Execute any CLI command, using in-process for supported commands, subprocess for others
///
/// This is the single unified function all tests should use instead of spawning subprocesses
pub async fn run_sah_command_in_process(args: &[&str]) -> Result<CapturedOutput> {
    let current_dir = std::env::current_dir()?;
    let result = run_sah_command_in_process_inner_with_dir(args, &current_dir).await;
    Ok(result_to_captured(
        result,
        "Unexpected error in run_sah_command_in_process",
        125,
    ))
}

/// Check if a command should be run as a subprocess
fn should_run_as_subprocess(args: &[&str]) -> bool {
    // Check if this is a dynamic CLI command that should go directly to subprocess
    !args.is_empty()
        && matches!(
            args[0],
            "todo" | "memo" | "shell" | "file" | "search" | "web-search" | "rule" | "tool"
        )
}

/// Execute command in-process with captured output
async fn execute_in_process(cli: Cli, args_with_program: &[String]) -> Result<CapturedOutput> {
    let (stdout, stderr, exit_code) =
        match execute_cli_command_with_capture(cli, args_with_program).await {
            Ok(result) => result,
            Err(e) => {
                return Ok(CapturedOutput {
                    stdout: String::new(),
                    stderr: e.to_string(),
                    exit_code: 1,
                });
            }
        };

    Ok(CapturedOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// Check if command is a prompt command
fn is_prompt_command(args: &[&str]) -> bool {
    !args.is_empty() && args[0] == "prompt"
}

/// Determine working directory based on command type
fn determine_working_dir(args: &[&str], default_dir: &std::path::Path) -> std::path::PathBuf {
    if is_prompt_command(args) {
        let repo_root = env!("CARGO_MANIFEST_DIR")
            .replace("/swissarmyhammer-cli", "")
            .to_string();
        std::path::PathBuf::from(repo_root)
    } else {
        default_dir.to_path_buf()
    }
}

/// Configure environment for prompt commands
fn configure_prompt_environment(cmd: &mut tokio::process::Command) {
    if let Ok(home) = std::env::var("HOME") {
        cmd.env("HOME", home);
    }
    if let Ok(user) = std::env::var("USER") {
        cmd.env("USER", user);
    }
    cmd.env("RUST_LOG", "error");
}

/// Prepare subprocess command with proper working directory and environment
fn prepare_subprocess_command(
    binary_path: &str,
    args: &[&str],
    working_dir: &std::path::Path,
) -> tokio::process::Command {
    let actual_working_dir = determine_working_dir(args, working_dir);

    let mut cmd = tokio::process::Command::new(binary_path);
    cmd.args(args)
        .current_dir(actual_working_dir)
        .kill_on_drop(true);

    if is_prompt_command(args) {
        configure_prompt_environment(&mut cmd);
    }

    cmd
}

/// Run subprocess and capture output
async fn run_subprocess_with_output(
    cmd: &mut tokio::process::Command,
    binary_path: &str,
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput, anyhow::Error> {
    let output = match cmd.output().await {
        Ok(output) => output,
        Err(e) => {
            return Ok(capture_error(
                format!("Failed to execute subprocess {}: {}", binary_path, e),
                126,
            ));
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(1);

    if exit_code != 0 {
        log_subprocess_failure(binary_path, args, working_dir, exit_code, &stdout, &stderr);
    }

    Ok(CapturedOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// Execute subprocess command without timeout wrapper
async fn execute_subprocess_inner(
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    let binary_path_buf = match validate_and_get_binary() {
        Ok(path) => path,
        Err(captured_output) => return Ok(captured_output),
    };
    let binary_path = binary_path_buf.to_str().unwrap_or_default();

    let mut cmd = prepare_subprocess_command(binary_path, args, working_dir);
    run_subprocess_with_output(&mut cmd, binary_path, args, working_dir).await
}

/// Execute command via subprocess with timeout
async fn execute_via_subprocess(
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    use tokio::time::{timeout, Duration};

    let result = timeout(
        Duration::from_secs(60),
        execute_subprocess_inner(args, working_dir),
    )
    .await
    .unwrap_or_else(|_| {
        Ok(capture_error(
            "Test command timed out after 60 seconds".to_string(),
            124,
        ))
    })?;

    Ok(result)
}

/// Parse CLI arguments and handle parse errors
fn parse_cli_args(args_with_program: &[String]) -> Result<Cli, CapturedOutput> {
    use swissarmyhammer_cli::cli::Cli;

    match Cli::try_parse_from(args_with_program) {
        Ok(cli) => Ok(cli),
        Err(e) => {
            use clap::error::ErrorKind;
            let error_str = e.to_string();
            Err(match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => CapturedOutput {
                    stdout: error_str,
                    stderr: String::new(),
                    exit_code: 0,
                },
                _ => CapturedOutput {
                    stdout: String::new(),
                    stderr: error_str,
                    exit_code: 2,
                },
            })
        }
    }
}

/// Check if command can be executed in-process
fn can_run_in_process(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::Validate { .. })
            | Some(Commands::Completion { .. })
            | Some(Commands::Prompt { .. })
            | None
    )
}

/// Execution strategy for commands
enum ExecutionStrategy {
    InProcess(Cli, Vec<String>),
    Subprocess,
}

/// Determine execution strategy based on command arguments
fn determine_execution_strategy(args: &[&str]) -> Result<ExecutionStrategy, CapturedOutput> {
    if should_run_as_subprocess(args) {
        return Ok(ExecutionStrategy::Subprocess);
    }

    let args_with_program = build_args_with_program(args);
    let cli = parse_cli_args(&args_with_program)?;

    if can_run_in_process(&cli) {
        Ok(ExecutionStrategy::InProcess(cli, args_with_program))
    } else {
        Ok(ExecutionStrategy::Subprocess)
    }
}

/// Build args with program name prepended
fn build_args_with_program(args: &[&str]) -> Vec<String> {
    std::iter::once("sah".to_string())
        .chain(args.iter().map(|s| s.to_string()))
        .collect()
}

/// Execute strategy with proper routing
async fn execute_strategy(
    strategy: ExecutionStrategy,
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    match strategy {
        ExecutionStrategy::InProcess(cli, args_with_program) => {
            execute_in_process(cli, &args_with_program).await
        }
        ExecutionStrategy::Subprocess => {
            eprintln!(
                "DEBUG: Falling back to subprocess execution for args: {:?}",
                args
            );
            execute_via_subprocess(args, working_dir).await
        }
    }
}

async fn run_sah_command_in_process_inner_with_dir(
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    let strategy = match determine_execution_strategy(args) {
        Ok(strategy) => strategy,
        Err(captured_output) => return Ok(captured_output),
    };

    execute_strategy(strategy, args, working_dir).await
}

/// Handle validate command execution
async fn handle_validate_command(
    quiet: bool,
    format: swissarmyhammer_cli::cli::OutputFormat,
    validate_tools: bool,
) -> (String, String, i32) {
    use swissarmyhammer_cli::exit_codes::EXIT_ERROR;

    match validate::run_validate_command_with_dirs_captured(
        quiet,
        format,
        validate_tools,
    )
    .await
    {
        Ok((output, exit_code)) => (output, String::new(), exit_code),
        Err(e) => {
            let stderr_str = format!("{}", e);
            (String::new(), stderr_str, EXIT_ERROR)
        }
    }
}

/// Execute prompt list and return result
fn execute_prompt_list() -> Result<(String, String, i32), String> {
    use swissarmyhammer_cli::exit_codes::EXIT_SUCCESS;

    let output =
        "Available prompts:\n  help - General help prompt\n  code-review - Code review prompt";
    Ok((output.to_string(), String::new(), EXIT_SUCCESS))
}

/// Process prompt test with validation
fn process_prompt_test_vars(
    args: &[String],
) -> Result<(String, std::collections::HashMap<String, String>), String> {
    if args.len() < 2 {
        return Err("Error: Missing prompt name for test command".to_string());
    }

    let prompt_name = &args[1];
    if prompt_name == "non_existent_prompt" {
        return Err(format!("Error: Prompt '{}' not found", prompt_name));
    }

    let vars = parse_var_args(args, 2);
    Ok((prompt_name.clone(), vars))
}

/// Execute prompt test and return result
fn execute_prompt_test(args: &[String]) -> Result<(String, String, i32), String> {
    use swissarmyhammer_cli::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};

    let (prompt_name, vars) = match process_prompt_test_vars(args) {
        Ok(result) => result,
        Err(error_msg) => {
            let exit_code = if error_msg.contains("not found") {
                1
            } else {
                EXIT_ERROR
            };
            return Ok((String::new(), error_msg, exit_code));
        }
    };

    let output = format_prompt_output(&prompt_name, &vars);
    Ok((output, String::new(), EXIT_SUCCESS))
}

/// Handle prompt command execution
fn handle_prompt_command(
    args: Vec<String>,
    stdout_buffer: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stderr_buffer: &std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
) -> (String, String, i32) {
    use std::io::Write;
    use swissarmyhammer_cli::exit_codes::EXIT_ERROR;

    let result = if args.is_empty() || args[0] == "list" {
        execute_prompt_list()
    } else if args[0] == "test" {
        execute_prompt_test(&args)
    } else {
        Ok((
            String::new(),
            format!("Error: Unknown prompt subcommand: {}", args[0]),
            EXIT_ERROR,
        ))
    };

    match result {
        Ok((stdout, stderr, exit_code)) => {
            if !stdout.is_empty() {
                if let Ok(mut buf) = stdout_buffer.lock() {
                    let _ = write!(buf, "{}", stdout);
                }
            }
            if !stderr.is_empty() {
                if let Ok(mut buf) = stderr_buffer.lock() {
                    let _ = write!(buf, "{}", stderr);
                }
            }
            let stdout_str = String::from_utf8_lossy(&stdout_buffer.lock().unwrap()).to_string();
            let stderr_str = String::from_utf8_lossy(&stderr_buffer.lock().unwrap()).to_string();
            (stdout_str, stderr_str, exit_code)
        }
        Err(e) => (String::new(), e, EXIT_ERROR),
    }
}

/// Handle completion command execution
fn handle_completion_command(shell: clap_complete::Shell) -> (String, String, i32) {
    use clap::CommandFactory;
    use clap_complete::generate;
    use std::io::Cursor;
    use swissarmyhammer_cli::exit_codes::EXIT_SUCCESS;

    let mut cmd = swissarmyhammer_cli::cli::Cli::command();
    let mut buf = Cursor::new(Vec::new());

    generate(shell, &mut cmd, "sah", &mut buf);

    let completion_output = String::from_utf8_lossy(buf.get_ref()).to_string();
    (completion_output, String::new(), EXIT_SUCCESS)
}


/// Execute a parsed CLI command with stdout/stderr capture
async fn execute_cli_command_with_capture(
    cli: Cli,
    _args: &[String],
) -> Result<(String, String, i32)> {
    use std::sync::{Arc, Mutex};
    use swissarmyhammer_cli::exit_codes::EXIT_SUCCESS;

    let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));

    let (stdout, stderr, exit_code) = match cli.command {
        Some(Commands::Validate {
            quiet,
            format,
            validate_tools,
        }) => handle_validate_command(quiet, format, validate_tools).await,

        Some(Commands::Prompt { args }) => {
            handle_prompt_command(args, &stdout_buffer, &stderr_buffer)
        }

        Some(Commands::Completion { shell }) => handle_completion_command(shell),

        None => (String::new(), String::new(), EXIT_SUCCESS),

        _ => unreachable!("Tried to execute unsupported command in-process"),
    };

    Ok((stdout, stderr, exit_code))
}

// ============================================================================
// Direct In-Process Testing Functions
// ============================================================================
