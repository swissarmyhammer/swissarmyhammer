//! Notification creation tool for MCP operations
//!
//! This module provides the NotifyTool for sending messages from LLMs to users through the
//! logging system. The tool enables LLMs to communicate important information, status updates,
//! and contextual feedback during workflow execution.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::Error as McpError;
use serde::Deserialize;
use serde_json::Value as JsonValue;

/// Request structure for creating a notification
#[derive(Debug, Deserialize)]
pub struct NotifyCreateRequest {
    /// The message to notify the user about (required)
    pub message: String,
    /// The notification level: "info", "warn", or "error" (default: "info")
    pub level: Option<String>,
    /// Optional structured JSON data for the notification
    pub context: Option<JsonValue>,
}

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

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Apply rate limiting for notifications
        context
            .rate_limiter
            .check_rate_limit("unknown", "notify_create", 1)
            .map_err(|e| {
                tracing::warn!("Rate limit exceeded for notification: {}", e);
                McpError::invalid_params(e.to_string(), None)
            })?;

        tracing::debug!("Creating notification: {}", request.message);

        // Validate message is not empty
        McpValidation::validate_not_empty(&request.message, "notification message")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate notification message"))?;

        // Get the notification level, defaulting to "info"
        let level = request.level.as_deref().unwrap_or("info");

        // Get the context, defaulting to empty object
        let notification_context = request.context.unwrap_or_default();

        // Send the notification through the tracing system with the "llm_notify" target
        match level {
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

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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
        let result: Result<NotifyCreateRequest, rmcp::Error> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_message() {
        let mut args = serde_json::Map::new();
        args.insert(
            "message".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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

        let result: Result<NotifyCreateRequest, rmcp::Error> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_message_type() {
        let mut args = serde_json::Map::new();
        args.insert("message".to_string(), serde_json::Value::Number(42.into()));

        let result: Result<NotifyCreateRequest, rmcp::Error> = BaseToolImpl::parse_arguments(args);
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

            let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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

        let request: NotifyCreateRequest = BaseToolImpl::parse_arguments(args).unwrap();
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
}
