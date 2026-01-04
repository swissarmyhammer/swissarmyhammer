//! Error handling tests for the configuration system
//!
//! Tests malformed config files, missing files, invalid environment variables,
//! and verifies that error messages are helpful and informative.

use serde_json::json;
use std::env;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use swissarmyhammer_common::IsolatedTestEnvironment;
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

/// Helper to create a project config directory for testing with proper isolation
fn create_project_config_dir() -> std::path::PathBuf {
    let temp_dir = TempDir::new().expect("Failed to create temp dir");
    let config_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&config_dir).expect("Failed to create project config dir");

    // Change to the temp directory so config discovery works
    env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

    // Keep temp dir alive by leaking it - IsolatedTestEnvironment will handle proper cleanup
    std::mem::forget(temp_dir);

    config_dir
}

#[test]
fn test_malformed_toml_config_error() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Test 1: Verify we can load a valid config
    let valid_toml = r#"app_name = "ValidTestApp""#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, valid_toml).expect("Failed to write valid config");

    let result_valid = TemplateContext::load_for_cli();
    match result_valid {
        Ok(context) => {
            // Should succeed and contain the app_name
            assert_eq!(
                context.get("app_name"),
                Some(&serde_json::json!("ValidTestApp"))
            );
        }
        Err(error) => {
            panic!("Valid TOML should succeed but failed: {}", error);
        }
    }

    // Test 2: Verify malformed config fails
    let malformed_toml = r#"app_name = "TestApp"#; // Unclosed string - clearly malformed
    fs::write(&config_file, malformed_toml).expect("Failed to write malformed config");

    let result_malformed = TemplateContext::load_for_cli();
    match result_malformed {
        Ok(context) => {
            panic!(
                "Malformed TOML should fail but succeeded! Variables: {:?}",
                context.to_hash_map()
            );
        }
        Err(error) => {
            // Should fail with a parse error - this is what we expect
            let error_msg = format!("{}", error);
            assert!(!error_msg.is_empty(), "Error message should not be empty");
            assert!(
                error_msg.contains("parse")
                    || error_msg.contains("TOML")
                    || error_msg.contains("invalid"),
                "Error should mention parsing issue: {}",
                error_msg
            );
        }
    }

    // Clean up
    fs::remove_file(&config_file).expect("Failed to remove config file");
}

#[test]
fn test_malformed_yaml_config_error() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Test 1: Verify we can load a valid YAML config
    let valid_yaml = r#"app_name: "ValidTestApp""#;
    let config_file = config_dir.join("sah.yaml");
    fs::write(&config_file, valid_yaml).expect("Failed to write valid YAML config");

    let result_valid = TemplateContext::load_for_cli();
    match result_valid {
        Ok(context) => {
            // Should succeed and contain the app_name
            assert_eq!(
                context.get("app_name"),
                Some(&serde_json::json!("ValidTestApp"))
            );
        }
        Err(error) => {
            panic!("Valid YAML should succeed but failed: {}", error);
        }
    }

    // Test 2: Verify malformed YAML fails
    let malformed_yaml = "app_name: TestApp\n\tport: 8080"; // Tab character - YAML doesn't allow tabs
    fs::write(&config_file, malformed_yaml).expect("Failed to write malformed YAML config");

    let result_malformed = TemplateContext::load_for_cli();
    match result_malformed {
        Ok(context) => {
            panic!(
                "Malformed YAML should fail but succeeded! Variables: {:?}",
                context.to_hash_map()
            );
        }
        Err(error) => {
            // Should fail with a parse error - this is what we expect
            let error_msg = format!("{}", error);
            assert!(!error_msg.is_empty(), "Error message should not be empty");
            assert!(
                error_msg.contains("parse")
                    || error_msg.contains("YAML")
                    || error_msg.contains("invalid")
                    || error_msg.contains("tab"),
                "Error should mention parsing issue: {}",
                error_msg
            );
        }
    }

    // Clean up
    fs::remove_file(&config_file).expect("Failed to remove YAML config file");
}

#[test]
fn test_malformed_json_config_error() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create malformed JSON files with the correct names that the config system will load
    let malformed_configs = vec![
        // Trailing comma - use sah.json so it gets loaded
        (
            "sah.json",
            r#"{
  "app_name": "TestApp",
  "port": 8080,
}"#,
        ),
    ];

    for (filename, content) in malformed_configs {
        let config_file = config_dir.join(filename);
        fs::write(&config_file, content).expect("Failed to write malformed JSON config");

        let result = TemplateContext::load_for_cli();
        assert!(
            result.is_err(),
            "Should fail to parse malformed JSON: {}",
            filename
        );

        let error = result.expect_err("Already confirmed this is an error");
        let error_msg = format!("{}", error);

        // Error message should be informative
        assert!(
            !error_msg.is_empty(),
            "Error message should not be empty for {}",
            filename
        );

        // Clean up for next test
        fs::remove_file(&config_file).expect("Failed to remove malformed JSON config");
    }
}

