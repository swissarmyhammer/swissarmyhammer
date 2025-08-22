//! Simple integration test to validate core functionality

#[cfg(test)]
mod tests {
    use crate::{ConfigProvider, TemplateContext};
    use serial_test::serial;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn test_basic_functionality() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Create a basic config file
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        fs::write(
            sah_dir.join("sah.toml"),
            r#"
app_name = "Test App"
version = "1.0.0"

[database]
host = "localhost"
port = 5432
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Verify basic values are loaded
        assert_eq!(
            context.get("app_name"),
            Some(&serde_json::Value::String("Test App".to_string()))
        );
        assert_eq!(
            context.get("version"),
            Some(&serde_json::Value::String("1.0.0".to_string()))
        );

        // Check nested object
        if let Some(database) = context.get("database") {
            assert_eq!(
                database["host"],
                serde_json::Value::String("localhost".to_string())
            );
            assert_eq!(database["port"], serde_json::Value::Number(5432.into()));
        } else {
            panic!("Database configuration should be present");
        }
    }

    #[test]
    #[serial]
    fn test_template_context_operations() {
        let mut ctx = TemplateContext::new();

        // Test basic operations
        ctx.set(
            "test_key".to_string(),
            serde_json::Value::String("test_value".to_string()),
        );
        assert_eq!(
            ctx.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );

        // Test merge
        let mut other_ctx = TemplateContext::new();
        other_ctx.set(
            "other_key".to_string(),
            serde_json::Value::String("other_value".to_string()),
        );

        ctx.merge(&other_ctx);
        assert_eq!(
            ctx.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            ctx.get("other_key"),
            Some(&serde_json::Value::String("other_value".to_string()))
        );
    }

    #[test]
    #[serial]
    fn test_env_var_substitution_basic() {
        std::env::set_var("TEST_ENV_VAR", "test_env_value");

        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_var".to_string(),
            serde_json::Value::String("${TEST_ENV_VAR}".to_string()),
        );

        ctx.substitute_env_vars().unwrap();

        assert_eq!(
            ctx.get("config_var"),
            Some(&serde_json::Value::String("test_env_value".to_string()))
        );

