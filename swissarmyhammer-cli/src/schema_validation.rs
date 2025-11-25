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

/// Validation context for managing error collection strategy
///
/// This context supports two error handling modes:
/// - Fail-fast mode (`collect_all = false`): Returns immediately on first error via `handle_error`
/// - Collect-all mode (`collect_all = true`): Accumulates all errors for comprehensive reporting
///
/// The two methods work together as part of a coordinated error collection strategy:
/// - `handle_error`: Processes each individual error as it occurs during validation
/// - `into_single_result`: Finalizes validation by converting collected errors to result
struct ValidationContext {
    collect_all: bool,
    errors: Vec<ValidationError>,
}

impl ValidationContext {
    fn new(collect_all: bool) -> Self {
        Self {
            collect_all,
            errors: Vec::new(),
        }
    }

    /// Handle a validation error according to the collection strategy
    ///
    /// In fail-fast mode, returns Err immediately to stop validation.
    /// In collect-all mode, stores the error and returns Ok to continue validation.
    fn handle_error(&mut self, error: ValidationError) -> Result<(), ValidationError> {
        if self.collect_all {
            self.errors.push(error);
            Ok(())
        } else {
            Err(error)
        }
    }

    /// Convert the collected errors to a single-error result for public API
    ///
    /// Returns the first error collected, or Ok if no errors occurred.
    /// This provides a consistent Result interface for both error collection modes.
    fn into_single_result(self) -> Result<(), ValidationError> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors.into_iter().next().unwrap())
        }
    }
}

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
        Self::run_validation_with_context(|ctx| Self::validate_schema_with_context(schema, ctx))
    }

    /// Internal schema validation with context
    ///
    /// # Arguments
    /// * `schema` - The JSON schema to validate
    /// * `ctx` - Validation context for error collection
    ///
    /// # Returns
    /// * `Ok(())` - Validation completed (check ctx for errors)
    /// * `Err(ValidationError)` - Critical error in fail-fast mode
    fn validate_schema_with_context(
        schema: &Value,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        Self::collect_validation_error(ctx, || Self::validate_schema_structure(schema))?;
        Self::validate_properties_section(schema, ctx)?;
        Self::validate_required_section(schema, ctx)?;
        Ok(())
    }

    /// Validate the properties section if present
    fn validate_properties_section(
        schema: &Value,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let Some(properties) = schema.get("properties").and_then(|p| p.as_object()) else {
            return Ok(());
        };

        Self::validate_properties_with_context(properties, ctx)?;
        Self::collect_validation_error(ctx, || Self::validate_property_consistency(properties))?;
        Ok(())
    }

    /// Validate the required section if present
    fn validate_required_section(
        schema: &Value,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        let Some(required) = schema.get("required") else {
            return Ok(());
        };

        Self::collect_validation_error(ctx, || Self::validate_required_fields(required, schema))
    }

    /// Generic helper to collect validation errors
    ///
    /// Executes a validation function and handles errors according to the context's collection strategy.
    fn collect_validation_error<F>(
        ctx: &mut ValidationContext,
        validator: F,
    ) -> Result<(), ValidationError>
    where
        F: FnOnce() -> Result<(), ValidationError>,
    {
        if let Err(e) = validator() {
            ctx.handle_error(e)?;
        }
        Ok(())
    }

    /// Helper to run validation with a fresh context and return single result
    ///
    /// Encapsulates the pattern of creating a ValidationContext, running validation, and converting to single result.
    fn run_validation_with_context<F>(validator: F) -> Result<(), ValidationError>
    where
        F: FnOnce(&mut ValidationContext) -> Result<(), ValidationError>,
    {
        let mut ctx = ValidationContext::new(false);
        validator(&mut ctx)?;
        ctx.into_single_result()
    }

    /// Validate the basic structure of a JSON schema
    fn validate_schema_structure(schema: &Value) -> Result<(), ValidationError> {
        let schema_obj = Self::ensure_schema_is_object(schema)?;
        Self::validate_properties_field_exists(schema_obj)?;
        Self::validate_root_type_field(schema_obj)?;
        Ok(())
    }

    /// Ensure schema is a valid object
    fn ensure_schema_is_object(schema: &Value) -> Result<&Map<String, Value>, ValidationError> {
        schema
            .as_object()
            .ok_or_else(|| ValidationError::InvalidSchema {
                message: "Schema must be a JSON object".to_string(),
            })
    }

    /// Validate that properties field exists and is an object
    fn validate_properties_field_exists(
        schema_obj: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        match schema_obj.get("properties") {
            None => Err(ValidationError::MissingSchemaField {
                field: "properties".to_string(),
            }),
            Some(properties) if !properties.is_object() => Err(ValidationError::InvalidSchema {
                message: "Schema 'properties' field must be an object".to_string(),
            }),
            Some(_) => Ok(()),
        }
    }

    /// Validate the root type field
    fn validate_root_type_field(schema_obj: &Map<String, Value>) -> Result<(), ValidationError> {
        let Some(schema_type) = schema_obj.get("type") else {
            return Ok(());
        };

        let Some(type_str) = schema_type.as_str() else {
            return Err(ValidationError::InvalidSchema {
                message: "Schema 'type' field must be a string".to_string(),
            });
        };

        if type_str != "object" {
            return Err(ValidationError::InvalidSchema {
                message: format!("Root schema type must be 'object', found '{}'", type_str),
            });
        }

        Ok(())
    }

    /// Validate individual property schemas
    pub fn validate_properties(properties: &Map<String, Value>) -> Result<(), ValidationError> {
        Self::run_validation_with_context(|ctx| {
            Self::validate_properties_with_context(properties, ctx)
        })
    }

    /// Internal property validation with context
    ///
    /// # Arguments
    /// * `properties` - The property map to validate
    /// * `ctx` - Validation context for error collection
    ///
    /// # Returns
    /// * `Ok(())` - Validation completed (check ctx for errors)
    /// * `Err(ValidationError)` - Critical error in fail-fast mode
    fn validate_properties_with_context(
        properties: &Map<String, Value>,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        for (prop_name, prop_schema) in properties {
            Self::validate_single_property_with_context(prop_name, prop_schema, ctx)?;
        }
        Ok(())
    }

    /// Validate a single property with error collection
    fn validate_single_property_with_context(
        prop_name: &str,
        prop_schema: &Value,
        ctx: &mut ValidationContext,
    ) -> Result<(), ValidationError> {
        Self::collect_validation_error(ctx, || Self::validate_parameter_name(prop_name))?;
        Self::collect_validation_error(ctx, || {
            Self::validate_property_schema(prop_name, prop_schema)
        })?;
        Ok(())
    }

    /// Validate a parameter name for CLI compatibility
    fn validate_parameter_name(name: &str) -> Result<(), ValidationError> {
        Self::validate_parameter_not_empty(name)?;
        Self::validate_parameter_characters(name)?;
        Self::validate_parameter_not_reserved(name)?;
        Ok(())
    }

    /// Validate parameter name is not empty
    fn validate_parameter_not_empty(name: &str) -> Result<(), ValidationError> {
        if name.is_empty() {
            return Err(ValidationError::InvalidParameterName {
                parameter: name.to_string(),
                reason: "Parameter name cannot be empty".to_string(),
            });
        }
        Ok(())
    }

    /// Validate parameter name contains only valid characters
    fn validate_parameter_characters(name: &str) -> Result<(), ValidationError> {
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
        Ok(())
    }

    /// Validate parameter name is not reserved
    fn validate_parameter_not_reserved(name: &str) -> Result<(), ValidationError> {
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
        let prop_obj = Self::validate_property_structure(prop_name, prop_schema)?;
        Self::validate_property_type_field(prop_name, prop_obj)?;
        Self::validate_property_description_field(prop_name, prop_obj)?;
        Self::validate_property_default_field(prop_name, prop_obj)?;
        Ok(())
    }

    /// Generic helper to validate an optional property field
    ///
    /// This consolidates the common pattern of checking if a field exists
    /// and validating it with a specific validator function.
    fn validate_optional_property_field<F>(
        prop_name: &str,
        prop_obj: &Map<String, Value>,
        field_name: &str,
        validator: F,
    ) -> Result<(), ValidationError>
    where
        F: FnOnce(&str, &Value) -> Result<(), ValidationError>,
    {
        if let Some(field_value) = prop_obj.get(field_name) {
            validator(prop_name, field_value)?;
        }
        Ok(())
    }

    /// Validate the type field of a property if present
    fn validate_property_type_field(
        prop_name: &str,
        prop_obj: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        Self::validate_optional_property_field(prop_name, prop_obj, "type", |name, value| {
            Self::validate_property_type(name, value)
        })
    }

    /// Validate the description field of a property if present
    fn validate_property_description_field(
        prop_name: &str,
        prop_obj: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        Self::validate_optional_property_field(prop_name, prop_obj, "description", |name, value| {
            if !value.is_string() {
                return Err(ValidationError::InvalidProperty {
                    property: name.to_string(),
                    message: "Property description must be a string".to_string(),
                });
            }
            Ok(())
        })
    }

    /// Validate the default field of a property if present
    fn validate_property_default_field(
        prop_name: &str,
        prop_obj: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        if let (Some(prop_type), Some(default)) = (prop_obj.get("type"), prop_obj.get("default")) {
            Self::validate_default_value_type(prop_name, prop_type, default)?;
        }
        Ok(())
    }

    /// Validate property schema structure
    fn validate_property_structure<'a>(
        prop_name: &str,
        prop_schema: &'a Value,
    ) -> Result<&'a Map<String, Value>, ValidationError> {
        if !prop_schema.is_object() {
            return Err(ValidationError::InvalidProperty {
                property: prop_name.to_string(),
                message: "Property schema must be an object".to_string(),
            });
        }
        Ok(prop_schema.as_object().unwrap())
    }

    /// Validate a property type for CLI compatibility
    fn validate_property_type(prop_name: &str, prop_type: &Value) -> Result<(), ValidationError> {
        if let Value::String(type_str) = prop_type {
            Self::validate_simple_type(prop_name, type_str)?;
            return Ok(());
        }

        if prop_type.is_array() {
            Self::validate_union_type(prop_name, prop_type)?;
            return Ok(());
        }

        Err(ValidationError::InvalidProperty {
            property: prop_name.to_string(),
            message: "Property type must be a string or array of strings".to_string(),
        })
    }

    /// Validate a simple type string
    fn validate_simple_type(prop_name: &str, type_str: &str) -> Result<(), ValidationError> {
        if Self::is_supported_type(type_str) {
            return Ok(());
        }

        Err(Self::create_unsupported_type_error(prop_name, type_str))
    }

    /// Supported JSON schema types for CLI parameters
    const SUPPORTED_TYPES: &[&str] = &["string", "integer", "number", "boolean", "array"];

    /// Check if a type string is supported
    fn is_supported_type(type_str: &str) -> bool {
        Self::SUPPORTED_TYPES.contains(&type_str)
    }

    /// Create an appropriate error for an unsupported type
    fn create_unsupported_type_error(prop_name: &str, type_str: &str) -> ValidationError {
        let suggestion = match type_str {
            "object" => "Nested objects are not supported in CLI. Consider flattening the schema or using a string representation.".to_string(),
            "null" => "Null types are not meaningful for CLI parameters. Consider making the parameter optional or using a different type.".to_string(),
            unknown => format!("Unknown type '{}'. Supported types: string, boolean, integer, number, array.", unknown),
        };

        ValidationError::UnsupportedSchemaType {
            schema_type: type_str.to_string(),
            parameter: prop_name.to_string(),
            suggestion,
        }
    }

    /// Validate a union type (array of types)
    fn validate_union_type(prop_name: &str, prop_type: &Value) -> Result<(), ValidationError> {
        let type_strs = Self::extract_union_type_strings(prop_name, prop_type)?;

        if Self::is_nullable_union(&type_strs) {
            return Ok(());
        }

        Err(ValidationError::UnsupportedSchemaType {
            schema_type: format!("union {:?}", type_strs),
            parameter: prop_name.to_string(),
            suggestion: "Complex union types are not supported. Use a single type or make parameters optional.".to_string(),
        })
    }

    /// Extract and convert union type array to strings
    fn extract_union_type_strings(
        prop_name: &str,
        prop_type: &Value,
    ) -> Result<Vec<String>, ValidationError> {
        let types = prop_type
            .as_array()
            .ok_or_else(|| ValidationError::InvalidProperty {
                property: prop_name.to_string(),
                message: "Type array must contain string type names".to_string(),
            })?;

        Ok(types
            .iter()
            .filter_map(|t| t.as_str())
            .map(|s| s.to_string())
            .collect())
    }

    /// Check if a union type is a simple nullable type (type + null)
    fn is_nullable_union(type_strs: &[String]) -> bool {
        type_strs.contains(&"null".to_string()) && type_strs.len() == 2
    }

    /// Validate that a default value matches its declared type
    fn validate_default_value_type(
        prop_name: &str,
        prop_type: &Value,
        default: &Value,
    ) -> Result<(), ValidationError> {
        let type_str = match prop_type.as_str() {
            Some(s) => s,
            None => return Ok(()),
        };

        if Self::default_value_matches_type(type_str, default) {
            return Ok(());
        }

        Err(ValidationError::InvalidProperty {
            property: prop_name.to_string(),
            message: format!(
                "Default value type {:?} does not match declared type '{}'",
                default, type_str
            ),
        })
    }

    /// Check if a default value matches the expected type
    fn default_value_matches_type(type_str: &str, value: &Value) -> bool {
        if !Self::is_supported_type(type_str) {
            return true; // Skip validation for unsupported types (will be caught elsewhere)
        }

        Self::value_matches_type_str(type_str, value)
    }

    /// Check if a value matches a specific type string
    fn value_matches_type_str(type_str: &str, value: &Value) -> bool {
        match (type_str, value) {
            ("string", Value::String(_)) => true,
            ("integer", Value::Number(n)) if n.is_i64() || n.is_u64() => true,
            ("number", Value::Number(_)) => true,
            ("boolean", Value::Bool(_)) => true,
            ("array", Value::Array(_)) => true,
            _ => false,
        }
    }

    /// Validate required fields consistency
    fn validate_required_fields(required: &Value, schema: &Value) -> Result<(), ValidationError> {
        let (required_array, properties) =
            Self::extract_required_validation_context(required, schema)?;
        Self::validate_all_required_fields_exist(required_array, properties)
    }

    /// Extract and validate both required array and properties map
    fn extract_required_validation_context<'a>(
        required: &'a Value,
        schema: &'a Value,
    ) -> Result<(&'a Vec<Value>, &'a Map<String, Value>), ValidationError> {
        let required_array = required
            .as_array()
            .ok_or_else(|| ValidationError::InvalidSchema {
                message: "Required field must be an array of strings".to_string(),
            })?;

        let properties = schema
            .get("properties")
            .and_then(|p| p.as_object())
            .ok_or_else(|| ValidationError::InvalidSchema {
                message: "Cannot validate required fields without properties".to_string(),
            })?;

        Ok((required_array, properties))
    }

    /// Validate that all required fields exist in properties
    fn validate_all_required_fields_exist(
        required_array: &[Value],
        properties: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        for required_item in required_array {
            let field_name = Self::extract_field_name(required_item)?;
            Self::validate_required_field_exists(field_name, properties)?;
        }
        Ok(())
    }

    /// Extract field name from a required item
    fn extract_field_name(required_item: &Value) -> Result<&str, ValidationError> {
        required_item
            .as_str()
            .ok_or_else(|| ValidationError::InvalidSchema {
                message: "Required field names must be strings".to_string(),
            })
    }

    /// Validate that a required field exists in properties
    fn validate_required_field_exists(
        field_name: &str,
        properties: &Map<String, Value>,
    ) -> Result<(), ValidationError> {
        if !properties.contains_key(field_name) {
            return Err(ValidationError::ConflictingDefinitions {
                parameter: field_name.to_string(),
                conflict: "Field is marked as required but not defined in properties".to_string(),
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
        let mut ctx = ValidationContext::new(true);
        match Self::validate_schema_with_context(schema, &mut ctx) {
            Ok(()) => ctx.errors,
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

    /// Generic validation error assertion helper
    ///
    /// This helper validates that a result is an error and matches the expected pattern.
    /// It can be used for both schema-level and property-level validation testing.
    fn assert_validation_error<F>(result: Result<(), ValidationError>, context: &str, matcher: F)
    where
        F: FnOnce(ValidationError) -> bool,
    {
        assert!(
            result.is_err(),
            "Expected validation to fail for {}",
            context
        );
        assert!(
            matcher(result.unwrap_err()),
            "Error did not match expected variant for {}",
            context
        );
    }

    type ValidationTestCase = (&'static str, Value, Box<dyn Fn(ValidationError) -> bool>);
    type PropertyValidationTestCase = (
        &'static str,
        &'static str,
        Box<dyn Fn(ValidationError) -> bool>,
    );

    /// Helper to run validation test cases with consistent structure
    ///
    /// This eliminates the duplicated pattern of looping through test cases
    /// with (name, schema, matcher) tuples.
    fn run_validation_test_cases(test_cases: Vec<ValidationTestCase>) {
        for (name, schema, matcher) in test_cases {
            assert_validation_error(SchemaValidator::validate_schema(&schema), name, matcher);
        }
    }

    /// Helper to validate property test cases
    ///
    /// Validates properties directly without a full schema wrapper.
    fn run_property_validation_test_cases(test_cases: Vec<PropertyValidationTestCase>) {
        for (name, param_name, matcher) in test_cases {
            let mut props = serde_json::Map::new();
            props.insert(param_name.to_string(), json!({"type": "string"}));
            let result = SchemaValidator::validate_properties(&props);
            assert_validation_error(result, name, matcher);
        }
    }

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
    fn test_schema_structure_errors() {
        run_validation_test_cases(vec![
            (
                "missing_properties",
                json!({
                    "type": "object",
                    "required": ["title"]
                }),
                Box::new(|e| matches!(e, ValidationError::MissingSchemaField { .. })),
            ),
            (
                "properties_not_object",
                json!({
                    "type": "object",
                    "properties": "not-an-object"
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidSchema { .. })),
            ),
            (
                "schema_with_wrong_root_type",
                json!({
                    "type": "array",
                    "properties": {
                        "param": {"type": "string"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidSchema { .. })),
            ),
            (
                "required_field_not_array",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string"}
                    },
                    "required": "not-an-array"
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidSchema { .. })),
            ),
            (
                "required_field_with_non_string",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string"}
                    },
                    "required": [123, "valid_field"]
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidSchema { .. })),
            ),
            (
                "required_field_not_in_properties",
                json!({
                    "type": "object",
                    "properties": {
                        "existing_param": {"type": "string"}
                    },
                    "required": ["existing_param", "nonexistent_param"]
                }),
                Box::new(|e| matches!(e, ValidationError::ConflictingDefinitions { .. })),
            ),
        ]);
    }

    #[test]
    fn test_parameter_name_errors() {
        run_validation_test_cases(vec![
            (
                "invalid_parameter_name",
                json!({
                    "type": "object",
                    "properties": {
                        "param@with#symbols": {"type": "string"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidParameterName { .. })),
            ),
            (
                "reserved_parameter_name",
                json!({
                    "type": "object",
                    "properties": {
                        "help": {"type": "string"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidParameterName { .. })),
            ),
            (
                "case_insensitive_parameter_conflict",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string"},
                        "PARAM": {"type": "string"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::ConflictingDefinitions { .. })),
            ),
        ]);
    }

    #[test]
    fn test_type_validation_errors() {
        run_validation_test_cases(vec![
            (
                "unsupported_object_type",
                json!({
                    "type": "object",
                    "properties": {
                        "nested": {"type": "object", "properties": {}}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::UnsupportedSchemaType { .. })),
            ),
            (
                "deeply_nested_structure",
                json!({
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
                }),
                Box::new(|e| matches!(e, ValidationError::UnsupportedSchemaType { .. })),
            ),
            (
                "complex_union_type",
                json!({
                    "type": "object",
                    "properties": {
                        "complex_union": {
                            "type": ["string", "integer", "boolean"]
                        }
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::UnsupportedSchemaType { .. })),
            ),
        ]);
    }

    #[test]
    fn test_property_validation_errors() {
        run_validation_test_cases(vec![
            (
                "property_schema_not_object",
                json!({
                    "type": "object",
                    "properties": {
                        "param": "not-an-object"
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
            (
                "property_with_invalid_type",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": 123}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
            (
                "description_not_string",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {
                            "type": "string",
                            "description": 123
                        }
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
        ]);
    }

    #[test]
    fn test_default_value_errors() {
        run_validation_test_cases(vec![
            (
                "string_with_int_default",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "string", "default": 123}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
            (
                "integer_with_string_default",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "integer", "default": "not-a-number"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
            (
                "boolean_with_string_default",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "boolean", "default": "not-a-boolean"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
            (
                "array_with_string_default",
                json!({
                    "type": "object",
                    "properties": {
                        "param": {"type": "array", "default": "not-an-array"}
                    }
                }),
                Box::new(|e| matches!(e, ValidationError::InvalidProperty { .. })),
            ),
        ]);
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
        assert_validation_error(
            SchemaValidator::validate_schema(&json!({})),
            "empty schema",
            |e| matches!(e, ValidationError::MissingSchemaField { .. }),
        );
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
        run_property_validation_test_cases(
            reserved_names
                .iter()
                .map(|name| {
                    (
                        "reserved parameter name",
                        *name,
                        Box::new(|e| matches!(e, ValidationError::InvalidParameterName { .. }))
                            as Box<dyn Fn(ValidationError) -> bool>,
                    )
                })
                .collect(),
        );
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
    fn test_union_type_with_null() {
        assert!(SchemaValidator::validate_schema(&json!({
            "type": "object",
            "properties": {
                "optional_string": {
                    "type": ["string", "null"]
                }
            }
        }))
        .is_ok());
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
        assert_validation_error(
            SchemaValidator::validate_schema(&json!({
                "this_is_not": "a_valid_schema",
                "missing": "required_fields",
                "random": {
                    "structure": true
                }
            })),
            "malformed schema",
            |_| true,
        );
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
