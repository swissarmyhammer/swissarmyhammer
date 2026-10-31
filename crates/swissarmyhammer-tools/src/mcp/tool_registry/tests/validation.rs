//! Tool validation, schema validation and validation error types.

use super::fixtures::*;
use crate::mcp::tool_registry::validation::SchemaValidator;
use crate::mcp::tool_registry::*;
use swissarmyhammer_common::{ErrorSeverity, Severity};

/// The error `InvalidSchemaTool` produces: its nested `object` property is not
/// supported for CLI generation.
fn invalid_schema_error() -> ToolValidationError {
    ToolValidationError::SchemaValidation {
        tool_name: "invalid_schema_test".to_string(),
        error: ValidationError::UnsupportedSchemaType {
            schema_type: "object".to_string(),
            parameter: "nested".to_string(),
        },
    }
}

/// The error `MissingCategoryTool` produces: visible in the CLI without a category.
fn missing_category_error() -> ToolValidationError {
    ToolValidationError::MissingCliCategory {
        tool_name: "missing_category_test".to_string(),
    }
}

#[test]
fn test_tool_validation_valid_tool() {
    let mut registry = ToolRegistry::new();
    let tool = ValidTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert!(result.is_ok());
}

#[test]
fn test_tool_validation_invalid_schema() {
    let mut registry = ToolRegistry::new();
    let tool = InvalidSchemaTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert_eq!(result, Err(vec![invalid_schema_error()]));
}

#[test]
fn test_tool_validation_missing_category() {
    let mut registry = ToolRegistry::new();
    let tool = MissingCategoryTool;
    registry.register(tool);

    let result = registry.validate_cli_tools();
    assert_eq!(result, Err(vec![missing_category_error()]));
}

#[test]
fn test_tool_validation_report() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);
    registry.register(MissingCategoryTool);

    let report = registry.validate_all_tools();

    assert_eq!(report.total_tools, 3);
    assert_eq!(report.valid_tools, 1);
    assert_eq!(report.invalid_tools, 2);
    assert!(!report.is_valid());
    assert!(!report.errors.is_empty());

    let summary = report.summary();
    assert!(summary.contains("✗"));
    assert!(summary.contains("2 of 3 tools"));
}

#[test]
fn test_tool_validation_warnings() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);

    let warnings = registry.get_tool_validation_warnings();

    // Should have warning for InvalidSchemaTool but not for ValidTool
    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].tool_name, "invalid_schema_test");
}

#[test]
fn test_validation_error_display() {
    let error = ToolValidationError::SchemaValidation {
        tool_name: "test_tool".to_string(),
        error: ValidationError::UnsupportedSchemaType {
            schema_type: "object".to_string(),
            parameter: "test_param".to_string(),
        },
    };

    assert_display_contains(error, &["schema validation failed", "test_tool"]);
}

#[test]
fn test_individual_tool_validation() {
    let registry = ToolRegistry::new();

    assert_eq!(registry.validate_tool(&ValidTool), Ok(()));

    assert_eq!(
        registry.validate_tool(&InvalidSchemaTool),
        Err(vec![invalid_schema_error()])
    );

    assert_eq!(
        registry.validate_tool(&MissingCategoryTool),
        Err(vec![missing_category_error()])
    );
}

/// Assert that the `Display` text of `value` contains every expected fragment.
fn assert_display_contains(value: impl std::fmt::Display, expected: &[&str]) {
    let text = value.to_string();
    for fragment in expected {
        assert!(
            text.contains(fragment),
            "display text {text:?} should contain {fragment:?}"
        );
    }
}

/// Helper function to assert error severity with descriptive message
fn assert_error_severity<E: Severity>(error: E, expected: ErrorSeverity, context: &str) {
    assert_eq!(
        error.severity(),
        expected,
        "{} should be {:?}",
        context,
        expected
    );
}

#[test]
fn test_tool_validation_error_severity() {
    // Test Critical severity for schema validation
    let schema_error = ToolValidationError::SchemaValidation {
        tool_name: "test".to_string(),
        error: ValidationError::InvalidSchema {
            message: "test".to_string(),
        },
    };
    assert_error_severity(schema_error, ErrorSeverity::Critical, "Schema validation");

    // Test Error severity for configuration issues
    let missing_category = ToolValidationError::MissingCliCategory {
        tool_name: "test".to_string(),
    };
    assert_error_severity(
        missing_category,
        ErrorSeverity::Error,
        "Missing CLI category",
    );

    let invalid_cli_name = ToolValidationError::InvalidCliName {
        tool_name: "test".to_string(),
        cli_name: "123invalid".to_string(),
        reason: "starts with number".to_string(),
    };
    assert_error_severity(invalid_cli_name, ErrorSeverity::Error, "Invalid CLI name");

    let invalid_description = ToolValidationError::InvalidDescription {
        tool_name: "test".to_string(),
        reason: "too short".to_string(),
    };
    assert_error_severity(
        invalid_description,
        ErrorSeverity::Error,
        "Invalid description",
    );

    let name_conflict = ToolValidationError::NameConflict {
        tool_name: "test".to_string(),
        conflicting_tool: "other".to_string(),
    };
    assert_error_severity(name_conflict, ErrorSeverity::Error, "Name conflict");
}

