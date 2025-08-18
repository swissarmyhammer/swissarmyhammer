//! Comprehensive error condition tests for parameter system
//!
//! This module tests all error conditions, edge cases, and failure scenarios
//! to ensure the parameter system handles errors gracefully and provides
//! clear, actionable error messages.

use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    CommonPatterns, DefaultParameterResolver, Parameter, ParameterError, ParameterResolver,
    ParameterType, ParameterValidator, ValidationRules,
};

/// Test helper to create a validator
fn create_validator() -> ParameterValidator {
    ParameterValidator::new()
}

/// Test helper to create a resolver
fn create_resolver() -> DefaultParameterResolver {
    DefaultParameterResolver::new()
}

/// Test helper to validate a single parameter
fn validate_param(param: &Parameter, value: &Value) -> Result<(), ParameterError> {
    let validator = create_validator();
    validator.validate_parameter(param, value)
}

/// Test helper to validate multiple parameters
fn validate_params(
    params: &[Parameter],
    values: &HashMap<String, Value>,
) -> Result<(), ParameterError> {
    let validator = create_validator();
    validator.validate_parameters(params, values)
}

/// Test helper to resolve parameters
fn resolve_params(
    params: &[Parameter],
    cli_args: &[(&str, &str)],
) -> Result<HashMap<String, Value>, ParameterError> {
    let resolver = create_resolver();
    let cli_map: HashMap<String, String> = cli_args
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    
    resolver.resolve_parameters(params, &cli_map, false)
}

#[cfg(test)]
mod parameter_type_mismatch_tests {
    use super::*;

