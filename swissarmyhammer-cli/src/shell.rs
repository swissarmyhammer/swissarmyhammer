//! Shell command execution CLI handlers
//!
//! This module provides CLI command handlers for executing shell commands
//! through the SwissArmyHammer shell MCP tool.

use crate::cli::{ShellCommands, ShellOutputFormat};
use crate::mcp_integration::{response_formatting, CliToolContext};
use serde_json::json;
use std::collections::HashMap;
use std::process;

/// Handle shell CLI commands by delegating to MCP tools
pub async fn handle_shell_command(
    command: ShellCommands,
) -> Result<(), Box<dyn std::error::Error>> {
    let context = CliToolContext::new().await?;

    match command {
        ShellCommands::Execute {
            command,
            working_directory,
            timeout,
            environment,
            format,
            show_metadata,
            quiet,
        } => {
            execute_shell_command(
                &context,
                command,
                working_directory,
                timeout,
                environment,
                format,
                show_metadata,
                quiet,
            )
            .await?;
        }
    }

    Ok(())
}

/// Execute a shell command via the MCP shell tool
async fn execute_shell_command(
    context: &CliToolContext,
    command: String,
    working_directory: Option<std::path::PathBuf>,
    timeout: u64,
    environment_args: Vec<String>,
    format: ShellOutputFormat,
    show_metadata: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    // Parse environment variables from CLI arguments
    let environment = if environment_args.is_empty() {
        None
    } else {
        Some(parse_environment_variables(&environment_args)?)
    };

    // Build MCP tool arguments
    let args = context.create_arguments(vec![
        ("command", json!(command)),
        (
            "working_directory",
            json!(working_directory.as_ref().map(|d| d.display().to_string())),
        ),
        ("timeout", json!(timeout)),
        ("environment", json!(environment)),
    ]);

    // Execute shell command via MCP tool
    let result = context.execute_tool("shell_execute", args).await?;

    // Display results based on format
    display_shell_results(result, format, show_metadata, quiet).await
}

/// Parse environment variables from CLI arguments
fn parse_environment_variables(
    env_args: &[String],
) -> Result<HashMap<String, String>, Box<dyn std::error::Error>> {
    let mut env_vars = HashMap::new();

    for env_arg in env_args {
        if let Some((key, value)) = env_arg.split_once('=') {
            if key.is_empty() {
                return Err(format!("Empty environment variable name in: {}", env_arg).into());
            }
            env_vars.insert(key.to_string(), value.to_string());
        } else {
            return Err(format!(
                "Invalid environment variable format '{}'. Expected KEY=VALUE format.",
                env_arg
            )
            .into());
        }
    }

    Ok(env_vars)
}

/// Extract JSON response from shell command results, handling both success and failure cases
///
/// Unlike the standard extract_json_data, this function extracts JSON even when
/// is_error is true, because for shell commands, is_error=true just means the
/// command had a non-zero exit code, but we still get structured execution data.
///
/// Special case: For timeout errors, the response has two content items:
/// 1. Plain text timeout message
/// 2. JSON with timeout metadata
fn extract_shell_json_response(
    result: &rmcp::model::CallToolResult,
) -> Result<serde_json::Value, String> {
    // Check if this is a timeout error (2 content items, second one is JSON)
    if result.is_error.unwrap_or(false) && result.content.len() >= 2 {
        if let Some(content) = result.content.get(1) {
            if let rmcp::model::RawContent::Text(text_content) = &content.raw {
                if let Ok(timeout_metadata) =
                    serde_json::from_str::<serde_json::Value>(&text_content.text)
                {
                    if timeout_metadata.get("timeout_seconds").is_some() {
                        // This is a timeout error - return the timeout metadata
                        return Ok(timeout_metadata);
                    }
                }
            }
        }
    }

    // Normal case: try to get the text content from the first content item
    let text_content = response_formatting::extract_text_content(result)
        .ok_or_else(|| "No text content in shell command response".to_string())?;

    // Try to parse as JSON - shell execution results should always be JSON
    let json_data: serde_json::Value = serde_json::from_str(&text_content)
        .map_err(|e| format!("Failed to parse shell response as JSON: {}", e))?;

    // Check if this looks like a valid shell execution result by checking for required fields
    if json_data.get("command").is_some() && json_data.get("exit_code").is_some() {
        Ok(json_data)
    } else {
        // This doesn't look like a shell execution result - it's probably a tool error message
        Err(text_content)
    }
}

