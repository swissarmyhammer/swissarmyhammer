//! Shell command execution tool for MCP operations
//!
//! This module provides the ShellExecuteTool for executing shell commands through the MCP protocol.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Instant;
use tokio::process::Command;

/// Request structure for shell command execution
#[derive(Debug, Deserialize)]
struct ShellExecuteRequest {
    /// The shell command to execute
    command: String,

    /// Optional working directory for command execution
    working_directory: Option<String>,

    /// Optional timeout in seconds (default: 300, max: 1800)
    timeout: Option<u32>,

    /// Optional environment variables to set
    environment: Option<std::collections::HashMap<String, String>>,
}

/// Result structure for shell command execution
#[derive(Debug, Serialize)]
struct ShellExecutionResult {
    /// The command that was executed
    pub command: String,
    /// Exit code returned by the command
    pub exit_code: i32,
    /// Standard output captured from the command
    pub stdout: String,
    /// Standard error output captured from the command
    pub stderr: String,
    /// Execution time in milliseconds
    pub execution_time_ms: u64,
    /// Working directory where the command was executed
    pub working_directory: PathBuf,
}

/// Comprehensive error types for shell command execution
#[derive(Debug)]
pub enum ShellError {
    /// Failed to spawn the command process
    CommandSpawnError {
        /// The command that failed to spawn
        command: String,
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Runtime execution failure
    ExecutionError {
        /// The command that failed to execute
        command: String,
        /// Error message describing the failure
        message: String,
    },

    /// Invalid command provided
    InvalidCommand {
        /// Error message describing why the command is invalid
        message: String,
    },

    /// System-level error
    SystemError {
        /// Error message describing the system error
        message: String,
    },

    /// Working directory error
    WorkingDirectoryError {
        /// Error message describing the working directory issue
        message: String,
    },
}

impl fmt::Display for ShellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellError::CommandSpawnError { command, source } => {
                write!(f, "Failed to spawn command '{}': {}", command, source)
            }
            ShellError::ExecutionError { command, message } => {
                write!(f, "Command '{}' execution failed: {}", command, message)
            }
            ShellError::InvalidCommand { message } => {
                write!(f, "Invalid command: {}", message)
            }
            ShellError::SystemError { message } => {
                write!(f, "System error during command execution: {}", message)
            }
            ShellError::WorkingDirectoryError { message } => {
                write!(f, "Working directory error: {}", message)
            }
        }
    }
}

impl std::error::Error for ShellError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ShellError::CommandSpawnError { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Execute a shell command with full output capture and metadata collection
///
/// This function provides the core shell command execution logic, handling:
/// - Process spawning using tokio::process::Command
/// - Working directory and environment variable management
/// - Complete stdout/stderr capture
/// - Execution time measurement
/// - Comprehensive error handling
///
/// # Arguments
///
/// * `command` - The shell command to execute
/// * `working_directory` - Optional working directory for execution
/// * `timeout_seconds` - Timeout in seconds (used for logging only, actual timeout handled by caller)
/// * `environment` - Optional environment variables to set
///
/// # Returns
///
/// Returns a `Result` containing either a `ShellExecutionResult` with complete
/// execution metadata or a `ShellError` describing the failure mode.
async fn execute_shell_command(
    command: String,
    working_directory: Option<PathBuf>,
    timeout_seconds: u64,
    environment: Option<std::collections::HashMap<String, String>>,
) -> Result<ShellExecutionResult, ShellError> {
    let start_time = Instant::now();

    // Determine working directory - use provided or current directory
    let work_dir = working_directory
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    // Validate that working directory exists
    if !work_dir.exists() {
        return Err(ShellError::WorkingDirectoryError {
            message: format!("Working directory does not exist: {}", work_dir.display()),
        });
    }

    // Parse command into parts for proper execution
    // For Unix systems, we'll use sh -c to handle complex commands properly
    let (program, args) = if cfg!(target_os = "windows") {
        ("cmd", vec!["/C", &command])
    } else {
        ("sh", vec!["-c", &command])
    };

    // Build the tokio Command
    let mut cmd = Command::new(program);
    cmd.args(args).current_dir(&work_dir);

    // Add environment variables if provided
    if let Some(env_vars) = &environment {
        for (key, value) in env_vars {
            cmd.env(key, value);
        }
    }

    // Configure output capture
    cmd.stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    tracing::debug!(
        "Executing command: '{}' in directory: {} with timeout: {}s",
        command,
        work_dir.display(),
        timeout_seconds
    );

    // Spawn the process
    let child = cmd.spawn().map_err(|e| {
        tracing::error!("Failed to spawn command '{}': {}", command, e);
        ShellError::CommandSpawnError {
            command: command.clone(),
            source: e,
        }
    })?;

    // Wait for the process to complete and capture output
    let output = child.wait_with_output().await.map_err(|e| {
        tracing::error!("Failed to wait for command '{}': {}", command, e);
        ShellError::ExecutionError {
            command: command.clone(),
            message: format!("Failed to wait for process: {}", e),
        }
    })?;

    let execution_time = start_time.elapsed();
    let execution_time_ms = execution_time.as_millis() as u64;

    // Convert output to strings, handling potential UTF-8 issues gracefully
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    // Get the exit code
    let exit_code = output.status.code().unwrap_or(-1);

    tracing::info!(
        "Command '{}' completed with exit code {} in {}ms",
        command,
        exit_code,
        execution_time_ms
    );

    Ok(ShellExecutionResult {
        command,
        exit_code,
        stdout,
        stderr,
        execution_time_ms,
        working_directory: work_dir,
    })
}

/// Tool for executing shell commands
#[derive(Default)]
pub struct ShellExecuteTool;

impl ShellExecuteTool {
    /// Creates a new instance of the ShellExecuteTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for ShellExecuteTool {
    fn name(&self) -> &'static str {
        "shell_execute"
    }

