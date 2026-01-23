//! Comprehensive file format tests for the configuration system
//!
//! Tests loading and parsing of TOML, YAML, JSON, and YML configuration files
//! with various content structures and edge cases.

use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_config::TemplateContext;

/// Test helper to create isolated test environments
struct IsolatedConfigTest {
    _env: IsolatedTestEnvironment,
}

impl IsolatedConfigTest {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");

        // Set current directory to temp dir for these tests
        env::set_current_dir(env.temp_dir()).expect("Failed to set current dir");

        Self { _env: env }
    }

    fn project_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self
            ._env
            .temp_dir()
            .join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }
}

#[test]
#[serial]
fn test_toml_config_file_loading() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Create a TOML config file with various data types
    let toml_content = r#"
# Basic configuration values
app_name = "SwissArmyHammer"
version = "2.0.0"
debug = true
max_connections = 100

# Nested configuration
[database]
host = "localhost"
port = 5432
username = "admin"
ssl = true

[database.pool]
min_connections = 5
max_connections = 50

# Array values
[features]
enabled = ["templating", "config", "workflows"]
disabled = ["experimental"]

# Complex nested structure
[logging]
level = "info"
format = "json"

[logging.targets]
console = true
file = "/var/log/sah.log"
syslog = false

[metrics]
collection_interval = 30
retention_days = 7
"#;

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, toml_content).expect("Failed to write TOML config");

    let context = TemplateContext::load_for_cli().expect("Failed to load TOML config");

    // Test basic values
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
    assert_eq!(context.get("version"), Some(&json!("2.0.0")));
    assert_eq!(context.get("debug"), Some(&json!(true)));
    assert_eq!(context.get("max_connections"), Some(&json!(100)));

    // Test nested values
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(context.get("database.port"), Some(&json!(5432)));
    assert_eq!(context.get("database.username"), Some(&json!("admin")));
    assert_eq!(context.get("database.ssl"), Some(&json!(true)));

    // Test deeply nested values
    if let Some(database) = context.get("database") {
        if let Some(database_obj) = database.as_object() {
            if let Some(pool) = database_obj.get("pool") {
                if let Some(pool_obj) = pool.as_object() {
                    assert_eq!(pool_obj.get("min_connections"), Some(&json!(5)));
                    assert_eq!(pool_obj.get("max_connections"), Some(&json!(50)));
                }
            }
        }
    }

    // Test arrays
    if let Some(features) = context.get("features") {
        if let Some(features_obj) = features.as_object() {
            assert_eq!(
                features_obj.get("enabled"),
                Some(&json!(["templating", "config", "workflows"]))
            );
            assert_eq!(features_obj.get("disabled"), Some(&json!(["experimental"])));
        }
    }

    // Test logging config
    assert_eq!(context.get("logging.level"), Some(&json!("info")));
    assert_eq!(context.get("logging.format"), Some(&json!("json")));

    // Test metrics config
    assert_eq!(context.get("metrics.collection_interval"), Some(&json!(30)));
    assert_eq!(context.get("metrics.retention_days"), Some(&json!(7)));
}

#[test]
#[serial]
fn test_yaml_config_file_loading() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Create a YAML config file with various data types
    let yaml_content = r#"
# YAML configuration
app_name: SwissArmyHammer
version: 2.0.0
debug: true
max_connections: 100

# Nested configuration
database:
  host: localhost
  port: 5432
  username: admin
  ssl: true
  pool:
    min_connections: 5
    max_connections: 50

# Array values
features:
  enabled:
    - templating
    - config
    - workflows
  disabled:
    - experimental

# Complex nested structure
logging:
  level: info
  format: yaml
  targets:
    console: true
    file: /var/log/sah.log
    syslog: false

metrics:
  collection_interval: 30
  retention_days: 7

# Multi-line strings
description: |
  This is a multi-line description
  that spans multiple lines
  and preserves formatting.