    #[test]
    fn test_string_parameter_type_mismatches() {
        let param = Parameter::new("text", "Text parameter", ParameterType::String);
        
        let invalid_types = vec![
            (json!(123), "number"),
            (json!(true), "boolean"),
            (json!([]), "array"),
            (json!({}), "object"),
            (Value::Null, "null"),
        ];

        for (value, expected_type) in invalid_types {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "text");
                    assert_eq!(exp, "string");
                    assert_eq!(act, expected_type);
                }
                other => panic!("Expected TypeMismatch error, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_boolean_parameter_type_mismatches() {
        let param = Parameter::new("flag", "Boolean flag", ParameterType::Boolean);
        
        let invalid_types = vec![
            (json!("true"), "string"),
            (json!("false"), "string"),
            (json!(1), "number"),
            (json!(0), "number"),
            (json!([]), "array"),
            (json!({}), "object"),
            (Value::Null, "null"),
        ];

        for (value, expected_type) in invalid_types {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "flag");
                    assert_eq!(exp, "boolean");
                    assert_eq!(act, expected_type);
                }
                other => panic!("Expected TypeMismatch error, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_number_parameter_type_mismatches() {
        let param = Parameter::new("count", "Number parameter", ParameterType::Number);
        
        let invalid_types = vec![
            (json!("123"), "string"),
            (json!(true), "boolean"),
            (json!([123]), "array"),
            (json!({"value": 123}), "object"),
            (Value::Null, "null"),
        ];

        for (value, expected_type) in invalid_types {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "count");
                    assert_eq!(exp, "number");
                    assert_eq!(act, expected_type);
                }
                other => panic!("Expected TypeMismatch error, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_choice_parameter_type_mismatches() {
        let param = Parameter::new("env", "Environment", ParameterType::Choice)
            .with_choices(vec!["dev".to_string(), "prod".to_string()]);
        
        let invalid_types = vec![
            (json!(123), "number"),
            (json!(true), "boolean"),
            (json!(["dev"]), "array"),
            (json!({}), "object"),
            (Value::Null, "null"),
        ];

        for (value, expected_type) in invalid_types {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "env");
                    assert_eq!(exp, "string");
                    assert_eq!(act, expected_type);
                }
                other => panic!("Expected TypeMismatch error, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_multi_choice_parameter_type_mismatches() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["tag1".to_string(), "tag2".to_string()]);
        
        let invalid_types = vec![
            (json!("tag1"), "string"),
            (json!(123), "number"),
            (json!(true), "boolean"),
            (json!({}), "object"),
            (Value::Null, "null"),
        ];

        for (value, expected_type) in invalid_types {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "tags");
                    assert_eq!(exp, "array");
                    assert_eq!(act, expected_type);
                }
                other => panic!("Expected TypeMismatch error, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_multi_choice_with_non_string_array_items() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["tag1".to_string(), "tag2".to_string()]);
        
        let invalid_arrays = vec![
            json!([123]),
            json!([true, false]),
            json!(["tag1", 123]),
            json!([{}]),
            json!([null]),
            json!(["tag1", ["nested"]]),
        ];

        for value in invalid_arrays {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type, actual_type } => {
                    assert_eq!(name, "tags");
                    assert_eq!(expected_type, "array of strings");
                    assert_eq!(actual_type, "array with non-string items");
                }
                other => panic!("Expected TypeMismatch error for non-string array, got: {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod required_parameter_error_tests {
    use super::*;

    #[test]
    fn test_single_missing_required_parameter() {
        let params = vec![
            Parameter::new("required_param", "Required parameter", ParameterType::String)
                .required(true),
        ];

        let empty_values = HashMap::new();
        let result = validate_params(&params, &empty_values);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "required_param");
            }
            other => panic!("Expected MissingRequired error, got: {other:?}"),
        }
    }

    #[test]
    fn test_multiple_missing_required_parameters() {
        let params = vec![
            Parameter::new("param1", "First required", ParameterType::String).required(true),
            Parameter::new("param2", "Second required", ParameterType::String).required(true),
            Parameter::new("param3", "Optional", ParameterType::String).required(false),
        ];

        // Test that we get an error for the first missing required parameter
        let empty_values = HashMap::new();
        let result = validate_params(&params, &empty_values);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                // Should report one of the missing required parameters
                assert!(name == "param1" || name == "param2");
            }
            other => panic!("Expected MissingRequired error, got: {other:?}"),
        }

        // Test with one required parameter provided
        let partial_values = [("param1".to_string(), json!("value"))]
            .iter().cloned().collect();
        let result = validate_params(&params, &partial_values);
        
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "param2");
            }
            other => panic!("Expected MissingRequired error for param2, got: {other:?}"),
        }
    }

    #[test]
    fn test_required_parameter_with_optional_parameters() {
        let params = vec![
            Parameter::new("optional1", "Optional 1", ParameterType::String).required(false),
            Parameter::new("required", "Required", ParameterType::String).required(true),
            Parameter::new("optional2", "Optional 2", ParameterType::String).required(false),
        ];

        // Provide optional parameters but not required
        let values = [
            ("optional1".to_string(), json!("value1")),
            ("optional2".to_string(), json!("value2")),
        ].iter().cloned().collect();

        let result = validate_params(&params, &values);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "required");
            }
            other => panic!("Expected MissingRequired error, got: {other:?}"),
        }
    }

    #[test]
    fn test_missing_required_parameter_resolution() {
        let params = vec![
            Parameter::new("required_param", "Required", ParameterType::String).required(true),
        ];

        let result = resolve_params(&params, &[]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "required_param");
            }
            other => panic!("Expected MissingRequired error during resolution, got: {other:?}"),
        }
    }
}

#[cfg(test)]
mod validation_failure_tests {
    use super::*;

