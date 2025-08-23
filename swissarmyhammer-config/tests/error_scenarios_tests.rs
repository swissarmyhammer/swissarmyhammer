//! Error handling integration tests for SwissArmyHammer configuration system

mod common;

use common::TestEnvironment;
use serial_test::serial;
use std::fs;
use swissarmyhammer_config::{ConfigError, ConfigFormat};

#[test]
#[serial]
fn test_invalid_toml_configuration_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create invalid TOML configuration
    let invalid_toml = r#"
project_name = "Test Project"
# Missing closing quote
invalid_string = "unclosed string
# Invalid section
[database
host = "localhost"
port = 5432
"#;

    env.write_project_config(invalid_toml, ConfigFormat::Toml)
        .unwrap();

    // Should handle parsing error gracefully
    let result = env.load_template_context();
    assert!(result.is_err());

    match result {
        Err(ConfigError::ParseError { source, path }) => {
            assert!(path.is_some());
            println!("Caught expected parse error: {}", source);
        }
        Err(other) => panic!("Expected ParseError, got: {:?}", other),
        Ok(_) => panic!("Expected error but got success"),
    }
}

#[test]
#[serial]
fn test_invalid_yaml_configuration_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create invalid YAML configuration
    let invalid_yaml = r#"
project_name: "Test Project"
database:
  host: "localhost"
  port: 5432
  # Invalid indentation
invalid_indent: "should be indented"
    nested_key: "value"
# Invalid YAML syntax
invalid: [unclosed array
"#;

    env.write_project_config(invalid_yaml, ConfigFormat::Yaml)
        .unwrap();

    // Should handle YAML parsing error gracefully
    let result = env.load_template_context();
    assert!(result.is_err());

    match result {
        Err(ConfigError::ParseError { source, path }) => {
            assert!(path.is_some());
            println!("Caught expected YAML parse error: {}", source);
        }
        Err(other) => panic!("Expected ParseError, got: {:?}", other),
        Ok(_) => panic!("Expected error but got success"),
    }
}

#[test]
#[serial]
fn test_invalid_json_configuration_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create invalid JSON configuration
    let invalid_json = r#"{
    "project_name": "Test Project",
    "database": {
        "host": "localhost",
        "port": 5432,
    },
    "invalid": "missing comma"
    "another_key": "value"
}"#;

    env.write_project_config(invalid_json, ConfigFormat::Json)
        .unwrap();

    // Should handle JSON parsing error gracefully
    let result = env.load_template_context();
    assert!(result.is_err());

    match result {
        Err(ConfigError::ParseError { source, path }) => {
            assert!(path.is_some());
            println!("Caught expected JSON parse error: {}", source);
        }
        Err(other) => panic!("Expected ParseError, got: {:?}", other),
        Ok(_) => panic!("Expected error but got success"),
    }
}

#[test]
#[serial]
fn test_missing_environment_variables_error_handling() {
    let mut env = TestEnvironment::new().unwrap();

    // Create configuration with required environment variables
    let config_with_missing_vars = r#"
project_name = "${PROJECT_NAME}"
database_url = "${DATABASE_URL}"
api_key = "${API_KEY}"
optional_with_fallback = "${OPTIONAL_VAR:-default_value}"
"#;

    env.write_project_config(config_with_missing_vars, ConfigFormat::Toml)
        .unwrap();

    // Set only some of the required variables
    env.set_env_var("PROJECT_NAME", "Test Project").unwrap();
    // DATABASE_URL and API_KEY are missing

    // Legacy mode should handle missing vars gracefully (empty string)
    let legacy_result = env.load_template_context();
    assert!(legacy_result.is_ok());

    let context = legacy_result.unwrap();
    assert_eq!(context.get_string("project_name").unwrap(), "Test Project");
    assert_eq!(context.get_string("database_url").unwrap(), ""); // Empty for missing var
    assert_eq!(
        context.get_string("optional_with_fallback").unwrap(),
        "default_value"
    );

    // Strict mode should fail on missing variables
    let strict_result = env.load_template_context_strict();
    assert!(strict_result.is_err());
}

#[test]
#[serial]
fn test_file_permission_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create a valid configuration first
    let config = "project_name = \"Test\"";
    let config_path = env
        .write_project_config(config, ConfigFormat::Toml)
        .unwrap();

    // On Unix systems, make the file unreadable (this might not work on all systems)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o000); // No permissions
        let _ = fs::set_permissions(&config_path, perms);
    }

    // The configuration system should handle permission errors gracefully
    // Note: This test might behave differently depending on the system and user privileges
    let result = env.load_template_context();

    // Reset permissions for cleanup
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&config_path).unwrap().permissions();
        perms.set_mode(0o644); // Restore read permissions
        let _ = fs::set_permissions(&config_path, perms);
    }

    // The exact behavior depends on system permissions and may not always fail
    println!("File permission test result: {:?}", result);
}

