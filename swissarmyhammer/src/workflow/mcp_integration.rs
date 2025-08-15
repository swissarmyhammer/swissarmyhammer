//! Enhanced shell execution for workflow actions
//!
//! This module provides enhanced shell execution capabilities using modern async patterns
//! and improved error handling while maintaining backward compatibility with existing workflows.

use crate::workflow::ActionError;
use serde_json::Value;
use std::collections::HashMap;
use std::path::Path;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

/// Enhanced shell execution result with detailed metadata
#[derive(Debug, Clone)]
pub struct EnhancedShellResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub execution_time_ms: u64,
    pub command: String,
}

/// Enhanced shell executor with advanced capabilities
pub struct EnhancedShellExecutor {
    max_output_size: usize,
}

impl Default for EnhancedShellExecutor {
    fn default() -> Self {
        Self {
            max_output_size: 10 * 1024 * 1024, // 10MB default limit
        }
    }
}

impl EnhancedShellExecutor {
    /// Create a new enhanced shell executor
    pub fn new() -> Self {
        Self::default()
    }

    /// Execute a shell command with enhanced capabilities
    pub async fn execute_shell_command(
        &self,
        command: &str,
        working_directory: Option<&str>,
        environment: &HashMap<String, String>,
        timeout_secs: Option<u32>,
    ) -> Result<EnhancedShellResult, ActionError> {
        let start_time = Instant::now();

        // Create platform-specific command
        let mut cmd = self.create_platform_command(command);

        // Set working directory if specified
        if let Some(work_dir) = working_directory {
            let path = Path::new(work_dir);
            if !path.exists() {
                return Err(ActionError::ExecutionError(format!(
                    "Working directory does not exist: {}",
                    work_dir
                )));
            }
            if !path.is_dir() {
                return Err(ActionError::ExecutionError(format!(
                    "Working directory is not a directory: {}",
                    work_dir
                )));
            }
            cmd.current_dir(path);
        }

        // Set environment variables
        for (key, value) in environment {
            cmd.env(key, value);
        }

        // Configure stdio
        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .stdin(Stdio::null());

        // Spawn the process
        let mut child = cmd.spawn().map_err(|e| {
            ActionError::ExecutionError(format!("Failed to spawn command '{}': {}", command, e))
        })?;

        // Execute with or without timeout
        let result = if let Some(timeout_duration) = timeout_secs {
            let duration = Duration::from_secs(timeout_duration as u64);
            self.execute_with_timeout(&mut child, duration, command)
                .await
        } else {
            self.execute_without_timeout(&mut child, command).await
        };

        let execution_time_ms = start_time.elapsed().as_millis() as u64;

        match result {
            Ok((exit_code, stdout, stderr)) => Ok(EnhancedShellResult {
                exit_code,
                stdout,
                stderr,
                execution_time_ms,
                command: command.to_string(),
            }),
            Err(e) => Err(e),
        }
    }

    /// Execute with timeout
    async fn execute_with_timeout(
        &self,
        child: &mut tokio::process::Child,
        timeout_duration: Duration,
        command: &str,
    ) -> Result<(i32, String, String), ActionError> {
        let output_future = async {
            // Get handles before waiting
            let stdout = child.stdout.take().ok_or_else(|| {
                ActionError::ExecutionError("Failed to get stdout handle".to_string())
            })?;
            let stderr = child.stderr.take().ok_or_else(|| {
                ActionError::ExecutionError("Failed to get stderr handle".to_string())
            })?;

            // Read outputs with size limits
            let stdout_task = self.read_output_with_limit(stdout, self.max_output_size);
            let stderr_task = self.read_output_with_limit(stderr, self.max_output_size);

            // Wait for process and read outputs concurrently
            let (status_result, stdout_result, stderr_result) =
                tokio::join!(child.wait(), stdout_task, stderr_task);

            let status = status_result
                .map_err(|e| ActionError::ExecutionError(format!("Process wait failed: {}", e)))?;

            let stdout = stdout_result?;
            let stderr = stderr_result?;
            let exit_code = status.code().unwrap_or(-1);

            Ok::<(i32, String, String), ActionError>((exit_code, stdout, stderr))
        };

        match timeout(timeout_duration, output_future).await {
            Ok(result) => result,
            Err(_) => {
                // Timeout occurred - terminate process
                tracing::warn!(
                    "Command '{}' timed out after {:?}",
                    command,
                    timeout_duration
                );

                // Kill the process
                if let Err(e) = child.kill().await {
                    tracing::error!("Failed to kill timed out process: {}", e);
                }

                Ok((-1, String::new(), "Command timed out".to_string()))
            }
        }
    }

    /// Execute without timeout
    async fn execute_without_timeout(
        &self,
        child: &mut tokio::process::Child,
        _command: &str,
    ) -> Result<(i32, String, String), ActionError> {
        // Get handles before taking ownership
        let stdout = child.stdout.take().ok_or_else(|| {
            ActionError::ExecutionError("Failed to get stdout handle".to_string())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            ActionError::ExecutionError("Failed to get stderr handle".to_string())
        })?;

        // Read outputs with size limits concurrently with waiting for process
        let stdout_task = self.read_output_with_limit(stdout, self.max_output_size);
        let stderr_task = self.read_output_with_limit(stderr, self.max_output_size);

        let (status_result, stdout_result, stderr_result) =
            tokio::join!(child.wait(), stdout_task, stderr_task);

        let status = status_result
            .map_err(|e| ActionError::ExecutionError(format!("Command execution failed: {}", e)))?;

        let stdout = stdout_result?;
        let stderr = stderr_result?;
        let exit_code = status.code().unwrap_or(-1);

        Ok((exit_code, stdout, stderr))
    }

