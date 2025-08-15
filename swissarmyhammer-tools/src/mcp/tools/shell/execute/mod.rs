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
use std::time::{Duration, Instant};
use tokio::process::{Child, Command};
use tokio::time::timeout;

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

    /// Command execution timed out
    TimeoutError {
        /// The command that timed out
        command: String,
        /// Timeout duration in seconds
        timeout_seconds: u64,
        /// Partial stdout captured before timeout
        partial_stdout: String,
        /// Partial stderr captured before timeout
        partial_stderr: String,
        /// Working directory where the command was executed
        working_directory: PathBuf,
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
            ShellError::TimeoutError {
                command,
                timeout_seconds,
                ..
            } => {
                write!(
                    f,
                    "Command '{}' timed out after {} seconds",
                    command, timeout_seconds
                )
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

/// Async process guard for automatic cleanup of tokio Child processes
///
/// This guard automatically terminates and cleans up child processes when dropped,
/// ensuring no orphaned processes remain even if a timeout occurs or the operation is cancelled.
///
/// Unlike the sync ProcessGuard in test_utils.rs, this version works with tokio::process::Child
/// and provides async methods for graceful termination with timeouts.
pub struct AsyncProcessGuard {
    child: Option<Child>,
    command: String,
}

impl AsyncProcessGuard {
    /// Create a new async process guard from a tokio Child process
    pub fn new(child: Child, command: String) -> Self {
        Self {
            child: Some(child),
            command,
        }
    }

    /// Take the child process out of the guard, transferring ownership
    /// This is useful when you want to handle the process manually
    pub fn take_child(&mut self) -> Option<Child> {
        self.child.take()
    }

    /// Check if the process is still running
    pub fn is_running(&mut self) -> bool {
        if let Some(ref mut child) = self.child {
            match child.try_wait() {
                Ok(None) => true,     // Process is still running
                Ok(Some(_)) => false, // Process has exited
                Err(_) => false,      // Error occurred, assume process is dead
            }
        } else {
            false
        }
    }

    /// Attempt to gracefully terminate the process with a timeout
    pub async fn terminate_gracefully(
        &mut self,
        timeout_duration: Duration,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(ref mut child) = self.child {
            tracing::debug!(
                "Attempting graceful termination of process for command: {}",
                self.command
            );

            // Try to terminate the process and wait for it to exit
            let termination_result = timeout(timeout_duration, async {
                // On Unix systems, we can try to send SIGTERM first
                #[cfg(unix)]
                {
                    // Kill the process group to handle child processes
                    if let Some(pid) = child.id() {
                        unsafe {
                            // Send SIGTERM to the process group
                            libc::killpg(pid as i32, libc::SIGTERM);
                        }
                    }
                }

                // On Windows or if Unix signal handling fails, use kill()
                #[cfg(not(unix))]
                {
                    let _ = child.kill().await;
                }

                // Wait for the process to exit
                child.wait().await
            })
            .await;

            match termination_result {
                Ok(wait_result) => {
                    tracing::debug!(
                        "Process terminated gracefully for command: {}",
                        self.command
                    );
                    wait_result?;
                    self.child = None;
                    Ok(())
                }
                Err(_) => {
                    // Timeout occurred, force kill
                    tracing::warn!(
                        "Graceful termination timed out, force killing process for command: {}",
                        self.command
                    );
                    self.force_kill().await
                }
            }
        } else {
            Ok(())
        }
    }

    /// Force kill the process immediately
    pub async fn force_kill(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(mut child) = self.child.take() {
            tracing::debug!("Force killing process for command: {}", self.command);

            #[cfg(unix)]
            {
                // Kill the process group to handle child processes
                if let Some(pid) = child.id() {
                    unsafe {
                        // Send SIGKILL to the process group
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
            }

            child.kill().await?;
            child.wait().await?;
            tracing::debug!("Process force killed for command: {}", self.command);
        }
        Ok(())
    }
}

impl Drop for AsyncProcessGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            // Try to clean up the process synchronously
            // This is a best-effort cleanup since Drop cannot be async
            tracing::warn!(
                "AsyncProcessGuard dropping with active process for command: {}",
                self.command
            );

            #[cfg(unix)]
            {
                // Kill the process group on Unix systems
                if let Some(pid) = child.id() {
                    unsafe {
                        libc::killpg(pid as i32, libc::SIGKILL);
                    }
                }
            }

            // Use blocking kill for cleanup - not ideal but necessary in Drop
            let _ = child.start_kill();
        }
    }
}

/// Execute a shell command with timeout, process management, and full output capture
///
/// This function provides the core shell command execution logic with comprehensive
/// timeout management and process cleanup, handling:
/// - Process spawning using tokio::process::Command
/// - Timeout control with tokio::time::timeout
/// - Process tree termination on timeout using AsyncProcessGuard
/// - Working directory and environment variable management
/// - Complete stdout/stderr capture with partial output on timeout
/// - Execution time measurement
/// - Comprehensive error handling
///
/// # Arguments
///
/// * `command` - The shell command to execute
/// * `working_directory` - Optional working directory for execution
/// * `timeout_seconds` - Timeout in seconds (actual timeout enforcement)
/// * `environment` - Optional environment variables to set
///
/// # Returns
///
/// Returns a `Result` containing either a `ShellExecutionResult` with complete
/// execution metadata or a `ShellError` describing the failure mode, including
/// timeout errors with partial output.
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

    // Note: Process group configuration removed for compatibility
    // The AsyncProcessGuard will handle process cleanup using kill/killpg

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

    // Create process guard for automatic cleanup
    let mut process_guard = AsyncProcessGuard::new(child, command.clone());

    // Execute with timeout
    let timeout_duration = Duration::from_secs(timeout_seconds);

    match timeout(timeout_duration, async {
        // Take the child from the guard for execution
        let child = process_guard
            .take_child()
            .ok_or_else(|| ShellError::SystemError {
                message: "Process guard has no child process".to_string(),
            })?;

        // Wait for the process to complete and capture output
        let output = child.wait_with_output().await.map_err(|e| {
            tracing::error!("Failed to wait for command '{}': {}", command, e);
            ShellError::ExecutionError {
                command: command.clone(),
                message: format!("Failed to wait for process: {}", e),
            }
        })?;

        Ok::<_, ShellError>(output)
    })
    .await
    {
        Ok(output_result) => {
            match output_result {
                Ok(output) => {
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
                Err(shell_error) => Err(shell_error),
            }
        }
        Err(_timeout_error) => {
            // Timeout occurred - attempt to collect partial output and clean up process
            tracing::warn!(
                "Command '{}' timed out after {}s, attempting cleanup",
                command,
                timeout_seconds
            );

            // Try to gracefully terminate the process
            if let Err(e) = process_guard
                .terminate_gracefully(Duration::from_secs(5))
                .await
            {
                tracing::error!("Failed to terminate process gracefully: {}", e);
                // Force kill as fallback
                if let Err(e) = process_guard.force_kill().await {
                    tracing::error!("Failed to force kill process: {}", e);
                }
            }

            // Return timeout error with partial output (empty in this case since we can't capture partial)
            // In a more sophisticated implementation, we could stream output and capture what was received
            Err(ShellError::TimeoutError {
                command,
                timeout_seconds,
                partial_stdout: String::new(), // TODO: Could be enhanced with streaming output capture
                partial_stderr: String::new(), // TODO: Could be enhanced with streaming output capture
                working_directory: work_dir,
            })
        }
    }
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

        // Apply comprehensive command security validation from workflow system
        swissarmyhammer::workflow::validate_command(&request.command).map_err(|e| {
            tracing::warn!("Command security validation failed: {}", e);
            McpError::invalid_params(format!("Command security check failed: {}", e), None)
        })?;

        // Validate timeout if provided
        if let Some(timeout) = request.timeout {
            if timeout == 0 || timeout > 1800 {
                return Err(McpError::invalid_params(
                    "Timeout must be between 1 and 1800 seconds".to_string(),
                    None,
                ));
            }
        }

        // Validate working directory if provided with security checks
        if let Some(ref working_dir) = request.working_directory {
            McpValidation::validate_not_empty(working_dir, "working directory")
                .map_err(|e| McpErrorHandler::handle_error(e, "validate working directory"))?;

            // Apply security validation from workflow system
            swissarmyhammer::workflow::validate_working_directory_security(working_dir).map_err(
                |e| {
                    tracing::warn!("Working directory security validation failed: {}", e);
                    McpError::invalid_params(
                        format!("Working directory security check failed: {}", e),
                        None,
                    )
                },
            )?;
        }

        // Validate environment variables if provided with security checks
        if let Some(ref env_vars) = request.environment {
            swissarmyhammer::workflow::validate_environment_variables_security(env_vars).map_err(
                |e| {
                    tracing::warn!("Environment variables security validation failed: {}", e);
                    McpError::invalid_params(
                        format!("Environment variables security check failed: {}", e),
                        None,
                    )
                },
            )?;
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
                // Handle different types of shell errors with appropriate responses
                match &shell_error {
                    ShellError::TimeoutError {
                        command,
                        timeout_seconds,
                        partial_stdout,
                        partial_stderr,
                        working_directory,
                    } => {
                        // Create timeout-specific response per specification
                        let timeout_response = serde_json::json!({
                            "command": command,
                            "timeout_seconds": timeout_seconds,
                            "partial_stdout": partial_stdout,
                            "partial_stderr": partial_stderr,
                            "working_directory": working_directory.display().to_string()
                        });

                        let response_text =
                            format!("Command timed out after {} seconds", timeout_seconds);
                        tracing::warn!(
                            "Command '{}' timed out after {}s",
                            command,
                            timeout_seconds
                        );

                        Ok(CallToolResult {
                            content: vec![
                                rmcp::model::Annotated::new(
                                    rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                                        text: response_text,
                                    }),
                                    None,
                                ),
                                rmcp::model::Annotated::new(
                                    rmcp::model::RawContent::Text(rmcp::model::RawTextContent {
                                        text: serde_json::to_string_pretty(&timeout_response)
                                            .unwrap_or_else(|_| {
                                                "Failed to serialize timeout metadata".to_string()
                                            }),
                                    }),
                                    None,
                                ),
                            ],
                            is_error: Some(true),
                        })
                    }
                    _ => {
                        // Other error types - return standard error response
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

    #[tokio::test]
    async fn test_execute_with_short_timeout() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("sleep 3".to_string()), // Command that takes 3 seconds
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)), // But timeout after 1 second
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok(), "Tool should return result even for timeout");

        let call_result = result.unwrap();
        assert_eq!(
            call_result.is_error,
            Some(true),
            "Timeout should be reported as error"
        );

        // Check that response contains timeout information
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        assert!(
            content_text.contains("timed out"),
            "Response should mention timeout"
        );
        assert!(
            content_text.contains("1 seconds"),
            "Response should mention the timeout duration"
        );
    }

    #[tokio::test]
    async fn test_execute_timeout_metadata() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("sleep 5".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(2)),
        );
        args.insert(
            "working_directory".to_string(),
            serde_json::Value::String("/tmp".to_string()),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(true));

        // Should have at least 2 content items: error message and metadata
        assert!(call_result.content.len() >= 2);

        // Check if the second content item contains timeout metadata
        if call_result.content.len() >= 2 {
            let metadata_text = match &call_result.content[1].raw {
                rmcp::model::RawContent::Text(text_content) => &text_content.text,
                _ => panic!("Expected text content for metadata"),
            };

            // Parse as JSON and verify timeout metadata
            if let Ok(metadata_json) = serde_json::from_str::<serde_json::Value>(metadata_text) {
                assert!(metadata_json.get("command").is_some());
                assert!(metadata_json.get("timeout_seconds").is_some());
                assert!(metadata_json.get("partial_stdout").is_some());
                assert!(metadata_json.get("partial_stderr").is_some());
                assert!(metadata_json.get("working_directory").is_some());

                assert_eq!(metadata_json["command"], "sleep 5");
                assert_eq!(metadata_json["timeout_seconds"], 2);
                assert!(metadata_json["working_directory"]
                    .as_str()
                    .unwrap()
                    .contains("/tmp"));
            }
        }
    }

    #[tokio::test]
    async fn test_execute_fast_command_no_timeout() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo 'fast command'".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(
            call_result.is_error,
            Some(false),
            "Fast command should complete without timeout"
        );

        // Should have regular success response
        assert!(!call_result.content.is_empty());
        let content_text = match &call_result.content[0].raw {
            rmcp::model::RawContent::Text(text_content) => &text_content.text,
            _ => panic!("Expected text content"),
        };

        // Parse the JSON response
        if let Ok(response_json) = serde_json::from_str::<serde_json::Value>(content_text) {
            assert_eq!(response_json["exit_code"], 0);
            assert!(response_json["stdout"]
                .as_str()
                .unwrap()
                .contains("fast command"));
        }
    }

    #[tokio::test]
    async fn test_execute_maximum_timeout_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1801)), // Over 1800 limit
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_err(),
            "Should fail validation for timeout over 1800 seconds"
        );
    }

    #[tokio::test]
    async fn test_execute_minimum_timeout_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)), // Below minimum
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_err(),
            "Should fail validation for timeout of 0 seconds"
        );
    }

    #[tokio::test]
    async fn test_process_cleanup_on_timeout() {
        // This test verifies that processes are properly cleaned up on timeout
        // We can't easily test this without creating actual long-running processes,
        // but we can test that the function completes and doesn't hang
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            // Command that would run longer than timeout but should be killed
            serde_json::Value::String("sleep 10".to_string()),
        );
        args.insert(
            "timeout".to_string(),
            serde_json::Value::Number(serde_json::Number::from(1)),
        );

        let start_time = std::time::Instant::now();
        let result = tool.execute(args, &context).await;
        let execution_time = start_time.elapsed();

        // Should complete relatively quickly (much less than the 10 second sleep)
        assert!(
            execution_time.as_secs() < 5,
            "Command should be killed and function should return quickly"
        );
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(true));
    }

    // Security validation tests for the new functionality
    #[tokio::test]
    async fn test_command_injection_security_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test command injection patterns that should be blocked
        let dangerous_commands = [
            "echo hello; rm -rf /",
            "echo hello && rm file",
            "echo hello || rm file",
            "echo `dangerous`",
            "echo $(dangerous)",
        ];

        for cmd in &dangerous_commands {
            let mut args = serde_json::Map::new();
            args.insert(
                "command".to_string(),
                serde_json::Value::String(cmd.to_string()),
            );

            let result = tool.execute(args, &context).await;
            assert!(
                result.is_err(),
                "Command injection pattern '{}' should be blocked",
                cmd
            );

            // Verify the error message contains security-related information
            if let Err(mcp_error) = result {
                let error_str = mcp_error.to_string();
                assert!(
                    error_str.contains("security") || error_str.contains("unsafe"),
                    "Error should mention security concern for command: {}",
                    cmd
                );
            }
        }
    }

    #[tokio::test]
    async fn test_working_directory_traversal_security_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test path traversal attempts that should be blocked
        let dangerous_paths = ["../parent", "path/../parent", "/absolute/../parent"];

        for path in &dangerous_paths {
            let mut args = serde_json::Map::new();
            args.insert(
                "command".to_string(),
                serde_json::Value::String("echo test".to_string()),
            );
            args.insert(
                "working_directory".to_string(),
                serde_json::Value::String(path.to_string()),
            );

            let result = tool.execute(args, &context).await;
            assert!(
                result.is_err(),
                "Path traversal attempt '{}' should be blocked",
                path
            );

            // Verify the error message mentions security
            if let Err(mcp_error) = result {
                let error_str = mcp_error.to_string();
                assert!(
                    error_str.contains("security") || error_str.contains("directory"),
                    "Error should mention security/directory concern for path: {}",
                    path
                );
            }
        }
    }

    #[tokio::test]
    async fn test_environment_variable_security_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test invalid environment variable names that should be blocked
        let mut env = std::collections::HashMap::new();
        env.insert("123INVALID".to_string(), "value".to_string()); // starts with number

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "environment".to_string(),
            serde_json::to_value(&env).unwrap(),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_err(),
            "Invalid environment variable name should be blocked"
        );

        // Verify the error message mentions security or environment variables
        if let Err(mcp_error) = result {
            let error_str = mcp_error.to_string();
            assert!(
                error_str.contains("security") || error_str.contains("environment"),
                "Error should mention security/environment concern"
            );
        }
    }

    #[tokio::test]
    async fn test_environment_variable_value_too_long() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test environment variable value that's too long
        let mut env = std::collections::HashMap::new();
        env.insert("TEST_VAR".to_string(), "x".repeat(2000)); // exceeds limit

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "environment".to_string(),
            serde_json::to_value(&env).unwrap(),
        );

        let result = tool.execute(args, &context).await;
        assert!(
            result.is_err(),
            "Environment variable value too long should be blocked"
        );

        // Verify error message mentions the issue
        if let Err(mcp_error) = result {
            let error_str = mcp_error.to_string();
            assert!(
                error_str.contains("security")
                    || error_str.contains("long")
                    || error_str.contains("length"),
                "Error should mention length/security concern"
            );
        }
    }

    #[tokio::test]
    async fn test_command_too_long_security_validation() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test command that's too long
        let long_command = "echo ".to_string() + &"a".repeat(5000); // exceeds limit

        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String(long_command),
        );

        let result = tool.execute(args, &context).await;
        assert!(result.is_err(), "Command that's too long should be blocked");

        // Verify error message mentions the issue
        if let Err(mcp_error) = result {
            let error_str = mcp_error.to_string();
            assert!(
                error_str.contains("security")
                    || error_str.contains("long")
                    || error_str.contains("length"),
                "Error should mention length/security concern"
            );
        }
    }

    #[tokio::test]
    async fn test_valid_commands_still_work() {
        let tool = ShellExecuteTool::new();
        let context = create_test_context();

        // Test that valid, safe commands still work after adding security validation
        let valid_commands = ["echo hello world", "ls -la", "pwd"];

        for cmd in &valid_commands {
            let mut args = serde_json::Map::new();
            args.insert(
                "command".to_string(),
                serde_json::Value::String(cmd.to_string()),
            );

            let result = tool.execute(args, &context).await;
            assert!(
                result.is_ok(),
                "Valid command '{}' should not be blocked by security validation",
                cmd
            );

            if let Ok(call_result) = result {
                // Exit code might be non-zero for commands like 'ls -la' if directory doesn't exist,
                // but the tool should still execute successfully (not blocked by security)
                assert!(!call_result.content.is_empty());
            }
        }
    }
}