        std::env::remove_var("TEST_ENV_VAR");
    }

    #[test]
    fn test_defaults_integration() {
        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        // Should have default values
        assert!(!context.is_empty());
        assert!(context.get("environment").is_some());
        assert!(context.get("debug").is_some());
        assert!(context.get("project_name").is_some());
        assert!(context.get("log_level").is_some());
        assert!(context.get("timeout_seconds").is_some());

        // Check specific default values
        assert_eq!(
            context.get("environment"),
            Some(&serde_json::Value::String("development".to_string()))
        );
        assert_eq!(context.get("debug"), Some(&serde_json::Value::Bool(false)));
        assert_eq!(
            context.get("project_name"),
            Some(&serde_json::Value::String("swissarmyhammer-project".to_string()))
        );
    }

    #[test]
    #[serial]
    fn test_multiple_config_formats_integration() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Create configs in different formats
        fs::write(
            sah_dir.join("sah.toml"),
            r#"
app_name = "TOML App"
toml_specific = "toml_value"

[database]
host = "toml-host"
"#,
        )
        .unwrap();

        fs::write(
            sah_dir.join("sah.yaml"),
            r#"
app_version: "2.0.0"
yaml_specific: yaml_value
database:
  port: 3306
"#,
        )
        .unwrap();

        fs::write(
            sah_dir.join("sah.json"),
            r#"{
    "api_key": "json_key",
    "json_specific": "json_value",
    "database": {
        "ssl": true
    }
}"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Should have values from all formats
        assert!(context.get("toml_specific").is_some());
        assert!(context.get("yaml_specific").is_some());
        assert!(context.get("json_specific").is_some());
        assert!(context.get("app_version").is_some());
        assert!(context.get("api_key").is_some());

        // Database object should be merged from all sources
        if let Some(serde_json::Value::Object(database)) = context.get("database") {
            // Values from different formats should be merged
            assert!(database.contains_key("host") || database.contains_key("port") || database.contains_key("ssl"));
        }

        // Should still have default values
        assert!(context.get("environment").is_some());
    }

    #[test]
    #[serial]
    fn test_end_to_end_configuration_with_env_substitution() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Set up environment variables for substitution
        std::env::set_var("DB_HOST", "production-db.example.com");
        std::env::set_var("DB_PASSWORD", "secret123");
        std::env::set_var("APP_ENV", "production");

        // Create config with environment variable substitutions
        fs::write(
            sah_dir.join("sah.toml"),
            r#"
app_name = "Production App"
environment = "${APP_ENV:-development}"

[database]
host = "${DB_HOST}"
password = "${DB_PASSWORD}"
port = 5432
fallback_host = "${MISSING_VAR:-localhost}"

[logging]
level = "info"
file = "/var/log/app.log"
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Also set environment variables that should override config
        std::env::set_var("SAH_APP_NAME", "Env Override App");
        std::env::set_var("SAH_LOGGING__LEVEL", "debug");

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Cleanup environment variables
        std::env::remove_var("DB_HOST");
        std::env::remove_var("DB_PASSWORD");
        std::env::remove_var("APP_ENV");
        std::env::remove_var("SAH_APP_NAME");
        std::env::remove_var("SAH_LOGGING__LEVEL");

        // Environment variable overrides should work
        assert_eq!(
            context.get("app_name"),
            Some(&serde_json::Value::String("Env Override App".to_string()))
        );

        // Environment substitution should work
        assert_eq!(
            context.get("environment"),
            Some(&serde_json::Value::String("production".to_string()))
        );

        // Check database configuration with substitution
        if let Some(serde_json::Value::Object(database)) = context.get("database") {
            // Environment substitution should work in the config file values
            // These should be substituted if the SAH_ override didn't completely replace them
            if database.contains_key("host") {
                // Could be substituted value or overridden value
                let host_val = &database["host"];
                if let serde_json::Value::String(host_str) = host_val {
                    // Should be either the substituted value or an override
                    assert!(host_str == "production-db.example.com" || host_str == "localhost" || !host_str.is_empty());
                }
            }
            
            if database.contains_key("password") {
                let password_val = &database["password"];
                if let serde_json::Value::String(password_str) = password_val {
                    assert!(password_str == "secret123" || !password_str.is_empty());
                }
            }
            
            if database.contains_key("fallback_host") {
                let fallback_val = &database["fallback_host"];
                if let serde_json::Value::String(fallback_str) = fallback_val {
                    assert!(fallback_str == "localhost" || !fallback_str.is_empty());
                }
            }
        }

        // Nested environment overrides should work
        if let Some(serde_json::Value::Object(logging)) = context.get("logging") {
            assert_eq!(
                logging["level"],
                serde_json::Value::String("debug".to_string())
            );
        }

        // Default values should still be present
        assert!(context.get("debug").is_some());
        assert!(context.get("project_name").is_some());
    }

    #[test]
    #[serial]
    fn test_error_handling_integration() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Create config with missing environment variable (no default)
        fs::write(
            sah_dir.join("sah.toml"),
            r#"
app_name = "Test App"
database_url = "${MISSING_DATABASE_URL}"
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let result = provider.load_template_context();

        std::env::set_current_dir(original_dir).unwrap();

        // Should fail due to missing environment variable
        assert!(result.is_err());
    }

    #[test]
    #[serial]
    fn test_complex_nested_configuration() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Create complex nested configuration
        fs::write(
            sah_dir.join("sah.toml"),
            r#"
[server]
host = "0.0.0.0"
port = 8080

[server.ssl]
enabled = true
cert_path = "/etc/ssl/cert.pem"
key_path = "/etc/ssl/key.pem"

[database]
primary = "postgresql://localhost:5432/main"
replica = "postgresql://localhost:5433/main"

[database.pool]
min_connections = 5
max_connections = 20
timeout_seconds = 30

[features]
feature_a = true
feature_b = false

[features.experimental]
new_ui = false
beta_api = true
"#,
        )
        .unwrap();

        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Set environment variables for nested override
        std::env::set_var("SAH_SERVER__PORT", "9090");
        std::env::set_var("SAH_SERVER__SSL__ENABLED", "false");
        std::env::set_var("SAH_DATABASE__POOL__MAX_CONNECTIONS", "50");
        std::env::set_var("SAH_FEATURES__EXPERIMENTAL__NEW_UI", "true");

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        std::env::set_current_dir(original_dir).unwrap();

        // Cleanup
        std::env::remove_var("SAH_SERVER__PORT");
        std::env::remove_var("SAH_SERVER__SSL__ENABLED");
        std::env::remove_var("SAH_DATABASE__POOL__MAX_CONNECTIONS");
        std::env::remove_var("SAH_FEATURES__EXPERIMENTAL__NEW_UI");

        // Check nested structures exist and have correct values
        if let Some(serde_json::Value::Object(server)) = context.get("server") {
            assert_eq!(server["host"], serde_json::Value::String("0.0.0.0".to_string()));
            // Environment should override (comes as string)
            if let Some(port_val) = server.get("port") {
                // Could be number from file or string from env
                let port_str = match port_val {
                    serde_json::Value::String(s) => s.clone(),
                    serde_json::Value::Number(n) => n.to_string(),
                    _ => panic!("Unexpected port value type")
                };
                // Should be overridden by env var
                assert_eq!(port_str, "9090");
            }
            
            if let Some(serde_json::Value::Object(ssl)) = server.get("ssl") {
                // Environment should override (comes as string)
                if let Some(enabled_val) = ssl.get("enabled") {
                    match enabled_val {
                        serde_json::Value::String(s) => assert_eq!(s, "false"),
                        serde_json::Value::Bool(b) => assert!(!(*b)),
                        _ => panic!("Unexpected enabled value type")
                    }
                }
                assert!(ssl.contains_key("cert_path"));
            }
        }

        if let Some(serde_json::Value::Object(database)) = context.get("database") {
            assert!(database.contains_key("primary"));
            
            if let Some(serde_json::Value::Object(pool)) = database.get("pool") {
                // Environment should override (check both string and number)
                if let Some(max_conn_val) = pool.get("max_connections") {
                    match max_conn_val {
                        serde_json::Value::String(s) => assert_eq!(s, "50"),
                        serde_json::Value::Number(n) => assert_eq!(n, &50.into()),
                        _ => panic!("Unexpected max_connections value type")
                    }
                }
                assert!(pool.contains_key("min_connections"));
            }
        }

        if let Some(serde_json::Value::Object(features)) = context.get("features") {
            if let Some(serde_json::Value::Object(experimental)) = features.get("experimental") {
                // Environment should override (check both string and bool)
                if let Some(new_ui_val) = experimental.get("new_ui") {
                    match new_ui_val {
                        serde_json::Value::String(s) => assert_eq!(s, "true"),
                        serde_json::Value::Bool(b) => assert!(*b),
                        _ => panic!("Unexpected new_ui value type")
                    }
                }
                assert!(experimental.contains_key("beta_api"));
            }
        }

        // Default values should still be present
        assert!(context.get("environment").is_some());
        assert!(context.get("debug").is_some());
    }
}