#[test]
fn test_validation_error_severity() {
    // Test Critical severity for invalid schema
    let invalid_schema = ValidationError::InvalidSchema {
        message: "schema is malformed".to_string(),
    };
    assert_error_severity(invalid_schema, ErrorSeverity::Critical, "Invalid schema");

    // Test Error severity for schema issues
    let unsupported_type = ValidationError::UnsupportedSchemaType {
        schema_type: "object".to_string(),
        parameter: "param".to_string(),
    };
    assert_error_severity(
        unsupported_type,
        ErrorSeverity::Error,
        "Unsupported schema type",
    );

    let missing_field = ValidationError::MissingSchemaField {
        field: "type".to_string(),
    };
    assert_error_severity(missing_field, ErrorSeverity::Error, "Missing schema field");
}

// --- ValidationError Display tests ---

#[test]
fn test_validation_error_invalid_schema_display() {
    let err = ValidationError::InvalidSchema {
        message: "schema is broken".to_string(),
    };
    assert_display_contains(err, &["invalid schema structure", "schema is broken"]);
}

#[test]
fn test_validation_error_missing_field_display() {
    let err = ValidationError::MissingSchemaField {
        field: "properties".to_string(),
    };
    assert_display_contains(err, &["missing required schema field", "properties"]);
}

#[test]
fn test_validation_error_unsupported_type_display() {
    let err = ValidationError::UnsupportedSchemaType {
        schema_type: "object".to_string(),
        parameter: "nested".to_string(),
    };
    assert_display_contains(err, &["unsupported schema type", "object", "nested"]);
}

// --- ToolValidationError Display tests ---

#[test]
fn test_tool_validation_error_missing_category_display() {
    let err = ToolValidationError::MissingCliCategory {
        tool_name: "my_tool".to_string(),
    };
    assert_display_contains(err, &["my_tool", "missing CLI category"]);
}

#[test]
fn test_tool_validation_error_invalid_cli_name_display() {
    let err = ToolValidationError::InvalidCliName {
        tool_name: "my_tool".to_string(),
        cli_name: "".to_string(),
        reason: "CLI name cannot be empty".to_string(),
    };
    assert_display_contains(err, &["my_tool", "invalid CLI name"]);
}

#[test]
fn test_tool_validation_error_invalid_description_display() {
    let err = ToolValidationError::InvalidDescription {
        tool_name: "my_tool".to_string(),
        reason: "too short".to_string(),
    };
    assert_display_contains(err, &["my_tool", "invalid description"]);
}

#[test]
fn test_tool_validation_error_name_conflict_display() {
    let err = ToolValidationError::NameConflict {
        tool_name: "tool_a".to_string(),
        conflicting_tool: "tool_b".to_string(),
    };
    assert_display_contains(err, &["tool_a", "tool_b", "conflicts"]);
}

// --- ToolValidationReport tests ---

#[test]
fn test_validation_report_valid_summary() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);

    let report = registry.validate_all_tools();
    assert!(report.is_valid());
    let summary = report.summary();
    assert!(summary.contains("All"));
    assert!(summary.contains("1 tools"));
}

// --- SchemaValidator edge cases ---

#[test]
fn test_schema_validator_non_object_schema() {
    let schema = serde_json::json!("not an object");
    let result = SchemaValidator::validate_schema(&schema);
    assert_eq!(
        result,
        Err(ValidationError::InvalidSchema {
            message: "Schema must be a JSON object".to_string(),
        })
    );
}

#[test]
fn test_schema_validator_missing_properties() {
    let schema = serde_json::json!({"type": "object"});
    let result = SchemaValidator::validate_schema(&schema);
    assert_eq!(
        result,
        Err(ValidationError::MissingSchemaField {
            field: "properties".to_string(),
        })
    );
}

#[test]
fn test_schema_validator_supported_types() {
    for type_name in &["string", "integer", "number", "boolean", "array"] {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "param": {"type": type_name}
            }
        });
        let result = SchemaValidator::validate_schema(&schema);
        assert!(result.is_ok(), "type '{}' should be valid", type_name);
    }
}

#[test]
fn test_schema_validator_unsupported_type_null() {
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"type": "null"}
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert_eq!(
        result,
        Err(ValidationError::UnsupportedSchemaType {
            schema_type: "null".to_string(),
            parameter: "param".to_string(),
        })
    );
}

#[test]
fn test_schema_validator_property_without_type() {
    // Properties without a "type" field should pass validation
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": {"description": "no type specified"}
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert!(result.is_ok());
}

