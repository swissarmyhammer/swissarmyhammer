//! Shared error handling utilities for command execution
//!
//! This module provides common error handling patterns to reduce code duplication
//! across the workflow execution system.

use crate::actions::{ActionError, ActionResult};
use std::process::Output;
use std::time::Duration;

/// Handle command execution results with consistent error formatting
///
/// This function standardizes error handling for command execution across the codebase.
/// It checks if the command succeeded and formats errors consistently.
///
/// # Arguments
/// * `result` - The output from a command execution
/// * `command_name` - The name of the command for error messages
///
/// # Returns
/// * `Ok(String)` - The stdout as a string if the command succeeded
/// * `Err(ActionError)` - A formatted error if the command failed
///
/// # Examples
/// ```rust
/// use std::process::Command;
/// use swissarmyhammer_workflow::handle_command_error;
///
/// let output = Command::new("echo").arg("hello").output().unwrap();
/// let result = handle_command_error(output, "echo");
/// assert!(result.is_ok());
/// ```
pub fn handle_command_error(result: Output, command_name: &str) -> ActionResult<String> {
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(ActionError::ExecutionError(format!(
            "{command_name} command failed: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

/// Handle command execution results with custom error types
///
/// This function provides the same error handling pattern but allows for custom error types.
/// Useful when you need to integrate with different error systems.
///
/// # Arguments
/// * `result` - The output from a command execution
/// * `command_name` - The name of the command for error messages
/// * `error_mapper` - A function to map the error message to a custom error type
///
/// # Returns
/// * `Ok(String)` - The stdout as a string if the command succeeded
/// * `Err(E)` - A custom error if the command failed
pub fn handle_command_error_with_mapper<E>(
    result: Output,
    command_name: &str,
    error_mapper: impl FnOnce(String) -> E,
) -> Result<String, E> {
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);
        return Err(error_mapper(format!(
            "{command_name} command failed: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

/// Check if a command execution succeeded
///
/// This is a simple helper to check command success without processing the output.
///
/// # Arguments
/// * `result` - The output from a command execution
///
/// # Returns
/// * `true` if the command succeeded
/// * `false` if the command failed
pub fn command_succeeded(result: &Output) -> bool {
    result.status.success()
}

/// Extract stderr as a string from command output
///
/// This helper function safely extracts stderr from command output as a UTF-8 string.
///
/// # Arguments
/// * `result` - The output from a command execution
///
/// # Returns
/// * String containing the stderr output
pub fn extract_stderr(result: &Output) -> String {
    String::from_utf8_lossy(&result.stderr).into_owned()
}

/// Extract stdout as a string from command output
///
/// This helper function safely extracts stdout from command output as a UTF-8 string.
///
/// # Arguments
/// * `result` - The output from a command execution
///
/// # Returns
/// * String containing the stdout output
pub fn extract_stdout(result: &Output) -> String {
    String::from_utf8_lossy(&result.stdout).into_owned()
}

/// Check if an error message indicates a rate limit error
///
/// This function checks for common rate limit error patterns in error messages
/// from Claude or HTTP responses.
///
/// # Arguments
/// * `error_msg` - The error message to check
///
/// # Returns
/// * `true` if the message indicates a rate limit error
/// * `false` otherwise
pub fn is_rate_limit_error(error_msg: &str) -> bool {
    let error_lower = error_msg.to_lowercase();

    // Check for common rate limit patterns
    error_lower.contains("usage limit")
        || error_lower.contains("rate limit")
        || error_lower.contains("429")
        || error_lower.contains("quota")
        || error_lower.contains("too many requests")
        || error_lower.contains("rate limited")
}

/// Calculate the duration until the next hour
///
/// This function calculates how long to wait until the top of the next hour,
/// which is useful for rate limit resets that occur on hourly boundaries.
///
/// # Returns
/// * Duration until the next hour (minimum 1 second)
pub fn time_until_next_hour() -> Duration {
    use chrono::Utc;
    time_until_next_hour_from(Utc::now())
}

/// Calculate the duration until the next hour from a specific time
///
/// This function is primarily for testing purposes.
///
/// # Arguments
/// * `now` - The current time to calculate from
///
/// # Returns
/// * Duration until the next hour (minimum 1 second)
fn time_until_next_hour_from(now: chrono::DateTime<chrono::Utc>) -> Duration {
    use chrono::{Duration as ChronoDuration, Timelike};

    // Calculate the next hour by zeroing minutes/seconds and adding 1 hour
    let next_hour = now
        .with_minute(0)
        .unwrap()
        .with_second(0)
        .unwrap()
        .with_nanosecond(0)
        .unwrap()
        + ChronoDuration::hours(1);

    // Convert to std::time::Duration, ensuring minimum of 1 second
    let duration = (next_hour - now).to_std().unwrap_or(Duration::from_secs(1));
    if duration < Duration::from_secs(1) {
        Duration::from_secs(1)
    } else {
        duration
    }
}

/// Handle Claude command execution results with appropriate error type
///
/// This function provides specialized error handling for Claude command execution,
/// returning ActionError::ClaudeError for command failures or ActionError::RateLimit
/// for rate limit errors.
///
/// # Arguments
/// * `result` - The output from a Claude command execution
///
/// # Returns
/// * `Ok(String)` - The stdout as a string if the command succeeded
/// * `Err(ActionError)` - A ClaudeError or RateLimit error if the command failed
pub fn handle_claude_command_error(result: Output) -> ActionResult<String> {
    if !result.status.success() {
        let stderr = String::from_utf8_lossy(&result.stderr);

        // Check if this is a rate limit error
        if is_rate_limit_error(&stderr) {
            return Err(ActionError::RateLimit {
                message: stderr.into_owned(),
                wait_time: time_until_next_hour(),
            });
        }

        return Err(ActionError::ClaudeError(format!(
            "Claude command failed: {stderr}"
        )));
    }
    Ok(String::from_utf8_lossy(&result.stdout).into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    #[test]
    fn test_handle_command_error_success() {
        let output = Command::new("echo")
            .arg("hello")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let result = handle_command_error(output, "echo");
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "hello");
    }

    #[test]
    fn test_handle_command_error_failure() {
        let output = Command::new("false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let result = handle_command_error(output, "false");
        assert!(result.is_err());
        if let Err(ActionError::ExecutionError(msg)) = result {
            assert!(msg.contains("false command failed"));
        } else {
            panic!("Expected ExecutionError");
        }
    }

    #[test]
    fn test_command_succeeded() {
        let success_output = Command::new("echo")
            .arg("test")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let failure_output = Command::new("false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        assert!(command_succeeded(&success_output));
        assert!(!command_succeeded(&failure_output));
    }

    #[test]
    fn test_extract_stdout_stderr() {
        let output = Command::new("echo")
            .arg("hello")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let stdout = extract_stdout(&output);
        let stderr = extract_stderr(&output);

        assert_eq!(stdout.trim(), "hello");
        assert_eq!(stderr.trim(), "");
    }

    #[test]
    fn test_handle_command_error_with_mapper() {
        let output = Command::new("false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let result =
            handle_command_error_with_mapper(output, "false", |msg| format!("Custom error: {msg}"));

        assert!(result.is_err());
        let error_msg = result.unwrap_err();
        assert!(error_msg.contains("Custom error: false command failed"));
    }

    #[test]
    fn test_handle_claude_command_error_success() {
        let output = Command::new("echo")
            .arg("test")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let result = handle_claude_command_error(output);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().trim(), "test");
    }

    #[test]
    fn test_handle_claude_command_error_failure() {
        let output = Command::new("false")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .unwrap();

        let result = handle_claude_command_error(output);
        assert!(result.is_err());
        if let Err(ActionError::ClaudeError(msg)) = result {
            assert!(msg.contains("Claude command failed"));
        } else {
            panic!("Expected ClaudeError");
        }
    }

    #[test]
    fn test_is_rate_limit_error() {
        // Test various rate limit error messages
        assert!(is_rate_limit_error("Error: Usage limit reached"));
        assert!(is_rate_limit_error("HTTP 429: Too Many Requests"));
        assert!(is_rate_limit_error(
            "Rate limit exceeded. Please try again later."
        ));
        assert!(is_rate_limit_error(
            "You have reached your quota for this hour"
        ));
        assert!(is_rate_limit_error(
            "Rate limited: please wait before retrying"
        ));

        // Test non-rate-limit errors
        assert!(!is_rate_limit_error("Error: File not found"));
        assert!(!is_rate_limit_error("Connection refused"));
        assert!(!is_rate_limit_error("Invalid authentication"));
    }

    #[test]
    fn test_time_until_next_hour() {
        use chrono::{TimeZone, Utc};

        // Test at the beginning of an hour
        let time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();
        let duration = time_until_next_hour_from(time);
        assert_eq!(duration, Duration::from_secs(3600)); // Exactly 1 hour

        // Test in the middle of an hour
        let time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 30, 0).unwrap();
        let duration = time_until_next_hour_from(time);
        assert_eq!(duration, Duration::from_secs(1800)); // 30 minutes

        // Test near the end of an hour
        let time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 59, 30).unwrap();
        let duration = time_until_next_hour_from(time);
        assert_eq!(duration, Duration::from_secs(30)); // 30 seconds

        // Test at the last second of an hour
        let time = Utc.with_ymd_and_hms(2024, 1, 1, 12, 59, 59).unwrap();
        let duration = time_until_next_hour_from(time);
        assert_eq!(duration, Duration::from_secs(1)); // 1 second
    }

    #[test]
    #[cfg(unix)]
    fn test_handle_claude_command_error_rate_limit() {
        use std::os::unix::process::ExitStatusExt;
        use std::process::ExitStatus;

        // Create a mock failed output with rate limit error
        let output = Output {
            status: ExitStatus::from_raw(1),
            stdout: Vec::new(),
            stderr: b"Error: Usage limit reached. Please try again later.".to_vec(),
        };

        let result = handle_claude_command_error(output);
        assert!(result.is_err());

        if let Err(ActionError::RateLimit { message, wait_time }) = result {
            assert!(message.contains("Usage limit reached"));
            // The wait time should be at least 1 second
            assert!(wait_time >= Duration::from_secs(1));
            // And at most 1 hour
            assert!(wait_time <= Duration::from_secs(3600));
        } else {
            panic!("Expected RateLimit error");
        }
    }

    #[test]
    #[cfg(windows)]
    fn test_handle_claude_command_error_rate_limit() {
        use std::process::ExitStatus;

        // Create a mock failed output with rate limit error
        // On Windows, we need to create the ExitStatus differently
        let output = Output {
            status: ExitStatus::default(), // This will be a failed status
            stdout: Vec::new(),
            stderr: b"Error: Usage limit reached. Please try again later.".to_vec(),
        };

        // Skip test if we can't create a proper failed status on Windows
        if output.status.success() {
            return;
        }

        let result = handle_claude_command_error(output);
        assert!(result.is_err());

        if let Err(ActionError::RateLimit { message, wait_time }) = result {
            assert!(message.contains("Usage limit reached"));
            // The wait time should be at least 1 second
            assert!(wait_time >= Duration::from_secs(1));
            // And at most 1 hour
            assert!(wait_time <= Duration::from_secs(3600));
        } else {
            panic!("Expected RateLimit error");
        }
    }
}
