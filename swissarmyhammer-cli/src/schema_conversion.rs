//! JSON Schema to Clap Argument Conversion
//!
//! This module provides utilities to convert JSON Schema definitions (used by MCP tools)
//! into Clap argument definitions for dynamic CLI generation.

use clap::{Arg, ArgAction};
use serde_json::Value;
use std::collections::HashSet;

/// Errors that can occur during schema conversion
#[derive(Debug, thiserror::Error)]
pub enum SchemaConversionError {
    #[error("Schema is not an object")]
    SchemaNotObject,

    #[error("Properties field is missing from schema")]
    MissingProperties,

    #[error("Properties field is not an object")]
    PropertiesNotObject,

    #[error("Property '{property}' has invalid type: {type_value}")]
    InvalidPropertyType {
        property: String,
        type_value: String,
    },

    #[error("Property '{property}' is missing required 'type' field")]
    MissingPropertyType { property: String },

    #[error("Required field list is not an array")]
    RequiredFieldsNotArray,

    #[error("Unsupported property type '{property_type}' for property '{property}'")]
    UnsupportedPropertyType {
        property: String,
        property_type: String,
    },
}

/// Utility for converting JSON Schema definitions to Clap arguments
pub struct SchemaConverter;

impl SchemaConverter {
    /// Convert a JSON Schema object into a vector of Clap arguments
    ///
    /// # Arguments
    /// * `schema` - JSON Schema definition as serde_json::Value
    ///
    /// # Returns
    /// * `Ok(Vec<Arg>)` - Vector of clap arguments
    /// * `Err(SchemaConversionError)` - Conversion error
    ///
    /// # Example
    /// ```
    /// use serde_json::json;
    /// use swissarmyhammer_cli::schema_conversion::SchemaConverter;
    ///
    /// let schema = json!({
    ///     "type": "object",
    ///     "properties": {
    ///         "title": {"type": "string", "description": "Title of the item"},
    ///         "count": {"type": "integer", "description": "Number of items"}
    ///     },
    ///     "required": ["title"]
    /// });
    ///
    /// let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
    /// assert_eq!(args.len(), 2);
    /// ```
    pub fn schema_to_clap_args(schema: &Value) -> Result<Vec<Arg>, SchemaConversionError> {
        // Ensure schema is an object
        let schema_obj = schema
            .as_object()
            .ok_or(SchemaConversionError::SchemaNotObject)?;

        // Get properties object
        let properties = schema_obj
            .get("properties")
            .ok_or(SchemaConversionError::MissingProperties)?
            .as_object()
            .ok_or(SchemaConversionError::PropertiesNotObject)?;

        // Extract required fields
        let required_fields = Self::extract_required_fields(schema)?;

        // Convert each property to a clap argument
        let mut args = Vec::new();
        for (property_name, property_schema) in properties {
            let is_required = required_fields.contains(property_name);
            let arg = Self::json_property_to_clap_arg(
                property_name.clone(),
                property_schema,
                is_required,
            )?;
            args.push(arg);
        }

        Ok(args)
    }

