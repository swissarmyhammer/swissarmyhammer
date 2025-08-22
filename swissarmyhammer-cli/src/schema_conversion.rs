//! Schema Conversion Module
//!
//! Provides bidirectional conversion between JSON schemas and Clap argument matching:
//! - JSON Schema → Clap Args (for dynamic CLI generation)
//! - Clap ArgMatches → JSON Arguments (for MCP tool execution)
//!
//! This module enables the dynamic CLI builder infrastructure by handling the
//! reverse conversion from parsed CLI arguments back to the JSON format expected
//! by MCP tools.

use clap::ArgMatches;
use serde_json::{Map, Value};

use thiserror::Error;

/// Errors that can occur during schema conversion
#[derive(Debug, Error)]
pub enum ConversionError {
    #[error("Missing required argument: {0}")]
    MissingRequired(String),

    #[error("Invalid argument type for {name}: expected {expected}, got {actual}")]
    InvalidType {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("Schema validation failed: {0}")]
    SchemaValidation(String),

    #[error("Failed to parse {field} as {data_type}: {message}")]
    ParseError {
        field: String,
        data_type: String,
        message: String,
    },

    #[error("Unsupported schema type: {schema_type}")]
    UnsupportedSchemaType { schema_type: String },
}

/// Schema converter for bidirectional JSON Schema ↔ Clap conversion
#[derive(Default)]
pub struct SchemaConverter;

impl SchemaConverter {
    /// Convert Clap ArgMatches back to JSON arguments using a JSON schema
    ///
    /// Takes parsed command line arguments and converts them back to the JSON
    /// format expected by MCP tools. Uses the schema to understand expected
    /// types and validate required fields.
    ///
    /// # Arguments
    /// * `matches` - Parsed command line arguments from Clap
    /// * `schema` - JSON Schema defining expected argument structure
    ///
    /// # Returns
    /// Map of argument names to JSON values, ready for MCP tool execution
    ///
    /// # Errors
    /// Returns `ConversionError` for missing required fields, type mismatches,
    /// or schema validation failures
    pub fn matches_to_json_args(
        matches: &ArgMatches,
        schema: &Value,
    ) -> Result<Map<String, Value>, ConversionError> {
        let mut args = Map::new();

        // Extract properties from schema
        let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
            return Err(ConversionError::SchemaValidation(
                "Schema missing 'properties' object".to_string(),
            ));
        };

        // Extract required fields list
        let required_fields: Vec<&str> = schema
            .get("required")
            .and_then(|r| r.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_default();

        // Process each property in the schema
        for (prop_name, prop_schema) in properties {
            let is_required = required_fields.contains(&prop_name.as_str());

            match Self::extract_clap_value(matches, prop_name, prop_schema)? {
                Some(value) => {
                    args.insert(prop_name.clone(), value);
                }
                None if is_required => {
                    return Err(ConversionError::MissingRequired(prop_name.clone()));
                }
                None => {
                    // Optional field not provided - don't include in args
                }
            }
        }

        Ok(args)
    }

    /// Extract a single value from Clap matches based on schema type
    ///
    /// Handles type-specific extraction and conversion from Clap's parsed
    /// arguments to JSON values based on the property's schema definition.
    ///
    /// # Arguments
    /// * `matches` - Parsed command line arguments
    /// * `prop_name` - Name of the property to extract
    /// * `prop_schema` - JSON Schema for this specific property
    ///
    /// # Returns
    /// * `Ok(Some(Value))` - Successfully extracted and converted value
    /// * `Ok(None)` - Property not present in arguments (optional field)
    /// * `Err(ConversionError)` - Type conversion or parsing error
    fn extract_clap_value(
        matches: &ArgMatches,
        prop_name: &str,
        prop_schema: &Value,
    ) -> Result<Option<Value>, ConversionError> {
        let schema_type = prop_schema
            .get("type")
            .and_then(|t| t.as_str())
            .unwrap_or("string"); // Default to string if type not specified

        match schema_type {
            "boolean" => Self::extract_boolean(matches, prop_name, prop_schema),
            "string" => Self::extract_string(matches, prop_name),
            "integer" => Self::extract_integer(matches, prop_name),
            "number" => Self::extract_number(matches, prop_name),
            "array" => Self::extract_array(matches, prop_name, prop_schema),
            unsupported => Err(ConversionError::UnsupportedSchemaType {
                schema_type: unsupported.to_string(),
            }),
        }
    }