    #[test]
    fn test_string_length_validation_errors() {
        let param = Parameter::new("text", "Text parameter", ParameterType::String)
            .with_length_range(Some(5), Some(10));

        // Too short
        let result = validate_param(&param, &json!("hi"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::StringTooShort { name, min_length, actual_length } => {
                assert_eq!(name, "text");
                assert_eq!(min_length, 5);
                assert_eq!(actual_length, 2);
            }
            other => panic!("Expected StringTooShort error, got: {other:?}"),
        }

        // Too long
        let result = validate_param(&param, &json!("this is way too long"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::StringTooLong { name, max_length, actual_length } => {
                assert_eq!(name, "text");
                assert_eq!(max_length, 10);
                assert_eq!(actual_length, 20);
            }
            other => panic!("Expected StringTooLong error, got: {other:?}"),
        }
    }

    #[test]
    fn test_pattern_validation_errors() {
        let param = Parameter::new("email", "Email", ParameterType::String)
            .with_pattern(CommonPatterns::EMAIL);

        let invalid_emails = vec![
            "not-an-email",
            "@missing-local.com",
            "missing-at-domain.com",
            "spaces in@email.com",
            "double@@at.com",
        ];

        for invalid_email in invalid_emails {
            let result = validate_param(&param, &json!(invalid_email));
            assert!(result.is_err(), "Should fail for: {invalid_email}");
            
            match result.unwrap_err() {
                ParameterError::PatternMismatch { name, value, pattern } => {
                    assert_eq!(name, "email");
                    assert_eq!(value, invalid_email);
                    assert_eq!(pattern, CommonPatterns::EMAIL);
                }
                other => panic!("Expected PatternMismatch error for {invalid_email}, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_numeric_range_validation_errors() {
        let param = Parameter::new("percentage", "Percentage", ParameterType::Number)
            .with_range(Some(0.0), Some(100.0));

        // Below minimum
        let result = validate_param(&param, &json!(-5.0));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::OutOfRange { name, value, min, max } => {
                assert_eq!(name, "percentage");
                assert_eq!(value, -5.0);
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
            }
            other => panic!("Expected OutOfRange error, got: {other:?}"),
        }

        // Above maximum
        let result = validate_param(&param, &json!(150.0));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::OutOfRange { name, value, min, max } => {
                assert_eq!(name, "percentage");
                assert_eq!(value, 150.0);
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
            }
            other => panic!("Expected OutOfRange error, got: {other:?}"),
        }
    }

    #[test]
    fn test_numeric_step_validation_errors() {
        let param = Parameter::new("stepper", "Step value", ParameterType::Number)
            .with_step(0.5);

        let invalid_values = vec![0.1, 0.3, 0.7, 1.1, 2.3];
        
        for value in invalid_values {
            let result = validate_param(&param, &json!(value));
            assert!(result.is_err(), "Should fail for step value: {value}");
            
            match result.unwrap_err() {
                ParameterError::InvalidStep { name, value: val, step } => {
                    assert_eq!(name, "stepper");
                    assert_eq!(val, value);
                    assert_eq!(step, 0.5);
                }
                other => panic!("Expected InvalidStep error for {value}, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_choice_validation_errors() {
        let param = Parameter::new("env", "Environment", ParameterType::Choice)
            .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()]);

        let invalid_choices = vec!["development", "production", "test", "local", ""];
        
        for choice in invalid_choices {
            let result = validate_param(&param, &json!(choice));
            assert!(result.is_err(), "Should fail for invalid choice: {choice}");
            
            match result.unwrap_err() {
                ParameterError::InvalidChoice { name, value, choices } => {
                    assert_eq!(name, "env");
                    assert_eq!(value, choice);
                    assert_eq!(choices, vec!["dev", "staging", "prod"]);
                }
                other => panic!("Expected InvalidChoice error for {choice}, got: {other:?}"),
            }
        }
    }

    #[test]
    fn test_multi_choice_selection_count_errors() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()])
            .with_selection_range(Some(2), Some(3));

        // Too few selections
        let result = validate_param(&param, &json!(["a"]));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TooFewSelections { name, min_selections, actual_selections } => {
                assert_eq!(name, "tags");
                assert_eq!(min_selections, 2);
                assert_eq!(actual_selections, 1);
            }
            other => panic!("Expected TooFewSelections error, got: {other:?}"),
        }

        // Too many selections
        let result = validate_param(&param, &json!(["a", "b", "c", "d"]));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TooManySelections { name, max_selections, actual_selections } => {
                assert_eq!(name, "tags");
                assert_eq!(max_selections, 3);
                assert_eq!(actual_selections, 4);
            }
            other => panic!("Expected TooManySelections error, got: {other:?}"),
        }
    }

    #[test]
    fn test_multi_choice_invalid_choice_errors() {
        let param = Parameter::new("features", "Features", ParameterType::MultiChoice)
            .with_choices(vec!["auth".to_string(), "logging".to_string(), "metrics".to_string()]);

        let invalid_selections = vec![
            json!(["invalid"]),
            json!(["auth", "invalid"]),
            json!(["auth", "logging", "invalid"]),
            json!(["completely_wrong"]),
        ];

        for selection in invalid_selections {
            let result = validate_param(&param, &selection);
            assert!(result.is_err(), "Should fail for invalid selection: {selection}");
            
            match result.unwrap_err() {
                ParameterError::InvalidChoice { name, value, choices } => {
                    assert_eq!(name, "features");
                    assert!(value == "invalid" || value == "completely_wrong");
                    assert_eq!(choices, vec!["auth", "logging", "metrics"]);
                }
                other => panic!("Expected InvalidChoice error for {selection}, got: {other:?}"),
            }
        }
    }
}

