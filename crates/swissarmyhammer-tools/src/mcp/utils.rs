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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_convert_prompt_arguments_strings_passed_through() {
        let mut args = HashMap::new();
        args.insert("key".to_string(), Value::String("hello".to_string()));

        let result = convert_prompt_arguments(&args);
        assert_eq!(result.get("key"), Some(&"hello".to_string()));
    }

    #[test]
    fn test_convert_prompt_arguments_non_string_serialized() {
        let mut args = HashMap::new();
        args.insert("num".to_string(), Value::Number(42.into()));
        args.insert("flag".to_string(), Value::Bool(true));
        args.insert("nothing".to_string(), Value::Null);

        let result = convert_prompt_arguments(&args);
        assert_eq!(result.get("num"), Some(&"42".to_string()));
        assert_eq!(result.get("flag"), Some(&"true".to_string()));
        assert_eq!(result.get("nothing"), Some(&"null".to_string()));
    }

    #[test]
    fn test_convert_prompt_arguments_empty() {
        let args = HashMap::new();
        let result = convert_prompt_arguments(&args);
        assert!(result.is_empty());
    }

    #[test]
    fn test_json_map_to_string_map_strings_passed_through() {
        let mut map = serde_json::Map::new();
        map.insert("name".to_string(), Value::String("Alice".to_string()));

        let result = json_map_to_string_map(&map);
        assert_eq!(result.get("name"), Some(&"Alice".to_string()));
    }

    #[test]
    fn test_json_map_to_string_map_non_string_serialized() {
        let mut map = serde_json::Map::new();
        map.insert("count".to_string(), Value::Number(7.into()));
        map.insert("active".to_string(), Value::Bool(false));

        let result = json_map_to_string_map(&map);
        assert_eq!(result.get("count"), Some(&"7".to_string()));
        assert_eq!(result.get("active"), Some(&"false".to_string()));
    }

    #[test]
    fn test_json_map_to_string_map_empty() {
        let map = serde_json::Map::new();
        let result = json_map_to_string_map(&map);
        assert!(result.is_empty());
    }

    #[test]
    fn test_generate_tool_schema_returns_non_empty_for_schema_type() {
        use schemars::JsonSchema;
        use serde::Deserialize;

        #[derive(JsonSchema, Deserialize)]
        #[allow(dead_code)]
        struct TestParams {
            /// The name parameter
            name: String,
            /// An optional count
            count: Option<i32>,
        }

        let schema = generate_tool_schema::<TestParams>();
        assert!(
            !schema.is_empty(),
            "schema should not be empty for a typed struct"
        );
    }
}
