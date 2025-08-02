//! Comprehensive error scenario testing for sah.toml configuration system
//!
//! This module tests all error conditions and edge cases to ensure
//! robust error handling and appropriate error messages.

use crate::toml_config::*;
use std::collections::HashMap;
use std::fs;
use tempfile::TempDir;

/// Test comprehensive error scenarios for ConfigError
mod config_error_tests {
    use super::*;

    #[test]
    fn test_io_error_file_not_found() {
        let parser = ConfigParser::new();
        let result = parser.parse_file(std::path::Path::new("nonexistent_file.toml"));
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::Io(_)));
        assert!(error.to_string().contains("No such file"));
    }

    #[test]
    fn test_io_error_permission_denied() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("readonly.toml");
        
        // Create file and make it unreadable (Unix-specific)
        fs::write(&config_path, "name = \"test\"").unwrap();
        
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&config_path).unwrap().permissions();
            perms.set_mode(0o000);
            fs::set_permissions(&config_path, perms).unwrap();
            
            let parser = ConfigParser::new();
            let result = parser.parse_file(&config_path);
            
            assert!(result.is_err());
            let error = result.unwrap_err();
            assert!(matches!(error, ConfigError::Io(_)));
        }
    }

    #[test]
    fn test_invalid_utf8_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_utf8.toml");
        
        // Write invalid UTF-8 bytes
        let invalid_utf8_bytes = vec![0xFF, 0xFE, 0xFD, 0xFC];
        fs::write(&config_path, invalid_utf8_bytes).unwrap();
        
        let parser = ConfigParser::new();
        let result = parser.parse_file(&config_path);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::InvalidUtf8(_)));
        assert!(error.to_string().contains("invalid utf-8"));
    }

    #[test]
    fn test_file_too_large_error() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("large_file.toml");
        
        // Create content larger than limit
        let large_content = "x".repeat(2_000_000); // 2MB > 1MB limit
        let toml_content = format!("large_string = \"{}\"", large_content);
        fs::write(&config_path, toml_content).unwrap();
        
        let parser = ConfigParser::new();
        let result = parser.parse_file(&config_path);
        
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::FileTooLarge { .. }));
        assert!(error.to_string().contains("Configuration file is too large"));
        assert!(error.to_string().contains("maximum:"));
    }

    #[test]
    fn test_toml_parse_errors() {
        let parser = ConfigParser::new();
        
        // Test unclosed string
        let invalid_toml = r#"
            name = "unclosed string
            version = "1.0.0"
        "#;
        
        let result = parser.parse_string(invalid_toml, None);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.is_parse_error());
        
        // Test invalid table syntax
        let invalid_table = r#"
            [incomplete table
            name = "value"
        "#;
        
        let result = parser.parse_string(invalid_table, None);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.is_parse_error());
        
        // Test duplicate keys
        let duplicate_keys = r#"
            name = "first"
            name = "second"
        "#;
        
        let result = parser.parse_string(duplicate_keys, None);
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(error.is_parse_error());
    }

    #[test]
    fn test_validation_errors() {
        let parser = ConfigParser::new();
        
        // Test invalid variable names
        let invalid_names = r#"
            123invalid = "starts with number"
            "invalid-key" = "contains hyphen"
            if = "reserved keyword"
        "#;
        
        let result = parser.parse_string(invalid_names, None);
        // Parser should succeed but validation should fail
        if let Ok(config) = result {
            let validation_result = config.validate();
            assert!(validation_result.is_err());
            let error = validation_result.unwrap_err();
            assert!(matches!(error, ConfigError::Validation { .. }));
        }
    }

    #[test]
    fn test_nesting_too_deep_error() {
        // Create deeply nested structure that exceeds limit
        fn create_deep_toml(depth: usize) -> String {
            if depth == 0 {
                "value = \"deep\"".to_string()
            } else {
                format!("[{}]\n{}", "nested".repeat(depth), create_deep_toml(depth - 1))
            }
        }
        
        let _deep_toml = create_deep_toml(15); // Exceeds MAX_NESTING_DEPTH of 10
        
        // Since TOML doesn't naturally create deep nesting, test via ConfigValue
        let deep_value = create_deep_config_value(15);
        let validation_result = deep_value.validate(0);
        
        assert!(validation_result.is_err());
        let error = validation_result.unwrap_err();
        assert!(matches!(error, ConfigError::NestingTooDeep { .. }));
        assert!(error.to_string().contains("Configuration nesting depth too deep"));
        assert!(error.to_string().contains("maximum:"));
    }

    fn create_deep_config_value(depth: usize) -> ConfigValue {
        if depth == 0 {
            ConfigValue::String("value".to_string())
        } else {
            let mut table = HashMap::new();
            table.insert("nested".to_string(), create_deep_config_value(depth - 1));
            ConfigValue::Table(table)
        }
    }

    #[test]
    fn test_string_too_large_error() {
        // Create a very large string that exceeds limits
        let large_string = "x".repeat(ValidationLimits::MAX_STRING_SIZE + 1);
        let value = ConfigValue::String(large_string);
        
        let validation_result = value.validate(0);
        assert!(validation_result.is_err());
        let error = validation_result.unwrap_err();
        assert!(matches!(error, ConfigError::StringTooLarge { .. }));
        assert!(error.to_string().contains("String value too large"));
        assert!(error.to_string().contains("maximum:"));
    }

    #[test]
    fn test_array_too_large_error() {
        // Create array that exceeds size limits
        let large_array = vec![ConfigValue::Integer(1); ValidationLimits::MAX_ARRAY_SIZE + 1];
        let value = ConfigValue::Array(large_array);
        
        let validation_result = value.validate(0);
        assert!(validation_result.is_err());
        let error = validation_result.unwrap_err();
        assert!(matches!(error, ConfigError::ArrayTooLarge { .. }));
        assert!(error.to_string().contains("Array too large"));
        assert!(error.to_string().contains("maximum:"));
    }

    #[test]
    fn test_environment_variable_errors() {
        let mut value = ConfigValue::String("${NONEXISTENT_REQUIRED_VAR}".to_string());
        
        let result = value.substitute_env_vars();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::EnvVarSubstitution { .. }));
        assert!(error.to_string().contains("Environment variable"));
        assert!(error.to_string().contains("not found"));
    }

    #[test]
    fn test_type_coercion_errors() {
        // Test array to string coercion (should fail)
        let array_value = ConfigValue::Array(vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::String("item2".to_string()),
        ]);
        
        let result = array_value.coerce_to_string();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::TypeCoercion { .. }));
        assert!(error.to_string().contains("Cannot coerce value of type"));
        assert!(error.to_string().contains("array"));
        assert!(error.to_string().contains("string"));
        
        // Test table to integer coercion (should fail)
        let mut table = HashMap::new();
        table.insert("key".to_string(), ConfigValue::String("value".to_string()));
        let table_value = ConfigValue::Table(table);
        
        let result = table_value.coerce_to_integer();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::TypeCoercion { .. }));
        assert!(error.to_string().contains("Cannot coerce value of type"));
        assert!(error.to_string().contains("table"));
        assert!(error.to_string().contains("integer"));
        
        // Test invalid string to number coercion
        let invalid_number = ConfigValue::String("not_a_number".to_string());
        
        let result = invalid_number.coerce_to_integer();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::TypeCoercion { .. }));
        
        let result = invalid_number.coerce_to_float();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::TypeCoercion { .. }));
        
        // Test invalid string to boolean coercion
        let invalid_bool = ConfigValue::String("maybe".to_string());
        let result = invalid_bool.coerce_to_boolean();
        assert!(result.is_err());
        let error = result.unwrap_err();
        assert!(matches!(error, ConfigError::TypeCoercion { .. }));
    }

    #[test]
    fn test_error_display_and_debug() {
        // Test that all error types implement Display and Debug properly
        let errors = vec![
            ConfigError::validation("Test validation error".to_string()),
            ConfigError::type_coercion("string".to_string(), "integer".to_string()),
            ConfigError::env_var_substitution("TEST_VAR".to_string(), "Variable not found".to_string()),
            ConfigError::file_too_large(2000000, 1000000),
            ConfigError::string_too_large(500000, 100000),
            ConfigError::array_too_large(10000, 1000),
            ConfigError::nesting_too_deep(15, 10),
        ];
        
        for error in errors {
            // Test Display implementation
            let display_str = error.to_string();
            assert!(!display_str.is_empty());
            
            // Test Debug implementation
            let debug_str = format!("{:?}", error);
            assert!(!debug_str.is_empty());
            
            // Test error categorization - all errors should fall into at least one category
            let is_categorized = error.is_validation_error() 
                || error.is_parse_error() 
                || error.is_size_limit_error()
                || matches!(error, ConfigError::TypeCoercion { .. })
                || matches!(error, ConfigError::EnvVarSubstitution { .. })
                || matches!(error, ConfigError::CircularReference)
                || matches!(error, ConfigError::Io(_));
            assert!(is_categorized, "Error type not categorized: {:?}", error);
        }
    }

    #[test]
    fn test_error_source_chain() {
        use std::error::Error;
        
        // Test IO error source chain
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let config_error = ConfigError::Io(io_error);
        
        assert!(config_error.source().is_some());
        
        // Test that error chain can be walked
        let mut current_error: &dyn Error = &config_error;
        let mut error_count = 0;
        while let Some(source) = current_error.source() {
            current_error = source;
            error_count += 1;
            if error_count > 10 {
                break; // Prevent infinite loops
            }
        }
        assert!(error_count > 0);
    }

    #[test] 
    fn test_concurrent_error_handling() {
        use std::sync::Arc;
        use std::thread;
        
        // Test that errors can be safely shared across threads
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("concurrent_test.toml");
        fs::write(&config_path, "name = \"ConcurrentTest\"").unwrap();
        
        let config_path = Arc::new(config_path);
        let mut handles = vec![];
        
        // Spawn multiple threads that might encounter errors
        for i in 0..5 {
            let path = config_path.clone();
            let handle = thread::spawn(move || {
                let parser = ConfigParser::new();
                if i % 2 == 0 {
                    // Try to parse valid file
                    parser.parse_file(&*path)
                } else {
                    // Try to parse nonexistent file
                    parser.parse_file(std::path::Path::new("nonexistent.toml"))
                }
            });
            handles.push(handle);
        }
        
        // Collect results and verify error handling
        let mut success_count = 0;
        let mut error_count = 0;
        
        for handle in handles {
            match handle.join().unwrap() {
                Ok(_) => success_count += 1,
                Err(_) => error_count += 1,
            }
        }
        
        assert!(success_count > 0);
        assert!(error_count > 0);
    }

    #[test]
    fn test_error_recovery_scenarios() {
        let parser = ConfigParser::new();
        
        // Test that parser can recover from errors and continue working
        let invalid_toml = "invalid = syntax here";
        let result1 = parser.parse_string(invalid_toml, None);
        assert!(result1.is_err());
        
        // Parser should still work after encountering an error
        let valid_toml = "name = \"ValidConfig\"";
        let result2 = parser.parse_string(valid_toml, None);
        assert!(result2.is_ok());
        
        let config = result2.unwrap();
        assert_eq!(
            config.get("name").unwrap().coerce_to_string().unwrap(),
            "ValidConfig"
        );
    }

    #[test]
    fn test_complex_error_scenarios() {
        let parser = ConfigParser::new();
        
        // Test combination of issues
        let complex_invalid = r#"
            # Invalid key name
            123invalid = "starts with number"
            
            # Missing closing quote
            unclosed = "string value
            
            # Invalid array syntax
            broken_array = [missing, closing, bracket
            
            # Invalid table
            [incomplete table
        "#;
        
        let result = parser.parse_string(complex_invalid, None);
        assert!(result.is_err());
        
        // Should get parse error first (parsing happens before validation)
        let error = result.unwrap_err();
        assert!(error.is_parse_error());
    }

    #[test]
    fn test_malformed_environment_variables() {
        let parser = ConfigParser::new();
        
        // Test various malformed environment variable patterns
        let malformed_patterns = vec![
            "${", // Incomplete opening
            "${VAR", // Missing closing brace
            "${}", // Empty variable name
            "${123VAR}", // Invalid variable name starting with number
            "${VAR-NAME}", // Invalid character in variable name
            "${VAR:-}", // Empty default value (should be valid)
            "${VAR:}", // Invalid default syntax
        ];
        
        for pattern in malformed_patterns {
            let toml_content = format!("test_var = \"{}\"", pattern);
            
            // Most malformed patterns should either be ignored or handled gracefully
            let result = parser.parse_string(&toml_content, None);
            
            // Parser should not crash, though some patterns may cause substitution errors
            match result {
                Ok(config) => {
                    // If parsing succeeds, the malformed pattern should be left as-is
                    // or handled gracefully during environment substitution
                    assert!(config.get("test_var").is_some());
                },
                Err(error) => {
                    // If parsing fails, it should be a specific error type
                    assert!(error.is_parse_error() || error.is_validation_error());
                }
            }
        }
    }
}

/// Test edge cases and boundary conditions
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_empty_configuration() {
        let parser = ConfigParser::new();
        
        // Test completely empty file
        let config = parser.parse_string("", None).unwrap();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);
        
        // Test file with only comments and whitespace
        let whitespace_only = r#"
            # This is a comment
            
            # Another comment
            
        "#;
        
        let config = parser.parse_string(whitespace_only, None).unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn test_maximum_valid_values() {
        // Test values at the boundary of limits
        let max_string = "x".repeat(ValidationLimits::MAX_STRING_SIZE);
        let value = ConfigValue::String(max_string);
        assert!(value.validate(0).is_ok());
        
        let max_array = vec![ConfigValue::Integer(1); ValidationLimits::MAX_ARRAY_SIZE];
        let value = ConfigValue::Array(max_array);
        assert!(value.validate(0).is_ok());
        
        // Test nesting at maximum depth
        let max_depth_value = create_nested_value(ValidationLimits::MAX_NESTING_DEPTH);
        assert!(max_depth_value.validate(0).is_ok());
    }

    fn create_nested_value(depth: usize) -> ConfigValue {
        if depth == 0 {
            ConfigValue::String("leaf".to_string())
        } else {
            let mut table = HashMap::new();
            table.insert("nested".to_string(), create_nested_value(depth - 1));
            ConfigValue::Table(table)
        }
    }

    #[test]
    fn test_unicode_and_special_characters() {
        let parser = ConfigParser::new();
        
        let unicode_toml = r#"
            emoji = "🚀"
            chinese = "你好世界"
            arabic = "مرحبا بالعالم"
            russian = "Привет мир"
            mixed = "Hello 世界 🌍"
            
            [unicode_keys]
            "🔑" = "emoji key"
            "键" = "chinese key"
        "#;
        
        let config = parser.parse_string(unicode_toml, None).unwrap();
        
        assert_eq!(
            config.get("emoji").unwrap().coerce_to_string().unwrap(),
            "🚀"
        );
        assert_eq!(
            config.get("chinese").unwrap().coerce_to_string().unwrap(),
            "你好世界"
        );
        assert_eq!(
            config.get("unicode_keys.🔑").unwrap().coerce_to_string().unwrap(),
            "emoji key"
        );
    }

    #[test]
    fn test_numeric_edge_cases() {
        let parser = ConfigParser::new();
        
        let numeric_toml = r#"
            max_i64 = 9223372036854775807
            min_i64 = -9223372036854775808
            zero = 0
            negative_zero = -0
            max_float = 1.7976931348623157e+308
            min_float = -1.7976931348623157e+308
            tiny_float = 2.2250738585072014e-308
            infinity_str = "inf"
            nan_str = "nan"
        "#;
        
        let config = parser.parse_string(numeric_toml, None).unwrap();
        
        assert_eq!(
            config.get("max_i64").unwrap().coerce_to_integer().unwrap(),
            i64::MAX
        );
        assert_eq!(
            config.get("min_i64").unwrap().coerce_to_integer().unwrap(),
            i64::MIN
        );
        assert_eq!(
            config.get("zero").unwrap().coerce_to_integer().unwrap(),
            0
        );
    }

    #[test]
    fn test_boolean_edge_cases() {
        let parser = ConfigParser::new();
        
        let boolean_toml = r#"
            true_val = true
            false_val = false
            true_str = "true"
            false_str = "false"
            yes_str = "yes"
            no_str = "no"  
            on_str = "on"
            off_str = "off"
            one_str = "1"
            zero_str = "0"
        "#;
        
        let config = parser.parse_string(boolean_toml, None).unwrap();
        
        // Test direct boolean values
        assert!(config.get("true_val").unwrap().coerce_to_boolean().unwrap());
        assert!(!config.get("false_val").unwrap().coerce_to_boolean().unwrap());
        
        // Test string to boolean coercion
        assert!(config.get("true_str").unwrap().coerce_to_boolean().unwrap());
        assert!(!config.get("false_str").unwrap().coerce_to_boolean().unwrap());
        assert!(config.get("yes_str").unwrap().coerce_to_boolean().unwrap());
        assert!(!config.get("no_str").unwrap().coerce_to_boolean().unwrap());
        assert!(config.get("on_str").unwrap().coerce_to_boolean().unwrap());
        assert!(!config.get("off_str").unwrap().coerce_to_boolean().unwrap());
        assert!(config.get("one_str").unwrap().coerce_to_boolean().unwrap());
        assert!(!config.get("zero_str").unwrap().coerce_to_boolean().unwrap());
    }

    #[test]
    fn test_array_edge_cases() {
        let parser = ConfigParser::new();
        
        let array_toml = r#"
            empty_array = []
            single_item = ["alone"]
            mixed_types = ["string", 42, true, 3.14]
            nested_arrays = [["a", "b"], ["c", "d"]]
            trailing_comma = ["item1", "item2",]
        "#;
        
        let config = parser.parse_string(array_toml, None).unwrap();
        
        // Test empty array
        let empty = config.get("empty_array").unwrap().coerce_to_array().unwrap();
        assert_eq!(empty.len(), 0);
        
        // Test single item array
        let single = config.get("single_item").unwrap().coerce_to_array().unwrap();
        assert_eq!(single.len(), 1);
        assert_eq!(single[0], ConfigValue::String("alone".to_string()));
        
        // Test mixed types
        let mixed = config.get("mixed_types").unwrap().coerce_to_array().unwrap();
        assert_eq!(mixed.len(), 4);
        assert_eq!(mixed[0], ConfigValue::String("string".to_string()));
        assert_eq!(mixed[1], ConfigValue::Integer(42));
        assert_eq!(mixed[2], ConfigValue::Boolean(true));
        assert_eq!(mixed[3], ConfigValue::Float(3.14));
    }

    #[test]
    fn test_table_edge_cases() {
        let parser = ConfigParser::new();
        
        let table_toml = r#"
            [empty_table]
            
            [single_key]
            key = "value"
            
            [deep.nested.table]
            value = "deeply nested"
            
            [table_with_mixed_types]
            string_val = "text"
            int_val = 42
            bool_val = true
            array_val = [1, 2, 3]
            
            [table_with_subtable]
            key = "parent"
            
            [table_with_subtable.child]
            key = "child"
        "#;
        
        let config = parser.parse_string(table_toml, None).unwrap();
        
        // Test empty table exists
        assert!(config.contains_key("empty_table"));
        let empty_table = config.get("empty_table").unwrap().coerce_to_table().unwrap();
        assert_eq!(empty_table.len(), 0);
        
        // Test deeply nested access
        assert_eq!(
            config.get("deep.nested.table.value").unwrap().coerce_to_string().unwrap(),
            "deeply nested"
        );
        
        // Test parent/child relationships
        assert_eq!(
            config.get("table_with_subtable.key").unwrap().coerce_to_string().unwrap(),
            "parent"
        );
        assert_eq!(
            config.get("table_with_subtable.child.key").unwrap().coerce_to_string().unwrap(),
            "child"
        );
    }
}