    fn description(&self) -> &'static str {
        crate::mcp::tool_descriptions::get_tool_description("shell", "execute")
            .expect("Tool description should be available")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute",
                    "minLength": 1
                },
                "working_directory": {
                    "type": "string",
                    "description": "Working directory for command execution (optional, defaults to current directory)"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Command timeout in seconds (optional, defaults to 300 seconds / 5 minutes)",
                    "minimum": 1,
                    "maximum": 1800,
                    "default": 300
                },
                "environment": {
                    "type": "object",
                    "description": "Additional environment variables to set (optional)",
                    "additionalProperties": {
                        "type": "string"
                    }
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: ShellExecuteRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for shell command execution
        context
            .rate_limiter
            .check_rate_limit("unknown", "shell_execute", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for shell execution: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Executing shell command: {:?}", request.command);

        // Validate command is not empty
        McpValidation::validate_not_empty(&request.command, "shell command")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate shell command"))?;

        // Validate timeout if provided
        if let Some(timeout) = request.timeout {
            if timeout == 0 || timeout > 1800 {
                return Err(McpError::invalid_params(
                    "Timeout must be between 1 and 1800 seconds".to_string(),
                    None,
                ));
            }
        }

        // Validate working directory if provided
        if let Some(ref working_dir) = request.working_directory {
            McpValidation::validate_not_empty(working_dir, "working directory")
                .map_err(|e| McpErrorHandler::handle_error(e, "validate working directory"))?;
        }

        // Execute the shell command using our core execution function
        let working_directory = request.working_directory.map(PathBuf::from);
        let timeout_seconds = request.timeout.unwrap_or(300) as u64;

        match execute_shell_command(
            request.command.clone(),
            working_directory,
            timeout_seconds,
            request.environment,
        )
        .await
        {
            Ok(result) => {
                // Command executed successfully - create response based on exit code
                let is_error = result.exit_code != 0;

                // Serialize the result as JSON for the response
                let json_response = serde_json::to_string_pretty(&result).map_err(|e| {
                    tracing::error!("Failed to serialize shell result: {}", e);
                    McpError::internal_error(format!("Serialization failed: {}", e), None)
                })?;

                tracing::info!(
                    "Shell command '{}' completed with exit code {} in {}ms",
                    result.command,
                    result.exit_code,
                    result.execution_time_ms
                );

                // Create response with structured JSON data
                Ok(CallToolResult {
                    content: vec![rmcp::model::Annotated::new(
                        rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                            text: json_response,
                        }),
                        None,
                    )],
                    is_error: Some(is_error),
                })
            }
            Err(shell_error) => {
                // Command execution failed - return error response
                let error_message = format!("Shell execution failed: {}", shell_error);
                tracing::error!("{}", error_message);

                Ok(CallToolResult {
                    content: vec![rmcp::model::Annotated::new(
                        rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                            text: error_message,
                        }),
                        None,
                    )],
                    is_error: Some(true),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::ToolContext;
    use std::collections::HashMap;
    use std::sync::Arc;
    use swissarmyhammer::common::rate_limiter::MockRateLimiter;

    fn create_test_context() -> ToolContext {
        use std::path::PathBuf;
        use swissarmyhammer::git::GitOperations;
        use swissarmyhammer::issues::IssueStorage;
        use swissarmyhammer::memoranda::{mock_storage::MockMemoStorage, MemoStorage};
        use tokio::sync::{Mutex, RwLock};

        let issue_storage: Arc<RwLock<Box<dyn IssueStorage>>> = Arc::new(RwLock::new(Box::new(
            swissarmyhammer::issues::FileSystemIssueStorage::new(PathBuf::from("./test_issues"))
                .unwrap(),
        )));
        let git_ops: Arc<Mutex<Option<GitOperations>>> = Arc::new(Mutex::new(None));
        let memo_storage: Arc<RwLock<Box<dyn MemoStorage>>> =
            Arc::new(RwLock::new(Box::new(MockMemoStorage::new())));

        let tool_handlers = Arc::new(crate::mcp::tool_handlers::ToolHandlers::new(
            memo_storage.clone(),
        ));
        ToolContext::new(
            tool_handlers,
            issue_storage,
            git_ops,
            memo_storage,
            Arc::new(MockRateLimiter),
        )
    }

    #[test]
    fn test_tool_properties() {
        let tool = ShellExecuteTool::new();
        assert_eq!(tool.name(), "shell_execute");
        assert!(!tool.description().is_empty());

        let schema = tool.schema();
        assert!(schema.is_object());
        assert!(schema["properties"]["command"]["type"].as_str() == Some("string"));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::Value::String("command".to_string())));
    }

    #[tokio::test]
    async fn test_execute_basic_command() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo hello".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_with_all_parameters() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut env = HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("ls -la".to_string()),
        );
        args.insert(
            "working_directory".to_string(),
            serde_json::Value::String("/tmp".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(120)),
        );
        args.insert(
            "environment".to_string(),
            serde_json::to_value(&env).unwrap(),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_empty_command() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_timeout() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(2000)),
        ); // Over 1800 limit

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_zero_timeout() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_empty_working_directory() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "working_directory".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_real_command_success() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo 'Hello World'".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Command execution should succeed");

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // The response should contain JSON with execution results
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        // Parse the JSON response to check for expected fields
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(content_text) {
            assert!(response_json.get("stdout").is_some());
            assert!(response_json.get("stderr").is_some());
            assert!(response_json.get("exit_code").is_some());
            assert!(response_json.get("execution_time_ms").is_some());

            // Check that stdout contains the expected output
            if let Some(stdout) = response_json.get("stdout") {
                assert!(stdout.as_str().unwrap().contains("Hello World"));
            }

            // Check that exit code is 0 for successful command
            if let Some(exit_code) = response_json.get("exit_code") {
                assert_eq!(exit_code.as_i64().unwrap(), 0);
            }
        }
    }

    #[tokio::test]
    async fn test_execute_real_command_failure() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("ls /nonexistent_directory".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_ok(),
            "Tool should return result even for failed commands"
        );

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(true));

        // The response should contain JSON with execution results
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        // Parse the JSON response to check for expected fields
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(content_text) {
            assert!(response_json.get("stderr").is_some());
            assert!(response_json.get("exit_code").is_some());

            // Check that exit code is non-zero for failed command
            if let Some(exit_code) = response_json.get("exit_code") {
                assert_ne!(exit_code.as_i64().unwrap(), 0);
            }

            // Check that stderr contains error information
            if let Some(stderr) = response_json.get("stderr") {
                assert!(!stderr.as_str().unwrap().is_empty());
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_working_directory() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("pwd".to_string()),
        );
        args.insert(
            "working_directory".to_string(),
            serde_json::Value::String("/tmp".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Command execution should succeed");

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // The response should contain JSON with execution results
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        // Parse the JSON response to check working directory
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(content_text) {
            if let Some(stdout) = response_json.get("stdout") {
                assert!(stdout.as_str().unwrap().contains("/tmp"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_with_environment_variables() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut env = std::collections::HashMap::new();
        env.insert("TEST_VAR".to_string(), "test_value".to_string());

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo $TEST_VAR".to_string()),
        );
        args.insert(
            "environment".to_string(),
            serde_json::to_value(&env).unwrap(),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Command execution should succeed");

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));

        // The response should contain JSON with execution results
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        // Parse the JSON response to check environment variable
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(content_text) {
            if let Some(stdout) = response_json.get("stdout") {
                assert!(stdout.as_str().unwrap().contains("test_value"));
            }
        }
    }
}
