//! Comprehensive unit tests for parameter validation system
//! 
//! This module provides exhaustive testing of all parameter types, validation rules,
//! and error conditions to ensure the parameter system works correctly across all
//! supported scenarios and edge cases.

use serde_json::{json, Value};
use std::collections::HashMap;
use swissarmyhammer::common::parameters::{
    CommonPatterns, Parameter, ParameterError, ParameterResult, ParameterType, ParameterValidator,
    ValidationRules,
};

/// Test helper to create a parameter validator
fn create_validator() -> ParameterValidator {
    ParameterValidator::new()
}

/// Test helper to validate a parameter with a value
fn validate_param(param: &Parameter, value: &Value) -> ParameterResult<()> {
    let validator = create_validator();
    validator.validate_parameter(param, value)
}

/// Test helper to validate multiple parameters
fn validate_params(
    params: &[Parameter],
    values: &HashMap<String, Value>,
) -> ParameterResult<()> {
    let validator = create_validator();
    validator.validate_parameters(params, values)
}

#[cfg(test)]
mod string_parameter_tests {
    use super::*;

    #[test]
    fn test_string_parameter_basic_validation() {
        let param = Parameter::new("text", "Text parameter", ParameterType::String);
        
        // Valid string
        assert!(validate_param(&param, &json!("hello world")).is_ok());
        
        // Invalid types
        assert!(validate_param(&param, &json!(123)).is_err());
        assert!(validate_param(&param, &json!(true)).is_err());
        assert!(validate_param(&param, &json!([])).is_err());
        assert!(validate_param(&param, &json!({})).is_err());
        assert!(validate_param(&param, &Value::Null).is_err());
    }