    /// Extract boolean value from flag presence
    fn extract_boolean(
        matches: &ArgMatches,
        prop_name: &str,
        _prop_schema: &Value,
    ) -> Result<Option<Value>, ConversionError> {
        let flag_value = matches.get_flag(prop_name);

        if flag_value {
            // Flag was provided and is true, always include it
            Ok(Some(Value::Bool(true)))
        } else {
            // Flag was not provided, let MCP tool handle the default or absence
            Ok(None)
        }
    }

    /// Extract string value
    fn extract_string(
        matches: &ArgMatches,
        prop_name: &str,
    ) -> Result<Option<Value>, ConversionError> {
        if let Some(value) = matches.get_one::<String>(prop_name) {
            Ok(Some(Value::String(value.clone())))
        } else {
            Ok(None)
        }
    }

    /// Extract integer value with parsing
    fn extract_integer(
        matches: &ArgMatches,
        prop_name: &str,
    ) -> Result<Option<Value>, ConversionError> {
        if let Some(value_str) = matches.get_one::<String>(prop_name) {
            match value_str.parse::<i64>() {
                Ok(parsed) => Ok(Some(Value::Number(parsed.into()))),
                Err(e) => Err(ConversionError::ParseError {
                    field: prop_name.to_string(),
                    data_type: "integer".to_string(),
                    message: e.to_string(),
                }),
            }
        } else {
            Ok(None)
        }
    }

    /// Extract number value (float) with parsing
    fn extract_number(
        matches: &ArgMatches,
        prop_name: &str,
    ) -> Result<Option<Value>, ConversionError> {
        if let Some(value_str) = matches.get_one::<String>(prop_name) {
            match value_str.parse::<f64>() {
                Ok(parsed) => {
                    let number = serde_json::Number::from_f64(parsed).ok_or_else(|| {
                        ConversionError::ParseError {
                            field: prop_name.to_string(),
                            data_type: "number".to_string(),
                            message: "Invalid floating point number".to_string(),
                        }
                    })?;
                    Ok(Some(Value::Number(number)))
                }
                Err(e) => Err(ConversionError::ParseError {
                    field: prop_name.to_string(),
                    data_type: "number".to_string(),
                    message: e.to_string(),
                }),
            }
        } else {
            Ok(None)
        }
    }

