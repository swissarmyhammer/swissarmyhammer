//! Schema Conversion Module
//!
//! Provides bidirectional conversion between JSON schemas and Clap argument matching:
//! - JSON Schema → Clap Args (for dynamic CLI generation)
//! - Clap ArgMatches → JSON Arguments (for MCP tool execution)
//!
//! This module enables the dynamic CLI builder infrastructure by handling the
//! reverse conversion from parsed CLI arguments back to the JSON format expected
//! by MCP tools.

use crate::schema_validation::{SchemaValidator, ValidationError};
use clap::ArgMatches;
use serde_json::{Map, Value};
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Errors that can occur during schema conversion
#[derive(Debug, Error)]
#[allow(dead_code)]
pub enum ConversionError {
    #[error("Missing required argument: {field}")]
    MissingRequired { field: String },

    #[error("Invalid argument type for {name}: expected {expected}, got {actual}")]
    InvalidType {
        name: String,
        expected: String,
        actual: String,
    },

    #[error("Schema validation failed: {message}")]
    SchemaValidation { message: String },

    #[error("Failed to parse {field} as {data_type}: {message}")]
    ParseError {
        field: String,
        data_type: String,
        message: String,
    },

    #[error("Unsupported schema type: {schema_type} for parameter {parameter}")]
    UnsupportedSchemaType {
        schema_type: String,
        parameter: String,
    },

    #[error("Schema validation error: {0}")]
    ValidationError(#[from] ValidationError),
}

impl Severity for ConversionError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ConversionError::MissingRequired { .. } => ErrorSeverity::Error,
            ConversionError::InvalidType { .. } => ErrorSeverity::Error,
            ConversionError::SchemaValidation { .. } => ErrorSeverity::Critical,
            ConversionError::ParseError { .. } => ErrorSeverity::Error,
            ConversionError::UnsupportedSchemaType { .. } => ErrorSeverity::Error,
            ConversionError::ValidationError(validation_err) => validation_err.severity(),
        }
    }
}

/// Schema converter for bidirectional JSON Schema ↔ Clap conversion
#[derive(Default)]
#[allow(dead_code)]
pub struct SchemaConverter;