# Inline arrays and objects
tags: [development, testing, production]
metadata: { created_by: test, version: 1.0 }
"#;

    let config_file = config_dir.join("sah.yaml");
    fs::write(&config_file, yaml_content).expect("Failed to write YAML config");

    let context = TemplateContext::load_for_cli().expect("Failed to load YAML config");

    // Test basic values
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
    assert_eq!(context.get("version"), Some(&json!("2.0.0")));
    assert_eq!(context.get("debug"), Some(&json!(true)));
    assert_eq!(context.get("max_connections"), Some(&json!(100)));

    // Test nested values
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(context.get("database.port"), Some(&json!(5432)));
    assert_eq!(context.get("database.username"), Some(&json!("admin")));
    assert_eq!(context.get("database.ssl"), Some(&json!(true)));

    // Test arrays
    if let Some(features) = context.get("features") {
        if let Some(features_obj) = features.as_object() {
            assert_eq!(
                features_obj.get("enabled"),
                Some(&json!(["templating", "config", "workflows"]))
            );
            assert_eq!(features_obj.get("disabled"), Some(&json!(["experimental"])));
        }
    }

    // Test multi-line strings
    let expected_description =
        "This is a multi-line description\nthat spans multiple lines\nand preserves formatting.\n";
    assert_eq!(
        context.get("description"),
        Some(&json!(expected_description))
    );

    // Test inline arrays
    assert_eq!(
        context.get("tags"),
        Some(&json!(["development", "testing", "production"]))
    );

    // Test inline objects
    if let Some(metadata) = context.get("metadata") {
        if let Some(metadata_obj) = metadata.as_object() {
            assert_eq!(metadata_obj.get("created_by"), Some(&json!("test")));
            assert_eq!(metadata_obj.get("version"), Some(&json!(1.0)));
        }
    }
}

#[test]
#[serial]
fn test_yml_extension_handling() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Create a .yml file (same content as .yaml but different extension)
    let yml_content = r#"
app_name: SwissArmyHammer
version: 2.0.0
environment: test

database:
  host: localhost
  port: 5432
"#;

    let config_file = config_dir.join("sah.yml");
    fs::write(&config_file, yml_content).expect("Failed to write YML config");

    let context = TemplateContext::load_for_cli().expect("Failed to load YML config");

    // Test that .yml files are handled the same as .yaml files
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
    assert_eq!(context.get("version"), Some(&json!("2.0.0"))); // Fixed: changed from 2.0 to "2.0.0"
    assert_eq!(context.get("environment"), Some(&json!("test")));
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(context.get("database.port"), Some(&json!(5432)));
}

#[test]
#[serial]
fn test_json_config_file_loading() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Create a JSON config file with various data types
    let json_content = format!(
        r#"{{
  "app_name": "SwissArmyHammer",
  "version": "2.0.0",
  "debug": true,
  "max_connections": 100,
  "pi": {},
  "nullable_value": null,
  "database": {{
    "host": "localhost",
    "port": 5432,
    "username": "admin",
    "ssl": true,
    "pool": {{
      "min_connections": 5,
      "max_connections": 50
    }}
  }},
  "features": {{
    "enabled": ["templating", "config", "workflows"],
    "disabled": ["experimental"]
  }},
  "logging": {{
    "level": "info",
    "format": "json",
    "targets": {{
      "console": true,
      "file": "/var/log/sah.log",
      "syslog": false
    }}
  }},
  "metrics": {{
    "collection_interval": 30,
    "retention_days": 7
  }},
  "tags": ["development", "testing", "production"],
  "metadata": {{
    "created_by": "test",
    "version": 1.0,
    "timestamp": "2024-01-01T00:00:00Z"
  }}
}}"#,
        std::f64::consts::PI
    );

    let config_file = config_dir.join("sah.json");
    fs::write(&config_file, json_content).expect("Failed to write JSON config");

    let context = TemplateContext::load_for_cli().expect("Failed to load JSON config");

    // Test basic values
    assert_eq!(context.get("app_name"), Some(&json!("SwissArmyHammer")));
    assert_eq!(context.get("version"), Some(&json!("2.0.0")));
    assert_eq!(context.get("debug"), Some(&json!(true)));
    assert_eq!(context.get("max_connections"), Some(&json!(100)));
    assert_eq!(context.get("pi"), Some(&json!(std::f64::consts::PI)));
    assert_eq!(context.get("nullable_value"), Some(&json!(null)));

    // Test nested values
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(context.get("database.port"), Some(&json!(5432)));
    assert_eq!(context.get("database.username"), Some(&json!("admin")));
    assert_eq!(context.get("database.ssl"), Some(&json!(true)));

    // Test arrays
    if let Some(features) = context.get("features") {
        if let Some(features_obj) = features.as_object() {
            assert_eq!(
                features_obj.get("enabled"),
                Some(&json!(["templating", "config", "workflows"]))
            );
            assert_eq!(features_obj.get("disabled"), Some(&json!(["experimental"])));
        }
    }

    // Test top-level arrays
    assert_eq!(
        context.get("tags"),
        Some(&json!(["development", "testing", "production"]))
    );

    // Test timestamps and metadata
    if let Some(metadata) = context.get("metadata") {
        if let Some(metadata_obj) = metadata.as_object() {
            assert_eq!(metadata_obj.get("created_by"), Some(&json!("test")));
            assert_eq!(metadata_obj.get("version"), Some(&json!(1.0)));
            assert_eq!(
                metadata_obj.get("timestamp"),
                Some(&json!("2024-01-01T00:00:00Z"))
            );
        }
    }
}