    #[test]
    fn test_string_length_validation_comprehensive() {
        let param = Parameter::new("text", "Text parameter", ParameterType::String)
            .with_length_range(Some(5), Some(10));

        // Exactly minimum length
        assert!(validate_param(&param, &json!("12345")).is_ok());
        
        // Exactly maximum length
        assert!(validate_param(&param, &json!("1234567890")).is_ok());
        
        // Within range
        assert!(validate_param(&param, &json!("1234567")).is_ok());
        
        // Below minimum
        let result = validate_param(&param, &json!("1234"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::StringTooShort { name, min_length, actual_length } => {
                assert_eq!(name, "text");
                assert_eq!(min_length, 5);
                assert_eq!(actual_length, 4);
            }
            _ => panic!("Expected StringTooShort error"),
        }
        
        // Above maximum
        let result = validate_param(&param, &json!("12345678901"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::StringTooLong { name, max_length, actual_length } => {
                assert_eq!(name, "text");
                assert_eq!(max_length, 10);
                assert_eq!(actual_length, 11);
            }
            _ => panic!("Expected StringTooLong error"),
        }
    }

    #[test]
    fn test_string_length_validation_unicode() {
        let param = Parameter::new("unicode", "Unicode text", ParameterType::String)
            .with_length_range(Some(3), Some(6));

        // Unicode characters should be counted properly
        assert!(validate_param(&param, &json!("你好世界")).is_ok()); // 4 Chinese characters
        assert!(validate_param(&param, &json!("😀🎉🚀")).is_ok()); // 3 emoji
        assert!(validate_param(&param, &json!("café")).is_ok()); // 4 characters with accent
        
        // Complex emoji sequences
        assert!(validate_param(&param, &json!("👨‍👩‍👧‍👦")).is_ok()); // Family emoji (counts as 1)
        
        // Mixed Unicode
        assert!(validate_param(&param, &json!("test😀")).is_ok()); // 5 characters total
    }

    #[test]
    fn test_string_length_validation_edge_cases() {
        // Only minimum length specified
        let min_only = Parameter::new("min_only", "Min only", ParameterType::String)
            .with_length_range(Some(3), None);
        
        assert!(validate_param(&min_only, &json!("ab")).is_err());
        assert!(validate_param(&min_only, &json!("abc")).is_ok());
        assert!(validate_param(&min_only, &json!("very long string")).is_ok());
        
        // Only maximum length specified  
        let max_only = Parameter::new("max_only", "Max only", ParameterType::String)
            .with_length_range(None, Some(5));
        
        assert!(validate_param(&max_only, &json!("")).is_ok());
        assert!(validate_param(&max_only, &json!("hello")).is_ok());
        assert!(validate_param(&max_only, &json!("hello world")).is_err());
        
        // Zero length allowed
        let zero_ok = Parameter::new("zero_ok", "Zero OK", ParameterType::String)
            .with_length_range(Some(0), Some(3));
        
        assert!(validate_param(&zero_ok, &json!("")).is_ok());
        assert!(validate_param(&zero_ok, &json!("a")).is_ok());
    }

    #[test]
    fn test_string_pattern_validation_comprehensive() {
        // Email validation
        let email_param = Parameter::new("email", "Email", ParameterType::String)
            .with_pattern(CommonPatterns::EMAIL);

        // Valid emails
        let valid_emails = vec![
            "test@example.com",
            "user.name@domain.org",
            "user+tag@example.co.uk",
            "firstname-lastname@example.com",
            "123@numbers.com",
            "test@sub.domain.com",
        ];

        for email in valid_emails {
            assert!(
                validate_param(&email_param, &json!(email)).is_ok(),
                "Email should be valid: {email}"
            );
        }

        // Invalid emails
        let invalid_emails = vec![
            "not-an-email",
            "@example.com",
            "user@",
            "user name@example.com", // space in local part
            "user@domain",           // no TLD
            "user@@domain.com",      // double @
            "",                      // empty string
        ];

        for email in invalid_emails {
            let result = validate_param(&email_param, &json!(email));
            assert!(
                result.is_err(),
                "Email should be invalid: {email}"
            );
            match result.unwrap_err() {
                ParameterError::PatternMismatch { name, value, pattern } => {
                    assert_eq!(name, "email");
                    assert_eq!(value, email);
                    assert_eq!(pattern, CommonPatterns::EMAIL);
                }
                _ => panic!("Expected PatternMismatch error for {email}"),
            }
        }
    }

    #[test]
    fn test_string_pattern_validation_common_patterns() {
        // URL validation
        let url_param = Parameter::new("url", "URL", ParameterType::String)
            .with_pattern(CommonPatterns::URL);

        assert!(validate_param(&url_param, &json!("https://example.com")).is_ok());
        assert!(validate_param(&url_param, &json!("http://test.org/path")).is_ok());
        assert!(validate_param(&url_param, &json!("https://api.example.com/v1/users?id=123")).is_ok());
        
        assert!(validate_param(&url_param, &json!("ftp://example.com")).is_err());
        assert!(validate_param(&url_param, &json!("not-a-url")).is_err());

        // UUID validation
        let uuid_param = Parameter::new("uuid", "UUID", ParameterType::String)
            .with_pattern(CommonPatterns::UUID);

        assert!(validate_param(&uuid_param, &json!("550e8400-e29b-41d4-a716-446655440000")).is_ok());
        assert!(validate_param(&uuid_param, &json!("6ba7b810-9dad-11d1-80b4-00c04fd430c8")).is_ok());
        
        assert!(validate_param(&uuid_param, &json!("550e8400-e29b-41d4-a716-44665544000")).is_err()); // too short
        assert!(validate_param(&uuid_param, &json!("not-a-uuid")).is_err());

        // ULID validation
        let ulid_param = Parameter::new("ulid", "ULID", ParameterType::String)
            .with_pattern(CommonPatterns::ULID);

        assert!(validate_param(&ulid_param, &json!("01ARZ3NDEKTSV4RRFFQ69G5FAV")).is_ok());
        assert!(validate_param(&ulid_param, &json!("01BX5ZZKBKACTAV9WEVGEMMVRY")).is_ok());
        
        assert!(validate_param(&ulid_param, &json!("01ARZ3NDEKTSV4RRFFQ69G5FA")).is_err()); // too short
        assert!(validate_param(&ulid_param, &json!("not-a-ulid")).is_err());
    }

    #[test]
    fn test_string_pattern_validation_custom_patterns() {
        // Custom pattern: alphanumeric only
        let alphanum_param = Parameter::new("alphanum", "Alphanumeric", ParameterType::String)
            .with_pattern(r"^[a-zA-Z0-9]+$");

        assert!(validate_param(&alphanum_param, &json!("abc123")).is_ok());
        assert!(validate_param(&alphanum_param, &json!("ABC")).is_ok());
        assert!(validate_param(&alphanum_param, &json!("123")).is_ok());
        
        assert!(validate_param(&alphanum_param, &json!("abc-123")).is_err()); // hyphen not allowed
        assert!(validate_param(&alphanum_param, &json!("abc 123")).is_err()); // space not allowed
        assert!(validate_param(&alphanum_param, &json!("")).is_err()); // empty not allowed

        // Custom pattern: version number
        let version_param = Parameter::new("version", "Version", ParameterType::String)
            .with_pattern(r"^\d+\.\d+\.\d+(-[a-zA-Z0-9]+)?$");

        assert!(validate_param(&version_param, &json!("1.2.3")).is_ok());
        assert!(validate_param(&version_param, &json!("10.20.30")).is_ok());
        assert!(validate_param(&version_param, &json!("1.0.0-beta")).is_ok());
        assert!(validate_param(&version_param, &json!("2.1.0-rc1")).is_ok());
        
        assert!(validate_param(&version_param, &json!("1.2")).is_err()); // missing patch
        assert!(validate_param(&version_param, &json!("v1.2.3")).is_err()); // v prefix not allowed
        assert!(validate_param(&version_param, &json!("1.2.3-")).is_err()); // empty suffix
    }

    #[test]
    fn test_string_validation_combined_rules() {
        // Combine pattern and length validation
        let strong_password = Parameter::new("password", "Strong password", ParameterType::String)
            .with_length_range(Some(8), Some(128))
            .with_pattern(r"^(?=.*[a-z])(?=.*[A-Z])(?=.*\d)(?=.*[@$!%*?&])[A-Za-z\d@$!%*?&]+$");

        // Valid strong passwords
        assert!(validate_param(&strong_password, &json!("MyPassword123!")).is_ok());
        assert!(validate_param(&strong_password, &json!("Secure$Pass1")).is_ok());
        assert!(validate_param(&strong_password, &json!("Complex@123")).is_ok());
        
        // Too short (fails length validation first)
        let result = validate_param(&strong_password, &json!("Pass1!"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::StringTooShort { .. } => (),
            _ => panic!("Expected StringTooShort error for short password"),
        }
        
        // Correct length but missing requirements (fails pattern validation)
        let result = validate_param(&strong_password, &json!("mypassword123"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::PatternMismatch { .. } => (),
            _ => panic!("Expected PatternMismatch error for weak password"),
        }
    }
}

#[cfg(test)]
mod number_parameter_tests {
    use super::*;

    #[test]
    fn test_number_parameter_basic_validation() {
        let param = Parameter::new("count", "Count", ParameterType::Number);
        
        // Valid numbers
        assert!(validate_param(&param, &json!(42)).is_ok());
        assert!(validate_param(&param, &json!(42.5)).is_ok());
        assert!(validate_param(&param, &json!(0)).is_ok());
        assert!(validate_param(&param, &json!(-10)).is_ok());
        assert!(validate_param(&param, &json!(0.001)).is_ok());
        
        // Invalid types
        assert!(validate_param(&param, &json!("123")).is_err());
        assert!(validate_param(&param, &json!(true)).is_err());
        assert!(validate_param(&param, &json!([])).is_err());
        assert!(validate_param(&param, &json!({})).is_err());
        assert!(validate_param(&param, &Value::Null).is_err());
    }

    #[test]
    fn test_number_range_validation_comprehensive() {
        let param = Parameter::new("percentage", "Percentage", ParameterType::Number)
            .with_range(Some(0.0), Some(100.0));

        // Within range
        assert!(validate_param(&param, &json!(0.0)).is_ok());
        assert!(validate_param(&param, &json!(50.0)).is_ok());
        assert!(validate_param(&param, &json!(100.0)).is_ok());
        assert!(validate_param(&param, &json!(99.99)).is_ok());
        assert!(validate_param(&param, &json!(0.01)).is_ok());
        
        // Below minimum
        let result = validate_param(&param, &json!(-0.1));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::OutOfRange { name, value, min, max } => {
                assert_eq!(name, "percentage");
                assert_eq!(value, -0.1);
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
            }
            _ => panic!("Expected OutOfRange error"),
        }
        
        // Above maximum
        let result = validate_param(&param, &json!(100.1));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::OutOfRange { name, value, min, max } => {
                assert_eq!(name, "percentage");
                assert_eq!(value, 100.1);
                assert_eq!(min, Some(0.0));
                assert_eq!(max, Some(100.0));
            }
            _ => panic!("Expected OutOfRange error"),
        }
    }

    #[test]
    fn test_number_range_validation_edge_cases() {
        // Only minimum specified
        let min_only = Parameter::new("min_only", "Min only", ParameterType::Number)
            .with_range(Some(10.0), None);
        
        assert!(validate_param(&min_only, &json!(9.9)).is_err());
        assert!(validate_param(&min_only, &json!(10.0)).is_ok());
        assert!(validate_param(&min_only, &json!(1000000.0)).is_ok());
        
        // Only maximum specified
        let max_only = Parameter::new("max_only", "Max only", ParameterType::Number)
            .with_range(None, Some(100.0));
        
        assert!(validate_param(&max_only, &json!(-1000000.0)).is_ok());
        assert!(validate_param(&max_only, &json!(100.0)).is_ok());
        assert!(validate_param(&max_only, &json!(100.1)).is_err());
        
        // Very large numbers
        let large_range = Parameter::new("large", "Large numbers", ParameterType::Number)
            .with_range(Some(f64::MIN / 2.0), Some(f64::MAX / 2.0));
        
        assert!(validate_param(&large_range, &json!(1e10)).is_ok());
        assert!(validate_param(&large_range, &json!(-1e10)).is_ok());
    }

    #[test]
    fn test_number_step_validation_comprehensive() {
        let param = Parameter::new("stepper", "Step value", ParameterType::Number)
            .with_step(0.5);

        // Valid multiples of step
        assert!(validate_param(&param, &json!(0.0)).is_ok());
        assert!(validate_param(&param, &json!(0.5)).is_ok());
        assert!(validate_param(&param, &json!(1.0)).is_ok());
        assert!(validate_param(&param, &json!(2.5)).is_ok());
        assert!(validate_param(&param, &json!(-1.5)).is_ok());
        assert!(validate_param(&param, &json!(10.0)).is_ok());
        
        // Invalid steps
        let invalid_values = vec![0.1, 0.3, 0.7, 1.1, 2.3, -0.3];
        
        for value in invalid_values {
            let result = validate_param(&param, &json!(value));
            assert!(result.is_err(), "Value {value} should fail step validation");
            match result.unwrap_err() {
                ParameterError::InvalidStep { name, value: val, step } => {
                    assert_eq!(name, "stepper");
                    assert_eq!(val, value);
                    assert_eq!(step, 0.5);
                }
                _ => panic!("Expected InvalidStep error for value {value}"),
            }
        }
    }

    #[test]
    fn test_number_step_validation_edge_cases() {
        // Integer steps
        let int_step = Parameter::new("int_step", "Integer step", ParameterType::Number)
            .with_step(5.0);
        
        assert!(validate_param(&int_step, &json!(0)).is_ok());
        assert!(validate_param(&int_step, &json!(5)).is_ok());
        assert!(validate_param(&int_step, &json!(10)).is_ok());
        assert!(validate_param(&int_step, &json!(-15)).is_ok());
        
        assert!(validate_param(&int_step, &json!(3)).is_err());
        assert!(validate_param(&int_step, &json!(7)).is_err());
        
        // Very small steps
        let small_step = Parameter::new("small_step", "Small step", ParameterType::Number)
            .with_step(0.001);
        
        assert!(validate_param(&small_step, &json!(0.000)).is_ok());
        assert!(validate_param(&small_step, &json!(0.001)).is_ok());
        assert!(validate_param(&small_step, &json!(0.002)).is_ok());
        assert!(validate_param(&small_step, &json!(1.234)).is_ok());
        
        // Due to floating point precision, some values close to valid steps might fail
        // This tests the epsilon handling in step validation
        assert!(validate_param(&small_step, &json!(0.0005)).is_err());
    }

    #[test]
    fn test_number_validation_combined_rules() {
        // Combine range and step validation
        let param = Parameter::new("range_step", "Range with step", ParameterType::Number)
            .with_range(Some(0.0), Some(10.0))
            .with_step(0.5);

        // Valid: within range and correct step
        assert!(validate_param(&param, &json!(0.0)).is_ok());
        assert!(validate_param(&param, &json!(0.5)).is_ok());
        assert!(validate_param(&param, &json!(5.0)).is_ok());
        assert!(validate_param(&param, &json!(10.0)).is_ok());
        
        // Invalid: correct step but out of range
        let result = validate_param(&param, &json!(10.5));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::OutOfRange { .. } => (),
            _ => panic!("Expected OutOfRange error for value outside range"),
        }
        
        // Invalid: within range but wrong step
        let result = validate_param(&param, &json!(0.3));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::InvalidStep { .. } => (),
            _ => panic!("Expected InvalidStep error for wrong step value"),
        }
    }
}

