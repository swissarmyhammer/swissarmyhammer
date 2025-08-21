//! In-process test utilities for CLI commands
//!
//! This module provides utilities to test CLI functionality without spawning
//! external processes, significantly improving test performance.

use anyhow::Result;
use clap::Parser;
use swissarmyhammer_cli::cli::{Cli, Commands};
use swissarmyhammer_cli::{flow, validate};

/// Captures output from in-process CLI command execution
pub struct CapturedOutput {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// Execute any CLI command, using in-process for supported commands, subprocess for others
/// 
/// This is the single unified function all tests should use instead of spawning subprocesses
pub async fn run_sah_command_in_process(args: &[&str]) -> Result<CapturedOutput> {
    use swissarmyhammer_cli::cli::Cli;
    
    // Create CLI with the provided arguments (skip program name)
    let args_with_program: Vec<String> = std::iter::once("sah".to_string())
        .chain(args.iter().map(|s| s.to_string()))
        .collect();
    
    // Parse the CLI arguments to see what command we're dealing with
    let cli = match Cli::try_parse_from(args_with_program.clone()) {
        Ok(cli) => cli,
        Err(e) => {
            use clap::error::ErrorKind;
            // Handle help/version which are "successful" parse errors
            let error_str = e.to_string();
            match e.kind() {
                ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                    return Ok(CapturedOutput {
                        stdout: error_str,
                        stderr: String::new(),
                        exit_code: 0,
                    });
                }
                _ => {
                    // Return actual parse errors as failed execution
                    return Ok(CapturedOutput {
                        stdout: String::new(),
                        stderr: error_str,
                        exit_code: 2,
                    });
                }
            }
        }
    };
    
    // Check if this is a command we can run in-process
    let can_run_in_process = matches!(
        cli.command,
        Some(Commands::Validate { .. }) |
        Some(Commands::Completion { .. }) |
        None
    );
    