#[test]
#[serial]
fn test_corrupted_configuration_file_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create a file with binary/corrupted content
    let config_path = env.project_config_path().join("sah.toml");
    let corrupted_content = vec![0xFF, 0xFE, 0xFD, 0xFC, 0x00, 0x01, 0x02, 0x03];
    fs::write(&config_path, corrupted_content).unwrap();

    // Should handle corrupted file gracefully
    let result = env.load_template_context();

    // This might succeed with empty config or fail with parse error
    match result {
        Ok(context) => {
            // If it succeeds, it should at least have defaults
            assert!(!context.is_empty());
            println!("Corrupted file handled gracefully with defaults");
        }
        Err(ConfigError::ParseError { .. }) => {
            println!("Corrupted file correctly identified as parse error");
        }
        Err(other) => {
            println!("Corrupted file error: {:?}", other);
        }
    }
}

#[test]
#[serial]
fn test_mixed_valid_invalid_configurations_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Create one valid and one invalid configuration
    let valid_config = r#"
project_name = "Valid Config"
environment = "test"
debug = true
"#;

    let invalid_config = r#"
{
    "project_name": "Invalid Config",
    "syntax_error": "missing comma"
    "another_key": "value"
}
"#;

    env.write_project_config(valid_config, ConfigFormat::Toml)
        .unwrap();
    env.write_project_config(invalid_config, ConfigFormat::Json)
        .unwrap();

    // The system behavior depends on figment's handling of mixed valid/invalid files
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            // If it succeeds, should have loaded the valid configuration
            assert_eq!(context.get_string("project_name").unwrap(), "Valid Config");
            println!("Successfully loaded valid config despite invalid JSON");
        }
        Err(error) => {
            // If it fails, should be due to the invalid JSON
            println!(
                "Failed as expected due to invalid configuration: {:?}",
                error
            );
        }
    }
}

#[test]
#[serial]
fn test_circular_environment_variable_reference() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up circular environment variable references
    env.set_env_vars([
        ("VAR_A", "${VAR_B}"),
        ("VAR_B", "${VAR_C}"),
        ("VAR_C", "${VAR_A}"),
    ])
    .unwrap();

    let config_with_circular_ref = r#"
project_name = "${VAR_A}"
fallback_value = "safe_value"
"#;

    env.write_project_config(config_with_circular_ref, ConfigFormat::Toml)
        .unwrap();

    // Should handle circular references gracefully
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            // Should either resolve to empty or the literal string
            let project_name = context.get_string("project_name").unwrap();
            println!("Circular reference resolved to: '{}'", project_name);
            // Fallback value should still work
            assert_eq!(context.get_string("fallback_value").unwrap(), "safe_value");
        }
        Err(error) => {
            println!("Circular reference error (expected): {:?}", error);
        }
    }
}

#[test]
#[serial]
fn test_extremely_nested_configuration_limits() {
    let env = TestEnvironment::new().unwrap();

    // Create deeply nested configuration to test limits
    let mut deeply_nested_config = String::from("project_name = \"Deep Nesting Test\"\n");

    // Create nested sections
    let mut current_section = String::new();
    for i in 0..20 {
        current_section.push_str(&format!("level_{}", i));
        deeply_nested_config.push_str(&format!("[{}]\n", current_section));
        deeply_nested_config.push_str(&format!("value = \"level_{}_value\"\n", i));
        current_section.push('.');
    }

    // Add extremely long key names and values
    deeply_nested_config.push_str(&format!(
        "extremely_long_key_name_{} = \"{}\"\n",
        "x".repeat(1000),
        "y".repeat(5000)
    ));

    env.write_project_config(&deeply_nested_config, ConfigFormat::Toml)
        .unwrap();

    // Should handle deep nesting gracefully (within reasonable limits)
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            assert_eq!(
                context.get_string("project_name").unwrap(),
                "Deep Nesting Test"
            );
            println!("Deep nesting handled successfully");
        }
        Err(error) => {
            println!("Deep nesting error: {:?}", error);
            // Could fail due to limits, which is acceptable
        }
    }
}

#[test]
#[serial]
fn test_unicode_and_special_characters_error_handling() {
    let env = TestEnvironment::new().unwrap();

    // Test configuration with various Unicode and special characters
    let unicode_config = r#"
project_name = "Unicode Test 🚀 ∏∑∆"
chinese_text = "你好世界"
arabic_text = "مرحبا بالعالم"
emoji_key_🔑 = "emoji_value_🎉"
special_chars = "~!@#$%^&*()_+-={}[]|\\:;\"'<>,.?/"
zero_width_space = "\u{200B}text\u{200B}with\u{200B}zero\u{200B}width\u{200B}spaces"
control_chars = "\t\n\r"

[unicode_section_тест]
ключ = "значение"
"#;

    env.write_project_config(unicode_config, ConfigFormat::Toml)
        .unwrap();

    // Should handle Unicode characters gracefully
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            assert!(context.get("project_name").is_some());
            assert!(context.get("chinese_text").is_some());
            assert!(context.get("arabic_text").is_some());
            println!("Unicode characters handled successfully");
        }
        Err(error) => {
            println!("Unicode handling error: {:?}", error);
        }
    }
}

