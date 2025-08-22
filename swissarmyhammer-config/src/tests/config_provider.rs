//! Integration tests for ConfigProvider

use crate::ConfigProvider;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_config_provider_empty_environment() {
    let provider = ConfigProvider::new();
    let _context = provider.load_template_context().unwrap();

    // Should work even with no configuration files
    // Note: context.len() is always >= 0 by definition
}

#[test]
fn test_config_provider_with_environment_variables() {
    // Set test environment variables
    std::env::set_var("SAH_TEST_CONFIG", "from_sah_env");
    std::env::set_var("SWISSARMYHAMMER_OTHER_CONFIG", "from_long_env");

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    // Check that environment variables are loaded
    // Note: figment may lowercase and transform the keys
    let has_test_config =
        context.get("test_config").is_some() || context.get("TEST_CONFIG").is_some();
    let has_other_config =
        context.get("other_config").is_some() || context.get("OTHER_CONFIG").is_some();

    assert!(has_test_config, "Should have test_config from SAH_ prefix");
    assert!(
        has_other_config,
        "Should have other_config from SWISSARMYHAMMER_ prefix"
    );

    // Clean up
    std::env::remove_var("SAH_TEST_CONFIG");
    std::env::remove_var("SWISSARMYHAMMER_OTHER_CONFIG");
}

#[test]
fn test_config_provider_with_project_files() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    // Create a project structure
    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create TOML config
    fs::write(
        sah_dir.join("sah.toml"),
        r#"
app_name = "Test App"
version = "1.0.0"

[database]
host = "${DB_HOST:-localhost}"
port = 5432
"#,
    )
    .unwrap();

    // Create YAML config that should override some TOML values
    fs::write(
        sah_dir.join("swissarmyhammer.yaml"),
        r#"
version: "2.0.0"
environment: "test"
features:
  - workflows
  - prompts
  - mcp
"#,
    )
    .unwrap();

    // Change to temp directory
    std::env::set_current_dir(temp_dir.path()).unwrap();

    // Set environment variable for substitution
    std::env::set_var("DB_HOST", "test-db.example.com");

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    // Restore directory
    std::env::set_current_dir(original_dir).unwrap();
    std::env::remove_var("DB_HOST");

    // Check loaded values
    assert_eq!(
        context.get("app_name"),
        Some(&serde_json::Value::String("Test App".to_string()))
    );
    assert_eq!(
        context.get("version"),
        Some(&serde_json::Value::String("2.0.0".to_string()))
    ); // YAML should override TOML
    assert_eq!(
        context.get("environment"),
        Some(&serde_json::Value::String("test".to_string()))
    );

    // Check nested object
    if let Some(db_config) = context.get("database") {
        assert_eq!(
            db_config["host"],
            serde_json::Value::String("test-db.example.com".to_string())
        );
        assert_eq!(db_config["port"], serde_json::Value::Number(5432.into()));
    } else {
        panic!("Database configuration should be present");
    }

    // Check array
    if let Some(features) = context.get("features") {
        if let serde_json::Value::Array(arr) = features {
            assert!(arr.contains(&serde_json::Value::String("workflows".to_string())));
            assert!(arr.contains(&serde_json::Value::String("mcp".to_string())));
        } else {
            panic!("Features should be an array");
        }
    }
}

#[test]
fn test_config_provider_json_format() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create JSON config
    fs::write(
        sah_dir.join("sah.json"),
        r#"{
    "app_name": "JSON App",
    "config": {
        "api_key": "${API_KEY:-default_key}",
        "endpoints": [
            "https://api.example.com",
            "https://backup.example.com"
        ]
    },
    "debug": true
}"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();
    std::env::set_var("API_KEY", "secret_key_123");

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    std::env::set_current_dir(original_dir).unwrap();
    std::env::remove_var("API_KEY");

    // Check loaded values
    assert_eq!(
        context.get("app_name"),
        Some(&serde_json::Value::String("JSON App".to_string()))
    );
    assert_eq!(context.get("debug"), Some(&serde_json::Value::Bool(true)));

    // Check environment substitution in nested structure
    if let Some(config) = context.get("config") {
        assert_eq!(
            config["api_key"],
            serde_json::Value::String("secret_key_123".to_string())
        );

        if let serde_json::Value::Array(endpoints) = &config["endpoints"] {
            assert_eq!(endpoints.len(), 2);
            assert!(endpoints.contains(&serde_json::Value::String(
                "https://api.example.com".to_string()
            )));
        } else {
            panic!("Endpoints should be an array");
        }
    } else {
        panic!("Config section should be present");
    }
}

#[test]
fn test_config_provider_no_caching() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    let config_file = sah_dir.join("sah.toml");

    // Write initial config
    fs::write(
        &config_file,
        r#"
version = "1.0.0"
"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let provider = ConfigProvider::new();

    // Load first time
    let context1 = provider.load_template_context().unwrap();
    assert_eq!(
        context1.get("version"),
        Some(&serde_json::Value::String("1.0.0".to_string()))
    );

    // Update config file
    fs::write(
        &config_file,
        r#"
version = "2.0.0"
new_feature = true
"#,
    )
    .unwrap();

    // Load second time - should see updated values (no caching)
    let context2 = provider.load_template_context().unwrap();
    assert_eq!(
        context2.get("version"),
        Some(&serde_json::Value::String("2.0.0".to_string()))
    );
    assert_eq!(
        context2.get("new_feature"),
        Some(&serde_json::Value::Bool(true))
    );

    std::env::set_current_dir(original_dir).unwrap();
}
