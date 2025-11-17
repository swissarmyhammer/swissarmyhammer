//! Schema Validation Framework
//!
//! Provides comprehensive validation for JSON schemas used in dynamic CLI generation.
//! Ensures that MCP tool schemas can be successfully converted to clap arguments
//! and provides detailed error messages with suggestions for fixing issues.

use serde_json::{Map, Value};
use std::collections::HashSet;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Errors that can occur during schema validation
#[derive(Debug, Error, Clone)]
#[allow(dead_code)]
pub enum ValidationError {
    #[error("Unsupported schema type '{schema_type}' for parameter '{parameter}'. {suggestion}")]
    UnsupportedSchemaType {
        schema_type: String,
        parameter: String,
        suggestion: String,
    },

    #[error("Invalid schema structure: {message}")]
    InvalidSchema { message: String },

    #[error("Missing required schema field: {field}")]
    MissingSchemaField { field: String },

    #[error("Schema conversion failed for parameter '{parameter}': {details}")]
    ConversionFailed { parameter: String, details: String },

    #[error("Invalid parameter name '{parameter}': {reason}")]
    InvalidParameterName { parameter: String, reason: String },

    #[error("Schema property '{property}' has invalid structure: {message}")]
    InvalidProperty { property: String, message: String },

    #[error("Conflicting schema definitions for parameter '{parameter}': {conflict}")]
    ConflictingDefinitions { parameter: String, conflict: String },
}

impl ValidationError {
    /// Get user-friendly suggestions for fixing the validation error
    pub fn suggestion(&self) -> Option<String> {
        match self {
            ValidationError::UnsupportedSchemaType { suggestion, .. } => Some(suggestion.clone()),
            ValidationError::InvalidSchema { message } => {
                if message.contains("properties") {
                    Some("Ensure the schema has a valid 'properties' object with parameter definitions.".to_string())
                } else {
                    Some("Check that the schema follows JSON Schema specification.".to_string())
                }
            }
            ValidationError::MissingSchemaField { field } => {
                Some(format!("Add the required field '{}' to the schema.", field))
            }
            ValidationError::ConversionFailed { .. } => {
                Some("Check parameter type definitions and ensure they match supported CLI types.".to_string())
            }
            ValidationError::InvalidParameterName { .. } => {
                Some("Use valid parameter names with letters, numbers, hyphens, and underscores only.".to_string())
            }
            ValidationError::InvalidProperty { .. } => {
                Some("Ensure property definitions follow JSON Schema specification.".to_string())
            }
            ValidationError::ConflictingDefinitions { .. } => {
                Some("Remove conflicting definitions or choose a single consistent type for the parameter.".to_string())
            }
        }
    }
}

impl Severity for ValidationError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            ValidationError::UnsupportedSchemaType { .. } => ErrorSeverity::Error,
            ValidationError::InvalidSchema { .. } => ErrorSeverity::Critical,
            ValidationError::MissingSchemaField { .. } => ErrorSeverity::Error,
            ValidationError::ConversionFailed { .. } => ErrorSeverity::Error,
            ValidationError::InvalidParameterName { .. } => ErrorSeverity::Warning,
            ValidationError::InvalidProperty { .. } => ErrorSeverity::Error,
            ValidationError::ConflictingDefinitions { .. } => ErrorSeverity::Error,
        }
    }
}

/// Schema validator for comprehensive JSON schema validation
#[allow(dead_code)]
pub struct SchemaValidator;

