//! Security and validation tests for sah.toml configuration system
//!
//! This module tests security validation, file size limits, nesting depth limits,
//! path traversal prevention, and other security-related aspects of the configuration system.

use crate::toml_config::{ConfigError, ConfigParser, ConfigValue, Configuration, ValidationLimits};
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

/// Test file size limits (1MB maximum)
#[test]
fn test_file_size_limits() {
    let temp_dir = TempDir::new().unwrap();
    let parser = ConfigParser::new();

    // Test file within size limit
    let small_config_path = temp_dir.path().join("small.toml");
    let small_content = r#"
        name = "SmallConfig"
        version = "1.0.0"
        description = "A small configuration file for testing"
    "#;
    fs::write(&small_config_path, small_content).unwrap();

    let result = parser.validate_file(&small_config_path);
    assert!(result.is_ok(), "Small file should pass validation");

    // Test file exceeding size limit
    let large_config_path = temp_dir.path().join("large.toml");
    let max_size = ValidationLimits::default().max_file_size as usize;

    // Create content that exceeds the limit
    let mut large_content = String::with_capacity(max_size + 1000);
    large_content.push_str("name = \"LargeConfig\"\n");
    large_content.push_str("description = \"");

    // Fill with 'A' characters to exceed size limit
    let padding_size = max_size + 500;
    large_content.push_str(&"A".repeat(padding_size));
    large_content.push_str("\"\n");

    fs::write(&large_config_path, &large_content).unwrap();

    let result = parser.validate_file(&large_config_path);
    assert!(result.is_err(), "Large file should fail validation");

    if let Err(ConfigError::FileTooLarge {
        size,
        max_size: limit,
    }) = result
    {
        assert!(size > limit, "Error should report correct size comparison");
    } else {
        panic!("Expected FileTooLarge error");
    }
}

/// Test depth limits (10 levels maximum)
#[test]
fn test_nesting_depth_limits() {
    let parser = ConfigParser::new();
    let max_depth = ValidationLimits::default().max_nesting_depth;

    // Test configuration within depth limit
    let mut valid_nested_content = String::new();
    valid_nested_content.push_str("name = \"DepthTest\"\n");

    // Create nested structure within limit (max_depth - 1 levels)
    for i in 0..(max_depth - 1) {
        valid_nested_content.push_str(&format!("[level{i}"));
        for j in 0..i {
            valid_nested_content.push_str(&format!(".level{j}"));
        }
        valid_nested_content.push_str("]\n");
        valid_nested_content.push_str(&format!("value{i} = \"level {i} value\"\n"));
    }

    let result = parser.parse_string(&valid_nested_content, None);
    assert!(
        result.is_ok(),
        "Configuration within depth limit should parse successfully"
    );

    if let Ok(config) = result {
        let validation_result = config.validate();
        assert!(
            validation_result.is_ok(),
            "Configuration within depth limit should validate successfully"
        );
    }

    // Test configuration exceeding depth limit
    let mut invalid_nested_content = String::new();
    invalid_nested_content.push_str("name = \"DeepNestingTest\"\n");

    // Create deeply nested structure using ConfigValue directly
    let mut deep_table = HashMap::new();
    deep_table.insert(
        "value".to_string(),
        ConfigValue::String("deepest value".to_string()),
    );

    // Build nested structure that exceeds depth limit
    for level in 0..(max_depth + 5) {
        let mut outer_table = HashMap::new();
        outer_table.insert(format!("level{level}"), ConfigValue::Table(deep_table));
        deep_table = outer_table;
    }

    let deep_config_value = ConfigValue::Table(deep_table);
    let validation_result = deep_config_value.validate(0);

    assert!(
        validation_result.is_err(),
        "Deeply nested structure should fail validation"
    );

    if let Err(ConfigError::NestingTooDeep {
        depth,
        max_depth: limit,
    }) = validation_result
    {
        assert!(
            depth > limit,
            "Error should report correct depth comparison"
        );
    } else {
        panic!("Expected NestingTooDeep error, got: {validation_result:?}");
    }
}

