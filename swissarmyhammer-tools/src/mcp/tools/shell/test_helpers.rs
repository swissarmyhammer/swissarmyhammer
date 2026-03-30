//! Test helper utilities for shell tool tests
//!
//! This module provides common test fixtures, builders, and assertion helpers
//! used across shell tool tests.

use crate::mcp::tool_registry::{McpTool, ToolContext};
use crate::test_utils::create_test_context;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::json;
use std::collections::HashMap;

use super::ShellExecuteTool;

/// Builder pattern for executing test commands with optional parameters
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

/// Parse the status-only text response into a key-value map.
///
/// The response format is:
/// ```text
/// command_id: 1
/// status: completed
/// exit_code: 0
/// lines: 47
/// duration: 1234ms
/// ```
pub(crate) fn parse_status_response(call_result: &CallToolResult) -> HashMap<String, String> {
    let text = extract_text(call_result);
    let mut map = HashMap::new();
    for line in text.lines() {
        if let Some((key, value)) = line.split_once(':') {
            map.insert(key.trim().to_string(), value.trim().to_string());
        }
    }
    map
}

/// Builder for declarative validation of shell execution results (status-only format)
pub(crate) struct ResultValidator {
    fields: HashMap<String, String>,
}

impl ResultValidator {
    /// Create a new validator from a CallToolResult
    pub(crate) fn new(call_result: &CallToolResult) -> Self {
        let fields = parse_status_response(call_result);
        assert!(!fields.is_empty(), "Expected non-empty status response");
        Self { fields }
    }

    /// Assert that a field exists in the result
    pub(crate) fn assert_field_exists(self, field: &str) -> Self {
        assert!(
            self.fields.contains_key(field),
            "Field '{}' should exist in result. Fields: {:?}",
            field,
            self.fields.keys().collect::<Vec<_>>()
        );
        self
    }

    /// Assert that the exit code matches the expected value
    pub(crate) fn assert_exit_code(self, expected: i64) -> Self {
        let exit_code: i64 = self
            .fields
            .get("exit_code")
            .expect("exit_code should exist")
            .parse()
            .expect("exit_code should be an integer");
        assert_eq!(exit_code, expected, "Exit code mismatch");
        self
    }

    /// Assert that exit code is non-zero
    pub(crate) fn assert_exit_code_nonzero(self) -> Self {
        let exit_code: i64 = self
            .fields
            .get("exit_code")
            .expect("exit_code should exist")
            .parse()
            .expect("exit_code should be an integer");
        assert_ne!(exit_code, 0, "Exit code should be non-zero");
        self
    }

    /// Assert that the line count is positive
    pub(crate) fn assert_has_lines(self) -> Self {
        let lines: usize = self
            .fields
            .get("lines")
            .expect("lines should exist")
            .parse()
            .expect("lines should be an integer");
        assert!(lines > 0, "Expected at least one line of output");
        self
    }

    /// Assert standard success fields for a successful command execution
    pub(crate) fn assert_success(self) -> Self {
        self.assert_field_exists("command_id")
            .assert_field_exists("exit_code")
            .assert_field_exists("lines")
            .assert_field_exists("duration")
            .assert_exit_code(0)
    }

    /// Assert standard failure fields for a failed command execution
    pub(crate) fn assert_failure(self) -> Self {
        self.assert_field_exists("exit_code")
            .assert_exit_code_nonzero()
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
    let fields = parse_status_response(&call_result);
    fields
        .get("command_id")
        .expect("command_id should exist in response")
        .parse::<usize>()
        .expect("command_id should be a number")
}

/// Generic helper function to assert that items are blocked by security validation
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
pub(crate) async fn create_security_test_fixtures() -> (ShellExecuteTool, ToolContext) {
    (
        ShellExecuteTool::new_isolated(),
        create_test_context().await,
    )
}

/// Helper function to assert that a list of paths are blocked by security validation
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
pub(crate) async fn test_blocked_commands_with_policy(
    policy: swissarmyhammer_shell::ShellSecurityPolicy,
    blocked_commands: &[&str],
    test_name: &str,
) {
    use swissarmyhammer_shell::ShellSecurityValidator;

    let validator = ShellSecurityValidator::new(policy).expect("Failed to create validator");
    assert_validator_blocks_commands(&validator, blocked_commands, test_name);
}
