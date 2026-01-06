//! CEL set tool for storing computed values as variables
//!
//! This tool evaluates a CEL expression in the current global context and stores
//! the result as a named variable that can be referenced in future expressions.

use crate::mcp::shared_utils::{McpErrorHandler, McpValidation};
use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext};
use async_trait::async_trait;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde::Deserialize;

/// Request structure for cel_set
#[derive(Debug, Deserialize)]
pub struct CelSetRequest {
    /// Name of the variable to store the result
    pub name: String,
    /// CEL expression to evaluate
    pub expression: String,
}

/// Tool for evaluating CEL expressions and storing results as variables
#[derive(Default)]
pub struct CelSetTool;

impl CelSetTool {
    /// Creates a new instance of CelSetTool
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl McpTool for CelSetTool {
    fn name(&self) -> &'static str {
        "cel_set"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Name of the variable to store the result"
                },
                "expression": {
                    "type": "string",
                    "description": "CEL expression to evaluate"
                }
            },
            "required": ["name", "expression"]
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
        let request: CelSetRequest = BaseToolImpl::parse_arguments(arguments)?;

        tracing::debug!(
            "CEL set: name='{}', expression='{}'",
            request.name,
            request.expression
        );

        // Validate inputs
        McpValidation::validate_not_empty(&request.name, "variable name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate variable name"))?;
        McpValidation::validate_not_empty(&request.expression, "expression")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate expression"))?;

        // Use the global CEL state to set the variable
        let state = super::CelState::global();
        let result = state
            .set(&request.name, &request.expression)
            .map_err(|e| McpError::internal_error(e, None))?;

        // Convert CEL value to JSON for response
        let json_result = super::cel_value_to_json(&result);

        tracing::info!("CEL set '{}' = {:?}", request.name, json_result);

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
    fn test_cel_set_tool_name() {
        let tool = CelSetTool::new();
        assert_eq!(tool.name(), "cel_set");
    }

    #[test]
    fn test_cel_set_tool_description() {
        let tool = CelSetTool::new();
        let description = tool.description();
        assert!(!description.is_empty());
        assert!(description.contains("CEL"));
    }

    #[test]
    fn test_cel_set_tool_schema() {
        let tool = CelSetTool::new();
        let schema = tool.schema();

        assert!(schema.is_object());
        let obj = schema.as_object().unwrap();
        assert!(obj.contains_key("properties"));

        let properties = obj["properties"].as_object().unwrap();
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("expression"));

        let required = obj["required"].as_array().unwrap();
        assert!(required.contains(&serde_json::Value::String("name".to_string())));
        assert!(required.contains(&serde_json::Value::String("expression".to_string())));
    }

    #[test]
    fn test_parse_valid_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("x".to_string()),
        );
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("10 + 5".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.name, "x");
        assert_eq!(request.expression, "10 + 5");
    }

    #[test]
    fn test_parse_missing_name() {
        let mut args = serde_json::Map::new();
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("10 + 5".to_string()),
        );

        let result: Result<CelSetRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_missing_expression() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("x".to_string()),
        );

        let result: Result<CelSetRequest, rmcp::ErrorData> = BaseToolImpl::parse_arguments(args);
        assert!(result.is_err());
    }
}