#[cfg(test)]
mod boolean_parameter_tests {
    use super::*;

    #[test]
    fn test_boolean_parameter_basic_validation() {
        let param = Parameter::new("enabled", "Enabled flag", ParameterType::Boolean);
        
        // Valid booleans
        assert!(validate_param(&param, &json!(true)).is_ok());
        assert!(validate_param(&param, &json!(false)).is_ok());
        
        // Invalid types
        assert!(validate_param(&param, &json!("true")).is_err());
        assert!(validate_param(&param, &json!("false")).is_err());
        assert!(validate_param(&param, &json!(1)).is_err());
        assert!(validate_param(&param, &json!(0)).is_err());
        assert!(validate_param(&param, &json!([])).is_err());
        assert!(validate_param(&param, &json!({})).is_err());
        assert!(validate_param(&param, &Value::Null).is_err());
    }

    #[test]
    fn test_boolean_parameter_type_mismatch_errors() {
        let param = Parameter::new("flag", "Flag", ParameterType::Boolean);
        
        let test_cases = vec![
            (json!("true"), "string"),
            (json!(1), "number"),
            (json!([]), "array"),
            (json!({}), "object"),
            (Value::Null, "null"),
        ];
        
        for (value, expected_type) in test_cases {
            let result = validate_param(&param, &value);
            assert!(result.is_err());
            match result.unwrap_err() {
                ParameterError::TypeMismatch { name, expected_type: exp, actual_type: act } => {
                    assert_eq!(name, "flag");
                    assert_eq!(exp, "boolean");
                    assert_eq!(act, expected_type);
                }
                _ => panic!("Expected TypeMismatch error for {value}"),
            }
        }
    }
}

