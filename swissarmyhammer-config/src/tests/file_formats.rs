//! Tests for different configuration file formats

use crate::ConfigProvider;
use serial_test::serial;
use std::fs;
use tempfile::TempDir;

#[test]
#[serial]
fn test_toml_format_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    fs::write(
        sah_dir.join("sah.toml"),
        r#"
# SwissArmyHammer Configuration
app_name = "TOML Test App"
version = "1.0.0"
debug = true

[database]
host = "localhost"
port = 5432
ssl = true

[logging]
level = "debug"
file = "/var/log/app.log"

[[services]]
name = "api"
port = 8080

[[services]]
name = "worker"
port = 8081

[features]
workflows = true
prompts = true
mcp = false
"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    std::env::set_current_dir(original_dir).unwrap();

    // Check basic values
    assert_eq!(
        context.get("app_name"),
        Some(&serde_json::Value::String("TOML Test App".to_string()))
    );
    assert_eq!(
        context.get("version"),
        Some(&serde_json::Value::String("1.0.0".to_string()))
    );
    assert_eq!(context.get("debug"), Some(&serde_json::Value::Bool(true)));

    // Check nested objects
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("localhost".to_string())
        );
        assert_eq!(database["port"], serde_json::Value::Number(5432.into()));
        assert_eq!(database["ssl"], serde_json::Value::Bool(true));
    } else {
        panic!("Database section should be present");
    }

    // Check arrays of objects
    if let Some(services) = context.get("services") {
        if let serde_json::Value::Array(arr) = services {
            assert_eq!(arr.len(), 2);
            // Check first service
            assert_eq!(arr[0]["name"], serde_json::Value::String("api".to_string()));
            assert_eq!(arr[0]["port"], serde_json::Value::Number(8080.into()));
        } else {
            panic!("Services should be an array");
        }
    }

    // Check nested objects with boolean values
    if let Some(features) = context.get("features") {
        assert_eq!(features["workflows"], serde_json::Value::Bool(true));
        assert_eq!(features["mcp"], serde_json::Value::Bool(false));
    } else {
        panic!("Features section should be present");
    }
}

#[test]
#[serial]
fn test_yaml_format_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Remove any other config files to ensure only YAML is loaded
    let _ = fs::remove_file(sah_dir.join("sah.toml"));
    let _ = fs::remove_file(sah_dir.join("sah.json"));

    fs::write(
        sah_dir.join("sah.yaml"),
        r#"
# SwissArmyHammer YAML Configuration
app_name: "YAML Test App"
version: "2.0.0"
debug: false

database:
  host: "yaml-db.example.com"
  port: 3306
  ssl: false
  options:
    - "timeout=30"
    - "charset=utf8"

logging:
  level: "info"
  outputs:
    - type: "file"
      path: "/tmp/app.log"
    - type: "stdout"
      format: "json"

environments:
  development:
    api_url: "https://dev-api.example.com"
    debug: true
  production:
    api_url: "https://api.example.com"
    debug: false

features:
  - workflows
  - prompts
  - mcp
  - analytics
"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    std::env::set_current_dir(original_dir).unwrap();

    // Check basic values
    assert_eq!(
        context.get("app_name"),
        Some(&serde_json::Value::String("YAML Test App".to_string()))
    );
    assert_eq!(
        context.get("version"),
        Some(&serde_json::Value::String("2.0.0".to_string()))
    );
    assert_eq!(context.get("debug"), Some(&serde_json::Value::Bool(false)));

    // Check nested objects
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("yaml-db.example.com".to_string())
        );
        assert_eq!(database["port"], serde_json::Value::Number(3306.into()));

        if let serde_json::Value::Array(options) = &database["options"] {
            assert!(options.contains(&serde_json::Value::String("timeout=30".to_string())));
        } else {
            panic!("Database options should be an array");
        }
    }

    // Check nested environments
    if let Some(environments) = context.get("environments") {
        assert_eq!(
            environments["development"]["debug"],
            serde_json::Value::Bool(true)
        );
        assert_eq!(
            environments["production"]["debug"],
            serde_json::Value::Bool(false)
        );
    }

    // Check simple array
    if let Some(serde_json::Value::Array(arr)) = context.get("features") {
        assert!(arr.contains(&serde_json::Value::String("workflows".to_string())));
        assert!(arr.contains(&serde_json::Value::String("mcp".to_string())));
        assert!(arr.contains(&serde_json::Value::String("analytics".to_string())));
    }
}

