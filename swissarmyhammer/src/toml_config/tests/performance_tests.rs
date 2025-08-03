//! Performance and caching tests for sah.toml configuration system
//!
//! This module tests performance characteristics, memory usage, and
//! caching behavior of the configuration system.

use crate::toml_config::*;
use std::fs;
use std::time::{Duration, Instant};
use tempfile::TempDir;

/// Test performance characteristics of configuration operations
mod performance_tests {
    use super::*;

    #[test]
    fn test_parsing_performance() {
        // Create a reasonably complex configuration file
        let complex_toml = create_complex_config_content(1000);

        // Measure parsing time
        let start = Instant::now();
        let parser = ConfigParser::new();
        let config = parser.parse_string(&complex_toml, None).unwrap();
        let parse_duration = start.elapsed();

        // Parsing should complete within reasonable time (< 100ms for 1000 items)
        assert!(parse_duration < Duration::from_millis(100));

        // Verify the configuration was parsed correctly
        assert!(!config.is_empty());
        assert!(config.len() > 900); // Should have most of the generated items

        println!(
            "Parsed {} config items in {:?}",
            config.len(),
            parse_duration
        );
    }

    #[test]
    fn test_large_file_performance() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("large_config.toml");

        // Create a large but valid configuration file
        let large_content = create_complex_config_content(5000);
        fs::write(&config_path, &large_content).unwrap();

        let parser = ConfigParser::new();

        // Measure file loading and parsing time
        let start = Instant::now();
        let config = parser.parse_file(&config_path).unwrap();
        let total_duration = start.elapsed();

        // Large file should still parse reasonably quickly (< 500ms)
        assert!(total_duration < Duration::from_millis(500));

        // Verify content integrity
        assert!(config.len() > 4000);
        assert!(config.contains_key("section_0"));
        assert!(config.contains_key("section_100"));