    /// Convert an individual JSON Schema property to a Clap argument
    ///
    /// # Arguments
    /// * `name` - Property name (used as argument name)
    /// * `property_schema` - JSON Schema for the property
    /// * `is_required` - Whether this property is in the required array
    ///
    /// # Returns
    /// * `Ok(Arg)` - Clap argument definition
    /// * `Err(SchemaConversionError)` - Conversion error
    fn json_property_to_clap_arg(
        name: String,
        property_schema: &Value,
        is_required: bool,
    ) -> Result<Arg, SchemaConversionError> {
        let property_obj = property_schema.as_object().ok_or_else(|| {
            SchemaConversionError::InvalidPropertyType {
                property: name.clone(),
                type_value: "not an object".to_string(),
            }
        })?;

        // Get the type field
        let type_value = property_obj
            .get("type")
            .ok_or_else(|| SchemaConversionError::MissingPropertyType {
                property: name.clone(),
            })?
            .as_str()
            .ok_or_else(|| SchemaConversionError::InvalidPropertyType {
                property: name.clone(),
                type_value: "type field is not a string".to_string(),
            })?;

        // Get optional description - convert to owned string
        let description = property_obj
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Create base argument with long flag (--property-name format)
        let arg_name = name.replace('_', "-"); // Convert snake_case to kebab-case for CLI

        // Use Box::leak to provide 'static lifetime strings for clap
        // This is necessary because clap requires 'static str references
        let name_static: &'static str = Box::leak(name.clone().into_boxed_str());
        let arg_name_static: &'static str = Box::leak(arg_name.into_boxed_str());
        let description_static: &'static str = Box::leak(description.into_boxed_str());

        // Use an approach that works with clap's ownership model
        let arg = match type_value {
            "string" => {
                let mut arg = Arg::new(name_static)
                    .long(arg_name_static)
                    .help(description_static)
                    .value_parser(clap::value_parser!(String));
                if is_required {
                    arg = arg.required(true);
                }
                arg
            }
            "boolean" => {
                // Boolean flags are never required as they default to false
                Arg::new(name_static)
                    .long(arg_name_static)
                    .help(description_static)
                    .action(ArgAction::SetTrue)
            }
            "integer" => {
                let mut arg = Arg::new(name_static)
                    .long(arg_name_static)
                    .help(description_static)
                    .value_parser(clap::value_parser!(i64));
                if is_required {
                    arg = arg.required(true);
                }
                arg
            }
            "array" => {
                let mut arg = Arg::new(name_static)
                    .long(arg_name_static)
                    .help(description_static)
                    .action(ArgAction::Append)
                    .value_parser(clap::value_parser!(String));
                if is_required {
                    arg = arg.required(true);
                }
                arg
            }
            _ => {
                return Err(SchemaConversionError::UnsupportedPropertyType {
                    property: name,
                    property_type: type_value.to_string(),
                });
            }
        };

        Ok(arg)
    }

    /// Extract required field names from a JSON Schema
    ///
    /// # Arguments
    /// * `schema` - JSON Schema definition
    ///
    /// # Returns
    /// * `Ok(HashSet<String>)` - Set of required field names
    /// * `Err(SchemaConversionError)` - Extraction error
    fn extract_required_fields(schema: &Value) -> Result<HashSet<String>, SchemaConversionError> {
        let schema_obj = schema
            .as_object()
            .ok_or(SchemaConversionError::SchemaNotObject)?;

        // If no required field, return empty set
        let Some(required_value) = schema_obj.get("required") else {
            return Ok(HashSet::new());
        };

        let required_array = required_value
            .as_array()
            .ok_or(SchemaConversionError::RequiredFieldsNotArray)?;

        let mut required_fields = HashSet::new();
        for field_value in required_array {
            if let Some(field_name) = field_value.as_str() {
                required_fields.insert(field_name.to_string());
            }
            // Silently skip non-string values in required array
        }

        Ok(required_fields)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_simple_string_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title of the item"}
            },
            "required": ["title"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let arg = &args[0];
        assert_eq!(arg.get_id(), "title");
        assert_eq!(arg.get_long(), Some("title"));
        assert!(arg.is_required_set());
    }