/// Display shell execution results in the requested format
async fn display_shell_results(
    result: rmcp::model::CallToolResult,
    format: ShellOutputFormat,
    show_metadata: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    match format {
        ShellOutputFormat::Human => display_human_format(result, show_metadata, quiet),
        ShellOutputFormat::Json => display_json_format(result),
        ShellOutputFormat::Yaml => display_yaml_format(result),
    }
}

/// Display results in human-readable format
fn display_human_format(
    result: rmcp::model::CallToolResult,
    show_metadata: bool,
    quiet: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let _is_error = result.is_error.unwrap_or(false);

    // For shell commands, we need to distinguish between:
    // 1. Tool execution errors (true errors - e.g., invalid params, security issues)
    // 2. Shell command failures (commands that run but return non-zero exit codes)
    
    // Try to extract JSON response - for shell commands, we expect JSON even for failed commands
    let json_response = match extract_shell_json_response(&result) {
        Ok(data) => data,
        Err(e) => {
            // This is likely a true tool error (security validation, etc.)
            eprintln!("Shell execution error: {}", e);
            process::exit(2); // Use CLI error code for tool failures
        }
    };

    // Check if this is timeout metadata (different structure)
    let is_timeout = json_response.get("timeout_seconds").is_some();

    let (exit_code, stdout, stderr, execution_time_ms, command, working_directory, output_truncated, binary_output_detected) = if is_timeout {
        // Handle timeout metadata structure
        let timeout_seconds = json_response["timeout_seconds"].as_u64().unwrap_or(0);
        let partial_stdout = json_response["partial_stdout"].as_str().unwrap_or("");
        let partial_stderr = json_response["partial_stderr"].as_str().unwrap_or("");
        let cmd = json_response["command"].as_str().unwrap_or("unknown");
        let work_dir = json_response["working_directory"].as_str().unwrap_or("unknown");

        // For timeouts, we simulate the structure
        (-1, partial_stdout, partial_stderr, timeout_seconds * 1000, cmd, work_dir, false, false)
    } else {
        // Handle normal execution result structure
        let exit_code = json_response["exit_code"].as_i64().unwrap_or(-1);
        let stdout = json_response["stdout"].as_str().unwrap_or("");
        let stderr = json_response["stderr"].as_str().unwrap_or("");
        let execution_time_ms = json_response["execution_time_ms"].as_u64().unwrap_or(0);
        let command = json_response["command"].as_str().unwrap_or("unknown");
        let working_directory = json_response["working_directory"].as_str().unwrap_or("unknown");
        let output_truncated = json_response["output_truncated"].as_bool().unwrap_or(false);
        let binary_output_detected = json_response["binary_output_detected"].as_bool().unwrap_or(false);

        (exit_code, stdout, stderr, execution_time_ms, command, working_directory, output_truncated, binary_output_detected)
    };

    // Display command output unless quiet mode is enabled
    if !quiet {
        if !stdout.is_empty() {
            print!("{}", stdout);
        }
        if !stderr.is_empty() {
            eprint!("{}", stderr);
        }
    }

    // Display metadata if requested
    if show_metadata {
        eprintln!();
        eprintln!("=== Execution Metadata ===");
        eprintln!("Command: {}", command);
        eprintln!("Working Directory: {}", working_directory);
        
        if is_timeout {
            let timeout_seconds = json_response["timeout_seconds"].as_u64().unwrap_or(0);
            eprintln!("Status: Timed out after {} seconds", timeout_seconds);
            eprintln!("Partial Output: {} bytes captured", stdout.len() + stderr.len());
        } else {
            eprintln!("Exit Code: {}", exit_code);
            eprintln!("Execution Time: {}ms", execution_time_ms);

            if output_truncated {
                eprintln!("Output Truncated: Yes (exceeded size limits)");
            }

            if binary_output_detected {
                eprintln!("Binary Output Detected: Yes");
            }
        }
    }

    // Exit with appropriate code
    if is_timeout {
        // Exit with error code for timeout
        process::exit(1);
    } else if exit_code != 0 {
        // Exit with the command's exit code for human format
        process::exit(exit_code as i32);
    }

    Ok(())
}