    /// Read output with size limits to prevent memory exhaustion
    async fn read_output_with_limit(
        &self,
        reader: impl AsyncReadExt + Unpin,
        max_size: usize,
    ) -> Result<String, ActionError> {
        let mut buffer = Vec::with_capacity(std::cmp::min(max_size, 8192)); // Reserve reasonable initial size

        reader
            .take(max_size as u64)
            .read_to_end(&mut buffer)
            .await
            .map_err(|e| ActionError::ExecutionError(format!("Failed to read output: {}", e)))?;

        self.safe_string_conversion(&buffer, max_size)
    }

    /// Safely convert bytes to string with size limits and binary detection
    fn safe_string_conversion(&self, bytes: &[u8], max_size: usize) -> Result<String, ActionError> {
        // Truncate if too large
        let bytes = if bytes.len() > max_size {
            &bytes[..max_size]
        } else {
            bytes
        };

        // Check for binary content
        if self.contains_binary_content(bytes) {
            return Ok(format!(
                "[Binary content detected ({} bytes){}]",
                bytes.len(),
                if bytes.len() >= max_size {
                    " - truncated"
                } else {
                    ""
                }
            ));
        }

        // Convert to string
        let mut result = String::from_utf8_lossy(bytes).to_string();

        // Add truncation marker if needed
        if bytes.len() >= max_size {
            result.push_str("\n[Output truncated - limit exceeded]");
        }

        Ok(result)
    }

    /// Detect binary content in byte array
    fn contains_binary_content(&self, bytes: &[u8]) -> bool {
        // Check for null bytes or high concentration of control characters
        let mut control_chars = 0;
        let sample_size = std::cmp::min(bytes.len(), 1024);

        for &byte in &bytes[..sample_size] {
            if byte == 0 {
                return true; // Null byte indicates binary
            }
            if byte < 32 && byte != b'\n' && byte != b'\r' && byte != b'\t' {
                control_chars += 1;
            }
        }

        // If more than 5% are control characters, consider it binary
        sample_size > 0 && (control_chars * 100 / sample_size) > 5
    }

    /// Create platform-specific command
    fn create_platform_command(&self, command: &str) -> Command {
        if cfg!(target_os = "windows") {
            let mut cmd = Command::new("cmd");
            cmd.args(["/C", command]);
            cmd
        } else {
            let mut cmd = Command::new("sh");
            cmd.args(["-c", command]);
            cmd
        }
    }
}

/// Enhanced shell execution context with workflow integration capabilities
pub struct WorkflowShellContext {
    executor: EnhancedShellExecutor,
}

impl WorkflowShellContext {
    /// Create new workflow shell context
    pub async fn new() -> Result<Self, ActionError> {
        Ok(Self {
            executor: EnhancedShellExecutor::new(),
        })
    }

    /// Execute shell command and return structured result data
    pub async fn execute_shell_command(
        &self,
        command: String,
        working_directory: Option<String>,
        environment: HashMap<String, String>,
        timeout_secs: Option<u32>,
    ) -> Result<Value, ActionError> {
        let result = self
            .executor
            .execute_shell_command(
                &command,
                working_directory.as_deref(),
                &environment,
                timeout_secs,
            )
            .await?;

        // Convert to JSON format expected by workflow system
        Ok(serde_json::json!({
            "exit_code": result.exit_code,
            "stdout": result.stdout,
            "stderr": result.stderr,
            "execution_time_ms": result.execution_time_ms,
            "command": result.command
        }))
    }
}

/// Utilities for processing enhanced shell results in workflow context
pub mod response_processing {
    use super::*;

    /// Extract JSON data from enhanced shell result
    pub fn extract_json_data(result: &Value) -> Result<Value, ActionError> {
        // The result is already in JSON format from enhanced shell execution
        Ok(result.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_workflow_shell_context_creation() {
        let result = WorkflowShellContext::new().await;
        assert!(
            result.is_ok(),
            "Failed to create WorkflowShellContext: {:?}",
            result.err()
        );
    }

    #[tokio::test]
    async fn test_enhanced_shell_executor() {
        let executor = EnhancedShellExecutor::new();
        let result = executor
            .execute_shell_command("echo 'test'", None, &HashMap::new(), Some(10))
            .await;

        assert!(
            result.is_ok(),
            "Command execution failed: {:?}",
            result.err()
        );
        let shell_result = result.unwrap();
        assert_eq!(shell_result.exit_code, 0);
        assert!(shell_result.stdout.contains("test"));
    }

    #[tokio::test]
    async fn test_binary_detection() {
        let executor = EnhancedShellExecutor::new();
        let binary_data = vec![0u8, 1, 2, 3, 255]; // Contains null byte
        let result = executor.safe_string_conversion(&binary_data, 1024);

        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("Binary content detected"));
    }
}
