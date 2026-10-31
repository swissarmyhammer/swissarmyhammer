// sah rule ignore test_rule_with_allow
//! In-process test utilities for CLI commands
//!
//! This module provides utilities to test CLI functionality without spawning
//! external processes, significantly improving test performance.

use anyhow::{Context, Result};
use clap::Parser;
use swissarmyhammer_cli::cli::{Cli, Commands};
use swissarmyhammer_cli::exit_codes::{EXIT_ERROR, EXIT_SUCCESS};
use swissarmyhammer_cli::validate;

/// Exit code for a failure inside the test harness itself, not in the command under test.
const EXIT_INTERNAL_ERROR: i32 = 125;

/// Exit code for a subprocess that cannot be started.
const EXIT_CANNOT_EXECUTE: i32 = 126;

/// Exit code for a binary that does not exist (the shell "command not found" code).
const EXIT_COMMAND_NOT_FOUND: i32 = 127;

/// Exit code for a subprocess that exceeded its time limit (the `timeout` command code).
const EXIT_TIMED_OUT: i32 = 124;

/// Exit code for a general failure when no more specific code applies.
const EXIT_GENERAL_FAILURE: i32 = 1;

/// Time limit, in seconds, for a command run as a subprocess.
const SUBPROCESS_TIMEOUT_SECS: u64 = 60;

/// Name of the `sah` binary and the program name given to the CLI parser.
const SAH_BINARY_NAME: &str = "sah";

/// Prefix on every debug line that reports a failed subprocess.
const SUBPROCESS_DEBUG_PREFIX: &str = "DEBUG SUBPROCESS";

/// Captures output from in-process CLI command execution
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
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
        Err(e) => capture_error(format!("{context}: {e:#}"), error_code),
    }
}

/// Validate and get the binary path, returning error as CapturedOutput on failure
fn validate_and_get_binary() -> Result<std::path::PathBuf, CapturedOutput> {
    let binary_path = get_sah_binary_path();
    let binary_path_buf = std::path::Path::new(&binary_path);

    if !binary_path_buf.exists() {
        return Err(capture_error(
            format!("binary not found at path: {}", binary_path),
            EXIT_COMMAND_NOT_FOUND,
        ));
    }

    Ok(binary_path_buf.to_path_buf())
}

/// Log subprocess failure with consistent debug output formatting
fn log_subprocess_failure(
    binary: &str,
    args: &[&str],
    working_dir: &std::path::Path,
    output: &CapturedOutput,
) {
    eprintln!("{SUBPROCESS_DEBUG_PREFIX}: command={binary} {args:?}");
    eprintln!("{SUBPROCESS_DEBUG_PREFIX}: working_dir={working_dir:?}");
    eprintln!("{SUBPROCESS_DEBUG_PREFIX}: exit_code={}", output.exit_code);
    eprintln!("{SUBPROCESS_DEBUG_PREFIX}: stderr={}", output.stderr);
    eprintln!("{SUBPROCESS_DEBUG_PREFIX}: stdout={}", output.stdout);
}

/// Helper function to get the sah binary path
///
/// `tests/integration/sah_serve_integration.rs` and
/// `tests/integration/sah_serve_tools_validation.rs` keep their own copies, because
/// they build in a separate compilation unit. Change those copies to match any
/// change made here.
fn get_sah_binary_path() -> String {
    if let Ok(path) = std::env::var("CARGO_BIN_EXE_sah") {
        if std::path::Path::new(&path)
            .file_name()
            .and_then(|name| name.to_str())
            == Some(SAH_BINARY_NAME)
        {
            return path;
        }
    }
    // Fallback to the correct binary location
    format!(
        "{}/target/debug/{}",
        env!("CARGO_MANIFEST_DIR").replace("/apps/swissarmyhammer-cli", ""),
        SAH_BINARY_NAME
    )
}

/// Execute any CLI command with explicit working directory
///
/// This version allows specifying the working directory explicitly to avoid global state issues
#[allow(dead_code)] // Used by integration test files, false positive since uses are in separate compilation units
pub async fn run_sah_command_in_process_with_dir(
    args: &[&str],
    working_dir: &std::path::Path,
) -> Result<CapturedOutput> {
    run_with_dir_and_context(
        args,
        working_dir,
        "Unexpected error in run_sah_command_in_process_with_dir",
    )
    .await
}

/// Run a command in a directory and fold a harness failure into a `CapturedOutput`
async fn run_with_dir_and_context(
    args: &[&str],
    working_dir: &std::path::Path,
    context: &str,
) -> Result<CapturedOutput> {
    let result = run_sah_command_in_process_inner_with_dir(args, working_dir).await;
    Ok(result_to_captured(result, context, EXIT_INTERNAL_ERROR))
}