/// Display results in JSON format
fn display_json_format(result: rmcp::model::CallToolResult) -> Result<(), Box<dyn std::error::Error>> {
    // Try to extract JSON response - for shell commands, we expect JSON even for failed commands
    match extract_shell_json_response(&result) {
        Ok(json_response) => {
            // Command executed successfully, display the JSON response
            println!("{}", serde_json::to_string_pretty(&json_response)?);
        }
        Err(e) => {
            // True tool error (security validation, etc.)
            let error_response = json!({
                "error": true,
                "message": e
            });
            println!("{}", serde_json::to_string_pretty(&error_response)?);
            process::exit(2); // Use CLI error code for tool failures
        }
    }

    Ok(())
}

/// Display results in YAML format
fn display_yaml_format(result: rmcp::model::CallToolResult) -> Result<(), Box<dyn std::error::Error>> {
    // Try to extract JSON response - for shell commands, we expect JSON even for failed commands
    match extract_shell_json_response(&result) {
        Ok(json_response) => {
            // Command executed successfully, display the YAML response
            let yaml_output = serde_yaml::to_string(&json_response)?;
            print!("{}", yaml_output);
        }
        Err(e) => {
            // True tool error (security validation, etc.)
            let error_response = serde_yaml::to_string(&json!({
                "error": true,
                "message": e
            }))?;
            print!("{}", error_response);
            process::exit(2); // Use CLI error code for tool failures
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_environment_variables() {
        // Test valid environment variables
        let env_args = vec![
            "KEY1=value1".to_string(),
            "KEY2=value2".to_string(),
            "PATH=/usr/bin".to_string(),
        ];
        let result = parse_environment_variables(&env_args).unwrap();
        assert_eq!(result.get("KEY1"), Some(&"value1".to_string()));
        assert_eq!(result.get("KEY2"), Some(&"value2".to_string()));
        assert_eq!(result.get("PATH"), Some(&"/usr/bin".to_string()));
    }

    #[test]
    fn test_parse_environment_variables_with_equals_in_value() {
        // Test environment variable with equals sign in the value
        let env_args = vec!["URL=https://example.com?param=value".to_string()];
        let result = parse_environment_variables(&env_args).unwrap();
        assert_eq!(
            result.get("URL"),
            Some(&"https://example.com?param=value".to_string())
        );
    }

    #[test]
    fn test_parse_environment_variables_empty_value() {
        // Test environment variable with empty value
        let env_args = vec!["EMPTY=".to_string()];
        let result = parse_environment_variables(&env_args).unwrap();
        assert_eq!(result.get("EMPTY"), Some(&"".to_string()));
    }

    #[test]
    fn test_parse_environment_variables_invalid_format() {
        // Test invalid format (no equals sign)
        let env_args = vec!["INVALID_NO_EQUALS".to_string()];
        let result = parse_environment_variables(&env_args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid environment variable format"));
    }

    #[test]
    fn test_parse_environment_variables_empty_key() {
        // Test invalid format (empty key)
        let env_args = vec!["=value".to_string()];
        let result = parse_environment_variables(&env_args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Empty environment variable name"));
    }

    #[test]
    fn test_parse_environment_variables_empty_input() {
        // Test empty input
        let env_args: Vec<String> = vec![];
        let result = parse_environment_variables(&env_args).unwrap();
        assert!(result.is_empty());
    }
}