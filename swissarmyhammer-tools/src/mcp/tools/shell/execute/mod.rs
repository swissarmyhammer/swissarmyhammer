//! Shell command execution tool for MCP operations
//!
//! This module provides the ShellExecuteTool for executing shell commands through the MCP protocol.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::Deserialize;

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
    #[allow(dead_code)] // Planned for future shell execution implementation
    environment: Option<std::collections::HashMap<String, String>>,
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

        // For now, return a placeholder response indicating the infrastructure is ready
        // The actual command execution will be implemented in subsequent issues
        let response_message = format!(
            "Shell tool infrastructure ready. Command '{}' would be executed with timeout {} seconds in directory '{}'",
            request.command,
            request.timeout.unwrap_or(300),
            request.working_directory.as_deref().unwrap_or("current directory")
        );

        tracing::info!(
            "Shell infrastructure validated for command: {}",
            request.command
        );
        Ok(BaseToolImpl::create_success_response(response_message))
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
}