#[test]
fn test_schema_validator_property_not_object() {
    // A property value that is not an object schema is a malformed schema
    let schema = serde_json::json!({
        "type": "object",
        "properties": {
            "param": "just a string"
        }
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert_eq!(
        result,
        Err(ValidationError::InvalidSchema {
            message: "property 'param' must be a JSON object schema".to_string(),
        })
    );
}

#[test]
fn test_schema_validator_properties_not_object() {
    // properties field that isn't an object
    let schema = serde_json::json!({
        "type": "object",
        "properties": "not an object"
    });
    let result = SchemaValidator::validate_schema(&schema);
    assert_eq!(
        result,
        Err(ValidationError::InvalidSchema {
            message: "'properties' must be a JSON object".to_string(),
        })
    );
}

// --- ToolValidationSeverity tests ---

#[test]
fn test_validation_severity_levels_are_distinct() {
    let levels = [
        ToolValidationSeverity::Warning,
        ToolValidationSeverity::Error,
        ToolValidationSeverity::Critical,
    ];
    let distinct: std::collections::HashSet<_> = levels.into_iter().collect();
    assert_eq!(distinct.len(), levels.len());
}

// --- ToolValidationWarning tests ---

#[test]
fn test_validation_warning_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(MissingCategoryTool);

    let warnings = registry.get_tool_validation_warnings();

    assert_eq!(warnings.len(), 1);
    assert_eq!(warnings[0].tool_name, "missing_category_test");
    assert_eq!(warnings[0].severity, ToolValidationSeverity::Warning);
    assert_eq!(warnings[0].error, missing_category_error());
}

// --- Error source chain and derived trait tests ---

#[test]
fn test_tool_validation_error_source_exposes_schema_error() {
    use std::error::Error;

    let inner = ValidationError::InvalidSchema {
        message: "schema is broken".to_string(),
    };
    let err = ToolValidationError::SchemaValidation {
        tool_name: "my_tool".to_string(),
        error: inner.clone(),
    };

    let source = err.source().expect("SchemaValidation wraps a source error");
    assert_eq!(source.to_string(), inner.to_string());
}

#[test]
fn test_tool_validation_error_source_is_none_without_wrapped_error() {
    use std::error::Error;

    let errors = [
        ToolValidationError::MissingCliCategory {
            tool_name: "my_tool".to_string(),
        },
        ToolValidationError::InvalidCliName {
            tool_name: "my_tool".to_string(),
            cli_name: "".to_string(),
            reason: "empty".to_string(),
        },
        ToolValidationError::InvalidDescription {
            tool_name: "my_tool".to_string(),
            reason: "too short".to_string(),
        },
        ToolValidationError::NameConflict {
            tool_name: "tool_a".to_string(),
            conflicting_tool: "tool_b".to_string(),
        },
    ];

    for err in &errors {
        assert!(err.source().is_none(), "{err} must not report a source");
    }
}

#[test]
fn test_validation_warnings_match_independently_built_expectation() {
    let mut registry = ToolRegistry::new();
    registry.register(InvalidSchemaTool);

    assert_eq!(
        registry.get_tool_validation_warnings(),
        vec![ToolValidationWarning {
            tool_name: "invalid_schema_test".to_string(),
            error: invalid_schema_error(),
            severity: ToolValidationSeverity::Warning,
        }]
    );
}

#[test]
fn test_validation_severity_usable_as_map_key() {
    let mut counts = std::collections::HashMap::new();
    *counts.entry(ToolValidationSeverity::Error).or_insert(0) += 1;
    *counts.entry(ToolValidationSeverity::Error).or_insert(0) += 1;
    *counts.entry(ToolValidationSeverity::Warning).or_insert(0) += 1;

    assert_eq!(counts[&ToolValidationSeverity::Error], 2);
    assert_eq!(counts[&ToolValidationSeverity::Warning], 1);
}

#[test]
fn test_validation_report_clone_copies_all_fields() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);
    let report = registry.validate_all_tools();

    let copy = report.clone();

    assert_eq!(copy.total_tools, report.total_tools);
    assert_eq!(copy.valid_tools, report.valid_tools);
    assert_eq!(copy.invalid_tools, report.invalid_tools);
    assert_eq!(copy.errors, report.errors);
}

#[test]
fn test_validation_severity_orders_warning_below_error_below_critical() {
    assert!(ToolValidationSeverity::Warning < ToolValidationSeverity::Error);
    assert!(ToolValidationSeverity::Error < ToolValidationSeverity::Critical);
    assert_eq!(
        [
            ToolValidationSeverity::Warning,
            ToolValidationSeverity::Critical,
            ToolValidationSeverity::Error,
        ]
        .into_iter()
        .max(),
        Some(ToolValidationSeverity::Critical)
    );
}

#[test]
fn test_validation_report_matches_independently_built_expectation() {
    let mut registry = ToolRegistry::new();
    registry.register(ValidTool);
    registry.register(InvalidSchemaTool);

    assert_eq!(
        registry.validate_all_tools(),
        ToolValidationReport {
            total_tools: 2,
            valid_tools: 1,
            invalid_tools: 1,
            errors: vec![invalid_schema_error()],
        }
    );
}

#[test]
fn test_tool_validation_error_deduplicates_in_hash_set() {
    let missing = |tool_name: &str| ToolValidationError::MissingCliCategory {
        tool_name: tool_name.to_string(),
    };
    let unique: std::collections::HashSet<ToolValidationError> =
        [missing("tool_a"), missing("tool_a"), missing("tool_b")]
            .into_iter()
            .collect();
    assert_eq!(unique.len(), 2);
}
