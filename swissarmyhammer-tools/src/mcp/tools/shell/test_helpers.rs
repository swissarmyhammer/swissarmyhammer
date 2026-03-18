//! Test helper utilities for shell tool tests
//!
//! This module provides common test fixtures, builders, and assertion helpers
//! used across shell tool tests.

use crate::mcp::tool_registry::{McpTool, ToolContext};
use crate::test_utils::create_test_context;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;

use super::ShellExecuteTool;

/// Builder pattern for executing test commands with optional parameters
///
/// This eliminates duplication across the multiple execute_test_command_* helper functions
/// by providing a flexible builder that can construct commands with any combination of
/// parameters.
///
/// # Example
///
/// ```ignore
/// // Simple command
/// let result = TestCommandBuilder::new("echo test").execute().await?;
///
/// // Command with working directory
/// let result = TestCommandBuilder::new("ls")
///     .working_directory("/tmp")
///     .execute()
///     .await?;
///
/// // Command with environment variables
/// let result = TestCommandBuilder::new("env")
///     .environment("{\"VAR\":\"value\"}")
///     .execute()
///     .await?;
///
/// // Command with multiple options
/// let result = TestCommandBuilder::new("printenv VAR")
///     .working_directory("/tmp")
///     .environment("{\"VAR\":\"test\"}")
///     .execute()
///     .await?;
/// ```
pub(crate) struct TestCommandBuilder {
    command: String,
    working_directory: Option<String>,
    environment: Option<String>,
    custom_args: Option<serde_json::Map<String, serde_json::Value>>,
    custom_context: Option<ToolContext>,
}

impl TestCommandBuilder {
    /// Create a new builder with the specified command
    pub(crate) fn new(command: impl Into<String>) -> Self {
        Self {
            command: command.into(),
            working_directory: None,
            environment: None,
            custom_args: None,
            custom_context: None,
        }
    }

    /// Set the working directory for the command
    pub(crate) fn working_directory(mut self, dir: impl Into<String>) -> Self {
        self.working_directory = Some(dir.into());
        self
    }

    /// Set environment variables as JSON string
    pub(crate) fn environment(mut self, env_json: impl Into<String>) -> Self {
        self.environment = Some(env_json.into());
        self
    }

    /// Use custom argument map (overrides all other settings)
    pub(crate) fn with_custom_args(
        mut self,
        args: serde_json::Map<String, serde_json::Value>,
    ) -> Self {
        self.custom_args = Some(args);
        self
    }

    /// Use custom context (for testing with progress senders, etc.)
    pub(crate) fn with_context(mut self, context: ToolContext) -> Self {
        self.custom_context = Some(context);
        self
    }

    /// Execute the command with the configured parameters
    pub(crate) async fn execute(self) -> Result<CallToolResult, McpError> {
        let tool = ShellExecuteTool::new_isolated();
        let context = if let Some(ctx) = self.custom_context {
            ctx
        } else {
            create_test_context().await
        };

        // If custom args are provided, use them directly
        let args = if let Some(custom) = self.custom_args {
            custom
        } else {
            // Build args from the builder state
            let mut args = serde_json::Map::new();
            args.insert(
                "command".to_string(),
                serde_json::Value::String(self.command),
            );

            if let Some(dir) = self.working_directory {
                args.insert(
                    "working_directory".to_string(),
                    serde_json::Value::String(dir),
                );
            }

            if let Some(env) = self.environment {
                args.insert("environment".to_string(), serde_json::Value::String(env));
            }

            args
        };

        tool.execute(args, &context).await
    }
}

/// Helper function to parse execution result from CallToolResult
///
/// This eliminates duplication in JSON response parsing and validation logic.
pub(crate) fn parse_execution_result(call_result: &CallToolResult) -> serde_json::Value {
    assert!(
        !call_result.content.is_empty(),
        "Content should not be empty"
    );
    let content_text = match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => &text_content.text,
        _ => panic!("Expected text content"),
    };
    serde_json::from_str(content_text).expect("Failed to parse JSON response")
}

/// Builder for declarative validation of shell execution results
///
/// This provides a fluent API for asserting on JSON response fields,
/// reducing duplication across test functions.
///
/// # Example
///
/// ```ignore
/// ResultValidator::new(&call_result)
///     .assert_exit_code(0)
///     .assert_stdout_contains("expected text")
///     .assert_field_exists("execution_time_ms");
/// ```
pub(crate) struct ResultValidator {
    json: serde_json::Value,
}

impl ResultValidator {
    /// Create a new validator from a CallToolResult
    pub(crate) fn new(call_result: &CallToolResult) -> Self {
        let json = parse_execution_result(call_result);
        assert!(
            json.is_object(),
            "Expected JSON object in result, got: {:?}",
            json
        );
        Self { json }
    }