    if can_run_in_process {
        // Execute in-process with stdout/stderr capture
        let (stdout, stderr, exit_code) = match execute_cli_command_with_capture(cli).await {
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
    } else {
        // Fall back to subprocess for commands we can't run in-process
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_sah"))
            .args(args)
            .output()
            .map_err(|e| anyhow::anyhow!("Failed to execute subprocess: {}", e))?;
        
        Ok(CapturedOutput {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(1),
        })
    }
}

/// Execute a parsed CLI command with stdout/stderr capture
async fn execute_cli_command_with_capture(cli: Cli) -> Result<(String, String, i32)> {
    use std::io::Write;
    use std::sync::{Arc, Mutex};
    use swissarmyhammer_cli::exit_codes::{EXIT_SUCCESS, EXIT_WARNING, EXIT_ERROR};
    
    // Create buffers to capture output
    let stdout_buffer = Arc::new(Mutex::new(Vec::new()));
    let stderr_buffer = Arc::new(Mutex::new(Vec::new()));
    
    // For completion command, we need to generate the actual completion script
    let (stdout, stderr, exit_code) = match cli.command {
        Some(Commands::Flow { subcommand }) => {
            // Flow commands are exposed and can be run in-process
            let stdout_capture = stdout_buffer.clone();
            let stderr_capture = stderr_buffer.clone();
            
            // Capture output from flow command
            let exit_code = match flow::run_flow_command(subcommand).await {
                Ok(_) => EXIT_SUCCESS,
                Err(e) => {
                    if let Ok(mut stderr) = stderr_capture.lock() {
                        let _ = writeln!(stderr, "{}", e);
                    }
                    EXIT_WARNING
                }
            };
            
            let stdout_str = String::from_utf8_lossy(&stdout_capture.lock().unwrap()).to_string();
            let stderr_str = String::from_utf8_lossy(&stderr_capture.lock().unwrap()).to_string();
            (stdout_str, stderr_str, exit_code)
        }
        Some(Commands::Validate { quiet, format, workflow_dirs }) => {
            // Use the captured version that returns output as a string
            match validate::run_validate_command_with_dirs_captured(quiet, format, workflow_dirs) {
                Ok((output, exit_code)) => (output, String::new(), exit_code),
                Err(e) => {
                    let stderr_str = format!("{}", e);
                    (String::new(), stderr_str, EXIT_ERROR)
                }
            }
        }
        Some(Commands::Completion { shell }) => {
            // For completion, we need to generate the actual completion script
            // This is a bit tricky because clap generates it to stdout
            use clap::CommandFactory;
            use clap_complete::generate;
            use std::io::Cursor;
            
            let mut cmd = swissarmyhammer_cli::cli::Cli::command();
            let mut buf = Cursor::new(Vec::new());
            
            match shell {
                clap_complete::Shell::Bash => generate(clap_complete::Shell::Bash, &mut cmd, "sah", &mut buf),
                clap_complete::Shell::Zsh => generate(clap_complete::Shell::Zsh, &mut cmd, "sah", &mut buf),
                clap_complete::Shell::Fish => generate(clap_complete::Shell::Fish, &mut cmd, "sah", &mut buf),
                clap_complete::Shell::PowerShell => generate(clap_complete::Shell::PowerShell, &mut cmd, "sah", &mut buf),
                clap_complete::Shell::Elvish => generate(clap_complete::Shell::Elvish, &mut cmd, "sah", &mut buf),
                _ => generate(clap_complete::Shell::Bash, &mut cmd, "sah", &mut buf), // fallback to bash
            }
            
            let completion_output = String::from_utf8_lossy(buf.get_ref()).to_string();
            (completion_output, String::new(), EXIT_SUCCESS)
        }
        None => {
            // No subcommand provided - show help
            (String::new(), String::new(), EXIT_SUCCESS)
        }
        _ => {
            // This shouldn't happen since we check can_run_in_process first
            unreachable!("Tried to execute unsupported command in-process")
        }
    };
    
    Ok((stdout, stderr, exit_code))
}

/// Execute a parsed CLI command (internal helper) - deprecated, use execute_cli_command_with_capture
async fn execute_cli_command(cli: Cli) -> Result<i32> {
    let (_stdout, _stderr, exit_code) = execute_cli_command_with_capture(cli).await?;
    Ok(exit_code)
}

/// Test wrapper for in-process flow command execution
pub async fn run_flow_test_in_process(
    workflow_name: &str,
    vars: Vec<String>,
    timeout: Option<String>,
    quiet: bool,
) -> Result<CapturedOutput> {
    // Build command args for "flow test"
    let mut args = vec!["flow", "test", workflow_name];
    
    // Add vars
    for var in &vars {
        args.push("--var");
        args.push(var);
    }
    
    // Add timeout if provided
    let timeout_str = timeout.unwrap_or_else(|| "2s".to_string());
    args.push("--timeout");
    args.push(&timeout_str);
    
    // Add quiet flag
    if quiet {
        args.push("--quiet");
    }
    
    run_sah_command_in_process(&args).await
}


/// Helper to run a simple workflow test in-process
pub async fn simple_workflow_test(workflow_name: &str) -> Result<bool> {
    let result = run_flow_test_in_process(workflow_name, vec![], None, false).await?;
    Ok(result.exit_code == 0)
}

/// Helper to run workflow test with variables
pub async fn workflow_test_with_vars(
    workflow_name: &str,
    vars: Vec<(&str, &str)>,
) -> Result<bool> {
    let var_strings: Vec<String> = vars
        .into_iter()
        .map(|(k, v)| format!("{}={}", k, v))
        .collect();
    
    let result = run_flow_test_in_process(workflow_name, var_strings, None, false).await?;
    Ok(result.exit_code == 0)
}

/// Helper to run "flow run" command with variables in-process
pub async fn run_flow_run_in_process(
    workflow_name: &str,
    vars: Vec<String>,
    dry_run: bool,
) -> Result<CapturedOutput> {
    let mut args = vec!["flow", "run", workflow_name];
    
    // Add vars
    for var in &vars {
        args.push("--var");
        args.push(var);
    }
    
    // Add dry-run if requested
    if dry_run {
        args.push("--dry-run");
    }
    
    // Use fast timeout for tests
    args.push("--timeout");
    args.push("2s");
    
    // Always quiet for tests
    args.push("--quiet");
    
    run_sah_command_in_process(&args).await
}

/// Helper to run "prompt test" command in-process
pub async fn run_prompt_test_in_process(
    prompt_name: &str,
    vars: Vec<String>,
) -> Result<CapturedOutput> {
    let mut args = vec!["prompt", "test", prompt_name];
    
    // Add vars
    for var in &vars {
        args.push("--var");
        args.push(var);
    }
    
    run_sah_command_in_process(&args).await
}

/// Helper for simple success/failure checks
pub async fn assert_sah_command_succeeds(args: &[&str]) -> Result<()> {
    let result = run_sah_command_in_process(args).await?;
    if result.exit_code != 0 {
        anyhow::bail!(
            "Command failed with exit code {}: {}",
            result.exit_code,
            result.stderr
        );
    }
    Ok(())
}

/// Helper for expected failure checks
pub async fn assert_sah_command_fails(args: &[&str]) -> Result<CapturedOutput> {
    let result = run_sah_command_in_process(args).await?;
    if result.exit_code == 0 {
        anyhow::bail!(
            "Command unexpectedly succeeded when it should have failed"
        );
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_in_process_utilities() {
        // Test with a non-existent workflow (should fail gracefully)
        let result = simple_workflow_test("nonexistent-workflow").await;
        assert!(result.is_ok(), "Should handle non-existent workflows gracefully");
        
        // The result will be false (failure) but the function should not panic
        let success = result.unwrap();
        assert!(!success, "Non-existent workflow should fail");
    }

    #[tokio::test]
    async fn test_workflow_with_vars() {
        // Test with variables
        let result = workflow_test_with_vars(
            "test-workflow",
            vec![("param1", "value1"), ("param2", "value2")]
        ).await;
        
        assert!(result.is_ok(), "Should handle workflow with vars gracefully");
    }
}

// ============================================================================
// Direct In-Process Testing Functions
// ============================================================================

/// Simple helper for tests that just need to assert success/failure
pub async fn assert_command_success(args: &[&str]) -> Result<()> {
    let result = run_sah_command_in_process(args).await?;
    if result.exit_code != 0 {
        anyhow::bail!(
            "Command failed with exit code {}: {}",
            result.exit_code,
            result.stderr
        );
    }
    Ok(())
}

/// Simple helper for tests that expect failure
pub async fn assert_command_failure(args: &[&str]) -> Result<CapturedOutput> {
    let result = run_sah_command_in_process(args).await?;
    if result.exit_code == 0 {
        anyhow::bail!(
            "Command unexpectedly succeeded when it should have failed"
        );
    }
    Ok(result)
}