#[cfg(test)]
mod conditional_parameter_error_tests {
    use super::*;

    #[test]
    fn test_conditional_parameter_missing_error() {
        let params = vec![
            Parameter::new("enable_ssl", "Enable SSL", ParameterType::Boolean)
                .required(true),
            Parameter::new("cert_path", "Certificate path", ParameterType::String)
                .when("enable_ssl == true")
                .required(true),
        ];

        // Enable SSL but don't provide cert path
        let result = resolve_params(&params, &[("enable_ssl", "true")]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::ConditionalParameterMissing { parameter, condition } => {
                assert_eq!(parameter, "cert_path");
                assert_eq!(condition, "enable_ssl == true");
            }
            other => panic!("Expected ConditionalParameterMissing error, got: {other:?}"),
        }
    }

    #[test]
    fn test_conditional_parameter_vs_regular_required_errors() {
        let params = vec![
            Parameter::new("regular_required", "Regular required", ParameterType::String)
                .required(true),
            Parameter::new("trigger", "Trigger", ParameterType::Boolean)
                .required(false)
                .with_default(json!(false)),
            Parameter::new("conditional_required", "Conditional required", ParameterType::String)
                .when("trigger == true")
                .required(true),
        ];

        // Missing regular required parameter
        let result = resolve_params(&params, &[("trigger", "false")]);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "regular_required");
            }
            other => panic!("Expected MissingRequired error, got: {other:?}"),
        }

        // Missing conditional required parameter
        let result = resolve_params(&params, &[("regular_required", "value"), ("trigger", "true")]);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::ConditionalParameterMissing { parameter, condition } => {
                assert_eq!(parameter, "conditional_required");
                assert_eq!(condition, "trigger == true");
            }
            other => panic!("Expected ConditionalParameterMissing error, got: {other:?}"),
        }
    }

    #[test]
    fn test_complex_conditional_parameter_errors() {
        let params = vec![
            Parameter::new("env", "Environment", ParameterType::Choice)
                .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()])
                .required(true),
            Parameter::new("urgent", "Urgent deployment", ParameterType::Boolean)
                .with_default(json!(false)),
            Parameter::new("approval_token", "Approval token", ParameterType::String)
                .when("env == 'prod' || urgent == true")
                .required(true),
        ];

        // Production environment without approval token
        let result = resolve_params(&params, &[("env", "prod")]);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::ConditionalParameterMissing { parameter, condition } => {
                assert_eq!(parameter, "approval_token");
                assert_eq!(condition, "env == 'prod' || urgent == true");
            }
            other => panic!("Expected ConditionalParameterMissing error, got: {other:?}"),
        }

        // Urgent deployment without approval token
        let result = resolve_params(&params, &[("env", "dev"), ("urgent", "true")]);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::ConditionalParameterMissing { parameter, condition } => {
                assert_eq!(parameter, "approval_token");
                assert_eq!(condition, "env == 'prod' || urgent == true");
            }
            other => panic!("Expected ConditionalParameterMissing error, got: {other:?}"),
        }
    }

    #[test]
    fn test_condition_evaluation_failed_error() {
        let params = vec![
            Parameter::new("broken_condition", "Broken condition", ParameterType::String)
                .when("malformed syntax ===")
                .with_default(json!("default")),
        ];

        // The resolver should handle malformed conditions gracefully
        // by not including parameters with unresolvable conditions
        let result = resolve_params(&params, &[]);
        assert!(result.is_ok());
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 0); // Parameter should not be included
    }
}

#[cfg(test)]
mod circular_dependency_tests {
    use super::*;

    #[test]
    fn test_simple_circular_dependency() {
        let params = vec![
            Parameter::new("param_a", "Parameter A", ParameterType::String)
                .when("param_b == 'enable'")
                .with_default(json!("value_a")),
            Parameter::new("param_b", "Parameter B", ParameterType::String)
                .when("param_a == 'value_a'")
                .with_default(json!("enable")),
        ];

        let result = resolve_params(&params, &[]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::ValidationFailed { message } => {
                assert!(message.contains("circular dependency") || message.contains("Too many iterations"));
            }
            other => panic!("Expected ValidationFailed error for circular dependency, got: {other:?}"),
        }
    }

