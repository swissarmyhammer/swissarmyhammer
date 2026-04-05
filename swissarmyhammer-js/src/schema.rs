//! JS-specific MCP schema generation
//!
//! Provides configuration for MCP schema generation tailored to JS expression operations.

use serde_json::{json, Value};
use swissarmyhammer_operations::{generate_mcp_schema, Operation, SchemaConfig};

/// Generate MCP schema for JS expression operations
pub fn generate_js_mcp_schema(operations: &[&dyn Operation]) -> Value {
    let config = SchemaConfig::new(
        "JavaScript expression evaluation. Set and get variables using JS expressions.",
    )
    .with_examples(generate_js_examples());

    generate_mcp_schema(operations, config)
}

/// Generate JS-specific usage examples
fn generate_js_examples() -> Vec<Value> {
    vec![
        json!({
            "description": "Set a variable with arithmetic",
            "value": {"op": "set expression", "name": "x", "expression": "10 + 5"}
        }),
        json!({
            "description": "Set a boolean variable",
            "value": {"op": "set expression", "name": "is_ready", "expression": "true"}
        }),
        json!({
            "description": "Set using key/value aliases",
            "value": {"op": "set expression", "key": "counter", "value": "42"}
        }),
        json!({
            "description": "Get a variable",
            "value": {"op": "get expression", "name": "x"}
        }),
        json!({
            "description": "Get with expression",
            "value": {"op": "get expression", "name": "x * 2 + 1"}
        }),
        json!({
            "description": "Set an object",
            "value": {"op": "set expression", "name": "config", "expression": "({retries: 3, timeout: 30})"}
        }),
        json!({
            "description": "Set an array",
            "value": {"op": "set expression", "name": "items", "expression": "[1, 2, 3]"}
        }),
        json!({
            "description": "String operations",
            "value": {"op": "set expression", "name": "greeting", "expression": "'Hello' + ' ' + 'World'"}
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expression::get::GetExpression;
    use crate::expression::set::SetExpression;

    #[test]
    fn test_generate_js_mcp_schema_returns_object() {
        let set_op = SetExpression {
            name: None,
            expression: None,
        };
        let get_op = GetExpression { name: None };
        let ops: Vec<&dyn Operation> = vec![&set_op, &get_op];

        let schema = generate_js_mcp_schema(&ops);
        assert!(schema.is_object());
    }

    #[test]
    fn test_generate_js_mcp_schema_has_description() {
        let set_op = SetExpression {
            name: None,
            expression: None,
        };
        let ops: Vec<&dyn Operation> = vec![&set_op];

        let schema = generate_js_mcp_schema(&ops);
        let desc = schema["description"].as_str().unwrap();
        assert!(desc.contains("JavaScript"));
    }

    #[test]
    fn test_generate_js_mcp_schema_has_examples() {
        let set_op = SetExpression {
            name: None,
            expression: None,
        };
        let ops: Vec<&dyn Operation> = vec![&set_op];

        let schema = generate_js_mcp_schema(&ops);
        let examples = &schema["examples"];
        assert!(examples.is_array());
        let examples_arr = examples.as_array().unwrap();
        assert!(!examples_arr.is_empty());
    }

    #[test]
    fn test_generate_js_mcp_schema_examples_have_descriptions() {
        let get_op = GetExpression { name: None };
        let ops: Vec<&dyn Operation> = vec![&get_op];

        let schema = generate_js_mcp_schema(&ops);
        if let Some(examples) = schema["examples"].as_array() {
            for example in examples {
                assert!(
                    example["description"].is_string(),
                    "Example missing description: {:?}",
                    example
                );
            }
        }
    }

    #[test]
    fn test_generate_js_mcp_schema_empty_ops() {
        let ops: Vec<&dyn Operation> = vec![];
        let schema = generate_js_mcp_schema(&ops);
        assert!(schema.is_object());
    }

    #[test]
    fn test_generate_js_mcp_schema_examples_include_set_expression() {
        let set_op = SetExpression {
            name: None,
            expression: None,
        };
        let ops: Vec<&dyn Operation> = vec![&set_op];

        let schema = generate_js_mcp_schema(&ops);
        let schema_str = serde_json::to_string(&schema).unwrap();
        assert!(schema_str.contains("set expression"));
    }

    #[test]
    fn test_generate_js_mcp_schema_examples_include_get_expression() {
        let get_op = GetExpression { name: None };
        let ops: Vec<&dyn Operation> = vec![&get_op];

        let schema = generate_js_mcp_schema(&ops);
        let schema_str = serde_json::to_string(&schema).unwrap();
        assert!(schema_str.contains("get expression"));
    }
}