    #[test]
    fn test_boolean_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "force": {"type": "boolean", "description": "Force the operation"}
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let arg = &args[0];
        assert_eq!(arg.get_id(), "force");
        assert!(matches!(arg.get_action(), ArgAction::SetTrue));
        assert!(!arg.is_required_set());
    }

    #[test]
    fn test_integer_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {"type": "integer", "description": "Number of items"}
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let arg = &args[0];
        assert_eq!(arg.get_id(), "count");
        // Value parser is checked during runtime, hard to test statically
    }

    #[test]
    fn test_array_property() {
        let schema = json!({
            "type": "object",
            "properties": {
                "tags": {"type": "array", "description": "List of tags"}
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let arg = &args[0];
        assert_eq!(arg.get_id(), "tags");
        assert!(matches!(arg.get_action(), ArgAction::Append));
    }

    #[test]
    fn test_snake_case_to_kebab_case() {
        let schema = json!({
            "type": "object",
            "properties": {
                "file_path": {"type": "string", "description": "Path to file"}
            }
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 1);

        let arg = &args[0];
        assert_eq!(arg.get_id(), "file_path"); // Internal name keeps snake_case
        assert_eq!(arg.get_long(), Some("file-path")); // CLI flag uses kebab-case
    }

    #[test]
    fn test_mixed_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string", "description": "Title"},
                "count": {"type": "integer", "description": "Count"},
                "force": {"type": "boolean", "description": "Force operation"},
                "tags": {"type": "array", "description": "Tags"}
            },
            "required": ["title", "count"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 4);

        // Find each argument and verify properties
        let title_arg = args.iter().find(|arg| arg.get_id() == "title").unwrap();
        assert!(title_arg.is_required_set());

        let count_arg = args.iter().find(|arg| arg.get_id() == "count").unwrap();
        assert!(count_arg.is_required_set());

        let force_arg = args.iter().find(|arg| arg.get_id() == "force").unwrap();
        assert!(!force_arg.is_required_set());
        assert!(matches!(force_arg.get_action(), ArgAction::SetTrue));

        let tags_arg = args.iter().find(|arg| arg.get_id() == "tags").unwrap();
        assert!(!tags_arg.is_required_set());
        assert!(matches!(tags_arg.get_action(), ArgAction::Append));
    }

    #[test]
    fn test_missing_properties() {
        let schema = json!({
            "type": "object"
        });

        let result = SchemaConverter::schema_to_clap_args(&schema);
        assert!(matches!(
            result,
            Err(SchemaConversionError::MissingProperties)
        ));
    }

    #[test]
    fn test_invalid_schema_type() {
        let schema = json!("not an object");

        let result = SchemaConverter::schema_to_clap_args(&schema);
        assert!(matches!(
            result,
            Err(SchemaConversionError::SchemaNotObject)
        ));
    }

    #[test]
    fn test_unsupported_property_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "data": {"type": "number", "description": "Some number"}
            }
        });

        let result = SchemaConverter::schema_to_clap_args(&schema);
        assert!(matches!(
            result,
            Err(SchemaConversionError::UnsupportedPropertyType { .. })
        ));
    }

    #[test]
    fn test_extract_required_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {"type": "string"},
                "content": {"type": "string"},
                "optional": {"type": "string"}
            },
            "required": ["title", "content"]
        });

        let required = SchemaConverter::extract_required_fields(&schema).unwrap();
        assert_eq!(required.len(), 2);
        assert!(required.contains("title"));
        assert!(required.contains("content"));
        assert!(!required.contains("optional"));
    }

    #[test]
    fn test_no_required_fields() {
        let schema = json!({
            "type": "object",
            "properties": {
                "optional": {"type": "string"}
            }
        });

        let required = SchemaConverter::extract_required_fields(&schema).unwrap();
        assert!(required.is_empty());
    }

    // Test with real MCP tool schema examples
    #[test]
    fn test_memo_create_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the memo"
                },
                "content": {
                    "type": "string",
                    "description": "Markdown content of the memo"
                }
            },
            "required": ["title", "content"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 2);

        let title_arg = args.iter().find(|arg| arg.get_id() == "title").unwrap();
        assert!(title_arg.is_required_set());

        let content_arg = args.iter().find(|arg| arg.get_id() == "content").unwrap();
        assert!(content_arg.is_required_set());
    }

    #[test]
    fn test_file_edit_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to modify"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact text to replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement text"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)",
                    "default": false
                }
            },
            "required": ["file_path", "old_string", "new_string"]
        });

        let args = SchemaConverter::schema_to_clap_args(&schema).unwrap();
        assert_eq!(args.len(), 4);

        // Check required fields
        let file_path_arg = args.iter().find(|arg| arg.get_id() == "file_path").unwrap();
        assert!(file_path_arg.is_required_set());
        assert_eq!(file_path_arg.get_long(), Some("file-path"));

        let old_string_arg = args
            .iter()
            .find(|arg| arg.get_id() == "old_string")
            .unwrap();
        assert!(old_string_arg.is_required_set());
        assert_eq!(old_string_arg.get_long(), Some("old-string"));

        let new_string_arg = args
            .iter()
            .find(|arg| arg.get_id() == "new_string")
            .unwrap();
        assert!(new_string_arg.is_required_set());
        assert_eq!(new_string_arg.get_long(), Some("new-string"));

        // Check optional boolean
        let replace_all_arg = args
            .iter()
            .find(|arg| arg.get_id() == "replace_all")
            .unwrap();
        assert!(!replace_all_arg.is_required_set());
        assert!(matches!(replace_all_arg.get_action(), ArgAction::SetTrue));
        assert_eq!(replace_all_arg.get_long(), Some("replace-all"));
    }
}