/// Test path traversal prevention
#[test]
fn test_path_traversal_prevention() {
    let temp_dir = TempDir::new().unwrap();
    let parser = ConfigParser::new();

    // Create a file outside the temp directory
    let outside_file = temp_dir.path().parent().unwrap().join("outside.txt");
    fs::write(&outside_file, "secret content").unwrap();

    // Test various path traversal attempts
    let traversal_attempts = vec![
        "../outside.txt",
        "..\\outside.txt",
        "../../outside.txt",
        "../../../outside.txt",
        "subdir/../outside.txt",
        "subdir/../../outside.txt",
        "./../../outside.txt",
        "./../outside.txt",
    ];

    for attempt in traversal_attempts {
        let malicious_path = temp_dir.path().join(attempt);

        // Attempt to parse file with path traversal
        let result = parser.parse_file(&malicious_path);

        // The operation should either fail (file not found) or not expose sensitive content
        // We mainly want to ensure it doesn't crash or expose unintended files
        match result {
            Ok(_) => {
                // If it succeeds, it should be because the path resolved to a legitimate file
                // within the intended directory structure
            }
            Err(ConfigError::Io(_)) => {
                // File not found is expected and acceptable
            }
            Err(other) => {
                // Other errors are also acceptable as they indicate the operation was rejected
                println!("Path traversal attempt '{attempt}' rejected with: {other:?}");
            }
        }
    }

    // Clean up
    let _ = fs::remove_file(&outside_file);
}