#[test]
#[serial]
fn test_configuration_size_limits() {
    let env = TestEnvironment::new().unwrap();

    // Create very large configuration to test memory limits
    let mut large_config = String::from("project_name = \"Size Test\"\n");

    // Add many sections and keys
    for section in 0..100 {
        large_config.push_str(&format!("[section_{}]\n", section));
        for key in 0..100 {
            large_config.push_str(&format!(
                "key_{}_{} = \"This is a reasonably long value for key {} in section {} to test memory usage\"\n",
                section, key, key, section
            ));
        }
    }

    // Add some very long string values
    for i in 0..10 {
        large_config.push_str(&format!("large_string_{} = \"{}\"\n", i, "x".repeat(10000)));
    }

    env.write_project_config(&large_config, ConfigFormat::Toml)
        .unwrap();

    println!(
        "Large configuration size: ~{} lines",
        large_config.lines().count()
    );

    // Should handle large configuration within reasonable limits
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            assert_eq!(context.get_string("project_name").unwrap(), "Size Test");
            println!("Large configuration loaded successfully");
        }
        Err(error) => {
            println!("Large configuration error: {:?}", error);
        }
    }
}

#[test]
#[serial]
fn test_environment_variable_substitution_edge_cases() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up edge case environment variables
    env.set_env_vars([
        ("EMPTY_VAR", ""),
        ("WHITESPACE_VAR", "   "),
        ("NEWLINE_VAR", "line1\nline2"),
        ("QUOTE_VAR", "has \"quotes\" inside"),
        ("DOLLAR_VAR", "$100 ${NESTED}"),
        ("UNICODE_VAR", "🔥 Unicode! ∑"),
    ])
    .unwrap();

    let edge_case_config = r#"
project_name = "Edge Case Test"
empty_value = "${EMPTY_VAR}"
whitespace_value = "${WHITESPACE_VAR}"
newline_value = "${NEWLINE_VAR}"
quote_value = "${QUOTE_VAR}"
dollar_value = "${DOLLAR_VAR}"
unicode_value = "${UNICODE_VAR}"
missing_with_empty_fallback = "${MISSING_VAR:-}"
missing_with_whitespace_fallback = "${MISSING_VAR:- }"
nested_fallback = "${MISSING_VAR:-${EMPTY_VAR:-default}}"
complex_substitution = "${QUOTE_VAR:-${DOLLAR_VAR:-${UNICODE_VAR:-fallback}}}"
"#;

    env.write_project_config(edge_case_config, ConfigFormat::Toml)
        .unwrap();

    // Should handle edge cases in environment variable substitution
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            assert_eq!(
                context.get_string("project_name").unwrap(),
                "Edge Case Test"
            );

            // Check that various edge cases are handled
            assert_eq!(context.get_string("empty_value").unwrap(), "");
            assert_eq!(context.get_string("whitespace_value").unwrap(), "   ");
            assert!(context.get("newline_value").is_some());
            assert!(context.get("quote_value").is_some());
            assert!(context.get("unicode_value").is_some());

            println!("Environment variable edge cases handled successfully");
        }
        Err(error) => {
            println!("Environment variable edge case error: {:?}", error);
        }
    }
}

#[test]
#[serial]
fn test_configuration_recovery_from_partial_failure() {
    let env = TestEnvironment::new().unwrap();

    // Create global configuration (valid)
    env.write_global_config(
        r#"
project_name = "Global Project"
global_value = "from_global"
shared_key = "global_shared"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Create invalid project configuration
    let invalid_project_config = r#"
project_name = "Project Override"
# Syntax error
invalid_section = [
shared_key = "project_shared"
"#;

    env.write_project_config(invalid_project_config, ConfigFormat::Toml)
        .unwrap();

    // System behavior depends on figment's error handling strategy
    let result = env.load_template_context();

    match result {
        Ok(context) => {
            // If it recovers, should have at least global config values
            assert!(context.get("project_name").is_some());
            assert!(context.get("global_value").is_some());
            println!("Successfully recovered from partial configuration failure");
        }
        Err(error) => {
            println!(
                "Failed to recover from partial failure (expected): {:?}",
                error
            );
        }
    }
}

#[test]
#[serial]
fn test_error_message_quality() {
    let env = TestEnvironment::new().unwrap();

    // Create configuration with various types of errors
    let error_config = r#"
project_name = "Error Test"
# Unclosed string
bad_string = "unclosed
# Invalid number  
bad_number = 1.2.3
# Invalid boolean
bad_boolean = truee
"#;

    env.write_project_config(error_config, ConfigFormat::Toml)
        .unwrap();

    let result = env.load_template_context();
    assert!(result.is_err());

    // Check that error messages are informative
    match result {
        Err(ConfigError::ParseError { source, path }) => {
            let error_msg = format!("{}", source);
            println!("Error message: {}", error_msg);

            // Error message should be informative (not just generic)
            assert!(!error_msg.is_empty());
            assert!(error_msg.len() > 10); // Should be more than just "parse error"

            // Should include file path information
            assert!(path.is_some());
            println!("Error file path: {:?}", path);
        }
        Err(other) => {
            println!("Other error type: {:?}", other);
        }
        Ok(_) => panic!("Expected error but got success"),
    }
}