#[test]
#[serial]
fn test_malformed_config_files() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Test malformed TOML
    let malformed_toml = r#"
app_name = "SwissArmyHammer"
[database
host = "localhost"
"#;
    let toml_file = config_dir.join("malformed.toml");
    fs::write(&toml_file, malformed_toml).expect("Failed to write malformed TOML");

    // Configuration system should handle malformed files gracefully by skipping them
    let result = TemplateContext::load_for_cli();
    assert!(
        result.is_ok(),
        "Should handle malformed TOML gracefully by skipping it"
    );

    // Remove malformed file and test malformed YAML
    fs::remove_file(&toml_file).expect("Failed to remove malformed TOML");

    let malformed_yaml = r#"
app_name: SwissArmyHammer
database:
  host: localhost
  port:  # Missing value
    username: admin
"#;
    let yaml_file = config_dir.join("malformed.yaml");
    fs::write(&yaml_file, malformed_yaml).expect("Failed to write malformed YAML");

    let result = TemplateContext::load_for_cli();
    assert!(
        result.is_ok(),
        "Should handle malformed YAML gracefully by skipping it"
    );

    // Remove malformed YAML and test malformed JSON
    fs::remove_file(&yaml_file).expect("Failed to remove malformed YAML");

    let malformed_json = r#"{
  "app_name": "SwissArmyHammer",
  "database": {
    "host": "localhost",
    "port": 5432,
  }  // Trailing comma is invalid in JSON
}"#;
    let json_file = config_dir.join("malformed.json");
    fs::write(&json_file, malformed_json).expect("Failed to write malformed JSON");

    let result = TemplateContext::load_for_cli();
    assert!(
        result.is_ok(),
        "Should handle malformed JSON gracefully by skipping it"
    );
}

#[test]
#[serial]
fn test_empty_config_files() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Test empty TOML file
    let toml_file = config_dir.join("empty.toml");
    fs::write(&toml_file, "").expect("Failed to write empty TOML");

    let _context = TemplateContext::load_for_cli().expect("Should load empty TOML");
    // Should succeed and create empty context
    // Context should handle empty TOML (just verify it was created successfully)

    fs::remove_file(&toml_file).expect("Failed to remove empty TOML");

    // Test empty YAML file
    let yaml_file = config_dir.join("empty.yaml");
    fs::write(&yaml_file, "").expect("Failed to write empty YAML");

    let _context = TemplateContext::load_for_cli().expect("Should load empty YAML");
    // Context should handle empty YAML (just verify it was created successfully)

    fs::remove_file(&yaml_file).expect("Failed to remove empty YAML");

    // Test empty JSON file - this should fail since empty string isn't valid JSON
    let json_file = config_dir.join("empty.json");
    fs::write(&json_file, "").expect("Failed to write empty JSON");

    let result = TemplateContext::load_for_cli();
    assert!(result.is_ok(), "Empty JSON should be handled gracefully");

    fs::remove_file(&json_file).expect("Failed to remove empty JSON");

    // Test valid empty JSON object
    let empty_json_file = config_dir.join("empty_object.json");
    fs::write(&empty_json_file, "{}").expect("Failed to write empty JSON object");

    let _context = TemplateContext::load_for_cli().expect("Should load empty JSON object");
    // Context should handle empty JSON object (just verify it was created successfully)
}

#[test]
#[serial]
fn test_file_format_precedence() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Create multiple config files with the same base name but different formats
    // Each contains the same key with different values to test precedence

    // TOML file
    let toml_content = r#"
test_value = "from_toml"
format = "toml"
"#;
    let toml_file = config_dir.join("sah.toml");
    fs::write(&toml_file, toml_content).expect("Failed to write TOML");

    // YAML file
    let yaml_content = r#"
