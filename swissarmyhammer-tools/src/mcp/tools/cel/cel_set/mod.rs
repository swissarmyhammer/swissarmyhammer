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
    /// Name of the variable to store the result (alias: key)
    #[serde(alias = "key")]
    pub name: Option<String>,
    /// Key name (alias for name)
    #[serde(skip)]
    pub key: Option<String>,
    /// CEL expression to evaluate (alias: value) - can be a string expression or any JSON value
    #[serde(alias = "value")]
    pub expression: Option<serde_json::Value>,
    /// Value expression (alias for expression)
    #[serde(skip)]
    pub value: Option<serde_json::Value>,
}

impl CelSetRequest {
    /// Get the variable name, checking both name and key fields
    pub fn get_name(&self) -> Result<String, String> {
        self.name
            .clone()
            .or_else(|| self.key.clone())
            .ok_or_else(|| "Either 'name' or 'key' parameter is required".to_string())
    }

    /// Get the expression, checking both expression and value fields
    /// Converts JSON values to CEL literal expressions
    pub fn get_expression(&self) -> Result<String, String> {
        let value = self
            .expression
            .clone()
            .or_else(|| self.value.clone())
            .ok_or_else(|| "Either 'expression' or 'value' parameter is required".to_string())?;

        // Convert JSON value to CEL expression
        Ok(match value {
            serde_json::Value::String(s) => s,
            serde_json::Value::Bool(b) => b.to_string(),
            serde_json::Value::Number(n) => n.to_string(),
            serde_json::Value::Null => "null".to_string(),
            serde_json::Value::Array(_) | serde_json::Value::Object(_) => {
                // For arrays and objects, serialize to JSON string and treat as a CEL expression
                serde_json::to_string(&value)
                    .map_err(|e| format!("Failed to serialize value to JSON: {}", e))?
            }
        })
    }
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

// No health checks needed for CEL tools
crate::impl_empty_doctorable!(CelSetTool);

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
                    "description": "Name of the variable to store the result (alias: key)"
                },
                "key": {
                    "type": "string",
                    "description": "Alias for 'name' - name of the variable to store the result"
                },
                "expression": {
                    "type": ["string", "boolean", "number", "object", "array", "null"],
                    "description": "CEL expression to evaluate or literal value (string, boolean, number, null, array, object) (alias: value)"
                },
                "value": {
                    "type": ["string", "boolean", "number", "object", "array", "null"],
                    "description": "Alias for 'expression' - CEL expression to evaluate or literal value (string, boolean, number, null, array, object)"
                }
            }
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

        // Get name and expression using alias support
        let name = request
            .get_name()
            .map_err(|e| McpError::invalid_params(e, None))?;
        let expression = request
            .get_expression()
            .map_err(|e| McpError::invalid_params(e, None))?;

        tracing::debug!("CEL set: name='{}', expression='{}'", name, expression);

        // Validate inputs
        McpValidation::validate_not_empty(&name, "variable name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate variable name"))?;
        McpValidation::validate_not_empty(&expression, "expression")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate expression"))?;

        // Use the global CEL state to set the variable
        let state = super::CelState::global();
        let result = state
            .set(&name, &expression)
            .map_err(|e| McpError::internal_error(e, None))?;

        // Convert CEL value to JSON for response
        let json_result = super::cel_value_to_json(&result);

        tracing::info!("CEL set '{}' = {:?}", name, json_result);

        Ok(BaseToolImpl::create_success_response(
            json_result.to_string(),
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
        assert!(properties.contains_key("key"));
        assert!(properties.contains_key("expression"));
        assert!(properties.contains_key("value"));

        // Schema should NOT contain oneOf (not supported by Claude API)
        assert!(!obj.contains_key("oneOf"));
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
        assert_eq!(request.get_name().unwrap(), "x");
        assert_eq!(request.get_expression().unwrap(), "10 + 5");
    }

    #[test]
    fn test_parse_boolean_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("flag".to_string()),
        );
        args.insert("value".to_string(), serde_json::Value::Bool(false));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "flag");
        assert_eq!(request.get_expression().unwrap(), "false");
    }

    #[test]
    fn test_parse_number_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("count".to_string()),
        );
        args.insert("value".to_string(), serde_json::json!(42));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "count");
        assert_eq!(request.get_expression().unwrap(), "42");
    }

    #[test]
    fn test_parse_boolean_true_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("is_enabled".to_string()),
        );
        args.insert("expression".to_string(), serde_json::Value::Bool(true));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "is_enabled");
        assert_eq!(request.get_expression().unwrap(), "true");
    }

    #[test]
    fn test_parse_float_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("pi".to_string()),
        );
        args.insert("value".to_string(), serde_json::json!(1.23456));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "pi");
        assert_eq!(request.get_expression().unwrap(), "1.23456");
    }

    #[test]
    fn test_parse_null_string() {
        // Test that passing the string "null" works as a CEL expression
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("empty".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::Value::String("null".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "empty");
        assert_eq!(request.get_expression().unwrap(), "null");
    }

    #[test]
    fn test_parse_array_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("items".to_string()),
        );
        args.insert("value".to_string(), serde_json::json!([1, 2, 3, 4, 5]));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "items");
        assert_eq!(request.get_expression().unwrap(), "[1,2,3,4,5]");
    }

    #[test]
    fn test_parse_object_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("config".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::json!({"name": "test", "enabled": true, "count": 42}),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "config");
        let expr = request.get_expression().unwrap();
        // JSON serialization may vary in order, so just check it's valid JSON
        let parsed: serde_json::Value = serde_json::from_str(&expr).unwrap();
        assert_eq!(parsed["name"], "test");
        assert_eq!(parsed["enabled"], true);
        assert_eq!(parsed["count"], 42);
    }

    #[test]
    fn test_parse_negative_number() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("temperature".to_string()),
        );
        args.insert("value".to_string(), serde_json::json!(-10));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "temperature");
        assert_eq!(request.get_expression().unwrap(), "-10");
    }

    #[test]
    fn test_parse_zero() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("zero".to_string()),
        );
        args.insert("value".to_string(), serde_json::json!(0));

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "zero");
        assert_eq!(request.get_expression().unwrap(), "0");
    }

    #[test]
    fn test_string_expression_still_works() {
        // Ensure string values are still treated as CEL expressions
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("sum".to_string()),
        );
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("10 + 20 * 2".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "sum");
        assert_eq!(request.get_expression().unwrap(), "10 + 20 * 2");
    }

    #[test]
    fn test_nested_array_value() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("matrix".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::json!([[1, 2], [3, 4], [5, 6]]),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "matrix");
        assert_eq!(request.get_expression().unwrap(), "[[1,2],[3,4],[5,6]]");
    }

    #[test]
    fn test_mixed_type_array() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("mixed".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::json!([1, "text", true, null, 2.72]),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "mixed");
        assert_eq!(
            request.get_expression().unwrap(),
            "[1,\"text\",true,null,2.72]"
        );
    }

    #[test]
    fn test_parse_with_key_alias() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("y".to_string()),
        );
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("20 + 10".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "y");
        assert_eq!(request.get_expression().unwrap(), "20 + 10");
    }

    #[test]
    fn test_parse_with_value_alias() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("z".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::Value::String("30 + 15".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "z");
        assert_eq!(request.get_expression().unwrap(), "30 + 15");
    }

    #[test]
    fn test_parse_with_both_aliases() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("w".to_string()),
        );
        args.insert(
            "value".to_string(),
            serde_json::Value::String("40 + 20".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "w");
        assert_eq!(request.get_expression().unwrap(), "40 + 20");
    }

    #[test]
    fn test_parse_missing_name() {
        let mut args = serde_json::Map::new();
        args.insert(
            "expression".to_string(),
            serde_json::Value::String("10 + 5".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert!(request.get_name().is_err());
    }

    #[test]
    fn test_parse_missing_expression() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("x".to_string()),
        );

        let request: CelSetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert!(request.get_expression().is_err());
    }
}