#[test]
fn test_unsupported_file_extension_error() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create config files with unsupported extensions
    let unsupported_files = vec![
        ("sah.txt", "app_name = TestApp"),
        ("sah.ini", "[app]\nname = TestApp"),
        ("sah.xml", "<config><app_name>TestApp</app_name></config>"),
        ("sah.conf", "app_name TestApp"),
        ("sah.properties", "app.name=TestApp"),
    ];

    for (filename, content) in unsupported_files {
        let config_file = config_dir.join(filename);
        fs::write(&config_file, content).expect("Failed to write unsupported config");

        let result = TemplateContext::load_for_cli();

        // Behavior depends on implementation - might ignore unsupported files or error
        if let Err(error) = result {
            let error_msg = format!("{}", error);

            // If it errors, the message should be informative
            assert!(
                error_msg.contains("extension")
                    || error_msg.contains("format")
                    || error_msg.contains("supported"),
                "Error message should mention file extension/format issue for {}: {}",
                filename,
                error_msg
            );
        }
        // If it succeeds, that's also acceptable (ignores unsupported files)

        fs::remove_file(&config_file).expect("Failed to remove unsupported config");
    }
}

#[test]
fn test_file_permission_errors() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create a valid config file
    let config_content = r#"
app_name = "TestApp"
port = 8080
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Make the config file unreadable
    let mut perms = fs::metadata(&config_file)
        .expect("Failed to get file metadata")
        .permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&config_file, perms).expect("Failed to set file permissions");

    let result = TemplateContext::load_for_cli();

    // Restore permissions first (for cleanup)
    let mut perms = fs::metadata(&config_file)
        .expect("Failed to get file metadata")
        .permissions();
    perms.set_mode(0o644);
    fs::set_permissions(&config_file, perms).expect("Failed to restore file permissions");

    // Check result (may succeed or fail depending on user privileges)
    if let Err(error) = result {
        let error_msg = format!("{}", error);

        // Error message should indicate permission problem
        assert!(
            error_msg.contains("permission")
                || error_msg.contains("access")
                || error_msg.contains("denied"),
            "Error message should mention permission issue: {}",
            error_msg
        );
    }
}

#[test]
fn test_directory_permission_errors() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create a valid config file
    let config_content = r#"app_name = "TestApp""#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Make the config directory unreadable
    let mut perms = fs::metadata(&config_dir)
        .expect("Failed to get dir metadata")
        .permissions();
    perms.set_mode(0o000);
    fs::set_permissions(&config_dir, perms).expect("Failed to set dir permissions");

    let result = TemplateContext::load_for_cli();

    // Restore permissions first (for cleanup)
    let mut perms = fs::metadata(&config_dir)
        .expect("Failed to get dir metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&config_dir, perms).expect("Failed to restore dir permissions");

    // Configuration loading should handle this gracefully or provide informative error
    // The exact behavior depends on the implementation
    if let Err(error) = result {
        let error_msg = format!("{}", error);

        // If it errors, should mention permission or access issue
        assert!(
            !error_msg.is_empty(),
            "Error message should not be empty for directory permission error"
        );
    }
    // If it succeeds, that's also acceptable (graceful handling)
}

#[test]
fn test_circular_environment_variable_references() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Set up circular environment variable references
    env::set_var("VAR1", "${VAR2}");
    env::set_var("VAR2", "${VAR3}");
    env::set_var("VAR3", "${VAR1}"); // Circular reference

    env::set_var("SAH_CIRCULAR_VALUE", "${VAR1}");

    // Ensure cleanup happens regardless of test outcome
    let cleanup = || {
        env::remove_var("VAR1");
        env::remove_var("VAR2");
        env::remove_var("VAR3");
        env::remove_var("SAH_CIRCULAR_VALUE");
    };

    struct CleanupGuard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for CleanupGuard<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
    let _cleanup_guard = CleanupGuard(Some(cleanup));

    // This should either resolve gracefully or provide informative error
    let result = TemplateContext::load_for_cli();

    if let Err(error) = result {
        let error_msg = format!("{}", error);

        // Error should mention circular reference or substitution issue
        assert!(
            error_msg.contains("circular")
                || error_msg.contains("recursive")
                || error_msg.contains("substitution")
                || error_msg.contains("reference"),
            "Error should mention circular reference issue: {}",
            error_msg
        );
    } else if let Ok(context) = result {
        // If it succeeds, verify that the circular reference was handled appropriately
        if let Some(value) = context.get("circular.value") {
            let value_str = value.as_str().unwrap_or("");
            // Should not result in infinite recursion
            assert!(
                value_str.len() < 1000,
                "Circular reference should not result in very long string"
            );
        }
    }
}