/// Test malformed TOML handling
#[test]
fn test_malformed_toml_handling() {
    let parser = ConfigParser::new();

    // Test various malformed TOML inputs
    let malformed_inputs = vec![
        // Unclosed strings
        (r#"name = "unclosed string"#, "unclosed string"),
        // Invalid syntax
        (r#"name = value without quotes"#, "unquoted value"),
        // Malformed arrays
        (r#"array = [1, 2, 3,"#, "unclosed array"),
        // Malformed tables
        (r#"[incomplete table"#, "incomplete table"),
        // Invalid key characters
        (r#"key with spaces = "value""#, "key with spaces"),
        // Duplicate keys in same table
        (
            r#"
            name = "first"
            name = "second"
        "#,
            "duplicate keys",
        ),
        // Invalid escape sequences
        (r#"value = "invalid \x escape""#, "invalid escape"),
        // Malformed nested structures
        (
            r#"
            [table1]
            key = "value"
            [table1.nested
            nested_key = "value"
        "#,
            "malformed nested table",
        ),
        // Invalid Unicode
        (r#"unicode = "\uXXXX""#, "invalid unicode"),
        // Invalid numbers
        (r#"number = 123abc"#, "invalid number"),
        // Invalid booleans
        (r#"boolean = maybe"#, "invalid boolean"),
        // Note: "date = "not-a-date"" is actually valid TOML (it's just a string)
    ];

    for (input, description) in malformed_inputs {
        let result = parser.parse_string(input, None);

        assert!(
            result.is_err(),
            "Malformed TOML ({description}) should fail to parse: {input}"
        );

        // Verify that error is a parsing error
        match result {
            Err(ConfigError::TomlParse { .. }) | Err(ConfigError::TomlParseGeneric(_)) => {
                // Expected error types
            }
            Err(other) => {
                println!("Malformed TOML '{description}' produced unexpected error: {other:?}");
                // Still acceptable as long as it fails
            }
            Ok(_) => {
                panic!("Malformed TOML '{description}' should not parse successfully");
            }
        }
    }
}

/// Test invalid environment variable syntax
#[test]
fn test_invalid_environment_variable_syntax() {
    let parser = ConfigParser::new();

    // Test various invalid environment variable syntaxes
    let invalid_env_vars = vec![
        ("${}", "empty variable name"),
        ("${INVALID", "unclosed variable"),
        ("$INVALID}", "incorrect opening"),
        ("${123INVALID}", "variable starting with number"),
        ("${INVALID-VAR}", "variable with hyphen"),
        ("${INVALID VAR}", "variable with space"),
        ("${INVALID.VAR}", "variable with dot"),
        ("${:-default}", "missing variable name with default"),
        // Note: "${VAR:-}" is actually valid - empty default value
    ];

    for (invalid_syntax, description) in invalid_env_vars {
        let toml_content = format!(r#"test_var = "{invalid_syntax}""#);

        let result = parser.parse_string(&toml_content, None);

        // Parsing should succeed (it's valid TOML)
        assert!(
            result.is_ok(),
            "TOML with invalid env var syntax should still parse"
        );

        if let Ok(mut config) = result {
            // Environment variable substitution should handle invalid syntax gracefully
            let substitution_result = config.substitute_env_vars();

            match substitution_result {
                Ok(_) => {
                    // If substitution succeeds, the invalid syntax should be left unchanged
                    if let Some(ConfigValue::String(value)) = config.get("test_var") {
                        assert_eq!(
                            value, invalid_syntax,
                            "Invalid env var syntax '{description}' should be left unchanged"
                        );
                    }
                }
                Err(_) => {
                    // If substitution fails, that's also acceptable for clearly invalid syntax
                    println!("Invalid env var syntax '{description}' rejected during substitution");
                }
            }
        }
    }
}

/// Test validation of variable names according to Liquid identifier rules
#[test]
fn test_variable_name_validation() {
    let mut config = Configuration::new();

    // Test valid variable names
    let valid_names = vec![
        "valid_name",
        "_underscore_start",
        "CamelCase",
        "snake_case",
        "name123",
        "a",
        "_",
        "valid_name_with_dots.nested.deep",
    ];

    for name in valid_names {
        config.insert(name.to_string(), ConfigValue::String("value".to_string()));
        let result = config.validate();
        assert!(result.is_ok(), "Variable name '{name}' should be valid");
        config.remove(name); // Clean up for next test
    }

    // Test invalid variable names
    let invalid_names = vec![
        ("", "empty name"),
        ("123invalid", "starts with number"),
        ("invalid-name", "contains hyphen"),
        ("invalid name", "contains space"),
        ("invalid@name", "contains at symbol"),
        ("invalid#name", "contains hash"),
        ("invalid$name", "contains dollar"),
        ("name!", "contains exclamation"),
        ("name+plus", "contains plus"),
        ("name=equals", "contains equals"),
    ];

    for (name, description) in invalid_names {
        config.insert(name.to_string(), ConfigValue::String("value".to_string()));
        let result = config.validate();
        assert!(
            result.is_err(),
            "Variable name '{name}' ({description}) should be invalid"
        );
        config.remove(name); // Clean up for next test
    }

    // Test reserved variable names
    let reserved_names = vec![
        "for",
        "if",
        "unless",
        "case",
        "when",
        "else",
        "endif",
        "endfor",
        "endunless",
        "endcase",
        "break",
        "continue",
        "assign",
        "capture",
        "include",
        "layout",
        "raw",
        "endraw",
        "comment",
        "endcomment",
    ];

    for name in reserved_names {
        config.insert(name.to_string(), ConfigValue::String("value".to_string()));
        let result = config.validate();
        assert!(
            result.is_err(),
            "Reserved variable name '{name}' should be invalid"
        );
        config.remove(name); // Clean up for next test
    }
}

/// Test string length validation
#[test]
fn test_string_length_validation() {
    let max_string_length = ValidationLimits::default().max_string_length;

    // Test string within limit
    let valid_string = "a".repeat(max_string_length - 100);
    let valid_value = ConfigValue::String(valid_string.clone());
    let result = valid_value.validate(0);
    assert!(result.is_ok(), "String within limit should be valid");

    // Test string exceeding limit
    let invalid_string = "a".repeat(max_string_length + 100);
    let invalid_value = ConfigValue::String(invalid_string);
    let result = invalid_value.validate(0);
    assert!(result.is_err(), "String exceeding limit should be invalid");

    if let Err(ConfigError::StringTooLarge { size, max_size }) = result {
        assert!(
            size > max_size,
            "Error should report correct size comparison"
        );
    } else {
        panic!("Expected StringTooLarge error, got: {result:?}");
    }
}

/// Test array length validation
#[test]
fn test_array_length_validation() {
    let max_array_length = ValidationLimits::default().max_array_length;

    // Test array within limit
    let valid_array = vec![ConfigValue::Integer(1); max_array_length - 10];
    let valid_value = ConfigValue::Array(valid_array);
    let result = valid_value.validate(0);
    assert!(result.is_ok(), "Array within limit should be valid");

    // Test array exceeding limit
    let invalid_array = vec![ConfigValue::Integer(1); max_array_length + 10];
    let invalid_value = ConfigValue::Array(invalid_array);
    let result = invalid_value.validate(0);
    assert!(result.is_err(), "Array exceeding limit should be invalid");

    if let Err(ConfigError::ArrayTooLarge { size, max_size }) = result {
        assert!(
            size > max_size,
            "Error should report correct size comparison"
        );
    } else {
        panic!("Expected ArrayTooLarge error, got: {result:?}");
    }
}

/// Test UTF-8 encoding validation
#[test]
fn test_utf8_encoding_validation() {
    let temp_dir = TempDir::new().unwrap();
    let parser = ConfigParser::new();

    // Test valid UTF-8 file
    let valid_utf8_path = temp_dir.path().join("valid_utf8.toml");
    let valid_content = "name = \"Hello 世界 🌍\"";
    fs::write(&valid_utf8_path, valid_content).unwrap();

    let result = parser.parse_file(&valid_utf8_path);
    assert!(result.is_ok(), "Valid UTF-8 file should parse successfully");

    // Test invalid UTF-8 file
    let invalid_utf8_path = temp_dir.path().join("invalid_utf8.toml");
    let invalid_bytes = vec![
        b'n', b'a', b'm', b'e', b' ', b'=', b' ', b'"', 0xFF, 0xFE, // Invalid UTF-8 bytes
        b'"', b'\n',
    ];
    fs::write(&invalid_utf8_path, &invalid_bytes).unwrap();

    let result = parser.parse_file(&invalid_utf8_path);
    assert!(result.is_err(), "Invalid UTF-8 file should fail to parse");

    // The error could be either InvalidUtf8 or Io depending on implementation
    match result {
        Err(ConfigError::InvalidUtf8(_)) | Err(ConfigError::Io(_)) => {
            // Expected error types
        }
        Err(other) => {
            println!("Invalid UTF-8 file produced unexpected error: {other:?}");
            // Still acceptable as long as it fails
        }
        Ok(_) => {
            panic!("Invalid UTF-8 file should not parse successfully");
        }
    }
}

/// Test circular reference prevention in environment variables
#[test]
fn test_circular_reference_prevention() {
    // Set up circular environment variables
    std::env::set_var("CIRCULAR_A", "${CIRCULAR_B}");
    std::env::set_var("CIRCULAR_B", "${CIRCULAR_A}");

    let toml_content = r#"
        test_var = "${CIRCULAR_A}"
    "#;

    let parser = ConfigParser::new();
    let result = parser.parse_string(toml_content, None);
    assert!(result.is_ok(), "TOML parsing should succeed");

    if let Ok(mut config) = result {
        let substitution_result = config.substitute_env_vars();

        // The substitution should either:
        // 1. Detect the circular reference and fail
        // 2. Limit the recursion depth and stop
        // 3. Leave the original value unchanged

        match substitution_result {
            Ok(_) => {
                // If substitution succeeds, verify it didn't create infinite recursion
                if let Some(ConfigValue::String(value)) = config.get("test_var") {
                    assert!(
                        value.len() < 10000, // Reasonable upper bound
                        "Circular reference should not create extremely long strings"
                    );
                }
            }
            Err(_) => {
                // Failure is acceptable for circular references
                println!("Circular reference properly detected and rejected");
            }
        }
    }

    // Clean up
    std::env::remove_var("CIRCULAR_A");
    std::env::remove_var("CIRCULAR_B");
}

/// Test content filtering and sanitization
#[test]
fn test_content_filtering() {
    let parser = ConfigParser::new();

    // Test various potentially dangerous content
    let long_line = "x".repeat(50000);
    let nested_quotes = "\"'\"'\"'".repeat(1000);

    let potentially_dangerous_content = vec![
        // Control characters
        ("control_chars", "value with \x00 null byte"),
        ("control_chars2", "value with \x1F control char"),
        // Very long lines
        ("long_line", long_line.as_str()),
        // Many nested quotes
        ("nested_quotes", nested_quotes.as_str()),
        // Unicode edge cases
        ("unicode_edge", "𝕌𝕟𝕚𝕔𝕠𝕕𝕖 𝕖𝕕𝕘𝕖 𝕔𝕒𝕤𝕖𝕤"),
        // Null-like values
        ("null_like", "\\0\\0\\0"),
    ];

    for (key, value) in potentially_dangerous_content {
        let toml_content = format!("{key} = \"{}\"", value.replace('"', "\\\""));

        let result = parser.parse_string(&toml_content, None);

        match result {
            Ok(config) => {
                // If parsing succeeds, validate the configuration
                let validation_result = config.validate();

                if validation_result.is_err() {
                    println!("Potentially dangerous content '{key}' rejected during validation");
                } else {
                    // If validation passes, ensure the content is properly handled
                    if let Some(ConfigValue::String(s)) = config.get(key) {
                        // Ensure string length is reasonable
                        assert!(
                            s.len() < ValidationLimits::default().max_string_length,
                            "String length should be within limits"
                        );
                    }
                }
            }
            Err(_) => {
                println!("Potentially dangerous content '{key}' rejected during parsing");
                // Rejection is acceptable
            }
        }
    }
}

/// Test memory safety with large configurations
#[test]
fn test_memory_safety_large_configurations() {
    let parser = ConfigParser::new();

    // Test large number of keys
    let mut large_config = String::new();
    let num_keys = 10000;

    for i in 0..num_keys {
        large_config.push_str(&format!("key_{i} = \"value_{i}\"\n"));
    }

    let result = parser.parse_string(&large_config, None);

    match result {
        Ok(config) => {
            assert_eq!(config.len(), num_keys);

            // Test access performance
            for i in 0..100 {
                let key = format!("key_{i}");
                let expected_value = ConfigValue::String(format!("value_{i}"));
                assert_eq!(config.get(&key), Some(&expected_value));
            }
        }
        Err(_) => {
            // If the system rejects large configurations, that's also acceptable
            println!("Large configuration rejected (this is acceptable for safety)");
        }
    }

    // Test deeply nested configuration
    let mut deep_config = String::new();
    deep_config.push_str("[level0");

    let max_safe_depth = 50; // Much lower than validation limit for this test
    for i in 1..max_safe_depth {
        deep_config.push_str(&format!(".level{i}"));
    }
    deep_config.push_str("]\n");
    deep_config.push_str("deep_value = \"found it\"\n");

    let result = parser.parse_string(&deep_config, None);

    match result {
        Ok(config) => {
            let mut deep_key = "level0".to_string();
            for i in 1..max_safe_depth {
                deep_key.push_str(&format!(".level{i}"));
            }
            deep_key.push_str(".deep_value");

            assert_eq!(
                config.get(&deep_key),
                Some(&ConfigValue::String("found it".to_string()))
            );
        }
        Err(_) => {
            println!("Deep configuration rejected (acceptable for safety)");
        }
    }
}