#[cfg(test)]
mod choice_parameter_tests {
    use super::*;

    #[test]
    fn test_choice_parameter_basic_validation() {
        let param = Parameter::new("env", "Environment", ParameterType::Choice)
            .with_choices(vec!["dev".to_string(), "staging".to_string(), "prod".to_string()]);
        
        // Valid choices
        assert!(validate_param(&param, &json!("dev")).is_ok());
        assert!(validate_param(&param, &json!("staging")).is_ok());
        assert!(validate_param(&param, &json!("prod")).is_ok());
        
        // Invalid choice
        let result = validate_param(&param, &json!("invalid"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::InvalidChoice { name, value, choices } => {
                assert_eq!(name, "env");
                assert_eq!(value, "invalid");
                assert_eq!(choices, vec!["dev", "staging", "prod"]);
            }
            _ => panic!("Expected InvalidChoice error"),
        }
        
        // Wrong type
        let result = validate_param(&param, &json!(123));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TypeMismatch { name, expected_type, actual_type } => {
                assert_eq!(name, "env");
                assert_eq!(expected_type, "string");
                assert_eq!(actual_type, "number");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }

    #[test]
    fn test_choice_parameter_edge_cases() {
        // Empty choices list
        let empty_choices = Parameter::new("empty", "Empty choices", ParameterType::Choice)
            .with_choices(vec![]);
        
        // Any value should be invalid
        assert!(validate_param(&empty_choices, &json!("any")).is_err());
        
        // Single choice
        let single_choice = Parameter::new("single", "Single choice", ParameterType::Choice)
            .with_choices(vec!["only".to_string()]);
        
        assert!(validate_param(&single_choice, &json!("only")).is_ok());
        assert!(validate_param(&single_choice, &json!("other")).is_err());
        
        // Choices with special characters
        let special_choices = Parameter::new("special", "Special choices", ParameterType::Choice)
            .with_choices(vec![
                "choice with spaces".to_string(),
                "choice-with-dashes".to_string(),
                "choice_with_underscores".to_string(),
                "choice.with.dots".to_string(),
                "choice/with/slashes".to_string(),
                "UPPERCASE".to_string(),
                "123numbers".to_string(),
                "special@chars!".to_string(),
            ]);
        
        // All special choices should be valid
        assert!(validate_param(&special_choices, &json!("choice with spaces")).is_ok());
        assert!(validate_param(&special_choices, &json!("choice-with-dashes")).is_ok());
        assert!(validate_param(&special_choices, &json!("choice_with_underscores")).is_ok());
        assert!(validate_param(&special_choices, &json!("choice.with.dots")).is_ok());
        assert!(validate_param(&special_choices, &json!("choice/with/slashes")).is_ok());
        assert!(validate_param(&special_choices, &json!("UPPERCASE")).is_ok());
        assert!(validate_param(&special_choices, &json!("123numbers")).is_ok());
        assert!(validate_param(&special_choices, &json!("special@chars!")).is_ok());
        
        // Case sensitivity
        assert!(validate_param(&special_choices, &json!("uppercase")).is_err());
    }

    #[test]
    fn test_choice_parameter_no_choices_list() {
        // Parameter with no choices list should not validate choices
        let param = Parameter::new("no_choices", "No choices", ParameterType::Choice);
        
        // Since there's no choices list, any string should be valid for type checking
        assert!(validate_param(&param, &json!("any_value")).is_ok());
        assert!(validate_param(&param, &json!("another_value")).is_ok());
        
        // But wrong types should still fail
        assert!(validate_param(&param, &json!(123)).is_err());
        assert!(validate_param(&param, &json!(true)).is_err());
    }
}

#[cfg(test)]
mod multi_choice_parameter_tests {
    use super::*;

    #[test]
    fn test_multi_choice_parameter_basic_validation() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["frontend".to_string(), "backend".to_string(), "database".to_string(), "testing".to_string()]);
        
        // Valid multi-choice arrays
        assert!(validate_param(&param, &json!(["frontend"])).is_ok());
        assert!(validate_param(&param, &json!(["frontend", "backend"])).is_ok());
        assert!(validate_param(&param, &json!(["frontend", "backend", "database"])).is_ok());
        assert!(validate_param(&param, &json!([])).is_ok()); // Empty array is valid
        
        // Wrong type (not array)
        let result = validate_param(&param, &json!("frontend"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TypeMismatch { name, expected_type, actual_type } => {
                assert_eq!(name, "tags");
                assert_eq!(expected_type, "array");
                assert_eq!(actual_type, "string");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
        
        // Invalid choice in array
        let result = validate_param(&param, &json!(["frontend", "invalid"]));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::InvalidChoice { name, value, choices } => {
                assert_eq!(name, "tags");
                assert_eq!(value, "invalid");
                assert!(choices.contains(&"frontend".to_string()));
            }
            _ => panic!("Expected InvalidChoice error"),
        }
        
        // Non-string item in array
        let result = validate_param(&param, &json!(["frontend", 123]));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TypeMismatch { name, expected_type, actual_type } => {
                assert_eq!(name, "tags");
                assert_eq!(expected_type, "array of strings");
                assert_eq!(actual_type, "array with non-string items");
            }
            _ => panic!("Expected TypeMismatch error"),
        }
    }

    #[test]
    fn test_multi_choice_selection_count_validation() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string(), "c".to_string(), "d".to_string()])
            .with_selection_range(Some(2), Some(3));

        // Valid selection counts
        assert!(validate_param(&param, &json!(["a", "b"])).is_ok()); // exactly min
        assert!(validate_param(&param, &json!(["a", "b", "c"])).is_ok()); // exactly max
        
        // Too few selections
        let result = validate_param(&param, &json!(["a"]));
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::TooFewSelections { name, min_selections, actual_selections } => {
                assert_eq!(name, "tags");
                assert_eq!(min_selections, 2);
                assert_eq!(actual_selections, 1);
            }
            _ => panic!("Expected TooFewSelections error"),
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
            _ => panic!("Expected TooManySelections error"),
        }
    }

    #[test]
    fn test_multi_choice_selection_count_edge_cases() {
        // Only minimum selections specified
        let min_only = Parameter::new("min_only", "Min only", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .with_selection_range(Some(2), None);
        
        assert!(validate_param(&min_only, &json!(["a"])).is_err());
        assert!(validate_param(&min_only, &json!(["a", "b"])).is_ok());
        assert!(validate_param(&min_only, &json!(["a", "b", "c"])).is_ok());
        
        // Only maximum selections specified
        let max_only = Parameter::new("max_only", "Max only", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string(), "c".to_string()])
            .with_selection_range(None, Some(2));
        
        assert!(validate_param(&max_only, &json!([])).is_ok());
        assert!(validate_param(&max_only, &json!(["a"])).is_ok());
        assert!(validate_param(&max_only, &json!(["a", "b"])).is_ok());
        assert!(validate_param(&max_only, &json!(["a", "b", "c"])).is_err());
        
        // Zero minimum allowed
        let zero_min = Parameter::new("zero_min", "Zero min", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string()])
            .with_selection_range(Some(0), Some(1));
        
        assert!(validate_param(&zero_min, &json!([])).is_ok());
        assert!(validate_param(&zero_min, &json!(["a"])).is_ok());
        assert!(validate_param(&zero_min, &json!(["a", "b"])).is_err());
    }

    #[test]
    fn test_multi_choice_duplicate_selections() {
        let param = Parameter::new("tags", "Tags", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string(), "c".to_string()]);
        
        // Duplicate selections should be treated as separate items for count
        assert!(validate_param(&param, &json!(["a", "a"])).is_ok());
        assert!(validate_param(&param, &json!(["a", "b", "a"])).is_ok());
        
        // With selection count constraints, duplicates count toward the total
        let constrained = Parameter::new("constrained", "Constrained", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string(), "b".to_string()])
            .with_selection_range(Some(1), Some(2));
        
        assert!(validate_param(&constrained, &json!(["a", "a"])).is_ok()); // 2 items, within limit
        
        let strict_constrained = Parameter::new("strict", "Strict", ParameterType::MultiChoice)
            .with_choices(vec!["a".to_string()])
            .with_selection_range(Some(1), Some(1));
        
        assert!(validate_param(&strict_constrained, &json!(["a", "a"])).is_err()); // 2 items, over limit
    }
}

