//! Unit tests for core configuration components
//!
//! This module tests individual components of the configuration system in isolation,
//! including ConfigValue conversion, Configuration operations, and parser functionality.

use crate::toml_config::{ConfigError, ConfigParser, ConfigValue, Configuration, ValidationLimits};
use std::collections::HashMap;

/// Test ConfigValue enum conversion and serialization
mod config_value_tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    #[test]
    fn test_config_value_string_operations() {
        let value = ConfigValue::String("test_string".to_string());

        // Test coercion
        assert_eq!(value.coerce_to_string().unwrap(), "test_string");
        assert!(value.coerce_to_integer().is_err());
        assert!(value.coerce_to_float().is_err());
        assert!(value.coerce_to_boolean().is_err());

        // Test liquid conversion
        let liquid_value = value.to_liquid_value();
        assert_eq!(liquid_value, liquid::model::Value::scalar("test_string"));

        // Test JSON conversion
        let json_value = value.to_json_value();
        assert_eq!(json_value, json!("test_string"));
    }

    #[test]
    fn test_config_value_integer_operations() {
        let value = ConfigValue::Integer(42);

        // Test coercion
        assert_eq!(value.coerce_to_integer().unwrap(), 42);
        assert_eq!(value.coerce_to_string().unwrap(), "42");
        assert_eq!(value.coerce_to_float().unwrap(), 42.0);
        // Integer to boolean coercion is supported: 0 -> false, non-zero -> true
        assert!(value.coerce_to_boolean().unwrap());

        // Test liquid conversion
        let liquid_value = value.to_liquid_value();
        assert_eq!(liquid_value, liquid::model::Value::scalar(42));
    }

    #[test]
    fn test_config_value_float_operations() {
        let value = ConfigValue::Float(3.15);

        // Test coercion
        assert_eq!(value.coerce_to_float().unwrap(), 3.15);
        assert_eq!(value.coerce_to_string().unwrap(), "3.15");
        // Float to integer coercion is supported with truncation
        assert_eq!(value.coerce_to_integer().unwrap(), 3);
        // Float to boolean coercion is not supported
        assert!(value.coerce_to_boolean().is_err());

        // Test liquid conversion
        let liquid_value = value.to_liquid_value();
        assert_eq!(liquid_value, liquid::model::Value::scalar(3.15));
    }

    #[test]
    fn test_config_value_boolean_operations() {
        let value_true = ConfigValue::Boolean(true);
        let value_false = ConfigValue::Boolean(false);

        // Test coercion for true
        assert!(value_true.coerce_to_boolean().unwrap());
        assert_eq!(value_true.coerce_to_string().unwrap(), "true");
        // Boolean to integer coercion is supported: true -> 1, false -> 0
        assert_eq!(value_true.coerce_to_integer().unwrap(), 1);

        // Test coercion for false
        assert!(!value_false.coerce_to_boolean().unwrap());
        assert_eq!(value_false.coerce_to_string().unwrap(), "false");
        assert_eq!(value_false.coerce_to_integer().unwrap(), 0);

        // Test liquid conversion
        let liquid_value = value_true.to_liquid_value();
        assert_eq!(liquid_value, liquid::model::Value::scalar(true));
    }

    #[test]
    fn test_config_value_array_operations() {
        let array = vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::Integer(42),
            ConfigValue::Boolean(true),
        ];
        let value = ConfigValue::Array(array.clone());

        // Test array access
        assert_eq!(value.coerce_to_array().unwrap(), &array);
        assert!(value.coerce_to_string().is_err());

        // Test liquid conversion
        let liquid_value = value.to_liquid_value();
        if let liquid::model::Value::Array(liquid_array) = liquid_value {
            assert_eq!(liquid_array.len(), 3);
            assert_eq!(liquid_array[0], liquid::model::Value::scalar("item1"));
            assert_eq!(liquid_array[1], liquid::model::Value::scalar(42));
            assert_eq!(liquid_array[2], liquid::model::Value::scalar(true));
        } else {
            panic!("Expected array value");
        }
    }

    #[test]
    fn test_config_value_table_operations() {
        let mut table = HashMap::new();
        table.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        table.insert("key2".to_string(), ConfigValue::Integer(100));
        let value = ConfigValue::Table(table.clone());

        // Test table access
        assert_eq!(value.coerce_to_table().unwrap(), &table);
        assert!(value.coerce_to_string().is_err());

        // Test liquid conversion
        let liquid_value = value.to_liquid_value();
        if let liquid::model::Value::Object(liquid_object) = liquid_value {
            assert_eq!(liquid_object.len(), 2);
            assert_eq!(
                liquid_object.get("key1"),
                Some(&liquid::model::Value::scalar("value1"))
            );
            assert_eq!(
                liquid_object.get("key2"),
                Some(&liquid::model::Value::scalar(100))
            );
        } else {
            panic!("Expected object value");
        }
    }

    #[test]
    fn test_config_value_validation() {
        // Test string length validation
        let long_string = "a".repeat(ValidationLimits::default().max_string_length + 1);
        let value = ConfigValue::String(long_string);
        assert!(value.validate(0).is_err());

        // Test array length validation
        let large_array =
            vec![ConfigValue::Integer(1); ValidationLimits::default().max_array_length + 1];
        let value = ConfigValue::Array(large_array);
        assert!(value.validate(0).is_err());

        // Test nesting depth validation
        let mut nested_table = HashMap::new();
        nested_table.insert("deep".to_string(), ConfigValue::String("value".to_string()));
        for _i in 0..ValidationLimits::default().max_nesting_depth {
            let mut outer_table = HashMap::new();
            outer_table.insert("nested".to_string(), ConfigValue::Table(nested_table));
            nested_table = outer_table;
        }
        let deep_value = ConfigValue::Table(nested_table);
        assert!(deep_value.validate(0).is_err());
    }

    #[test]
    fn test_environment_variable_substitution() {
        // Set up test environment variables with unique names to avoid conflicts
        let test_var_key = format!("TEST_VAR_{}", std::process::id());
        let test_number_key = format!("TEST_NUMBER_{}", std::process::id());
        std::env::set_var(&test_var_key, "test_value");
        std::env::set_var(&test_number_key, "123");

        // Test simple substitution
        let mut value = ConfigValue::String(format!("${{{}}}", test_var_key));
        value.substitute_env_vars().unwrap();
        assert_eq!(value, ConfigValue::String("test_value".to_string()));

        // Test substitution with default
        let nonexistent_key = format!("NONEXISTENT_VAR_{}", std::process::id());
        let mut value = ConfigValue::String(format!("${{{nonexistent_key}:-default_value}}"));
        value.substitute_env_vars().unwrap();
        assert_eq!(value, ConfigValue::String("default_value".to_string()));

        // Test mixed substitution
        let mut value = ConfigValue::String(format!("prefix_${{{}}}_suffix", test_var_key));
        value.substitute_env_vars().unwrap();
        assert_eq!(
            value,
            ConfigValue::String("prefix_test_value_suffix".to_string())
        );

        // Test array substitution
        let array = vec![
            ConfigValue::String(format!("${{{}}}", test_var_key)),
            ConfigValue::String(format!("${{{}}}", test_number_key)),
        ];
        let mut value = ConfigValue::Array(array);
        value.substitute_env_vars().unwrap();
        if let ConfigValue::Array(result_array) = value {
            assert_eq!(
                result_array[0],
                ConfigValue::String("test_value".to_string())
            );
            assert_eq!(result_array[1], ConfigValue::String("123".to_string()));
        } else {
            panic!("Expected array after substitution");
        }

        // Test table substitution
        let mut table = HashMap::new();
        table.insert(
            "key1".to_string(),
            ConfigValue::String(format!("${{{}}}", test_var_key)),
        );
        table.insert(
            "key2".to_string(),
            ConfigValue::String(format!("${{{}}}", test_number_key)),
        );
        let mut value = ConfigValue::Table(table);
        value.substitute_env_vars().unwrap();
        if let ConfigValue::Table(result_table) = value {
            assert_eq!(
                result_table.get("key1"),
                Some(&ConfigValue::String("test_value".to_string()))
            );
            assert_eq!(
                result_table.get("key2"),
                Some(&ConfigValue::String("123".to_string()))
            );
        } else {
            panic!("Expected table after substitution");
        }

        // Clean up
        std::env::remove_var(&test_var_key);
        std::env::remove_var(&test_number_key);
    }

    #[test]
    fn test_environment_variable_substitution_errors() {
        // Test missing required variable
        let missing_var_key = format!("REQUIRED_MISSING_VAR_{}", std::process::id());
        let mut value = ConfigValue::String(format!("${{{}}}", missing_var_key));
        assert!(value.substitute_env_vars().is_err());

        // Test invalid syntax
        let mut value = ConfigValue::String("${INVALID".to_string());
        value.substitute_env_vars().unwrap(); // Should not substitute invalid syntax
        assert_eq!(value, ConfigValue::String("${INVALID".to_string()));
    }

    proptest! {
        #[test]
        fn test_config_value_roundtrip_serialization(
            s in "[a-zA-Z0-9_]{1,100}",
            i in any::<i64>(),
            f in any::<f64>(),
            b in any::<bool>()
        ) {
            // Test string roundtrip
            let value = ConfigValue::String(s.clone());
            let json = serde_json::to_value(&value).unwrap();
            let deserialized: ConfigValue = serde_json::from_value(json).unwrap();
            assert_eq!(value, deserialized);

            // Test integer roundtrip
            let value = ConfigValue::Integer(i);
            let json = serde_json::to_value(&value).unwrap();
            let deserialized: ConfigValue = serde_json::from_value(json).unwrap();
            assert_eq!(value, deserialized);

            // Test float roundtrip (if finite)
            if f.is_finite() {
                let value = ConfigValue::Float(f);
                let json = serde_json::to_value(&value).unwrap();
                let deserialized: ConfigValue = serde_json::from_value(json).unwrap();
                assert_eq!(value, deserialized);
            }

            // Test boolean roundtrip
            let value = ConfigValue::Boolean(b);
            let json = serde_json::to_value(&value).unwrap();
            let deserialized: ConfigValue = serde_json::from_value(json).unwrap();
            assert_eq!(value, deserialized);
        }

        #[test]
        fn test_config_value_liquid_conversion_consistency(
            s in "[a-zA-Z0-9_\\s]{1,50}",
            i in -1000000i64..1000000i64,
            f in -1000000.0f64..1000000.0f64,
            b in any::<bool>()
        ) {
            // Test that liquid conversion is consistent
            let string_value = ConfigValue::String(s.clone());
            let liquid_string = string_value.to_liquid_value();
            let expected_string = string_value.coerce_to_string().unwrap();
            assert_eq!(liquid_string, liquid::model::Value::scalar(expected_string));

            let int_value = ConfigValue::Integer(i);
            let liquid_int = int_value.to_liquid_value();
            assert_eq!(liquid_int, liquid::model::Value::scalar(i));

            if f.is_finite() {
                let float_value = ConfigValue::Float(f);
                let liquid_float = float_value.to_liquid_value();
                assert_eq!(liquid_float, liquid::model::Value::scalar(f));
            }

            let bool_value = ConfigValue::Boolean(b);
            let liquid_bool = bool_value.to_liquid_value();
            assert_eq!(liquid_bool, liquid::model::Value::scalar(b));
        }

        #[test]
        fn test_environment_variable_pattern_matching(
            var_name in "[A-Z][A-Z0-9_]{0,30}",
            default_value in "[a-zA-Z0-9_]{0,20}"
        ) {
            // Test that environment variable patterns are parsed correctly
            let pattern_with_default = format!("${{{var_name}:-{default_value}}}");
            let pattern_without_default = format!("${{{var_name}}}");

            let mut value_with_default = ConfigValue::String(pattern_with_default.clone());
            let mut value_without_default = ConfigValue::String(pattern_without_default.clone());

            // If environment variable doesn't exist, substitution should use default or fail
            if std::env::var(&var_name).is_err() {
                // With default should succeed and use the default value
                let result_with_default = value_with_default.substitute_env_vars();
                if result_with_default.is_ok() {
                    assert_eq!(value_with_default.coerce_to_string().unwrap(), default_value);
                } else {
                    // If substitution fails, the original pattern should remain
                    assert_eq!(value_with_default.coerce_to_string().unwrap(), pattern_with_default);
                }

                // Without default should fail
                let result_without_default = value_without_default.substitute_env_vars();
                assert!(result_without_default.is_err(), "Environment variable substitution should fail when variable doesn't exist and no default is provided");
            }
        }

        #[test]
        fn test_configuration_key_validation(
            key in "[a-zA-Z_][a-zA-Z0-9_]{0,30}".prop_filter("Reserved names should be excluded", |k| {
                // Exclude Liquid reserved words
                !matches!(k.as_str(), "for" | "if" | "unless" | "case" | "when" | "else" | "endif" | "endfor" | "endunless" | "endcase" | "break" | "continue" | "assign" | "capture" | "include")
            }),
            value in "[a-zA-Z0-9_\\s]{1,50}"
        ) {
            let mut config = Configuration::new();

            // Valid key should work
            config.insert(key.clone(), ConfigValue::String(value.clone()));
            let validation_result = config.validate();
            assert!(validation_result.is_ok(), "Valid key '{key}' should pass validation");

            // Test some specific invalid keys
            let invalid_keys = vec!["123invalid", "invalid-key", "invalid space"];
            for invalid_key in invalid_keys {
                let mut invalid_config = Configuration::new();
                invalid_config.insert(invalid_key.to_string(), ConfigValue::String(value.clone()));
                let validation_result = invalid_config.validate();
                assert!(validation_result.is_err(), "Invalid key '{invalid_key}' should fail validation");
            }
        }

        #[test]
        fn test_dot_notation_consistency(
            section1 in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
            section2 in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
            key in "[a-zA-Z_][a-zA-Z0-9_]{0,10}",
            value in "[a-zA-Z0-9_\\s]{1,20}"
        ) {
            let mut config = Configuration::new();

            // Set value using dot notation
            let dot_key = format!("{section1}.{section2}.{key}");
            config.set(dot_key.clone(), ConfigValue::String(value.clone()));

            // Retrieve using dot notation should work
            assert_eq!(
                config.get(&dot_key),
                Some(&ConfigValue::String(value.clone()))
            );

            // Check that intermediate tables were created
            assert!(config.contains_key(&section1));
            assert!(config.contains_key(&format!("{section1}.{section2}")));

            // Remove using dot notation should work
            let removed = config.remove(&dot_key);
            assert_eq!(removed, Some(ConfigValue::String(value)));
            assert!(!config.contains_key(&dot_key));
        }
    }
}