        println!(
            "Loaded large config file ({} bytes) in {:?}",
            large_content.len(),
            total_duration
        );
    }

    #[test]
    fn test_dot_notation_access_performance() {
        let config = create_large_nested_config(100);

        // Measure dot notation access time
        let test_keys = [
            "section_0.subsection_0.item_0",
            "section_50.subsection_25.item_10",
            "section_99.subsection_99.item_99",
            "section_10.subsection_90.item_5",
        ];

        let start = Instant::now();
        for _ in 0..1000 {
            for key in &test_keys {
                let _value = config.get(key);
            }
        }
        let access_duration = start.elapsed();

        // 4000 access operations should complete quickly (< 10ms)
        assert!(access_duration < Duration::from_millis(10));

        println!(
            "Performed 4000 dot notation accesses in {access_duration:?}"
        );
    }

    #[test]
    fn test_contains_key_performance() {
        let config = create_large_nested_config(100);

        let test_keys: Vec<String> = (0..100)
            .map(|i| format!("section_{i}.subsection_{}.item_{}", i % 50, i % 10))
            .collect();

        let start = Instant::now();
        for _ in 0..1000 {
            for key in &test_keys {
                let _exists = config.contains_key(key);
            }
        }
        let contains_duration = start.elapsed();

        // 100,000 contains_key operations should be fast (< 500ms)
        assert!(contains_duration < Duration::from_millis(500));

        println!(
            "Performed 100,000 contains_key operations in {contains_duration:?}"
        );
    }

    #[test]
    fn test_environment_substitution_performance() {
        // Set up test environment variables
        for i in 0..100 {
            std::env::set_var(format!("PERF_TEST_VAR_{i}"), format!("value_{i}"));
        }

        let mut config = create_config_with_env_vars(100);

        let start = Instant::now();
        config.substitute_env_vars().unwrap();
        let substitution_duration = start.elapsed();

        // Environment variable substitution should be reasonably fast
        assert!(substitution_duration < Duration::from_millis(50));

        // Verify substitution worked
        assert_eq!(
            config.get("env_var_0").unwrap().coerce_to_string().unwrap(),
            "value_0"
        );
        assert_eq!(
            config
                .get("env_var_99")
                .unwrap()
                .coerce_to_string()
                .unwrap(),
            "value_99"
        );

        println!(
            "Performed environment substitution on 100 variables in {substitution_duration:?}"
        );

        // Clean up environment variables
        for i in 0..100 {
            std::env::remove_var(format!("PERF_TEST_VAR_{i}"));
        }
    }

    #[test]
    fn test_validation_performance() {
        let config = create_large_nested_config(200);

        let start = Instant::now();
        let validation_result = config.validate();
        let validation_duration = start.elapsed();

        // Validation should complete quickly even for large configs
        assert!(validation_duration < Duration::from_millis(20));
        assert!(validation_result.is_ok());

        println!("Validated large configuration in {validation_duration:?}");
    }

    #[test]
    fn test_liquid_conversion_performance() {
        let config = create_large_nested_config(100);

        let start = Instant::now();
        let liquid_object = config.to_liquid_object();
        let conversion_duration = start.elapsed();

        // Liquid conversion should be reasonably fast
        assert!(conversion_duration < Duration::from_millis(100));

        // Verify conversion worked
        assert!(!liquid_object.is_empty());

        println!(
            "Converted configuration to liquid object in {conversion_duration:?}"
        );
    }

    #[test]
    fn test_keys_generation_performance() {
        let config = create_large_nested_config(100);

        let start = Instant::now();
        let keys = config.keys();
        let keys_duration = start.elapsed();

        // Keys generation should be fast
        assert!(keys_duration < Duration::from_millis(20));

        // Should have generated many keys including nested ones
        assert!(keys.len() > 1000);

        println!("Generated {} keys in {:?}", keys.len(), keys_duration);
    }

    #[test]
    fn test_concurrent_access_performance() {
        use std::sync::Arc;
        use std::thread;

        let config = Arc::new(create_large_nested_config(50));
        let mut handles = vec![];

        let start = Instant::now();

        // Spawn multiple threads to access configuration concurrently
        for thread_id in 0..4 {
            let config_clone = config.clone();
            let handle = thread::spawn(move || {
                let mut access_count = 0;
                for i in 0..1000 {
                    let key = format!(
                        "section_{}.subsection_{}.item_{}",
                        i % 50,
                        (i + thread_id) % 25,
                        i % 10
                    );
                    if config_clone.contains_key(&key) {
                        let _value = config_clone.get(&key);
                        access_count += 1;
                    }
                }
                access_count
            });
            handles.push(handle);
        }

        let mut total_accesses = 0;
        for handle in handles {
            total_accesses += handle.join().unwrap();
        }

        let concurrent_duration = start.elapsed();

        // Concurrent access should not be significantly slower than serial access
        assert!(concurrent_duration < Duration::from_millis(100));
        assert!(total_accesses > 0);

        println!(
            "Performed {total_accesses} concurrent accesses in {concurrent_duration:?}"
        );
    }

    fn create_complex_config_content(num_sections: usize) -> String {
        let mut content = String::new();

        for i in 0..num_sections {
            content.push_str(&format!(
                r#"
[section_{}]
name = "Section {}"
id = {}
enabled = {}
priority = {}.5

"#,
                i,
                i,
                i,
                i % 2 == 0,
                i as f64
            ));

            // Add array every 10 sections
            if i % 10 == 0 {
                content.push_str(&format!(
                    r#"items = ["item_{}", "item_{}", "item_{}"]
"#,
                    i,
                    i + 1,
                    i + 2
                ));
            }

            // Add nested subsection every 5 sections
            if i % 5 == 0 {
                content.push_str(&format!(
                    r#"
[section_{}.subsection]
nested_value = "nested_{}"
nested_count = {}

"#,
                    i,
                    i,
                    i * 2
                ));
            }
        }

        content
    }

    pub fn create_large_nested_config(sections: usize) -> Configuration {
        let mut config = Configuration::new();

        for i in 0..sections {
            for j in 0..10 {
                for k in 0..10 {
                    let key = format!("section_{i}.subsection_{j}.item_{k}");
                    let value = ConfigValue::String(format!("value_{i}_{j}"));
                    config.set(key, value);
                }
            }
        }

        config
    }

    fn create_config_with_env_vars(count: usize) -> Configuration {
        let mut config = Configuration::new();

        for i in 0..count {
            let key = format!("env_var_{i}");
            let value = ConfigValue::String(format!("${{PERF_TEST_VAR_{i}}}"));
            config.insert(key, value);
        }

        config
    }
}

/// Test memory usage and resource management
mod memory_tests {
    use super::*;