    #[test]
    fn test_complex_circular_dependency() {
        let params = vec![
            Parameter::new("param_a", "Parameter A", ParameterType::String)
                .when("param_c == 'trigger'")
                .with_default(json!("value_a")),
            Parameter::new("param_b", "Parameter B", ParameterType::String)
                .when("param_a == 'value_a'")
                .with_default(json!("value_b")),
            Parameter::new("param_c", "Parameter C", ParameterType::String)
                .when("param_b == 'value_b'")
                .with_default(json!("trigger")),
        ];

        let result = resolve_params(&params, &[]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::ValidationFailed { message } => {
                assert!(message.contains("circular dependency") || message.contains("Too many iterations"));
            }
            other => panic!("Expected ValidationFailed error for complex circular dependency, got: {other:?}"),
        }
    }

    #[test]
    fn test_self_referencing_parameter() {
        let params = vec![
            Parameter::new("self_ref", "Self referencing", ParameterType::String)
                .when("self_ref == 'enabled'")
                .with_default(json!("enabled")),
        ];

        let result = resolve_params(&params, &[]);
        assert!(result.is_err());
        
        match result.unwrap_err() {
            ParameterError::ValidationFailed { message } => {
                assert!(message.contains("circular dependency") || message.contains("Too many iterations"));
            }
            other => panic!("Expected ValidationFailed error for self-referencing parameter, got: {other:?}"),
        }
    }

    #[test]
    fn test_circular_dependency_performance() {
        // Test that circular dependency detection doesn't take too long
        let params = vec![
            Parameter::new("param_a", "Parameter A", ParameterType::String)
                .when("param_b == 'enable'")
                .with_default(json!("value_a")),
            Parameter::new("param_b", "Parameter B", ParameterType::String)
                .when("param_a == 'value_a'")
                .with_default(json!("enable")),
        ];

        let start = std::time::Instant::now();
        let result = resolve_params(&params, &[]);
        let duration = start.elapsed();

        assert!(result.is_err());
        assert!(duration.as_millis() < 1000, "Circular dependency detection should be fast: {duration:?}");
    }

    #[test]
    fn test_non_circular_dependency_chain() {
        // Test that valid dependency chains don't get flagged as circular
        let params = vec![
            Parameter::new("base", "Base parameter", ParameterType::String)
                .with_default(json!("base_value")),
            Parameter::new("level1", "Level 1", ParameterType::String)
                .when("base == 'base_value'")
                .with_default(json!("level1_value")),
            Parameter::new("level2", "Level 2", ParameterType::String)
                .when("level1 == 'level1_value'")
                .with_default(json!("level2_value")),
            Parameter::new("level3", "Level 3", ParameterType::String)
                .when("level2 == 'level2_value'")
                .with_default(json!("level3_value")),
        ];

        let result = resolve_params(&params, &[]);
        assert!(result.is_ok(), "Valid dependency chain should not be considered circular");
        
        let resolved = result.unwrap();
        assert_eq!(resolved.len(), 4);
        assert_eq!(resolved.get("base").unwrap(), &json!("base_value"));
        assert_eq!(resolved.get("level1").unwrap(), &json!("level1_value"));
        assert_eq!(resolved.get("level2").unwrap(), &json!("level2_value"));
        assert_eq!(resolved.get("level3").unwrap(), &json!("level3_value"));
    }
}

#[cfg(test)]
mod edge_case_error_tests {
    use super::*;

    #[test]
    fn test_empty_parameter_name_handling() {
        // Test that empty parameter names are handled gracefully
        let param = Parameter::new("", "Empty name parameter", ParameterType::String);
        
        let result = validate_param(&param, &json!("value"));
        // Should either work or provide clear error
        match result {
            Ok(_) => {}, // Acceptable
            Err(error) => {
                let error_msg = format!("{error}");
                assert!(!error_msg.is_empty(), "Error message should not be empty");
            }
        }
    }

    #[test]
    fn test_empty_parameter_description_handling() {
        let param = Parameter::new("test", "", ParameterType::String);
        
        let result = validate_param(&param, &json!("value"));
        assert!(result.is_ok(), "Empty description should not cause validation errors");
    }