/// Test Configuration struct with nested tables and dot notation
mod configuration_tests {
    use super::*;

    #[test]
    fn test_configuration_creation() {
        let config = Configuration::new();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);
        assert_eq!(config.keys().len(), 0);
    }

    #[test]
    fn test_configuration_basic_operations() {
        let mut config = Configuration::new();

        // Insert basic values
        config.insert(
            "name".to_string(),
            ConfigValue::String("test_project".to_string()),
        );
        config.insert(
            "version".to_string(),
            ConfigValue::String("1.0.0".to_string()),
        );
        config.insert("debug".to_string(), ConfigValue::Boolean(true));
        config.insert("port".to_string(), ConfigValue::Integer(8080));

        // Test retrieval
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("test_project".to_string()))
        );
        assert_eq!(
            config.get("version"),
            Some(&ConfigValue::String("1.0.0".to_string()))
        );
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));
        assert_eq!(config.get("port"), Some(&ConfigValue::Integer(8080)));

        // Test contains_key
        assert!(config.contains_key("name"));
        assert!(config.contains_key("version"));
        assert!(!config.contains_key("nonexistent"));

        // Test basic properties
        assert!(!config.is_empty());
        assert_eq!(config.len(), 4);
    }

    #[test]
    fn test_configuration_dot_notation_get() {
        let mut config = Configuration::new();

        // Set up nested structure using dot notation
        config.set(
            "database.host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        config.set("database.port".to_string(), ConfigValue::Integer(5432));
        config.set(
            "database.credentials.username".to_string(),
            ConfigValue::String("user".to_string()),
        );
        config.set(
            "database.credentials.password".to_string(),
            ConfigValue::String("pass".to_string()),
        );

        // Test dot notation access
        assert_eq!(
            config.get("database.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            config.get("database.port"),
            Some(&ConfigValue::Integer(5432))
        );
        assert_eq!(
            config.get("database.credentials.username"),
            Some(&ConfigValue::String("user".to_string()))
        );
        assert_eq!(
            config.get("database.credentials.password"),
            Some(&ConfigValue::String("pass".to_string()))
        );

        // Test intermediate table access
        let database_config = config.get("database");
        assert!(database_config.is_some());
        assert!(matches!(database_config, Some(ConfigValue::Table(_))));

        let credentials = config.get("database.credentials");
        assert!(credentials.is_some());
        assert!(matches!(credentials, Some(ConfigValue::Table(_))));

        // Test contains_key with dot notation
        assert!(config.contains_key("database.host"));
        assert!(config.contains_key("database.credentials.username"));
        assert!(!config.contains_key("database.nonexistent"));
        assert!(!config.contains_key("database.credentials.nonexistent"));
    }

    #[test]
    fn test_configuration_dot_notation_set() {
        let mut config = Configuration::new();

        // Test creating nested structure via dot notation
        config.set(
            "app.name".to_string(),
            ConfigValue::String("TestApp".to_string()),
        );
        config.set(
            "app.version".to_string(),
            ConfigValue::String("2.0.0".to_string()),
        );
        config.set("app.features.auth".to_string(), ConfigValue::Boolean(true));
        config.set("app.features.api".to_string(), ConfigValue::Boolean(true));
        config.set(
            "app.database.primary.host".to_string(),
            ConfigValue::String("db1.example.com".to_string()),
        );
        config.set(
            "app.database.primary.port".to_string(),
            ConfigValue::Integer(5432),
        );

        // Verify structure was created correctly
        assert_eq!(
            config.get("app.name"),
            Some(&ConfigValue::String("TestApp".to_string()))
        );
        assert_eq!(
            config.get("app.version"),
            Some(&ConfigValue::String("2.0.0".to_string()))
        );
        assert_eq!(
            config.get("app.features.auth"),
            Some(&ConfigValue::Boolean(true))
        );
        assert_eq!(
            config.get("app.features.api"),
            Some(&ConfigValue::Boolean(true))
        );
        assert_eq!(
            config.get("app.database.primary.host"),
            Some(&ConfigValue::String("db1.example.com".to_string()))
        );
        assert_eq!(
            config.get("app.database.primary.port"),
            Some(&ConfigValue::Integer(5432))
        );

        // Test overwriting existing values
        config.set(
            "app.name".to_string(),
            ConfigValue::String("UpdatedApp".to_string()),
        );
        assert_eq!(
            config.get("app.name"),
            Some(&ConfigValue::String("UpdatedApp".to_string()))
        );

        // Test replacing table with scalar value
        config.set(
            "app.features".to_string(),
            ConfigValue::String("disabled".to_string()),
        );
        assert_eq!(
            config.get("app.features"),
            Some(&ConfigValue::String("disabled".to_string()))
        );
        assert!(config.get("app.features.auth").is_none()); // Should be gone
    }

    #[test]
    fn test_configuration_remove() {
        let mut config = Configuration::new();

        // Set up test data
        config.set(
            "simple".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config.set(
            "nested.key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config.set(
            "nested.key2".to_string(),
            ConfigValue::String("value2".to_string()),
        );
        config.set("deep.nested.key".to_string(), ConfigValue::Boolean(true));

        // Test removing simple key
        let removed = config.remove("simple");
        assert_eq!(removed, Some(ConfigValue::String("value".to_string())));
        assert!(!config.contains_key("simple"));

        // Test removing nested key
        let removed = config.remove("nested.key1");
        assert_eq!(removed, Some(ConfigValue::String("value1".to_string())));
        assert!(!config.contains_key("nested.key1"));
        assert!(config.contains_key("nested.key2")); // Other key should remain

        // Test removing deeply nested key
        let removed = config.remove("deep.nested.key");
        assert_eq!(removed, Some(ConfigValue::Boolean(true)));
        assert!(!config.contains_key("deep.nested.key"));

        // Test removing non-existent key
        let removed = config.remove("nonexistent");
        assert_eq!(removed, None);

        let removed = config.remove("nested.nonexistent");
        assert_eq!(removed, None);
    }

    #[test]
    fn test_configuration_keys() {
        let mut config = Configuration::new();

        config.set(
            "simple".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config.set("nested.key1".to_string(), ConfigValue::Integer(1));
        config.set("nested.key2".to_string(), ConfigValue::Integer(2));
        config.set("deep.nested.key".to_string(), ConfigValue::Boolean(true));

        let keys = config.keys();

        // Check that all expected keys are present
        assert!(keys.contains(&"simple".to_string()));
        assert!(keys.contains(&"nested".to_string()));
        assert!(keys.contains(&"nested.key1".to_string()));
        assert!(keys.contains(&"nested.key2".to_string()));
        assert!(keys.contains(&"deep".to_string()));
        assert!(keys.contains(&"deep.nested".to_string()));
        assert!(keys.contains(&"deep.nested.key".to_string()));
    }

    #[test]
    fn test_configuration_merge() {
        let mut config1 = Configuration::new();
        config1.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config1.insert(
            "shared".to_string(),
            ConfigValue::String("original".to_string()),
        );
        config1.insert(
            "nested".to_string(),
            ConfigValue::Table({
                let mut table = HashMap::new();
                table.insert("a".to_string(), ConfigValue::Integer(1));
                table
            }),
        );

        let mut config2 = Configuration::new();
        config2.insert(
            "key2".to_string(),
            ConfigValue::String("value2".to_string()),
        );
        config2.insert(
            "shared".to_string(),
            ConfigValue::String("updated".to_string()),
        );
        config2.insert(
            "nested".to_string(),
            ConfigValue::Table({
                let mut table = HashMap::new();
                table.insert("b".to_string(), ConfigValue::Integer(2));
                table
            }),
        );

        // Test merge without overwrite
        let mut merged = config1.clone();
        merged.merge(config2.clone(), false);

        assert_eq!(
            merged.get("key1"),
            Some(&ConfigValue::String("value1".to_string()))
        );
        assert_eq!(
            merged.get("key2"),
            Some(&ConfigValue::String("value2".to_string()))
        );
        assert_eq!(
            merged.get("shared"),
            Some(&ConfigValue::String("original".to_string()))
        ); // Should not overwrite

        // Test merge with overwrite
        let mut merged = config1.clone();
        merged.merge(config2, true);

        assert_eq!(
            merged.get("key1"),
            Some(&ConfigValue::String("value1".to_string()))
        );
        assert_eq!(
            merged.get("key2"),
            Some(&ConfigValue::String("value2".to_string()))
        );
        assert_eq!(
            merged.get("shared"),
            Some(&ConfigValue::String("updated".to_string()))
        ); // Should overwrite
    }

    #[test]
    fn test_configuration_liquid_conversion() {
        let mut config = Configuration::new();

        // Set up test configuration
        config.insert(
            "name".to_string(),
            ConfigValue::String("TestProject".to_string()),
        );
        config.insert(
            "version".to_string(),
            ConfigValue::String("1.0.0".to_string()),
        );
        config.insert("debug".to_string(), ConfigValue::Boolean(true));
        config.insert(
            "features".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("auth".to_string()),
                ConfigValue::String("api".to_string()),
            ]),
        );

        let mut database_table = HashMap::new();
        database_table.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        database_table.insert("port".to_string(), ConfigValue::Integer(5432));
        config.insert("database".to_string(), ConfigValue::Table(database_table));

        // Convert to liquid object
        let liquid_object = config.to_liquid_object();

        // Verify conversion
        assert_eq!(
            liquid_object.get("name"),
            Some(&liquid::model::Value::scalar("TestProject"))
        );
        assert_eq!(
            liquid_object.get("version"),
            Some(&liquid::model::Value::scalar("1.0.0"))
        );
        assert_eq!(
            liquid_object.get("debug"),
            Some(&liquid::model::Value::scalar(true))
        );

        // Test array conversion
        if let Some(liquid::model::Value::Array(features)) = liquid_object.get("features") {
            assert_eq!(features.len(), 2);
            assert_eq!(features[0], liquid::model::Value::scalar("auth"));
            assert_eq!(features[1], liquid::model::Value::scalar("api"));
        } else {
            panic!("Expected array for features");
        }

        // Test nested object conversion
        if let Some(liquid::model::Value::Object(database)) = liquid_object.get("database") {
            assert_eq!(
                database.get("host"),
                Some(&liquid::model::Value::scalar("localhost"))
            );
            assert_eq!(
                database.get("port"),
                Some(&liquid::model::Value::scalar(5432))
            );
        } else {
            panic!("Expected object for database");
        }
    }

    #[test]
    fn test_configuration_validation() {
        let mut config = Configuration::new();

        // Test valid configuration
        config.insert(
            "valid_name".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config.insert("another_valid".to_string(), ConfigValue::Integer(42));
        assert!(config.validate().is_ok());

        // Test invalid variable name
        config.insert(
            "123invalid".to_string(),
            ConfigValue::String("value".to_string()),
        );
        assert!(config.validate().is_err());

        // Clean up for next test
        config.remove("123invalid");

        // Test reserved variable name
        config.insert("for".to_string(), ConfigValue::String("value".to_string()));
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_configuration_environment_substitution() {
        // Set up test environment with unique names to avoid conflicts
        let config_var_key = format!("CONFIG_TEST_VAR_{}", std::process::id());
        let config_number_key = format!("CONFIG_NUMBER_{}", std::process::id());
        std::env::set_var(&config_var_key, "substituted_value");
        std::env::set_var(&config_number_key, "999");

        let mut config = Configuration::new();
        config.insert(
            "simple".to_string(),
            ConfigValue::String(format!("${{{}}}", config_var_key)),
        );
        let nonexistent_key = format!("NONEXISTENT_{}", std::process::id());
        config.insert(
            "with_default".to_string(),
            ConfigValue::String(format!("${{{nonexistent_key}:-default}}")),
        );
        config.insert(
            "number".to_string(),
            ConfigValue::String(format!("${{{}}}", config_number_key)),
        );

        // Test array with env vars
        let array = vec![
            ConfigValue::String(format!("${{{}}}", config_var_key)),
            ConfigValue::String("literal".to_string()),
        ];
        config.insert("array_test".to_string(), ConfigValue::Array(array));

        // Test nested table with env vars
        let mut table = HashMap::new();
        table.insert(
            "nested_var".to_string(),
            ConfigValue::String(format!("${{{}}}", config_var_key)),
        );
        config.insert("table_test".to_string(), ConfigValue::Table(table));

        // Perform substitution
        config.substitute_env_vars().unwrap();

        // Verify substitution results
        assert_eq!(
            config.get("simple"),
            Some(&ConfigValue::String("substituted_value".to_string()))
        );
        assert_eq!(
            config.get("with_default"),
            Some(&ConfigValue::String("default".to_string()))
        );
        assert_eq!(
            config.get("number"),
            Some(&ConfigValue::String("999".to_string()))
        );

        // Verify array substitution
        if let Some(ConfigValue::Array(array)) = config.get("array_test") {
            assert_eq!(
                array[0],
                ConfigValue::String("substituted_value".to_string())
            );
            assert_eq!(array[1], ConfigValue::String("literal".to_string()));
        } else {
            panic!("Expected array");
        }

        // Verify table substitution
        if let Some(ConfigValue::Table(table)) = config.get("table_test") {
            assert_eq!(
                table.get("nested_var"),
                Some(&ConfigValue::String("substituted_value".to_string()))
            );
        } else {
            panic!("Expected table");
        }

        // Clean up
        std::env::remove_var(&config_var_key);
        std::env::remove_var(&config_number_key);
    }
}

/// Test TOML parser with various valid and invalid inputs
mod parser_tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_parser_valid_toml() {
        let parser = ConfigParser::new();

        let toml_content = r#"
            name = "TestProject"
            version = "1.0.0"
            debug = true
            port = 8080
            timeout = 30.5
            
            keywords = ["rust", "cli", "config"]
            
            [database]
            host = "localhost"
            port = 5432
            
            [database.credentials]
            username = "user"
            password = "pass"
            
            [build]
            features = ["feature1", "feature2"]
            optimized = true
        "#;

        let config = parser.parse_string(toml_content, None).unwrap();

        // Test basic values
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("TestProject".to_string()))
        );
        assert_eq!(
            config.get("version"),
            Some(&ConfigValue::String("1.0.0".to_string()))
        );
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));
        assert_eq!(config.get("port"), Some(&ConfigValue::Integer(8080)));
        assert_eq!(config.get("timeout"), Some(&ConfigValue::Float(30.5)));

        // Test arrays
        if let Some(ConfigValue::Array(keywords)) = config.get("keywords") {
            assert_eq!(keywords.len(), 3);
            assert_eq!(keywords[0], ConfigValue::String("rust".to_string()));
            assert_eq!(keywords[1], ConfigValue::String("cli".to_string()));
            assert_eq!(keywords[2], ConfigValue::String("config".to_string()));
        } else {
            panic!("Expected array for keywords");
        }

        // Test nested structures
        assert_eq!(
            config.get("database.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            config.get("database.port"),
            Some(&ConfigValue::Integer(5432))
        );
        assert_eq!(
            config.get("database.credentials.username"),
            Some(&ConfigValue::String("user".to_string()))
        );
        assert_eq!(
            config.get("database.credentials.password"),
            Some(&ConfigValue::String("pass".to_string()))
        );

        // Test build configuration
        assert_eq!(
            config.get("build.optimized"),
            Some(&ConfigValue::Boolean(true))
        );
        if let Some(ConfigValue::Array(features)) = config.get("build.features") {
            assert_eq!(features.len(), 2);
            assert_eq!(features[0], ConfigValue::String("feature1".to_string()));
            assert_eq!(features[1], ConfigValue::String("feature2".to_string()));
        } else {
            panic!("Expected array for build.features");
        }
    }

    #[test]
    fn test_parser_invalid_toml() {
        let parser = ConfigParser::new();

        // Test invalid syntax
        let invalid_toml = r#"
            name = "unclosed string
            invalid = syntax
        "#;

        let result = parser.parse_string(invalid_toml, None);
        assert!(result.is_err());

        // Test malformed table
        let malformed_table = r#"
            [incomplete
            name = "value"
        "#;

        let result = parser.parse_string(malformed_table, None);
        assert!(result.is_err());

        // Test invalid key names
        let invalid_keys = r#"
            123invalid = "value"
            "key with spaces" = "value"
        "#;

        let result = parser.parse_string(invalid_keys, None);
        // Note: Parser should accept this but validation should catch it
        if let Ok(config) = result {
            assert!(config.validate().is_err());
        }
    }

    #[test]
    fn test_parser_file_operations() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let config_path = temp_dir.path().join("test.toml");

        let toml_content = r#"
            name = "FileTest"
            version = "2.0.0"
            
            [settings]
            debug = false
            timeout = 60
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let parser = ConfigParser::new();
        let config = parser.parse_file(&config_path).unwrap();

        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("FileTest".to_string()))
        );
        assert_eq!(
            config.get("version"),
            Some(&ConfigValue::String("2.0.0".to_string()))
        );
        assert_eq!(
            config.get("settings.debug"),
            Some(&ConfigValue::Boolean(false))
        );
        assert_eq!(
            config.get("settings.timeout"),
            Some(&ConfigValue::Integer(60))
        );

        // Test file path is preserved
        assert_eq!(config.file_path(), Some(&config_path));
    }

    #[test]
    fn test_parser_environment_variables() {
        // Set up test environment with unique names to avoid conflicts
        let parser_var_key = format!("PARSER_TEST_VAR_{}", std::process::id());
        let parser_port_key = format!("PARSER_TEST_PORT_{}", std::process::id());
        std::env::set_var(&parser_var_key, "env_value");
        std::env::set_var(&parser_port_key, "9000");

        let parser = ConfigParser::new();

        let nonexistent_var_key = format!("NONEXISTENT_VAR_{}", std::process::id());
        let toml_content = format!(
            r#"
            name = "EnvTest"
            database_url = "${{{}}}"
            port = "${{{}}}"
            fallback = "${{{}:-fallback_value}}"
            mixed = "prefix_${{{}}}_suffix"
        "#,
            parser_var_key, parser_port_key, nonexistent_var_key, parser_var_key
        );

        let config = parser.parse_string(&toml_content, None).unwrap();

        // Note: Parser automatically performs environment variable substitution
        // so we test the final result after substitution
        assert_eq!(
            config.get("database_url"),
            Some(&ConfigValue::String("env_value".to_string()))
        );
        assert_eq!(
            config.get("port"),
            Some(&ConfigValue::String("9000".to_string()))
        );
        assert_eq!(
            config.get("fallback"),
            Some(&ConfigValue::String("fallback_value".to_string()))
        );
        assert_eq!(
            config.get("mixed"),
            Some(&ConfigValue::String("prefix_env_value_suffix".to_string()))
        );

        // Clean up
        std::env::remove_var(&parser_var_key);
        std::env::remove_var(&parser_port_key);
    }

    #[test]
    fn test_parser_load_from_repo_root() {
        use std::panic;

        let temp_dir = crate::test_utils::create_temp_dir_with_retry();

        // Create .git directory to simulate repository root
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        // Create sah.toml in repo root
        let config_path = temp_dir.path().join("sah.toml");
        let toml_content = r#"
            name = "RepoTest"
            version = "3.0.0"
        "#;
        fs::write(&config_path, toml_content).unwrap();

        // Create subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Change to subdirectory and test loading with proper cleanup
        let original_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => {
                // Current directory is invalid, skip this test
                return;
            }
        };

        if std::env::set_current_dir(&sub_dir).is_err() {
            // Failed to change directory, skip this test
            return;
        }

        // Use panic::catch_unwind to ensure directory is restored even on panic
        let result = panic::catch_unwind(|| {
            let parser = ConfigParser::new();
            parser.load_from_repo_root()
        });

        // Always restore original directory
        let _ = std::env::set_current_dir(original_dir);

        let config_result = result.unwrap().unwrap();
        assert!(config_result.is_some());
        let config = config_result.unwrap();
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("RepoTest".to_string()))
        );
        assert_eq!(
            config.get("version"),
            Some(&ConfigValue::String("3.0.0".to_string()))
        );
    }

    #[test]
    fn test_parser_file_not_found() {
        let parser = ConfigParser::new();
        let result = parser.parse_file(std::path::Path::new("nonexistent.toml"));
        assert!(result.is_err());

        if let Err(ConfigError::Io(_)) = result {
            // Expected error type
        } else {
            panic!("Expected IO error for missing file");
        }
    }
}