#[cfg(test)]
mod parameter_validation_integration_tests {
    use super::*;

    #[test]
    fn test_validate_multiple_parameters_success() {
        let params = vec![
            Parameter::new("name", "Name", ParameterType::String)
                .required(true)
                .with_length_range(Some(2), Some(50)),
            Parameter::new("age", "Age", ParameterType::Number)
                .required(true)
                .with_range(Some(0.0), Some(150.0)),
            Parameter::new("active", "Active", ParameterType::Boolean)
                .required(false),
            Parameter::new("role", "Role", ParameterType::Choice)
                .with_choices(vec!["admin".to_string(), "user".to_string(), "guest".to_string()]),
        ];

        let values = [
            ("name".to_string(), json!("John Doe")),
            ("age".to_string(), json!(30)),
            ("active".to_string(), json!(true)),
            ("role".to_string(), json!("user")),
        ].iter().cloned().collect();

        assert!(validate_params(&params, &values).is_ok());
    }

    #[test]
    fn test_validate_multiple_parameters_missing_required() {
        let params = vec![
            Parameter::new("name", "Name", ParameterType::String).required(true),
            Parameter::new("email", "Email", ParameterType::String).required(true),
            Parameter::new("phone", "Phone", ParameterType::String).required(false),
        ];

        // Missing required parameter 'email'
        let values = [
            ("name".to_string(), json!("John")),
            ("phone".to_string(), json!("555-1234")),
        ].iter().cloned().collect();

        let result = validate_params(&params, &values);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::MissingRequired { name } => {
                assert_eq!(name, "email");
            }
            _ => panic!("Expected MissingRequired error"),
        }
    }

    #[test]
    fn test_validate_multiple_parameters_validation_failures() {
        let params = vec![
            Parameter::new("email", "Email", ParameterType::String)
                .with_pattern(CommonPatterns::EMAIL),
            Parameter::new("percentage", "Percentage", ParameterType::Number)
                .with_range(Some(0.0), Some(100.0)),
        ];

        // Invalid email format
        let values = [
            ("email".to_string(), json!("invalid-email")),
            ("percentage".to_string(), json!(50.0)),
        ].iter().cloned().collect();

        let result = validate_params(&params, &values);
        assert!(result.is_err());
        match result.unwrap_err() {
            ParameterError::PatternMismatch { name, .. } => {
                assert_eq!(name, "email");
            }
            _ => panic!("Expected PatternMismatch error"),
        }
    }

    #[test]
    fn test_validate_parameters_with_optional_missing() {
        let params = vec![
            Parameter::new("required_field", "Required", ParameterType::String).required(true),
            Parameter::new("optional_field", "Optional", ParameterType::String).required(false),
        ];

        // Only required parameter provided
        let values = [
            ("required_field".to_string(), json!("value")),
        ].iter().cloned().collect();

        assert!(validate_params(&params, &values).is_ok());
    }

    #[test]
    fn test_validate_parameters_comprehensive_scenario() {
        let params = vec![
            // String with pattern and length validation
            Parameter::new("username", "Username", ParameterType::String)
                .required(true)
                .with_length_range(Some(3), Some(20))
                .with_pattern(r"^[a-zA-Z0-9_]+$"),
            
            // Number with range and step validation
            Parameter::new("price", "Price", ParameterType::Number)
                .required(true)
                .with_range(Some(0.01), Some(9999.99))
                .with_step(0.01),
            
            // Boolean parameter
            Parameter::new("featured", "Featured", ParameterType::Boolean)
                .required(false),
            
            // Choice parameter
            Parameter::new("category", "Category", ParameterType::Choice)
                .required(true)
                .with_choices(vec!["electronics".to_string(), "clothing".to_string(), "books".to_string()]),
            
            // Multi-choice with selection constraints
            Parameter::new("tags", "Tags", ParameterType::MultiChoice)
                .required(false)
                .with_choices(vec!["new".to_string(), "sale".to_string(), "featured".to_string(), "limited".to_string()])
                .with_selection_range(Some(0), Some(3)),
        ];

        // Valid values for all parameters
        let valid_values = [
            ("username".to_string(), json!("john_doe_123")),
            ("price".to_string(), json!(29.99)),
            ("featured".to_string(), json!(false)),
            ("category".to_string(), json!("electronics")),
            ("tags".to_string(), json!(["new", "featured"])),
        ].iter().cloned().collect();

        assert!(validate_params(&params, &valid_values).is_ok());

        // Test various failure scenarios
        
        // Invalid username (contains invalid character)
        let invalid_username = [
            ("username".to_string(), json!("john-doe")), // hyphen not allowed
            ("price".to_string(), json!(29.99)),
            ("category".to_string(), json!("electronics")),
        ].iter().cloned().collect();
        
        assert!(validate_params(&params, &invalid_username).is_err());

        // Invalid price (wrong step)
        let invalid_price = [
            ("username".to_string(), json!("john_doe")),
            ("price".to_string(), json!(29.999)), // not multiple of 0.01
            ("category".to_string(), json!("electronics")),
        ].iter().cloned().collect();
        
        assert!(validate_params(&params, &invalid_price).is_err());

        // Invalid category
        let invalid_category = [
            ("username".to_string(), json!("john_doe")),
            ("price".to_string(), json!(29.99)),
            ("category".to_string(), json!("invalid")),
        ].iter().cloned().collect();
        
        assert!(validate_params(&params, &invalid_category).is_err());

        // Too many tag selections
        let too_many_tags = [
            ("username".to_string(), json!("john_doe")),
            ("price".to_string(), json!(29.99)),
            ("category".to_string(), json!("electronics")),
            ("tags".to_string(), json!(["new", "sale", "featured", "limited"])), // 4 tags, max is 3
        ].iter().cloned().collect();
        
        assert!(validate_params(&params, &too_many_tags).is_err());
    }
}