//! CEL get tool for evaluating expressions in the current context
//!
//! This tool evaluates a CEL expression in the current global context and returns
//! the result without storing it. Useful for querying computed values.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;

/// Request structure for cel_get
#[derive(Debug, Deserialize)]
pub struct CelGetRequest {
    /// CEL expression to evaluate
    pub expression: String,
}

/// Tool for evaluating CEL expressions and returning results
#[derive(Default)]
pub struct CelGetTool;

impl CelGetTool {
    /// Creates a new instance of CelGetTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for CelGetTool {
    fn name(&self) -> &'static str {
        "cel_get"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "expression": {
                    "type": "string",
                    "description": "CEL expression to evaluate"
                }
            },
            "required": ["expression"]
        })
    }

    fn hidden_from_cli(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, serde_json::Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let request: CelGetRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!("CEL get: expression='{}'", request.expression);

        // Validate input
        McpValidation::validate_not_empty(&request.expression, "expression")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate expression"))?;

        // Use the global CEL state to evaluate the expression
        let state = super::CelState::global();
        let result = state
            .get(&request.expression)
            .map_err(|e| McpError::internal_error(e, None))?;

        // Convert CEL value to JSON for response
        let json_result = super::cel_value_to_json(&result);

        tracing::info!("CEL get '{}' = {:?}", request.expression, json_result);

        Ok(BaseToolImpl::create_success_response(
            serde_json::json!({
                "result": json_result
            })
            .to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tool_registry::BaseToolImpl;

    #[test]
    fn test_cel_get_tool_name() {
        let tool = CelGetTool::new();
        assert_eq!(tool.name(), "cel_get");
    }

    #[test]
    fn test_cel_get_tool_description() {
        let tool = CelGetTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("CEL"));
    }

    #[test]
    fn test_cel_get_tool_schema() {
        let tool = CelGetTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("expression"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("expression".to_string())));
    }

    #[test]
    fn test_parse_valid_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("2 + 2".to_string()),
        );

        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.expression, "2 + 2");
    }

    #[test]
    fn test_parse_missing_expression() {
        let args = serde_json::Map::new();
        let result: Result<CelGetRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_expression() {
        let mut args = serde_json::Map::new();
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.expression, "");
    }
}