    #[test]
    fn test_memory_usage_scaling() {
        // Test that memory usage scales reasonably with config size
        let small_config = create_config_of_size(100);
        let large_config = create_config_of_size(1000);

        // We can't directly measure memory usage in a unit test,
        // but we can test that operations complete without issues
        assert!(small_config.len() > 80); // Allow for some overhead
        assert!(large_config.len() > 800);

        // Test that large config can still perform operations efficiently
        let start = Instant::now();
        let _keys = large_config.keys();
        let keys_duration = start.elapsed();

        assert!(keys_duration < Duration::from_millis(50));
    }

    #[test]
    fn test_deep_nesting_memory() {
        // Test memory usage with deep nesting (but within limits)
        let deep_config = create_deep_config(8); // Close to MAX_NESTING_DEPTH

        // Should be able to validate without stack overflow
        assert!(deep_config.validate().is_ok());

        // Should be able to access deeply nested values
        assert!(deep_config
            .contains_key("level_0.level_1.level_2.level_3.level_4.level_5.level_6.level_7.value"));
    }

    #[test]
    fn test_large_array_memory() {
        // Test memory usage with large arrays (but within limits)
        let large_array = (0..1000)
            .map(|i| ConfigValue::String(format!("item_{i}")))
            .collect();

        let mut config = Configuration::new();
        config.insert("large_array".to_string(), ConfigValue::Array(large_array));

        // Should be able to validate and access
        assert!(config.validate().is_ok());

        let array = config
            .get("large_array")
            .unwrap()
            .coerce_to_array()
            .unwrap();
        assert_eq!(array.len(), 1000);
        assert_eq!(array[999], ConfigValue::String("item_999".to_string()));
    }

    #[test]
    fn test_string_memory_efficiency() {
        // Test memory usage with various string sizes
        let mut config = Configuration::new();

        // Small strings
        for i in 0..100 {
            config.insert(
                format!("small_{i}"),
                ConfigValue::String(format!("value_{i}")),
            );
        }

        // Medium strings
        for i in 0..10 {
            let medium_string = "x".repeat(1000);
            config.insert(format!("medium_{i}"), ConfigValue::String(medium_string));
        }

        // Large string (but within limits)
        let large_string = "x".repeat(10000);
        config.insert("large".to_string(), ConfigValue::String(large_string));

        assert!(config.validate().is_ok());
        assert_eq!(
            config
                .get("large")
                .unwrap()
                .coerce_to_string()
                .unwrap()
                .len(),
            10000
        );
    }

    #[test]
    fn test_configuration_cloning() {
        let original = create_config_of_size(100);

        // Test that cloning works and doesn't cause memory issues
        let start = Instant::now();
        let cloned = original.clone();
        let clone_duration = start.elapsed();

        // Cloning should be reasonably fast
        assert!(clone_duration < Duration::from_millis(10));

        // Cloned config should be identical
        assert_eq!(original.len(), cloned.len());
        assert_eq!(original.keys(), cloned.keys());

        // Modifications to clone shouldn't affect original
        let mut modified_clone = cloned;
        modified_clone.insert(
            "new_key".to_string(),
            ConfigValue::String("new_value".to_string()),
        );

        assert!(!original.contains_key("new_key"));
        assert!(modified_clone.contains_key("new_key"));
    }

    fn create_config_of_size(size: usize) -> Configuration {
        let mut config = Configuration::new();

        for i in 0..size {
            let key = format!("key_{i}");
            let value = ConfigValue::String(format!("value_{i}"));
            config.insert(key, value);

            // Add some variety
            if i % 3 == 0 {
                config.insert(format!("int_{i}"), ConfigValue::Integer(i as i64));
            }
            if i % 5 == 0 {
                config.insert(format!("bool_{i}"), ConfigValue::Boolean(i % 2 == 0));
            }
        }

        config
    }

    fn create_deep_config(depth: usize) -> Configuration {
        let mut config = Configuration::new();

        // Create a nested structure
        let mut current_key = String::new();
        for i in 0..depth {
            if i > 0 {
                current_key.push('.');
            }
            current_key.push_str(&format!("level_{i}"));
        }
        current_key.push_str(".value");

        config.set(current_key, ConfigValue::String("deep_value".to_string()));
        config
    }
}

/// Test caching and optimization behavior
mod caching_tests {
    use super::*;