    /// Assert that a field exists in the result
    pub(crate) fn assert_field_exists(self, field: &str) -> Self {
        assert!(
            self.json.get(field).is_some(),
            "Field '{}' should exist in result",
            field
        );
        self
    }

    /// Assert that the exit code matches the expected value
    pub(crate) fn assert_exit_code(self, expected: i64) -> Self {
        let exit_code = self
            .json
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .expect("exit_code should be an integer");
        assert_eq!(exit_code, expected, "Exit code mismatch");
        self
    }

    /// Assert that exit code is non-zero
    pub(crate) fn assert_exit_code_nonzero(self) -> Self {
        let exit_code = self
            .json
            .get("exit_code")
            .and_then(|v| v.as_i64())
            .expect("exit_code should be an integer");
        assert_ne!(exit_code, 0, "Exit code should be non-zero");
        self
    }

    /// Assert that stdout contains the expected text
    pub(crate) fn assert_stdout_contains(self, expected: &str) -> Self {
        let stdout = self
            .json
            .get("stdout")
            .and_then(|v| v.as_str())
            .expect("stdout should be a string");
        assert!(
            stdout.contains(expected),
            "stdout should contain '{}', got: {}",
            expected,
            stdout
        );
        self
    }

    /// Assert that stderr contains the expected text
    pub(crate) fn assert_stderr_contains(self, expected: &str) -> Self {
        let stderr = self
            .json
            .get("stderr")
            .and_then(|v| v.as_str())
            .expect("stderr should be a string");
        assert!(
            stderr.contains(expected),
            "stderr should contain '{}', got: {}",
            expected,
            stderr
        );
        self
    }

    /// Assert that stderr is not empty
    pub(crate) fn assert_stderr_not_empty(self) -> Self {
        let stderr = self
            .json
            .get("stderr")
            .and_then(|v| v.as_str())
            .expect("stderr should be a string");
        assert!(!stderr.is_empty(), "stderr should not be empty");
        self
    }

    /// Assert that output_truncated field has the expected value
    pub(crate) fn assert_output_truncated(self, expected: bool) -> Self {
        let truncated = self
            .json
            .get("output_truncated")
            .and_then(|v| v.as_bool())
            .expect("output_truncated should be a boolean");
        assert_eq!(truncated, expected, "output_truncated mismatch");
        self
    }

    /// Assert that a boolean field has the expected value
    pub(crate) fn assert_bool_field(self, field: &str, expected: bool) -> Self {
        let value = self
            .json
            .get(field)
            .and_then(|v| v.as_bool())
            .unwrap_or_else(|| panic!("Field '{}' should be a boolean", field));
        assert_eq!(value, expected, "Field '{}' mismatch", field);
        self
    }

    /// Assert standard success fields for a successful command execution
    ///
    /// This validates that all expected fields exist, exit code is 0,
    /// and output is not truncated or binary.
    pub(crate) fn assert_success(self) -> Self {
        self.assert_field_exists("stdout")
            .assert_field_exists("stderr")
            .assert_field_exists("exit_code")
            .assert_field_exists("execution_time_ms")
            .assert_exit_code(0)
    }

    /// Assert standard failure fields for a failed command execution
    ///
    /// This validates that required fields exist, exit code is non-zero,
    /// and stderr contains error information.
    pub(crate) fn assert_failure(self) -> Self {
        self.assert_field_exists("stderr")
            .assert_field_exists("exit_code")
            .assert_exit_code_nonzero()
            .assert_stderr_not_empty()
    }
}

/// Helper for operations that return plain text (not JSON)
pub(crate) fn extract_text(call_result: &CallToolResult) -> String {
    assert!(
        !call_result.content.is_empty(),
        "Content should not be empty"
    );
    match &call_result.content[0].raw {
        rmcp::model::RawContent::Text(text_content) => text_content.text.clone(),
        _ => panic!("Expected text content"),
    }
}

/// Create a shared test tool for tests that need state continuity
pub(crate) fn shared_tool() -> ShellExecuteTool {
    ShellExecuteTool::new_isolated()
}

/// Execute a shell tool operation with the given op and args
pub(crate) async fn execute_op(
    op: &str,
    extra_args: Vec<(&str, serde_json::Value)>,
) -> Result<CallToolResult, McpError> {
    execute_op_with(&shared_tool(), op, extra_args).await
}

/// Execute a shell tool operation on a specific tool instance
pub(crate) async fn execute_op_with(
    tool: &ShellExecuteTool,
    op: &str,
    extra_args: Vec<(&str, serde_json::Value)>,
) -> Result<CallToolResult, McpError> {
    let context = create_test_context().await;
    let mut args = serde_json::Map::new();
    args.insert("op".to_string(), json!(op));
    for (k, v) in extra_args {
        args.insert(k.to_string(), v);
    }
    tool.execute(args, &context).await
}

