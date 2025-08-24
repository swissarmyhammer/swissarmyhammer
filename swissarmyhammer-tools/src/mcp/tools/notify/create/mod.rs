//! Notification creation tool for MCP operations
//!
//! This module provides the NotifyTool for sending messages from LLMs to users through the
//! logging system. The tool enables LLMs to communicate important information, status updates,
//! and contextual feedback during workflow execution.

use crate::mcp::notify_types::NotifyRequest;
use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;

/// Tool for sending notification messages to users through the logging system
#[derive(Default)]
pub struct NotifyTool;

impl NotifyTool {
    /// Creates a new instance of the NotifyTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for NotifyTool {
    fn name(&self) -> &'static str {
        "notify_create"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "message": {
                    "type": "string",
                    "description": "The message to notify the user about",
                    "minLength": 1
                },
                "level": {
                    "type": "string",
                    "enum": ["info", "warn", "error"],
                    "description": "The notification level (default: info)",
                    "default": "info"
                },
                "context": {
                    "type": "object",
                    "description": "Optional structured JSON data for the notification",
                    "default": {}
                }
            },
            "required": ["message"]
        })
    }

    fn hidden_from_cli(&self) -> bool {
        true
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: NotifyRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for notifications
        context
            .rate_limiter
            .check_rate_limit("unknown", "notify_create", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for notification: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Creating notification: {}", request.message);

        // Validate request using built-in validation
        request
            .validate()
            .map_err(|e| McpError::invalid_params(e, None))?;

        // Additional validation using shared utilities
        McpValidation::validate_not_empty(&request.message, "notification message")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate notification message"))?;

        // Get the typed notification level
        let level = request.get_level();
        let level_str: &str = level.into();

        // Get the context, defaulting to empty object
        let notification_context = request.context.unwrap_or_default();

        // Send the notification through the tracing system with the "llm_notify" target
        match level_str {
            "info" => tracing::info!(
                target: "llm_notify",
                context = %notification_context,
                "{}",
                request.message
            ),
            "warn" => tracing::warn!(
                target: "llm_notify",
                context = %notification_context,
                "{}",
                request.message
            ),
            "error" => tracing::error!(
                target: "llm_notify",
                context = %notification_context,
                "{}",
                request.message
            ),
            _ => tracing::info!(
                target: "llm_notify",
                context = %notification_context,
                "{}",
                request.message
            ),
        }

        tracing::debug!("Notification sent successfully: {}", request.message);
        Ok(BaseToolImpl::create_success_response(format!(
            "Notification sent: {}",
            request.message
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::notify_types::NotifyRequest;
    use crate::mcp::tool_registry::{BaseToolImpl, ToolRegistry};
    use serde_json::json;

    #[test]
    fn test_notify_tool_name() {
        let tool = NotifyTool::new();
        assert_eq!(tool.name(), "notify_create");
    }

    #[test]
    fn test_notify_tool_description() {
        let tool = NotifyTool::new();
        let description = tool.description();
        assert!(description.contains("notification"));
        assert!(description.contains("message"));
    }

    #[test]
    fn test_notify_tool_schema() {
        let tool = NotifyTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("message"));
        assert!(properties.contains_key("level"));
        assert!(properties.contains_key("context"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("message".to_string())));
    }

    #[test]
    fn test_parse_valid_arguments_minimal() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("Test notification message".to_string()),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.message, "Test notification message");
        assert_eq!(request.level, None);
        assert_eq!(request.context, None);
    }

    #[test]
    fn test_parse_valid_arguments_full() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("Test notification message".to_string()),
        );
        args.insert(
            "level".to_string(),
            serde_json::Value::String("warn".to_string()),
        );
        args.insert(
            "context".to_string(),
            json!({"stage": "analysis", "file_count": 42}),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.message, "Test notification message");
        assert_eq!(request.level, Some("warn".to_string()));
        assert_eq!(
            request.context,
            Some(json!({"stage": "analysis", "file_count": 42}))
        );
    }

    #[test]
    fn test_parse_missing_message() {
        let args = serde_json::Map::new();
        let result: Result<NotifyRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_message() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.message, "");
    }

    #[test]
    fn test_parse_invalid_level_type() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("Test message".to_string()),
        );
        args.insert("level".to_string(), serde_json::Value::Number(42.into()));

        let result: Result<NotifyRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let mut args = serde_json::Map::new();
        args.insert("message".to_string(), serde_json::Value::Number(42.into()));

        let result: Result<NotifyRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_valid_levels() {
        let levels = ["info", "warn", "error"];

        for level in levels {
            let mut args = serde_json::Map::new();
            args.insert(
                "message".to_string(),
                serde_json::Value::String("Test message".to_string()),
            );
            args.insert(
                "level".to_string(),
                serde_json::Value::String(level.to_string()),
            );

            let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
            assert_eq!(request.level, Some(level.to_string()));
        }
    }

    #[test]
    fn test_parse_complex_context() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("Complex notification".to_string()),
        );
        args.insert(
            "context".to_string(),
            json!({
                "nested": {
                    "data": "value",
                    "numbers": [1, 2, 3],
                    "boolean": true
                },
                "array": ["a", "b", "c"]
            }),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert!(request.context.is_some());
        let context = request.context.unwrap();
        assert!(context.is_object());
        assert!(context["nested"]["data"] == "value");
        assert!(context["array"].is_array());
    }

    #[test]
    fn test_parse_unicode_message() {
        let unicode_message = "通知消息 🔔 with émojis and ñoñ-ASCII characters";
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String(unicode_message.to_string()),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.message, unicode_message);
    }

    #[test]
    fn test_parse_long_message() {
        let long_message = "x".repeat(10000);
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String(long_message.clone()),
        );

        let request: NotifyRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.message, long_message);
    }

    #[test]
    fn test_tool_registration() {
        let mut registry = ToolRegistry::new();
        registry.register(NotifyTool::new());

        assert!(registry.get_tool("notify_create").is_some());
        assert!(registry
            .list_tool_names()
            .contains(&"notify_create".to_string()));
    }

    // ============================================================================
    // ASYNC EXECUTION TESTS - Following established patterns from other MCP tools
    // ============================================================================

    use crate::test_utils::create_test_context;

    #[tokio::test]
    async fn test_execute_success_minimal_message() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Test notification".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains("Notification sent"));
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains("Test notification"));
    }

    #[tokio::test]
    async fn test_execute_success_with_level_info() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Info level message".to_string()),
        );
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("info".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(!call_result.content.is_empty());
    }

    #[tokio::test]
    async fn test_execute_success_with_level_warn() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Warning message".to_string()),
        );
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("warn".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains("Warning message"));
    }

    #[tokio::test]
    async fn test_execute_success_with_level_error() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Error message".to_string()),
        );
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("error".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains("Error message"));
    }

    #[tokio::test]
    async fn test_execute_success_with_context() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Message with context".to_string()),
        );
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("info".to_string()),
        );
        arguments.insert(
            "context".to_string(),
            json!({"stage": "analysis", "file_count": 42}),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains("Message with context"));
    }

    #[tokio::test]
    async fn test_execute_invalid_level_defaults_to_info() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Message with invalid level".to_string()),
        );
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("invalid_level".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok()); // Should succeed, defaulting to info level

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_case_insensitive_levels() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let levels = ["INFO", "WARN", "ERROR", "Warn", "Error"];

        for level in levels {
            let mut arguments = serde_json::Map::new();
            arguments.insert(
                "message".to_string(),
                serde_json::Value::String(format!("Message with {level} level")),
            );
            arguments.insert(
                "level".to_string(),
                serde_json::Value::String(level.to_string()),
            );

            let result = tool.execute(arguments, &context).await;
            assert!(result.is_ok());

            let call_result = result.unwrap();
            assert_eq!(call_result.is_error, Some(false));
        }
    }

    // ============================================================================
    // PARAMETER VALIDATION TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_execute_empty_message_validation_error() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail validation
    }

    #[tokio::test]
    async fn test_execute_whitespace_only_message_validation_error() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("   \n\t   ".to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err()); // Should fail validation
    }

    #[tokio::test]
    async fn test_execute_missing_message_field_error() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "level".to_string(),
            serde_json::Value::String("info".to_string()),
        );
        // Missing required message field

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_argument_types() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        // Test invalid message type
        let mut arguments = serde_json::Map::new();
        arguments.insert("message".to_string(), serde_json::Value::Number(42.into()));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_execute_invalid_level_type() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Valid message".to_string()),
        );
        arguments.insert("level".to_string(), serde_json::Value::Number(42.into()));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_err());
    }

    // ============================================================================
    // CONTEXT HANDLING TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_execute_complex_context_data() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Complex context test".to_string()),
        );
        arguments.insert(
            "context".to_string(),
            json!({
                "nested": {
                    "data": "value",
                    "numbers": [1, 2, 3],
                    "boolean": true
                },
                "array": ["a", "b", "c"],
                "unicode": "通知消息 🔔"
            }),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_empty_context() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String("Message with empty context".to_string()),
        );
        arguments.insert("context".to_string(), json!({}));

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    // ============================================================================
    // EDGE CASE AND UNICODE TESTS
    // ============================================================================

    #[tokio::test]
    async fn test_execute_unicode_message() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let unicode_message = "通知消息 🔔 with émojis and ñoñ-ASCII characters";
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String(unicode_message.to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
        assert!(call_result.content[0]
            .as_text()
            .unwrap()
            .text
            .contains(unicode_message));
    }

    #[tokio::test]
    async fn test_execute_long_message() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let long_message = "x".repeat(10000);
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String(long_message.clone()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_special_characters_in_message() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let special_message = r#"Special chars: {}[]()\"'`~!@#$%^&*-_+=|\\/:;<>,.?"#;
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String(special_message.to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }

    #[tokio::test]
    async fn test_execute_multiline_message() {
        let tool = NotifyTool::new();
        let context = create_test_context().await;

        let multiline_message = "Line 1\nLine 2\nLine 3\n\nLine 5";
        let mut arguments = serde_json::Map::new();
        arguments.insert(
            "message".to_string(),
            serde_json::Value::String(multiline_message.to_string()),
        );

        let result = tool.execute(arguments, &context).await;
        assert!(result.is_ok());

        let call_result = result.unwrap();
        assert_eq!(call_result.is_error, Some(false));
    }
}