    #[test]
    fn test_parser_reuse() {
        let parser = ConfigParser::new();

        // Test that parser can be reused efficiently
        let configs = [
            "name = \"Config1\"",
            "name = \"Config2\"",
            "name = \"Config3\"",
        ];

        let start = Instant::now();
        let mut parsed_configs = Vec::new();

        for (i, config_str) in configs.iter().enumerate() {
            let config = parser.parse_string(config_str, None).unwrap();
            parsed_configs.push(config);

            // Verify each config was parsed correctly
            let expected_name = i + 1;
            assert_eq!(
                parsed_configs[i]
                    .get("name")
                    .unwrap()
                    .coerce_to_string()
                    .unwrap(),
                format!("Config{expected_name}")
            );
        }

        let reuse_duration = start.elapsed();

        // Parsing multiple configs with same parser should be efficient
        assert!(reuse_duration < Duration::from_millis(10));

        println!(
            "Parsed {} configs with reused parser in {:?}",
            configs.len(),
            reuse_duration
        );
    }

    #[test]
    fn test_repeated_access_patterns() {
        let config = performance_tests::create_large_nested_config(50);

        // Test that repeated access to same keys doesn't degrade performance
        let frequently_accessed_keys = [
            "section_0.subsection_0.item_0",
            "section_25.subsection_5.item_5",
            "section_49.subsection_9.item_9",
        ];

        let start = Instant::now();

        // Access the same keys many times
        for _ in 0..1000 {
            for key in &frequently_accessed_keys {
                let _value = config.get(key);
                let _exists = config.contains_key(key);
            }
        }

        let repeated_access_duration = start.elapsed();

        // Repeated access should remain fast (no performance degradation)
        assert!(repeated_access_duration < Duration::from_millis(20));

        println!(
            "Performed 6000 repeated accesses in {repeated_access_duration:?}"
        );
    }

    #[test]
    fn test_keys_caching_behavior() {
        let mut config = Configuration::new();

        // Add initial keys
        for i in 0..100 {
            config.insert(
                format!("initial_{i}"),
                ConfigValue::String(format!("value_{i}")),
            );
        }

        // Generate keys multiple times to test caching
        let start = Instant::now();
        let keys1 = config.keys();
        let first_keys_duration = start.elapsed();

        let start = Instant::now();
        let keys2 = config.keys();
        let second_keys_duration = start.elapsed();

        // Keys should be identical
        assert_eq!(keys1, keys2);

        // Note: We can't test caching directly, but we can verify consistency
        // and that performance remains acceptable
        assert!(first_keys_duration < Duration::from_millis(10));
        assert!(second_keys_duration < Duration::from_millis(10));

        // Add more keys and test again
        for i in 100..200 {
            config.insert(
                format!("added_{i}"),
                ConfigValue::String(format!("value_{i}")),
            );
        }

        let keys3 = config.keys();
        assert!(keys3.len() > keys1.len());
    }

    #[test]
    fn test_validation_caching() {
        let mut config = Configuration::new();

        // Add valid configuration
        for i in 0..100 {
            config.insert(
                format!("valid_key_{i}"),
                ConfigValue::String(format!("value_{i}")),
            );
        }

        // Validate multiple times
        let start = Instant::now();
        assert!(config.validate().is_ok());
        let first_validation = start.elapsed();

        let start = Instant::now();
        assert!(config.validate().is_ok());
        let second_validation = start.elapsed();

        // Both validations should be reasonably fast
        assert!(first_validation < Duration::from_millis(10));
        assert!(second_validation < Duration::from_millis(10));

        // Add invalid key and test that validation properly detects it
        config.insert(
            "123invalid".to_string(),
            ConfigValue::String("invalid key name".to_string()),
        );
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_environment_regex_caching() {
        // The environment variable regex should be cached for performance
        let mut values = Vec::new();

        for i in 0..100 {
            std::env::set_var(format!("CACHE_TEST_{i}"), format!("value_{i}"));
            values.push(ConfigValue::String(format!("${{CACHE_TEST_{i}}}"
            )));
        }

        let start = Instant::now();

        // Perform substitution on many values (should reuse cached regex)
        for value in &mut values {
            value.substitute_env_vars().unwrap();
        }

        let substitution_duration = start.elapsed();

        // Should be efficient due to regex caching
        assert!(substitution_duration < Duration::from_millis(50));

        // Verify substitution worked
        assert_eq!(values[0].coerce_to_string().unwrap(), "value_0");
        assert_eq!(values[99].coerce_to_string().unwrap(), "value_99");

        // Clean up
        for i in 0..100 {
            std::env::remove_var(format!("CACHE_TEST_{i}"));
        }

        println!(
            "Performed 100 environment substitutions in {substitution_duration:?}"
        );
    }
}