#[allow(dead_code)]
impl SchemaConverter {
    /// Convert Clap ArgMatches back to JSON arguments using a JSON schema
    ///
    /// Takes parsed command line arguments and converts them back to the JSON
    /// format expected by MCP tools. Uses the schema to understand expected
    /// types and validate required fields.
    ///
    /// This method now includes comprehensive schema validation before conversion
    /// to ensure robust error handling and user-friendly error messages.
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
    /// schema validation failures, or unsupported schema types
    pub fn matches_to_json_args(
        matches: &ArgMatches,
        schema: &Value,
    ) -> Result<Map<String, Value>, ConversionError> {
        // First validate the schema comprehensively
        SchemaValidator::validate_schema(schema)?;
        let mut args = Map::new();

        // Extract properties from schema
        let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
            return Err(ConversionError::SchemaValidation {
                message: "Schema missing 'properties' object".to_string(),
            });
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
                    return Err(ConversionError::MissingRequired {
                        field: prop_name.clone(),
                    });
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
                parameter: prop_name.to_string(),
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

    /// Provide user-friendly conversion suggestions based on schema type
    pub fn provide_conversion_suggestions(schema_type: &str) -> String {
        match schema_type {
            "object" => {
                "Nested objects are not supported. Consider flattening the schema or using a string representation.".to_string()
            }
            "null" => {
                "Null type parameters are not supported in CLI. Consider making the parameter optional or using a different type.".to_string()
            }
            unknown => {
                format!(
                    "Unknown type '{}'. Supported types: string, boolean, integer, number, array.",
                    unknown
                )
            }
        }
    }

    /// Format a conversion error with helpful context and suggestions
    pub fn format_conversion_error(error: &ConversionError, tool_name: &str) -> String {
        match error {
            ConversionError::MissingRequired { field } => {
                format!(
                    "❌ Missing required argument '--{}' for tool '{}'.\n\n💡 Use '--help' to see all required arguments.\n🔄 To fix this interactively, run: sah <command> --interactive",
                    field, tool_name
                )
            }
            ConversionError::InvalidType {
                name,
                expected,
                actual,
            } => {
                format!(
                    "❌ Invalid type for argument '--{}' in tool '{}': expected {}, got {}.\n\n💡 Please check the argument format and try again.\n📖 Use '--help' to see expected argument types.",
                    name, tool_name, expected, actual
                )
            }
            ConversionError::ParseError {
                field,
                data_type,
                message,
            } => {
                let suggestion = match data_type.as_str() {
                    "integer" => "Use whole numbers only (e.g., 42, -17, 0)",
                    "number" => "Use numeric values (e.g., 3.14, -2.5, 100)",
                    _ => "Check the format of your input",
                };

                format!(
                    "❌ Failed to parse '--{}' as {} for tool '{}': {}.\n\n💡 {}\n📖 Use '--help' to see expected argument formats.",
                    field, data_type, tool_name, message, suggestion
                )
            }
            ConversionError::SchemaValidation { message } => {
                format!(
                    "❌ Schema validation failed for tool '{}': {}.\n\n💡 This is likely an internal tool configuration error. Please report this issue.\n🔧 Tool developers should verify their schema follows JSON Schema specification.",
                    tool_name, message
                )
            }
            ConversionError::UnsupportedSchemaType {
                schema_type,
                parameter,
            } => {
                let suggestion = Self::provide_conversion_suggestions(schema_type);
                format!(
                    "❌ Tool '{}' uses unsupported argument type '{}' for parameter '{}'.\n\n💡 {}\n🔧 This tool may not be compatible with CLI execution.",
                    tool_name, schema_type, parameter, suggestion
                )
            }
            ConversionError::ValidationError(validation_err) => {
                format!(
                    "❌ Schema validation error in tool '{}': {}\n\n💡 {}\n🔧 Tool developers should fix this schema definition.",
                    tool_name,
                    validation_err,
                    validation_err.suggestion().unwrap_or_default()
                )
            }
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
            ConversionError::MissingRequired { .. }
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
    fn test_format_conversion_error() {
        let error = ConversionError::MissingRequired {
            field: "title".to_string(),
        };

        let formatted = SchemaConverter::format_conversion_error(&error, "test_tool");
        assert!(formatted.contains("Missing required argument '--title'"));
        assert!(formatted.contains("💡"));
        assert!(formatted.contains("🔄"));
    }

    #[test]
    fn test_provide_conversion_suggestions() {
        let object_suggestion = SchemaConverter::provide_conversion_suggestions("object");
        assert!(object_suggestion.contains("Nested objects are not supported"));

        let null_suggestion = SchemaConverter::provide_conversion_suggestions("null");
        assert!(null_suggestion.contains("Null type parameters are not supported"));

        let unknown_suggestion = SchemaConverter::provide_conversion_suggestions("unknown_type");
        assert!(unknown_suggestion.contains("Unknown type"));
        assert!(unknown_suggestion.contains("Supported types:"));
    }

    #[test]
    fn test_format_parse_error() {
        let error = ConversionError::ParseError {
            field: "count".to_string(),
            data_type: "integer".to_string(),
            message: "invalid digit found in string".to_string(),
        };

        let formatted = SchemaConverter::format_conversion_error(&error, "test_tool");
        assert!(formatted.contains("Failed to parse '--count' as integer"));
        assert!(formatted.contains("Use whole numbers only"));
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
            ConversionError::ValidationError(_)
        ));
    }

    #[test]
    fn test_conversion_error_severity_levels() {
        use crate::schema_validation::ValidationError;

        // Test Error severity for MissingRequired
        let missing_required = ConversionError::MissingRequired {
            field: "test_field".to_string(),
        };
        assert_eq!(missing_required.severity(), ErrorSeverity::Error);

        // Test Error severity for InvalidType
        let invalid_type = ConversionError::InvalidType {
            name: "test".to_string(),
            expected: "string".to_string(),
            actual: "number".to_string(),
        };
        assert_eq!(invalid_type.severity(), ErrorSeverity::Error);

        // Test Critical severity for SchemaValidation
        let schema_validation = ConversionError::SchemaValidation {
            message: "Invalid schema".to_string(),
        };
        assert_eq!(schema_validation.severity(), ErrorSeverity::Critical);

        // Test Error severity for ParseError
        let parse_error = ConversionError::ParseError {
            field: "count".to_string(),
            data_type: "integer".to_string(),
            message: "invalid digit".to_string(),
        };
        assert_eq!(parse_error.severity(), ErrorSeverity::Error);

        // Test Error severity for UnsupportedSchemaType
        let unsupported_type = ConversionError::UnsupportedSchemaType {
            schema_type: "object".to_string(),
            parameter: "param".to_string(),
        };
        assert_eq!(unsupported_type.severity(), ErrorSeverity::Error);

        // Test delegation to ValidationError severity
        let validation_error = ConversionError::ValidationError(ValidationError::InvalidSchema {
            message: "Bad schema".to_string(),
        });
        assert_eq!(validation_error.severity(), ErrorSeverity::Critical);

        // Test delegation for Warning level ValidationError
        let validation_warning =
            ConversionError::ValidationError(ValidationError::InvalidParameterName {
                parameter: "bad@name".to_string(),
                reason: "Invalid chars".to_string(),
            });
        assert_eq!(validation_warning.severity(), ErrorSeverity::Warning);
    }
}