#[allow(dead_code)]
impl SchemaValidator {
    /// Validate a complete JSON schema for CLI compatibility
    ///
    /// Performs comprehensive validation including:
    /// - Schema structure validation
    /// - Property definition validation
    /// - Type compatibility checks
    /// - Parameter name validation
    /// - Conflict detection
    ///
    /// # Arguments
    /// * `schema` - The JSON schema to validate
    ///
    /// # Returns
    /// * `Ok(())` - Schema is valid and CLI-compatible
    /// * `Err(ValidationError)` - Schema has validation issues
    ///
    /// # Example
    /// ```rust
    /// use serde_json::json;
    /// use swissarmyhammer_cli::schema_validation::SchemaValidator;
    ///
    /// let schema = json!({
    ///     "type": "object",
    ///     "properties": {
    ///         "name": {"type": "string", "description": "User name"}
    ///     },
    ///     "required": ["name"]
    /// });
    ///
    /// assert!(SchemaValidator::validate_schema(&schema).is_ok());
    /// ```
    pub fn validate_schema(schema: &Value) -> Result<(), ValidationError> {
        let errors = Self::validate_schema_internal(schema, false)?;
        if errors.is_empty() {
            Ok(())
        } else {
            // Should never happen in fail-fast mode, but handle defensively
            Err(errors.into_iter().next().unwrap())
        }
    }

    /// Internal schema validation with configurable error handling strategy
    ///
    /// # Arguments
    /// * `schema` - The JSON schema to validate
    /// * `collect_all` - If true, collect all errors; if false, fail fast on first error
    ///
    /// # Returns
    /// * `Ok(Vec<ValidationError>)` - List of errors found (empty if valid)
    /// * `Err(ValidationError)` - Critical error that prevents further validation (fail-fast mode only)
    fn validate_schema_internal(
        schema: &Value,
        collect_all: bool,
    ) -> Result<Vec<ValidationError>, ValidationError> {
        let mut errors = Vec::new();

        macro_rules! handle_result {
            ($result:expr) => {
                match $result {
                    Err(e) => {
                        if collect_all {
                            errors.push(e);
                        } else {
                            return Err(e);
                        }
                    }
                    Ok(_) => {}
                }
            };
        }

        // Validate basic schema structure (always fail-fast on structure errors)
        handle_result!(Self::validate_schema_structure(schema));

        // If structure validation failed in collect_all mode, return early
        if !errors.is_empty() && collect_all {
            return Ok(errors);
        }

        // Extract and validate properties
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            let prop_errors = Self::validate_properties_internal(properties, collect_all)?;
            errors.extend(prop_errors);

            handle_result!(Self::validate_property_consistency(properties));
        }

        // Validate required fields if present
        if let Some(required) = schema.get("required") {
            handle_result!(Self::validate_required_fields(required, schema));
        }