#[test]
fn test_invalid_environment_variable_substitution() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");

    // Set up environment variables with invalid substitution syntax
    env::set_var("SAH_INVALID_SYNTAX1", "${");
    env::set_var("SAH_INVALID_SYNTAX2", "${}");
    env::set_var("SAH_INVALID_SYNTAX3", "${UNCLOSED");
    env::set_var("SAH_INVALID_SYNTAX4", "REGULAR${UNCLOSED");
    env::set_var("SAH_INVALID_SYNTAX5", "${NESTED${INNER}}");

    // Ensure cleanup happens regardless of test outcome
    let cleanup = || {
        env::remove_var("SAH_INVALID_SYNTAX1");
        env::remove_var("SAH_INVALID_SYNTAX2");
        env::remove_var("SAH_INVALID_SYNTAX3");
        env::remove_var("SAH_INVALID_SYNTAX4");
        env::remove_var("SAH_INVALID_SYNTAX5");
    };

    struct CleanupGuard<F: FnOnce()>(Option<F>);
    impl<F: FnOnce()> Drop for CleanupGuard<F> {
        fn drop(&mut self) {
            if let Some(f) = self.0.take() {
                f();
            }
        }
    }
    let _cleanup_guard = CleanupGuard(Some(cleanup));

    let result = TemplateContext::load_for_cli();

    // Should handle invalid syntax gracefully
    if let Err(error) = result {
        let error_msg = format!("{}", error);

        // Error should be informative about substitution syntax
        assert!(
            !error_msg.is_empty(),
            "Error message should not be empty for invalid substitution syntax"
        );
    } else if let Ok(context) = result {
        // If it succeeds, check that invalid syntax was handled appropriately

        // Invalid syntax might be left as-is or result in empty values
        if let Some(value1) = context.get("invalid.syntax1") {
            let value1_str = value1.as_str().unwrap_or("");
            // Should not crash or hang
            assert!(
                value1_str.len() < 100,
                "Invalid syntax should not result in very long string"
            );
        }
    }
}

#[test]
fn test_configuration_with_extremely_large_values() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create config with very large values
    let large_string = "x".repeat(1_000_000); // 1MB string
    let _large_array = (0..10000).collect::<Vec<i32>>();

    let config_content = format!(
        r#"
app_name = "TestApp"
large_string = "{}"
large_number = 999999999999999999999999999999
"#,
        large_string
    );

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, &config_content).expect("Failed to write large config");

    let result = TemplateContext::load_for_cli();

    // Should handle large values gracefully
    match result {
        Ok(context) => {
            // If successful, verify large values are handled correctly
            if let Some(large_val) = context.get("large_string") {
                if let Some(large_str) = large_val.as_str() {
                    assert_eq!(large_str.len(), 1_000_000);
                    assert!(large_str.chars().all(|c| c == 'x'));
                }
            }
        }
        Err(error) => {
            let error_msg = format!("{}", error);
            // Error should be informative if large values cause issues
            assert!(
                !error_msg.is_empty(),
                "Error message should not be empty for large values"
            );
        }
    }
}

#[test]
fn test_deeply_nested_configuration_errors() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create deeply nested configuration that might cause parsing issues
    let mut nested_config = String::from("app_name = \"TestApp\"\n");

    // Create 100 levels of nesting
    for i in 0..100 {
        nested_config.push_str(&format!("[level{}]\n", i));
        nested_config.push_str(&format!("value{} = \"test{}\"\n", i, i));
    }

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, &nested_config).expect("Failed to write deeply nested config");

    let result = TemplateContext::load_for_cli();

    // Should handle deep nesting appropriately
    match result {
        Ok(context) => {
            // If successful, verify basic values are accessible
            assert_eq!(context.get("app_name"), Some(&json!("TestApp")));

            // Check that some nested values are present
            if let Some(level0) = context.get("level0") {
                if let Some(level0_obj) = level0.as_object() {
                    assert!(level0_obj.contains_key("value0"));
                }
            }
        }
        Err(error) => {
            let error_msg = format!("{}", error);
            // Error should be informative if deep nesting causes issues
            assert!(
                !error_msg.is_empty() && error_msg.len() < 10000,
                "Error message should be present but not excessively long for deep nesting"
            );
        }
    }
}