    /// Extract array value from multiple arguments
    fn extract_array(
        matches: &ArgMatches,
        prop_name: &str,
        _prop_schema: &Value,
    ) -> Result<Option<Value>, ConversionError> {
        if let Some(values) = matches.get_many::<String>(prop_name) {
            let json_values: Vec<Value> = values.map(|s| Value::String(s.clone())).collect();
            Ok(Some(Value::Array(json_values)))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Arg, ArgAction, ArgMatches, Command};
    use serde_json::json;

    fn create_test_matches(args: &[&str]) -> ArgMatches {
        Command::new("test")
            .arg(
                Arg::new("title")
                    .long("title")
                    .value_name("TITLE")
                    .help("Title of the item"),
            )
            .arg(
                Arg::new("content")
                    .long("content")
                    .value_name("CONTENT")
                    .help("Content of the item"),
            )
            .arg(
                Arg::new("count")
                    .long("count")
                    .value_name("COUNT")
                    .help("Number of items"),
            )
            .arg(
                Arg::new("enabled")
                    .long("enabled")
                    .action(ArgAction::SetTrue)
                    .help("Enable the feature"),
            )
            .arg(
                Arg::new("tags")
                    .long("tags")
                    .action(ArgAction::Append)
                    .help("Tags to apply"),
            )
            .try_get_matches_from(args)
            .unwrap()
    }

    #[test]
    fn test_extract_string_value() {
        let matches = create_test_matches(&["test", "--title", "Test Title"]);
        let result = SchemaConverter::extract_string(&matches, "title").unwrap();
        assert_eq!(result, Some(Value::String("Test Title".to_string())));
    }

    #[test]
    fn test_extract_missing_string() {
        let matches = create_test_matches(&["test"]);
        let result = SchemaConverter::extract_string(&matches, "title").unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_boolean_flag() {
        let matches = create_test_matches(&["test", "--enabled"]);
        let result =
            SchemaConverter::extract_boolean(&matches, "enabled", &json!({"type": "boolean"}))
                .unwrap();
        assert_eq!(result, Some(Value::Bool(true)));
    }

    #[test]
    fn test_extract_missing_boolean() {
        let matches = create_test_matches(&["test"]);
        let result =
            SchemaConverter::extract_boolean(&matches, "enabled", &json!({"type": "boolean"}))
                .unwrap();
        assert_eq!(result, None);
    }

    #[test]
    fn test_extract_integer_value() {
        let matches = create_test_matches(&["test", "--count", "42"]);
        let result = SchemaConverter::extract_integer(&matches, "count").unwrap();
        assert_eq!(result, Some(Value::Number(42.into())));
    }

    #[test]
    fn test_extract_invalid_integer() {
        let matches = create_test_matches(&["test", "--count", "not-a-number"]);
        let result = SchemaConverter::extract_integer(&matches, "count");
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::ParseError { .. }
        ));
    }

    #[test]
    fn test_extract_array_values() {
        let matches = create_test_matches(&["test", "--tags", "rust", "--tags", "cli"]);
        let result = SchemaConverter::extract_array(&matches, "tags", &json!({})).unwrap();
        assert_eq!(
            result,
            Some(Value::Array(vec![
                Value::String("rust".to_string()),
                Value::String("cli".to_string())
            ]))
        );
    }

    #[test]
    fn test_matches_to_json_args_complete() {
        let matches = create_test_matches(&[
            "test",
            "--title",
            "Test Title",
            "--content",
            "Test Content",
            "--enabled",
        ]);

        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title"},
                "content": {"type": "string", "description": "Content"},
                "enabled": {"type": "boolean", "description": "Enable flag"}
            },
            "required": ["title"]
        });

        let result = SchemaConverter::matches_to_json_args(&matches, &schema).unwrap();

        assert_eq!(
            result.get("title"),
            Some(&Value::String("Test Title".to_string()))
        );
        assert_eq!(
            result.get("content"),
            Some(&Value::String("Test Content".to_string()))
        );
        assert_eq!(result.get("enabled"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_matches_to_json_args_missing_required() {
        let matches = create_test_matches(&["test", "--content", "Test Content"]);

        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title"},
                "content": {"type": "string", "description": "Content"}
            },
            "required": ["title"]
        });

        let result = SchemaConverter::matches_to_json_args(&matches, &schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::MissingRequired(_)
        ));
    }

    #[test]
    fn test_matches_to_json_args_optional_missing() {
        let matches = create_test_matches(&["test", "--title", "Test Title"]);

        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title"},
                "content": {"type": "string", "description": "Content"}
            },
            "required": ["title"]
        });

        let result = SchemaConverter::matches_to_json_args(&matches, &schema).unwrap();

        assert_eq!(
            result.get("title"),
            Some(&Value::String("Test Title".to_string()))
        );
        assert_eq!(result.get("content"), None); // Optional field not included
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_unsupported_schema_type() {
        let matches = create_test_matches(&["test", "--title", "Test"]);

        let result =
            SchemaConverter::extract_clap_value(&matches, "title", &json!({"type": "object"}));

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::UnsupportedSchemaType { .. }
        ));
    }

    #[test]
    fn test_invalid_schema_structure() {
        let matches = create_test_matches(&["test"]);

        let invalid_schema = json!({
            "type": "object"
            // Missing properties
        });

        let result = SchemaConverter::matches_to_json_args(&matches, &invalid_schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConversionError::SchemaValidation(_)
        ));
    }
}