/// Execute any CLI command, using in-process for supported commands, subprocess for others
///
/// This is the single unified function all tests should use instead of spawning subprocesses
pub async fn run_sah_command_in_process(args: &[&str]) -> Result<CapturedOutput> {
    let current_dir = std::env::current_dir()
        .context("failed to get the current directory for the in-process test command")?;
    run_with_dir_and_context(
        args,
        &current_dir,
        "Unexpected error in run_sah_command_in_process",
    )
    .await
}

/// Check if a command should be run as a subprocess
fn should_run_as_subprocess(args: &[&str]) -> bool {
    // Check if this is a dynamic CLI command that should go directly to subprocess
    !args.is_empty()
        && matches!(
            args[0],
            "todo" | "memo" | "file" | "search" | "web-search" | "rule" | "tool"
        )
}

/// Execute command in-process with captured output
async fn execute_in_process(cli: Cli, args_with_program: &[String]) -> Result<CapturedOutput> {
    let (stdout, stderr, exit_code) =
        match execute_cli_command_with_capture(cli, args_with_program).await {
            Ok(result) => result,
            Err(e) => {
                return Ok(capture_error(format!("{e:#}"), EXIT_GENERAL_FAILURE));
            }
        };

    Ok(CapturedOutput {
        stdout,
        stderr,
        exit_code,
    })
}

/// Prepare subprocess command with proper working directory and environment
fn prepare_subprocess_command(
    binary_path: &str,
    args: &[&str],
    working_dir: &std::path::Path,
) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(binary_path);
    cmd.args(args).current_dir(working_dir).kill_on_drop(true);

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
                format!("failed to execute subprocess {binary_path}: {e:#}"),
                EXIT_CANNOT_EXECUTE,
            ));
        }
    };

    let captured = CapturedOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(EXIT_GENERAL_FAILURE),
    };

    if captured.exit_code != EXIT_SUCCESS {
        log_subprocess_failure(binary_path, args, working_dir, &captured);
    }

    Ok(captured)
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
        Duration::from_secs(SUBPROCESS_TIMEOUT_SECS),
        execute_subprocess_inner(args, working_dir),
    )
    .await
    .unwrap_or_else(|_| {
        Ok(capture_error(
            format!(
                "test command timed out after {} seconds",
                SUBPROCESS_TIMEOUT_SECS
            ),
            EXIT_TIMED_OUT,
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
                    exit_code: EXIT_SUCCESS,
                },
                _ => capture_error(error_str, EXIT_ERROR),
            })
        }
    }
}

/// Check if command can be executed in-process
fn can_run_in_process(cli: &Cli) -> bool {
    matches!(
        cli.command,
        Some(Commands::Validate { .. }) | Some(Commands::Completion { .. }) | None
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
    std::iter::once(SAH_BINARY_NAME.to_string())
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
    match validate::run_validate_command_with_dirs_captured(quiet, format, validate_tools).await {
        Ok((output, exit_code)) => (output, String::new(), exit_code),
        Err(e) => {
            let stderr_str = format!("{e:#}");
            (String::new(), stderr_str, EXIT_ERROR)
        }
    }
}

/// Handle completion command execution
fn handle_completion_command(shell: clap_complete::Shell) -> (String, String, i32) {
    use clap::CommandFactory;
    use clap_complete::generate;
    use std::io::Cursor;

    let mut cmd = swissarmyhammer_cli::cli::Cli::command();
    let mut buf = Cursor::new(Vec::new());

    generate(shell, &mut cmd, SAH_BINARY_NAME, &mut buf);

    let completion_output = String::from_utf8_lossy(buf.get_ref()).to_string();
    (completion_output, String::new(), EXIT_SUCCESS)
}

/// Execute a parsed CLI command with stdout/stderr capture
async fn execute_cli_command_with_capture(
    cli: Cli,
    _args: &[String],
) -> Result<(String, String, i32)> {
    let (stdout, stderr, exit_code) = match cli.command {
        Some(Commands::Validate {
            quiet,
            format,
            validate_tools,
        }) => handle_validate_command(quiet, format, validate_tools).await,

        Some(Commands::Completion { shell }) => handle_completion_command(shell),

        None => (String::new(), String::new(), EXIT_SUCCESS),

        _ => unreachable!("Tried to execute unsupported command in-process"),
    };

    Ok((stdout, stderr, exit_code))
}

// ============================================================================
// Direct In-Process Testing Functions
// ============================================================================