#[test]
#[serial]
fn test_json_format_comprehensive() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Remove any other config files to ensure only JSON is loaded
    let _ = fs::remove_file(sah_dir.join("sah.toml"));
    let _ = fs::remove_file(sah_dir.join("sah.yaml"));

    fs::write(
        sah_dir.join("sah.json"),
        r#"{
    "app_name": "JSON Test App",
    "version": "3.0.0",
    "debug": true,
    "database": {
        "host": "json-db.example.com",
        "port": 5432,
        "credentials": {
            "username": "app_user",
            "password_env": "${DB_PASSWORD:-default_pass}"
        },
        "pools": [
            {
                "name": "read",
                "size": 5
            },
            {
                "name": "write",
                "size": 2
            }
        ]
    },
    "api": {
        "version": "v1",
        "endpoints": {
            "users": "/api/v1/users",
            "auth": "/api/v1/auth",
            "health": "/api/v1/health"
        },
        "rate_limits": {
            "per_minute": 100,
            "per_hour": 1000,
            "burst": 10
        }
    },
    "features": [
        "workflows",
        "prompts",
        "mcp",
        "rest_api",
        "websockets"
    ],
    "metadata": {
        "created_at": "2024-01-01T00:00:00Z",
        "author": "SwissArmyHammer",
        "config_version": 1.2
    }
}"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();
    std::env::set_var("DB_PASSWORD", "secure_password_123");

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    std::env::set_current_dir(original_dir).unwrap();
    std::env::remove_var("DB_PASSWORD");

    // Check basic values
    assert_eq!(
        context.get("app_name"),
        Some(&serde_json::Value::String("JSON Test App".to_string()))
    );
    assert_eq!(
        context.get("version"),
        Some(&serde_json::Value::String("3.0.0".to_string()))
    );

    // Check deeply nested objects
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["credentials"]["username"],
            serde_json::Value::String("app_user".to_string())
        );
        assert_eq!(
            database["credentials"]["password_env"],
            serde_json::Value::String("secure_password_123".to_string())
        );

        // Check array of objects
        if let serde_json::Value::Array(pools) = &database["pools"] {
            assert_eq!(pools.len(), 2);
            assert_eq!(
                pools[0]["name"],
                serde_json::Value::String("read".to_string())
            );
            assert_eq!(pools[0]["size"], serde_json::Value::Number(5.into()));
        }
    }

    // Check nested objects with multiple levels
    if let Some(api) = context.get("api") {
        assert_eq!(
            api["endpoints"]["users"],
            serde_json::Value::String("/api/v1/users".to_string())
        );
        assert_eq!(
            api["rate_limits"]["per_minute"],
            serde_json::Value::Number(100.into())
        );
    }

    // Check mixed numeric types in metadata
    if let Some(metadata) = context.get("metadata") {
        if let Some(config_version) = metadata.get("config_version") {
            // Should handle floating point numbers
            assert!(config_version.is_number());
        }
    }
}

#[test]
#[serial]
fn test_format_precedence_same_file() {
    let temp_dir = TempDir::new().unwrap();
    let original_dir = std::env::current_dir().unwrap();

    let sah_dir = temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&sah_dir).unwrap();

    // Create multiple formats of the same config
    fs::write(
        sah_dir.join("sah.toml"),
        r#"
shared_key = "from_toml"
format_source = "toml"
"#,
    )
    .unwrap();

    fs::write(
        sah_dir.join("sah.yaml"),
        r#"
shared_key: "from_yaml"
format_source: "yaml"
"#,
    )
    .unwrap();

    fs::write(
        sah_dir.join("sah.json"),
        r#"{
    "shared_key": "from_json",
    "format_source": "json"
}"#,
    )
    .unwrap();

    std::env::set_current_dir(temp_dir.path()).unwrap();

    let provider = ConfigProvider::new();
    let context = provider.load_template_context().unwrap();

    std::env::set_current_dir(original_dir).unwrap();

    // The exact precedence depends on figment's implementation
    // but all formats should be loaded and merged
    assert!(context.get("shared_key").is_some());
    assert!(context.get("format_source").is_some());

    // We should have values from at least one format
    let shared_val = context.get("shared_key").unwrap().as_str().unwrap();
    assert!(shared_val == "from_toml" || shared_val == "from_yaml" || shared_val == "from_json");
}