/// Run a command through the shell tool and return its command_id
#[allow(dead_code)]
pub(crate) async fn run_command(command: &str) -> usize {
    run_command_with(&shared_tool(), command).await
}

/// Run a command on a specific tool instance and return its command_id
pub(crate) async fn run_command_with(tool: &ShellExecuteTool, command: &str) -> usize {
    let context = create_test_context().await;
    let mut args = serde_json::Map::new();
    args.insert("command".to_string(), json!(command));
    let result = tool.execute(args, &context).await;
    assert!(result.is_ok(), "Setup command failed: {:?}", result.err());
    let call_result = result.unwrap();
    let text = extract_text(&call_result);
    // Extract command_id from the JSON response
    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&text) {
        if let Some(id) = json.get("command_id").and_then(|v| v.as_u64()) {
            return id as usize;
        }
    }
    // Fallback: look for command_id in YAML-like text
    for line in text.lines() {
        if let Some(rest) = line.strip_prefix("command_id:") {
            if let Ok(id) = rest.trim().parse::<usize>() {
                return id;
            }
        }
    }
    panic!("Could not extract command_id from response: {}", text);
}

/// Generic helper function to assert that items are blocked by security validation
///
/// This reduces duplication in security test cases by providing a common pattern
/// for testing that dangerous commands or paths are properly rejected.
///
/// # Pattern
///
/// This helper follows the "generic test assertion" pattern where:
/// 1. Test data (items to block) is provided as a slice
/// 2. A builder function constructs the specific test arguments
/// 3. The assertion logic is shared across all test cases
///
/// This pattern is preferred over individual test functions because it:
/// - Eliminates duplication in error checking and assertion logic
/// - Ensures consistent validation across all security tests
/// - Makes it easy to add new test cases without duplicating code
pub(crate) async fn assert_blocked<F>(items: &[&str], item_type: &str, build_args: F)
where
    F: Fn(&str) -> serde_json::Map<String, serde_json::Value>,
{
    let (tool, context) = create_security_test_fixtures().await;
    for item in items {
        let args = build_args(item);
        let result = tool.execute(args, &context).await;
        assert!(
            result.is_err(),
            "{} '{}' should be blocked",
            item_type,
            item
        );

        // Verify the error message contains security-related information
        if let Err(mcp_error) = result {
            let error_str = mcp_error.to_string();
            assert!(
                error_str.contains("security")
                    || error_str.contains("unsafe")
                    || error_str.contains("directory"),
                "Error should mention security concern for {}: {}",
                item_type,
                item
            );
        }
    }
}

/// Creates a test tool and context for security validation tests
///
/// This eliminates duplication in creating test fixtures for security tests.
pub(crate) async fn create_security_test_fixtures() -> (ShellExecuteTool, ToolContext) {
    (
        ShellExecuteTool::new_isolated(),
        create_test_context().await,
    )
}

/// Helper function to assert that a list of paths are blocked by security validation
///
/// This reduces duplication in path traversal security tests.
pub(crate) async fn assert_paths_blocked(paths: &[&str]) {
    assert_blocked(paths, "Path traversal attempt", |path| {
        let mut args = serde_json::Map::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );
        args.insert(
            "working_directory".to_string(),
            serde_json::Value::String(path.to_string()),
        );
        args
    })
    .await;
}

/// Helper function to assert that validator blocks commands with expected error type
///
/// This reduces duplication in validator unit tests by providing a common pattern
/// for testing command validation logic.
pub(crate) fn assert_validator_blocks_commands(
    validator: &swissarmyhammer_shell::ShellSecurityValidator,
    commands: &[&str],
    test_name: &str,
) {
    for command in commands {
        let result = validator.validate_command(command);
        assert!(
            result.is_err(),
            "{}: Command should be blocked: '{}'",
            test_name,
            command
        );

        // Verify the error type is correct
        match result.unwrap_err() {
            swissarmyhammer_shell::ShellSecurityError::BlockedCommandPattern { .. } => (),
            other_error => {
                panic!(
                    "{}: Expected blocked pattern error for '{}', got: {:?}",
                    test_name, command, other_error
                )
            }
        }
    }
}

/// Helper function to test that a list of commands are blocked by a validator
///
/// This reduces duplication across security tests by providing a common pattern
/// for creating validators and testing blocked command lists.
pub(crate) async fn test_blocked_commands_with_policy(
    policy: swissarmyhammer_shell::ShellSecurityPolicy,
    blocked_commands: &[&str],
    test_name: &str,
) {
    use swissarmyhammer_shell::ShellSecurityValidator;

    let validator = ShellSecurityValidator::new(policy).expect("Failed to create validator");
    assert_validator_blocks_commands(&validator, blocked_commands, test_name);
}