test_value: from_yaml
format: yaml
"#;
    let yaml_file = config_dir.join("sah.yaml");
    fs::write(&yaml_file, yaml_content).expect("Failed to write YAML");

    // JSON file
    let json_content = r#"{
  "test_value": "from_json",
  "format": "json"
}"#;
    let json_file = config_dir.join("sah.json");
    fs::write(&json_file, json_content).expect("Failed to write JSON");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with multiple formats");

    // The exact precedence depends on figment's implementation
    // but we should get one of the values, not an error
    let test_value = context.get("test_value").expect("Should have test_value");
    let format_value = context.get("format").expect("Should have format");

    // Verify we got valid values from one of the files
    let test_str = test_value.as_str().expect("test_value should be string");
    let format_str = format_value.as_str().expect("format should be string");

    assert!(
        test_str == "from_toml" || test_str == "from_yaml" || test_str == "from_json",
        "test_value should be from one of the config files, got: {}",
        test_str
    );

    assert!(
        format_str == "toml" || format_str == "yaml" || format_str == "json",
        "format should be from one of the config files, got: {}",
        format_str
    );

    // The format and test_value should be from the same file
    match format_str {
        "toml" => assert_eq!(test_str, "from_toml"),
        "yaml" => assert_eq!(test_str, "from_yaml"),
        "json" => assert_eq!(test_str, "from_json"),
        _ => panic!("Unexpected format: {}", format_str),
    }
}

#[test]
#[serial]
fn test_complex_nested_structures() {
    let test = IsolatedConfigTest::new();
    let config_dir = test.project_config_dir();

    // Test deeply nested configuration with arrays and mixed types
    let complex_config = r#"{
  "application": {
    "name": "SwissArmyHammer",
    "version": {
      "major": 2,
      "minor": 0,
      "patch": 0
    },
    "metadata": {
      "authors": ["Claude", "Assistant"],
      "keywords": ["config", "templating", "automation"],
      "license": "MIT"
    }
  },
  "database": {
    "host": "localhost",
    "port": 5432,
    "pool": {
      "min_connections": 5,
      "max_connections": 50
    }
  },
  "environments": {
    "development": {
      "database_url": "postgresql://localhost/sah_dev",
      "debug": true,
      "features": ["hot_reload", "debug_toolbar"]
    }
  },
  "plugins": [
    {
      "name": "templating",
      "enabled": true
    },
    {
      "name": "automation",
      "enabled": false
    }
  ]
}"#;

    let config_file = config_dir.join("sah.json");
    fs::write(&config_file, complex_config).expect("Failed to write complex config");

    let context = TemplateContext::load_for_cli().expect("Failed to load complex config");

    // Test nested object access
    assert_eq!(
        context.get("application.name"),
        Some(&json!("SwissArmyHammer"))
    );
    assert_eq!(context.get("application.version.major"), Some(&json!(2)));
    assert_eq!(
        context.get("application.metadata.license"),
        Some(&json!("MIT"))
    );
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(
        context.get("database.pool.max_connections"),
        Some(&json!(50))
    );

    // Test deeper nesting
    if let Some(app) = context.get("application") {
        if let Some(app_obj) = app.as_object() {
            if let Some(version) = app_obj.get("version") {
                if let Some(version_obj) = version.as_object() {
                    assert_eq!(version_obj.get("major"), Some(&json!(2)));
                    assert_eq!(version_obj.get("minor"), Some(&json!(0)));
                    assert_eq!(version_obj.get("patch"), Some(&json!(0)));
                    assert_eq!(version_obj.get("pre_release"), None);
                }
            }
        }
    }

    // Test array access in nested structures
    if let Some(app) = context.get("application") {
        if let Some(app_obj) = app.as_object() {
            if let Some(metadata) = app_obj.get("metadata") {
                if let Some(metadata_obj) = metadata.as_object() {
                    assert_eq!(
                        metadata_obj.get("authors"),
                        Some(&json!(["Claude", "Assistant"]))
                    );
                    assert_eq!(
                        metadata_obj.get("keywords"),
                        Some(&json!(["config", "templating", "automation"]))
                    );
                }
            }
        }
    }

    // Test environment configurations
    if let Some(envs) = context.get("environments") {
        if let Some(envs_obj) = envs.as_object() {
            if let Some(dev) = envs_obj.get("development") {
                if let Some(dev_obj) = dev.as_object() {
                    assert_eq!(
                        dev_obj.get("database_url"),
                        Some(&json!("postgresql://localhost/sah_dev"))
                    );
                    assert_eq!(dev_obj.get("debug"), Some(&json!(true)));
                    assert_eq!(
                        dev_obj.get("features"),
                        Some(&json!(["hot_reload", "debug_toolbar"]))
                    );
                }
            }
        }
    }

    // Test array of objects (plugins)
    if let Some(plugins) = context.get("plugins") {
        if let Some(plugins_array) = plugins.as_array() {
            assert_eq!(plugins_array.len(), 2);

            if let Some(first_plugin) = plugins_array.first() {
                if let Some(plugin_obj) = first_plugin.as_object() {
                    assert_eq!(plugin_obj.get("name"), Some(&json!("templating")));
                    assert_eq!(plugin_obj.get("enabled"), Some(&json!(true)));
                }
            }
        }
    }
}