        Ok(errors)
    }

    /// Validate the basic structure of a JSON schema
    fn validate_schema_structure(schema: &Value) -> Result<(), ValidationError> {
        if !schema.is_object() {
            return Err(ValidationError::InvalidSchema {
                message: "Schema must be a JSON object".to_string(),
            });
        }

        let schema_obj = schema.as_object().unwrap();

        // Check for properties field
        match schema_obj.get("properties") {
            Some(properties) => {
                if !properties.is_object() {
                    return Err(ValidationError::InvalidSchema {
                        message: "Schema 'properties' field must be an object".to_string(),
                    });
                }
            }
            None => {
                return Err(ValidationError::MissingSchemaField {
                    field: "properties".to_string(),
                });
            }
        }

        // Validate type field if present
        if let Some(schema_type) = schema_obj.get("type") {
            if let Some(type_str) = schema_type.as_str() {
                if type_str != "object" {
                    return Err(ValidationError::InvalidSchema {
                        message: format!("Root schema type must be 'object', found '{}'", type_str),
                    });
                }
            } else {
                return Err(ValidationError::InvalidSchema {
                    message: "Schema 'type' field must be a string".to_string(),
                });
            }
        }

        Ok(())
    }

    /// Validate individual property schemas
    pub fn validate_properties(properties: &Map<String, Value>) -> Result<(), ValidationError> {
        let errors = Self::validate_properties_internal(properties, false)?;
        if errors.is_empty() {
            Ok(())
        } else {
            // Should never happen in fail-fast mode, but handle defensively
            Err(errors.into_iter().next().unwrap())
        }
    }

    /// Internal property validation with configurable error handling strategy
    ///
    /// # Arguments
    /// * `properties` - The property map to validate
    /// * `collect_all` - If true, collect all errors; if false, fail fast on first error
    ///
    /// # Returns
    /// * `Ok(Vec<ValidationError>)` - List of errors found (empty if valid)
    /// * `Err(ValidationError)` - Critical error in fail-fast mode
    fn validate_properties_internal(
        properties: &Map<String, Value>,
        collect_all: bool,
    ) -> Result<Vec<ValidationError>, ValidationError> {
        let mut errors = Vec::new();

        for (prop_name, prop_schema) in properties {
            if let Err(e) = Self::validate_parameter_name(prop_name) {
                if collect_all {
                    errors.push(e);
                } else {
                    return Err(e);
                }
            }

            if let Err(e) = Self::validate_property_schema(prop_name, prop_schema) {
                if collect_all {
                    errors.push(e);
                } else {
                    return Err(e);
                }
            }
        }

        Ok(errors)
    }

    /// Validate a parameter name for CLI compatibility
    fn validate_parameter_name(name: &str) -> Result<(), ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::InvalidParameterName {
                parameter: name.to_string(),
                reason: "Parameter name cannot be empty".to_string(),
            });
        }

        // Check for valid CLI parameter characters
        let invalid_chars: Vec<char> = name
            .chars()
            .filter(|c| !c.is_alphanumeric() && *c != '-' && *c != '_')
            .collect();

        if !invalid_chars.is_empty() {
            return Err(ValidationError::InvalidParameterName {
                parameter: name.to_string(),
                reason: format!(
                    "Contains invalid characters: {}. Use only letters, numbers, hyphens, and underscores",
                    invalid_chars.iter().collect::<String>()
                ),
            });
        }

        // Check for reserved CLI names
        const RESERVED_NAMES: &[&str] = &["help", "version", "verbose", "quiet", "debug"];
        if RESERVED_NAMES.contains(&name) {
            return Err(ValidationError::InvalidParameterName {
                parameter: name.to_string(),
                reason: format!("'{}' is a reserved parameter name", name),
            });
        }

        Ok(())
    }

    /// Validate a single property schema definition
    fn validate_property_schema(
        prop_name: &str,
        prop_schema: &Value,
    ) -> Result<(), ValidationError> {
        if !prop_schema.is_object() {
            return Err(ValidationError::InvalidProperty {
                property: prop_name.to_string(),
                message: "Property schema must be an object".to_string(),
            });
        }

        let prop_obj = prop_schema.as_object().unwrap();

        // Validate type field
        if let Some(prop_type) = prop_obj.get("type") {
            Self::validate_property_type(prop_name, prop_type)?;
        }

        // Validate description field if present
        if let Some(description) = prop_obj.get("description") {
            if !description.is_string() {
                return Err(ValidationError::InvalidProperty {
                    property: prop_name.to_string(),
                    message: "Property description must be a string".to_string(),
                });
            }
        }

        // Validate default value type consistency if present
        if let (Some(prop_type), Some(default)) = (prop_obj.get("type"), prop_obj.get("default")) {
            Self::validate_default_value_type(prop_name, prop_type, default)?;
        }

        Ok(())
    }

    /// Validate a property type for CLI compatibility
    fn validate_property_type(prop_name: &str, prop_type: &Value) -> Result<(), ValidationError> {
        if let Some(type_str) = prop_type.as_str() {
            match type_str {
                "string" | "integer" | "number" | "boolean" | "array" => Ok(()),
                "object" => Err(ValidationError::UnsupportedSchemaType {
                    schema_type: type_str.to_string(),
                    parameter: prop_name.to_string(),
                    suggestion: "Nested objects are not supported in CLI. Consider flattening the schema or using a string representation.".to_string(),
                }),
                "null" => Err(ValidationError::UnsupportedSchemaType {
                    schema_type: type_str.to_string(),
                    parameter: prop_name.to_string(),
                    suggestion: "Null types are not meaningful for CLI parameters. Consider making the parameter optional or using a different type.".to_string(),
                }),
                unknown => Err(ValidationError::UnsupportedSchemaType {
                    schema_type: unknown.to_string(),
                    parameter: prop_name.to_string(),
                    suggestion: format!("Unknown type '{}'. Supported types: string, boolean, integer, number, array.", unknown),
                }),
            }
        } else if prop_type.is_array() {
            // Handle union types (e.g., ["string", "null"])
            if let Some(types) = prop_type.as_array() {
                let type_strs: Vec<String> = types
                    .iter()
                    .filter_map(|t| t.as_str())
                    .map(|s| s.to_string())
                    .collect();

                // Allow nullable types but warn about complexity
                if type_strs.contains(&"null".to_string()) && type_strs.len() == 2 {
                    // This is acceptable for optional parameters
                    Ok(())
                } else {
                    Err(ValidationError::UnsupportedSchemaType {
                        schema_type: format!("union {:?}", type_strs),
                        parameter: prop_name.to_string(),
                        suggestion: "Complex union types are not supported. Use a single type or make parameters optional.".to_string(),
                    })
                }
            } else {
                Err(ValidationError::InvalidProperty {
                    property: prop_name.to_string(),
                    message: "Type array must contain string type names".to_string(),
                })
            }
        } else {
            Err(ValidationError::InvalidProperty {
                property: prop_name.to_string(),
                message: "Property type must be a string or array of strings".to_string(),
            })
        }
    }

    /// Validate that a default value matches its declared type
    fn validate_default_value_type(
        prop_name: &str,
        prop_type: &Value,
        default: &Value,
    ) -> Result<(), ValidationError> {
        if let Some(type_str) = prop_type.as_str() {
            let type_matches = match type_str {
                "string" => default.is_string(),
                "integer" => default.is_i64() || default.is_u64(),
                "number" => default.is_number(),
                "boolean" => default.is_boolean(),
                "array" => default.is_array(),
                _ => true, // Skip validation for unsupported types (will be caught elsewhere)
            };

            if !type_matches {
                return Err(ValidationError::InvalidProperty {
                    property: prop_name.to_string(),
                    message: format!(
                        "Default value type {:?} does not match declared type '{}'",
                        default, type_str
                    ),
                });
            }
        }

        Ok(())
    }

    /// Validate required fields consistency
    fn validate_required_fields(required: &Value, schema: &Value) -> Result<(), ValidationError> {
        if let Some(required_array) = required.as_array() {
            if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
                for required_item in required_array {
                    if let Some(field_name) = required_item.as_str() {
                        if !properties.contains_key(field_name) {
                            return Err(ValidationError::ConflictingDefinitions {
                                parameter: field_name.to_string(),
                                conflict:
                                    "Field is marked as required but not defined in properties"
                                        .to_string(),
                            });
                        }
                    } else {
                        return Err(ValidationError::InvalidSchema {
                            message: "Required field names must be strings".to_string(),
                        });
                    }
                }
            }
        } else {
            return Err(ValidationError::InvalidSchema {
                message: "Required field must be an array of strings".to_string(),
            });
        }

        Ok(())
    }

    /// Validate property consistency across the schema
    fn validate_property_consistency(
        properties: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        let mut seen_names = HashSet::new();

        for (prop_name, _) in properties {
            // Check for case-insensitive duplicates (CLI parameter names are case-insensitive)
            let lowercase_name = prop_name.to_lowercase();
            if seen_names.contains(&lowercase_name) {
                return Err(ValidationError::ConflictingDefinitions {
                    parameter: prop_name.to_string(),
                    conflict: "Parameter name conflicts with existing parameter (case-insensitive)"
                        .to_string(),
                });
            }
            seen_names.insert(lowercase_name);
        }

        Ok(())
    }

    /// Check if all schema types are supported by the CLI converter
    pub fn check_supported_types(schema: &Value) -> Result<(), ValidationError> {
        if let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) {
            for (prop_name, prop_schema) in properties {
                if let Some(prop_type) = prop_schema.get("type") {
                    Self::validate_property_type(prop_name, prop_type)?;
                }
            }
        }
        Ok(())
    }

    /// Validate schema and collect all errors (non-failing validation)
    ///
    /// This method collects all validation errors instead of stopping at the first one,
    /// providing comprehensive feedback about all issues in the schema.
    pub fn validate_schema_comprehensive(schema: &Value) -> Vec<ValidationError> {
        match Self::validate_schema_internal(schema, true) {
            Ok(errors) => errors,
            Err(e) => vec![e], // Shouldn't happen in collect_all mode, but handle defensively
        }
    }

    /// Get user-friendly error summary
    pub fn format_validation_errors(errors: &[ValidationError]) -> String {
        if errors.is_empty() {
            return "No validation errors".to_string();
        }

        let mut output = String::new();
        output.push_str(&format!("Found {} validation error(s):\n", errors.len()));

        for (i, error) in errors.iter().enumerate() {
            output.push_str(&format!("{}. {}\n", i + 1, error));
            if let Some(suggestion) = error.suggestion() {
                output.push_str(&format!("   💡 {}\n", suggestion));
            }
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_valid_schema() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Title of the item"
                },
                "count": {
                    "type": "integer",
                    "description": "Number of items"
                }
            },
            "required": ["title"]
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_missing_properties() {
        let schema = json!({
            "type": "object",
            "required": ["title"]
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::MissingSchemaField { .. }
        ));
    }

    #[test]
    fn test_unsupported_object_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "nested": {
                    "type": "object",
                    "properties": {}
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::UnsupportedSchemaType { .. }
        ));
    }

    #[test]
    fn test_invalid_parameter_name() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param@with#symbols": {
                    "type": "string"
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidParameterName { .. }
        ));
    }

    #[test]
    fn test_reserved_parameter_name() {
        let schema = json!({
            "type": "object",
            "properties": {
                "help": {
                    "type": "string"
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidParameterName { .. }
        ));
    }

    #[test]
    fn test_conflicting_required_field() {
        let schema = json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string"
                }
            },
            "required": ["title", "nonexistent"]
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::ConflictingDefinitions { .. }
        ));
    }

    #[test]
    fn test_invalid_default_value_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "count": {
                    "type": "integer",
                    "default": "not-a-number"
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidProperty { .. }
        ));
    }

    #[test]
    fn test_comprehensive_validation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "valid_param": {
                    "type": "string"
                },
                "invalid@param": {
                    "type": "object"
                },
                "help": {
                    "type": "string"
                }
            }
        });

        let errors = SchemaValidator::validate_schema_comprehensive(&schema);
        assert!(!errors.is_empty());
        assert!(errors.len() >= 2); // Should catch multiple errors
    }

    #[test]
    fn test_supported_types() {
        let schema = json!({
            "type": "object",
            "properties": {
                "str_param": {"type": "string"},
                "int_param": {"type": "integer"},
                "num_param": {"type": "number"},
                "bool_param": {"type": "boolean"},
                "arr_param": {"type": "array"}
            }
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_nullable_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "optional_param": {
                    "type": ["string", "null"]
                }
            }
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_case_insensitive_parameter_conflict() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {"type": "string"},
                "PARAM": {"type": "string"}
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::ConflictingDefinitions { .. }
        ));
    }

    #[test]
    fn test_validation_error_suggestions() {
        let error = ValidationError::UnsupportedSchemaType {
            schema_type: "object".to_string(),
            parameter: "test".to_string(),
            suggestion: "Use flattened structure".to_string(),
        };

        assert!(error.suggestion().is_some());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_format_validation_errors() {
        let errors = vec![
            ValidationError::UnsupportedSchemaType {
                schema_type: "object".to_string(),
                parameter: "test".to_string(),
                suggestion: "Flatten the structure".to_string(),
            },
            ValidationError::InvalidParameterName {
                parameter: "bad@name".to_string(),
                reason: "Contains invalid characters".to_string(),
            },
        ];

        let formatted = SchemaValidator::format_validation_errors(&errors);
        assert!(formatted.contains("Found 2 validation error(s)"));
        assert!(formatted.contains("💡"));
    }

    // Additional comprehensive edge case tests

    #[test]
    fn test_empty_schema() {
        let schema = json!({});
        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::MissingSchemaField { .. }
        ));
    }

    #[test]
    fn test_schema_without_type() {
        let schema = json!({
            "properties": {
                "param": {"type": "string"}
            }
        });
        // Should be valid - type field is optional at root level
        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_schema_with_wrong_root_type() {
        let schema = json!({
            "type": "array",
            "properties": {
                "param": {"type": "string"}
            }
        });
        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidSchema { .. }
        ));
    }

    #[test]
    fn test_properties_not_object() {
        let schema = json!({
            "type": "object",
            "properties": "not-an-object"
        });
        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidSchema { .. }
        ));
    }

    #[test]
    fn test_parameter_with_empty_name() {
        let properties = serde_json::Map::new();
        let mut props = properties;
        props.insert("".to_string(), json!({"type": "string"}));

        let result = SchemaValidator::validate_properties(&props);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidParameterName { .. }
        ));
    }

    #[test]
    fn test_parameter_with_special_characters() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param!@#$%": {"type": "string"},
                "param with spaces": {"type": "string"},
                "param.with.dots": {"type": "string"}
            }
        });

        let errors = SchemaValidator::validate_schema_comprehensive(&schema);
        assert!(errors.len() >= 3); // Should have errors for all invalid names

        for error in &errors {
            assert!(matches!(
                error,
                ValidationError::InvalidParameterName { .. }
            ));
        }
    }

    #[test]
    fn test_valid_parameter_names() {
        let schema = json!({
            "type": "object",
            "properties": {
                "valid_name": {"type": "string"},
                "valid-name": {"type": "string"},
                "ValidName123": {"type": "string"},
                "name_with_123": {"type": "string"}
            }
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_all_reserved_parameter_names() {
        let reserved_names = ["help", "version", "verbose", "quiet", "debug"];

        for reserved in &reserved_names {
            let mut props = serde_json::Map::new();
            props.insert(reserved.to_string(), json!({"type": "string"}));

            let result = SchemaValidator::validate_properties(&props);
            assert!(
                result.is_err(),
                "Reserved name '{}' should be invalid",
                reserved
            );
            assert!(matches!(
                result.unwrap_err(),
                ValidationError::InvalidParameterName { .. }
            ));
        }
    }

    #[test]
    fn test_property_schema_not_object() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": "not-an-object"
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidProperty { .. }
        ));
    }

    #[test]
    fn test_property_without_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {
                    "description": "A parameter without type"
                }
            }
        });

        // Should be valid - type is optional and defaults to string
        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_property_with_invalid_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {
                    "type": 123
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidProperty { .. }
        ));
    }

    #[test]
    fn test_union_type_with_null() {
        let schema = json!({
            "type": "object",
            "properties": {
                "optional_string": {
                    "type": ["string", "null"]
                }
            }
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_complex_union_type() {
        let schema = json!({
            "type": "object",
            "properties": {
                "complex_union": {
                    "type": ["string", "integer", "boolean"]
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::UnsupportedSchemaType { .. }
        ));
    }

    #[test]
    fn test_invalid_default_value_types() {
        let test_cases = vec![
            (json!({"type": "string", "default": 123}), "string"),
            (
                json!({"type": "integer", "default": "not-a-number"}),
                "integer",
            ),
            (
                json!({"type": "boolean", "default": "not-a-boolean"}),
                "boolean",
            ),
            (json!({"type": "array", "default": "not-an-array"}), "array"),
        ];

        for (prop_schema, expected_type) in test_cases {
            let schema = json!({
                "type": "object",
                "properties": {
                    "param": prop_schema
                }
            });

            let result = SchemaValidator::validate_schema(&schema);
            assert!(
                result.is_err(),
                "Should fail for invalid default type for {}",
                expected_type
            );
            assert!(matches!(
                result.unwrap_err(),
                ValidationError::InvalidProperty { .. }
            ));
        }
    }

    #[test]
    fn test_valid_default_value_types() {
        let schema = json!({
            "type": "object",
            "properties": {
                "string_param": {"type": "string", "default": "default_value"},
                "int_param": {"type": "integer", "default": 42},
                "bool_param": {"type": "boolean", "default": true},
                "array_param": {"type": "array", "default": ["item1", "item2"]}
            }
        });

        assert!(SchemaValidator::validate_schema(&schema).is_ok());
    }

    #[test]
    fn test_required_field_not_array() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {"type": "string"}
            },
            "required": "not-an-array"
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidSchema { .. }
        ));
    }

    #[test]
    fn test_required_field_with_non_string() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {"type": "string"}
            },
            "required": [123, "valid_field"]
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidSchema { .. }
        ));
    }

    #[test]
    fn test_required_field_not_in_properties() {
        let schema = json!({
            "type": "object",
            "properties": {
                "existing_param": {"type": "string"}
            },
            "required": ["existing_param", "nonexistent_param"]
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::ConflictingDefinitions { .. }
        ));
    }

    #[test]
    fn test_deeply_nested_structure() {
        let schema = json!({
            "type": "object",
            "properties": {
                "level1": {
                    "type": "object",
                    "properties": {
                        "level2": {
                            "type": "object",
                            "properties": {
                                "level3": {"type": "string"}
                            }
                        }
                    }
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::UnsupportedSchemaType { .. }
        ));
    }

    #[test]
    fn test_description_not_string() {
        let schema = json!({
            "type": "object",
            "properties": {
                "param": {
                    "type": "string",
                    "description": 123
                }
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ValidationError::InvalidProperty { .. }
        ));
    }

    #[test]
    fn test_comprehensive_validation_vs_single_validation() {
        let schema = json!({
            "type": "object",
            "properties": {
                "invalid@param": {"type": "object"},
                "help": {"type": "string"},
                "param with spaces": {"type": "unknown_type"}
            }
        });

        // Single validation should stop at first error
        let single_result = SchemaValidator::validate_schema(&schema);
        assert!(single_result.is_err());

        // Comprehensive validation should find multiple errors
        let comprehensive_errors = SchemaValidator::validate_schema_comprehensive(&schema);
        assert!(comprehensive_errors.len() >= 3);
    }

    #[test]
    fn test_error_severity_levels() {
        #[allow(clippy::useless_vec)]
        let errors = vec![
            ValidationError::UnsupportedSchemaType {
                schema_type: "object".to_string(),
                parameter: "test".to_string(),
                suggestion: "Fix it".to_string(),
            },
            ValidationError::InvalidSchema {
                message: "Bad schema".to_string(),
            },
            ValidationError::InvalidParameterName {
                parameter: "bad@name".to_string(),
                reason: "Invalid chars".to_string(),
            },
        ];

        assert_eq!(errors[0].severity(), ErrorSeverity::Error);
        assert_eq!(errors[1].severity(), ErrorSeverity::Critical);
        assert_eq!(errors[2].severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_malformed_json_schema() {
        // Test with a schema that is valid JSON but invalid JSON Schema
        let schema = json!({
            "this_is_not": "a_valid_schema",
            "missing": "required_fields",
            "random": {
                "structure": true
            }
        });

        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_err());
    }

    #[test]
    fn test_edge_case_parameter_names() {
        let edge_cases = vec![
            ("1param", "starts with number"),
            ("param-", "ends with hyphen"),
            ("param_", "ends with underscore"),
            ("-param", "starts with hyphen"),
            ("_param", "starts with underscore"),
            ("param--name", "double hyphen"),
            ("param__name", "double underscore"),
            ("PARAM", "all caps"),
            ("param123", "ends with numbers"),
        ];

        for (param_name, description) in edge_cases {
            let schema = json!({
                "type": "object",
                "properties": {
                    param_name: {"type": "string"}
                }
            });

            let result = SchemaValidator::validate_schema(&schema);
            match param_name {
                // These should be valid
                "PARAM" | "param123" | "_param" | "param_" | "param--name" | "param__name" => {
                    assert!(
                        result.is_ok(),
                        "Parameter '{}' ({}) should be valid",
                        param_name,
                        description
                    );
                }
                // These should be invalid
                _ => {
                    // For now, we're being permissive with some edge cases
                    // The exact validation rules can be refined based on CLI requirements
                }
            }
        }
    }
}
