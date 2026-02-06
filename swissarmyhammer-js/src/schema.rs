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
