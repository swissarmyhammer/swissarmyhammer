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
    /// Name of the variable to retrieve (alias: key)
    #[serde(alias = "key")]
    pub name: Option<String>,
    /// Key name (alias for name)
    #[serde(skip)]
    pub key: Option<String>,
}

impl CelGetRequest {
    /// Get the variable name, checking both name and key fields
    pub fn get_name(&self) -> Result<String, String> {
        self.name
            .clone()
            .or_else(|| self.key.clone())
            .ok_or_else(|| "Either 'name' or 'key' parameter is required".to_string())
    }
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
                "name": {
                    "type": "string",
                    "description": "Name of the variable to retrieve (alias: key)"
                },
                "key": {
                    "type": "string",
                    "description": "Alias for 'name' - name of the variable to retrieve"
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
        let request: CelGetRequest = BaseToolImpl::parse_arguments(arguments)?;

        // Get variable name using alias support
        let name = request
            .get_name()
            .map_err(|e| McpError::invalid_params(e, None))?;

        tracing::debug!("CEL get: name='{}'", name);

        // Validate input
        McpValidation::validate_not_empty(&name, "variable name")
            .map_err(|e| McpErrorHandler::handle_error(e, "validate variable name"))?;

        // Use the global CEL state to retrieve the variable value
        let state = super::CelState::global();
        let result = state
            .get(&name)
            .map_err(|e| McpError::internal_error(e, None))?;

        // Convert CEL value to JSON for response
        let json_result = super::cel_value_to_json(&result);

        tracing::info!("CEL get '{}' = {:?}", name, json_result);

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
        assert!(properties.contains_key("name"));
        assert!(properties.contains_key("key"));

        // Schema should NOT contain oneOf (not supported by Claude API)
        assert!(!obj.contains_key("oneOf"));
    }

    #[test]
    fn test_parse_valid_arguments() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("my_var".to_string()),
        );

        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "my_var");
    }

    #[test]
    fn test_parse_with_key_alias() {
        let mut args = serde_json::Map::new();
        args.insert(
            "key".to_string(),
            serde_json::Value::String("another_var".to_string()),
        );

        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "another_var");
    }

    #[test]
    fn test_parse_missing_name() {
        let args = serde_json::Map::new();
        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert!(request.get_name().is_err());
    }

    #[test]
    fn test_parse_empty_name() {
        let mut args = serde_json::Map::new();
        args.insert(
            "name".to_string(),
            serde_json::Value::String("".to_string()),
        );

        let request: CelGetRequest = BaseToolImpl::parse_arguments(args).unwrap();
        assert_eq!(request.get_name().unwrap(), "");
    }
}
