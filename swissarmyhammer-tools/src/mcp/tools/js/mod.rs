//! JavaScript expression evaluation tool
//!
//! This module provides a single MCP tool for JavaScript expression operations.
//! It follows the same multi-operation tool pattern as the kanban tool.

use crate::mcp::tool_registry::{BaseToolImpl, McpTool, ToolContext, ToolRegistry};
use async_trait::async_trait;
use once_cell::sync::Lazy;
use rmcp::model::CallToolResult;
use rmcp::ErrorData as McpError;
use serde_json::Value;
use swissarmyhammer_js::{
    expression::{GetExpression, SetExpression},
    JsContext, JsOperationProcessor, Operation, OperationProcessor,
};

// Static operation instances for metadata access
static SET_EXPRESSION: Lazy<SetExpression> = Lazy::new(|| SetExpression {
    name: None,
    expression: None,
});
static GET_EXPRESSION: Lazy<GetExpression> = Lazy::new(|| GetExpression { name: None });

/// All JS operations for schema generation
static JS_OPERATIONS: Lazy<Vec<&'static dyn Operation>> = Lazy::new(|| {
    vec![
        &*SET_EXPRESSION as &dyn Operation,
        &*GET_EXPRESSION as &dyn Operation,
    ]
});

/// MCP tool for JavaScript expression operations
#[derive(Default)]
pub struct JsTool;

impl JsTool {
    /// Creates a new instance of JsTool
    pub fn new() -> Self {
        Self
    }
}

// No health checks needed
crate::impl_empty_doctorable!(JsTool);

#[async_trait]
impl McpTool for JsTool {
    fn name(&self) -> &'static str {
        "js"
    }

    fn description(&self) -> &'static str {
        include_str!("description.md")
    }

    fn schema(&self) -> Value {
        swissarmyhammer_js::schema::generate_js_mcp_schema(&JS_OPERATIONS)
    }

    fn operations(&self) -> &'static [&'static dyn swissarmyhammer_operations::Operation] {
        let ops: &[&'static dyn Operation] = &JS_OPERATIONS;
        // SAFETY: JS_OPERATIONS is a static Lazy<Vec<...>> that lives for 'static
        unsafe {
            std::mem::transmute::<
                &[&dyn Operation],
                &'static [&'static dyn swissarmyhammer_operations::Operation],
            >(ops)
        }
    }

    fn hidden_from_cli(&self) -> bool {
        false
    }

    async fn execute(
        &self,
        arguments: serde_json::Map<String, Value>,
        _context: &ToolContext,
    ) -> std::result::Result<CallToolResult, McpError> {
        let ctx = JsContext::new();
        let processor = JsOperationProcessor::new();

        // Determine operation from the "op" field
        let op_str = arguments
            .get("op")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_lowercase();

        let result = match op_str.as_str() {
            "set expression" | "set" => {
                // Parse set expression parameters
                let name = arguments
                    .get("name")
                    .or_else(|| arguments.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing required field: 'name' or 'key'", None)
                    })?;

                let expression = arguments
                    .get("expression")
                    .or_else(|| arguments.get("value"))
                    .ok_or_else(|| {
                        McpError::invalid_params(
                            "Missing required field: 'expression' or 'value'",
                            None,
                        )
                    })?;

                // Convert JSON value to expression string
                let expr_str = match expression {
                    Value::String(s) => s.clone(),
                    Value::Bool(b) => b.to_string(),
                    Value::Number(n) => n.to_string(),
                    Value::Null => "null".to_string(),
                    Value::Array(_) | Value::Object(_) => {
                        format!(
                            "({})",
                            serde_json::to_string(expression)
                                .map_err(|e| McpError::internal_error(e.to_string(), None))?
                        )
                    }
                };

                let cmd = SetExpression {
                    name: Some(name.to_string()),
                    expression: Some(Value::String(expr_str)),
                };
                processor.process(&cmd, &ctx).await
            }

            "get expression" | "get" => {
                let name = arguments
                    .get("name")
                    .or_else(|| arguments.get("key"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| {
                        McpError::invalid_params("Missing required field: 'name' or 'key'", None)
                    })?;

                let cmd = GetExpression {
                    name: Some(name.to_string()),
                };
                processor.process(&cmd, &ctx).await
            }

            _ => {
                return Err(McpError::invalid_params(
                    format!(
                        "Unknown operation: '{}'. Valid operations: 'set expression', 'get expression'",
                        op_str
                    ),
                    None,
                ));
            }
        };

        match result {
            Ok(value) => Ok(BaseToolImpl::create_success_response(
                serde_json::to_string_pretty(&value).unwrap_or_else(|_| value.to_string()),
            )),
            Err(e) => Ok(BaseToolImpl::create_success_response(format!(
                "Error: {}",
                e
            ))),
        }
    }
}

/// Register all JS tools with the tool registry
pub fn register_js_tools(registry: &mut ToolRegistry) {
    registry.register(JsTool);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_js_tool_name() {
        let tool = JsTool::new();
        assert_eq!(tool.name(), "js");
    }

    #[test]
    fn test_js_tool_schema_structure() {
        let tool = JsTool::new();
        let schema = tool.schema();

        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["op"].is_object());

        // No oneOf (Claude API restriction)
        assert!(!schema.as_object().unwrap().contains_key("oneOf"));
        assert!(!schema.as_object().unwrap().contains_key("allOf"));
        assert!(!schema.as_object().unwrap().contains_key("anyOf"));
    }

    #[test]
    fn test_js_tool_has_operations() {
        let tool = JsTool::new();
        let ops = tool.operations();
        assert_eq!(ops.len(), 2);
        assert!(ops.iter().any(|o| o.op_string() == "set expression"));
        assert!(ops.iter().any(|o| o.op_string() == "get expression"));
    }
}