#[test]
fn test_config_with_unicode_and_special_characters() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create config with various Unicode and special characters
    let config_content = r#"
app_name = "测试应用"
emoji_value = "🚀 SwissArmyHammer 🔧"
special_chars = "!@#$%^&*()_+-=[]{}|;':\",./<>?"
unicode_key_测试 = "unicode key value"
null_byte = ""
tab_value = "	"
newline_value = "line1\nline2\nline3"
carriage_return = "line1\r\nline2"

[section_测试]
value_测试 = "nested unicode"
"#;

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write unicode config");

    let result = TemplateContext::load_for_cli();

    match result {
        Ok(context) => {
            // If successful, verify Unicode handling
            assert_eq!(context.get("app_name"), Some(&json!("测试应用")));
            assert_eq!(
                context.get("emoji_value"),
                Some(&json!("🚀 SwissArmyHammer 🔧"))
            );
            assert_eq!(
                context.get("special_chars"),
                Some(&json!("!@#$%^&*()_+-=[]{}|;':\",./<>?"))
            );
            assert_eq!(
                context.get("newline_value"),
                Some(&json!("line1\nline2\nline3"))
            );
        }
        Err(error) => {
            let error_msg = format!("{}", error);
            // If Unicode causes parsing issues, error should be informative
            assert!(
                !error_msg.is_empty(),
                "Error message should not be empty for Unicode handling"
            );

            // Error message itself should handle Unicode properly
            assert!(
                error_msg.len() < 10000,
                "Error message should be reasonable length"
            );
        }
    }
}

#[test]
fn test_empty_and_whitespace_only_files() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Test empty files with correct config names that will be loaded
    let test_files = vec![
        ("sah.json", ""),               // Empty JSON should fail
        ("sah.toml", ""),               // Empty TOML should succeed
        ("sah.yaml", "   \n\t  \n   "), // Whitespace-only YAML should succeed
    ];

    for (filename, content) in test_files {
        let config_file = config_dir.join(filename);
        fs::write(&config_file, content).expect("Failed to write test file");

        let result = TemplateContext::load_for_cli();

        match filename {
            "sah.json" => {
                // Empty JSON files should fail to parse
                assert!(result.is_err(), "Empty JSON file should fail: {}", filename);

                if let Err(error) = result {
                    let error_msg = format!("{}", error);
                    assert!(
                        !error_msg.is_empty(),
                        "JSON parse error should have message"
                    );
                }
            }
            _ => {
                // Empty TOML and YAML files should succeed (create empty context)
                match result {
                    Ok(_context) => {
                        // Should succeed with empty or minimal context - this is fine
                    }
                    Err(error) => {
                        let error_msg = format!("{}", error);
                        // If it fails, error should be informative
                        assert!(
                            !error_msg.is_empty(),
                            "Error message should not be empty for {}: {}",
                            filename,
                            error_msg
                        );
                    }
                }
            }
        }

        fs::remove_file(&config_file).expect("Failed to remove test file");
    }
}

#[test]
fn test_helpful_error_messages_contain_context() {
    let _guard = IsolatedTestEnvironment::new().expect("Failed to create test environment");
    let config_dir = create_project_config_dir();

    // Create a config file with a clear error
    let config_content = r#"
app_name = "TestApp"
version = "1.0.0"

[database]
host = "localhost"
port = "not_a_number"  # This should be a number
timeout = 30

[invalid_section
value = "missing closing bracket"
"#;

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write error config");

    let result = TemplateContext::load_for_cli();
    assert!(result.is_err(), "Should fail with syntax error");

    let error = result.expect_err("Already confirmed this is an error");
    let error_msg = format!("{}", error);

    // Error message should be helpful and contain context
    assert!(!error_msg.is_empty(), "Error message should not be empty");

    // Error message should be reasonably sized (not too short or too long)
    assert!(
        error_msg.len() > 10 && error_msg.len() < 10000,
        "Error message should be reasonably sized: {} chars",
        error_msg.len()
    );

    // Error message might contain file information, line numbers, or other context
    // The exact format depends on the underlying parsing library, but it should be informative
}