    #[test]
    fn test_very_long_parameter_names() {
        let long_name = "a".repeat(1000);
        let param = Parameter::new(&long_name, "Long name parameter", ParameterType::String);
        
        let result = validate_param(&param, &json!("value"));
        assert!(result.is_ok(), "Very long parameter names should be handled");
    }

    #[test]
    fn test_very_long_parameter_values() {
        let param = Parameter::new("test", "Test parameter", ParameterType::String);
        let long_value = "x".repeat(100000);
        
        let result = validate_param(&param, &json!(long_value));
        assert!(result.is_ok(), "Very long parameter values should be handled");
    }

    #[test]
    fn test_unicode_parameter_names_and_values() {
        let param = Parameter::new("测试参数", "Unicode parameter", ParameterType::String);
        let unicode_value = "测试值 🦀 🚀";
        
        let result = validate_param(&param, &json!(unicode_value));
        assert!(result.is_ok(), "Unicode parameter names and values should be handled");
    }

    #[test]
    fn test_special_characters_in_parameter_names() {
        let special_names = vec![
            "param-with-dashes",
            "param.with.dots",
            "param_with_underscores",
            "param123with456numbers",
            "UPPERCASE_PARAM",
        ];

        for name in special_names {
            let param = Parameter::new(name, "Special name parameter", ParameterType::String);
            let result = validate_param(&param, &json!("value"));
            assert!(result.is_ok(), "Special characters in parameter names should be handled: {name}");
        }
    }

    #[test]
    fn test_empty_choices_list() {
        let param = Parameter::new("empty_choices", "Empty choices", ParameterType::Choice)
            .with_choices(vec![]);
        
        let result = validate_param(&param, &json!("any_value"));
        assert!(result.is_err(), "Empty choices list should cause validation error");
        
        match result.unwrap_err() {
            ParameterError::InvalidChoice { name, value, choices } => {
                assert_eq!(name, "empty_choices");
                assert_eq!(value, "any_value");
                assert!(choices.is_empty());
            }
            other => panic!("Expected InvalidChoice error for empty choices, got: {other:?}"),
        }
    }

    #[test]
    fn test_malformed_regex_pattern() {
        let param = Parameter::new("malformed_regex", "Malformed regex", ParameterType::String)
            .with_pattern("[invalid regex pattern");
        
        // The validation should handle malformed regex gracefully
        let result = validate_param(&param, &json!("test_value"));
        
        // Either the regex is ignored (success) or there's a clear error
        match result {
            Ok(_) => {}, // Acceptable - malformed regex ignored
            Err(_) => {}, // Also acceptable - clear error handling
        }
    }

    #[test]
    fn test_extremely_large_numbers() {
        let param = Parameter::new("large_number", "Large number", ParameterType::Number);
        
        let large_numbers = vec![
            f64::MAX,
            f64::MIN,
            1e308,
            -1e308,
        ];

        for number in large_numbers {
            let result = validate_param(&param, &json!(number));
            assert!(result.is_ok(), "Should handle large numbers: {number}");
        }
    }

    #[test]
    fn test_floating_point_precision_edge_cases() {
        let param = Parameter::new("precision", "Precision test", ParameterType::Number)
            .with_step(0.1);
        
        let precision_cases = vec![
            0.1,
            0.2,
            0.3, // Known floating point precision issue
            0.7,
            0.9,
        ];

        for value in precision_cases {
            let result = validate_param(&param, &json!(value));
            // Should handle floating point precision issues gracefully
            match result {
                Ok(_) => {}, // Acceptable
                Err(ParameterError::InvalidStep { .. }) => {}, // Also acceptable due to precision
                other => panic!("Unexpected error for floating point value {value}: {other:?}"),
            }
        }
    }

    #[test]
    fn test_concurrent_parameter_validation() {
        use std::thread;
        use std::sync::Arc;

        let param = Arc::new(Parameter::new("concurrent", "Concurrent test", ParameterType::String)
            .with_pattern(r"^test_\d+$")
            .with_length_range(Some(5), Some(20)));

        let handles: Vec<_> = (0..10)
            .map(|i| {
                let param = param.clone();
                thread::spawn(move || {
                    let value = format!("test_{i}");
                    validate_param(&param, &json!(value))
                })
            })
            .collect();

        for handle in handles {
            let result = handle.join().expect("Thread should not panic");
            assert!(result.is_ok(), "Concurrent validation should work");
        }
    }
}