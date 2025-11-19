//! Utility functions for MCP operations

use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;

/// Convert a JSON map to a string map for template arguments
pub fn convert_prompt_arguments(arguments: &HashMap<String, Value>) -> HashMap<String, String> {
    arguments
        .iter()
        .map(|(k, v)| {
            let value_str = match v {
                Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            (k.clone(), value_str)
        })
        .collect()
}

/// Convert a JSON map to a string map
pub fn json_map_to_string_map(
    json_map: &serde_json::Map<String, Value>,
) -> HashMap<String, String> {
    json_map
        .iter()
        .map(|(k, v)| {
            let value_str = match v {
                Value::String(s) => s.clone(),
                _ => v.to_string(),
            };
            (k.clone(), value_str)
        })
        .collect()
}

/// Generate a JSON schema for a type that implements JsonSchema
pub fn generate_tool_schema<T>() -> Arc<serde_json::Map<String, Value>>
where
    T: schemars::JsonSchema,
{
    serde_json::to_value(schemars::schema_for!(T))
        .ok()
        .and_then(|v| v.as_object().map(|obj| Arc::new(obj.clone())))
        .unwrap_or_else(|| Arc::new(serde_json::Map::new()))
}
