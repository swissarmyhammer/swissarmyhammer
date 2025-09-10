//! Fixed environment variable tests using unique variable names
//!
//! Tests SAH_ and SWISSARMYHAMMER_ environment variable prefixes with proper
//! key transformation, type conversion, and precedence handling.

use serde_json::json;
use std::env;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

/// Test helper for isolated environment variable testing
struct IsolatedEnvTest {
    _temp_dir: TempDir,
    original_cwd: std::path::PathBuf,
    original_home: Option<String>,
    env_vars_to_restore: Vec<(String, Option<String>)>,
    timestamp: String,
}

impl IsolatedEnvTest {
    fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_cwd = env::current_dir().expect("Failed to get current dir");
        let original_home = env::var("HOME").ok();

        // Create unique timestamp for variable names
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos()
            .to_string();

        // Set up isolated environment
        let home_dir = temp_dir.path().join("home");
        fs::create_dir(&home_dir).expect("Failed to create home dir");
        env::set_var("HOME", &home_dir);
        env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        Self {
            _temp_dir: temp_dir,
            original_cwd,
            original_home,
            env_vars_to_restore: Vec::new(),
            timestamp,
        }
    }

    fn set_env_var(&mut self, key: &str, value: &str) {
        // Store original value for restoration
        let original = env::var(key).ok();
        self.env_vars_to_restore.push((key.to_string(), original));

        env::set_var(key, value);
    }
}

impl Drop for IsolatedEnvTest {
    fn drop(&mut self) {
        // Restore environment variables
        for (key, original_value) in &self.env_vars_to_restore {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }

        // Restore original environment
        let _ = env::set_current_dir(&self.original_cwd);
        if let Some(home) = &self.original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }
    }
}

#[test]
fn test_sah_prefix_basic_variables_fixed() {
    let mut test = IsolatedEnvTest::new();
    let key_suffix = test.timestamp.clone();

    // Set basic SAH_ prefixed environment variables with unique timestamps
    test.set_env_var(&format!("SAH_T{}_APP_NAME", key_suffix), "TestApp");
    test.set_env_var(&format!("SAH_T{}_DEBUG", key_suffix), "true");
    test.set_env_var(&format!("SAH_T{}_PORT", key_suffix), "8080");
    test.set_env_var(&format!("SAH_T{}_VERSION", key_suffix), "1.0.0");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with SAH_ env vars");

    // Variables should be available with dot notation (prefixed with timestamp)
    let app_key = format!("t{}.app.name", key_suffix);
    let debug_key = format!("t{}.debug", key_suffix);
    let port_key = format!("t{}.port", key_suffix);
    let version_key = format!("t{}.version", key_suffix);

    assert_eq!(context.get(&app_key), Some(&json!("TestApp")));
    assert_eq!(context.get(&debug_key), Some(&json!(true)));
    assert_eq!(context.get(&port_key), Some(&json!(8080)));
    assert_eq!(context.get(&version_key), Some(&json!("1.0.0")));
}

#[test]
fn test_swissarmyhammer_prefix_basic_variables_fixed() {
    let mut test = IsolatedEnvTest::new();
    let key_suffix = test.timestamp.clone();

    // Set basic SWISSARMYHAMMER_ prefixed environment variables with unique timestamps
    test.set_env_var(
        &format!("SWISSARMYHAMMER_T{}_APP_NAME", key_suffix),
        "SwissApp",
    );
    test.set_env_var(&format!("SWISSARMYHAMMER_T{}_DEBUG", key_suffix), "false");
    test.set_env_var(&format!("SWISSARMYHAMMER_T{}_PORT", key_suffix), "9090");
    test.set_env_var(&format!("SWISSARMYHAMMER_T{}_VERSION", key_suffix), "2.0.0");

    let context = TemplateContext::load_for_cli()
        .expect("Failed to load config with SWISSARMYHAMMER_ env vars");

    // Variables should be available with dot notation (prefixed with timestamp)
    let app_key = format!("t{}.app.name", key_suffix);
    let debug_key = format!("t{}.debug", key_suffix);
    let port_key = format!("t{}.port", key_suffix);
    let version_key = format!("t{}.version", key_suffix);

    assert_eq!(context.get(&app_key), Some(&json!("SwissApp")));
    assert_eq!(context.get(&debug_key), Some(&json!(false)));
    assert_eq!(context.get(&port_key), Some(&json!(9090)));
    assert_eq!(context.get(&version_key), Some(&json!("2.0.0")));
}

#[test]
fn test_nested_environment_variables_fixed() {
    let mut test = IsolatedEnvTest::new();
    let key_suffix = test.timestamp.clone();

    // Set nested environment variables with underscores and unique timestamps
    test.set_env_var(&format!("SAH_T{}_DATABASE_HOST", key_suffix), "localhost");
    test.set_env_var(&format!("SAH_T{}_DATABASE_PORT", key_suffix), "5432");
    test.set_env_var(&format!("SAH_T{}_DATABASE_USER", key_suffix), "admin");

    let context = TemplateContext::load_for_cli().expect("Failed to load nested env vars");

    // Nested values should be accessible with unique prefixes
    let db_host_key = format!("t{}.database.host", key_suffix);
    let db_port_key = format!("t{}.database.port", key_suffix);
    let db_user_key = format!("t{}.database.user", key_suffix);

    assert_eq!(context.get(&db_host_key), Some(&json!("localhost")));
    assert_eq!(context.get(&db_port_key), Some(&json!(5432)));
    assert_eq!(context.get(&db_user_key), Some(&json!("admin")));
}

#[test]
fn test_environment_variable_type_conversion_fixed() {
    let mut test = IsolatedEnvTest::new();
    let key_suffix = test.timestamp.clone();

    // Set environment variables that should be converted to appropriate types
    test.set_env_var(&format!("SAH_T{}_STRING_VALUE", key_suffix), "hello");
    test.set_env_var(&format!("SAH_T{}_INTEGER_VALUE", key_suffix), "42");
    test.set_env_var(&format!("SAH_T{}_BOOLEAN_TRUE", key_suffix), "true");
    test.set_env_var(&format!("SAH_T{}_BOOLEAN_FALSE", key_suffix), "false");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load env vars with type conversion");

    let string_key = format!("t{}.string.value", key_suffix);
    let integer_key = format!("t{}.integer.value", key_suffix);
    let bool_true_key = format!("t{}.boolean.true", key_suffix);
    let bool_false_key = format!("t{}.boolean.false", key_suffix);

    assert_eq!(context.get(&string_key), Some(&json!("hello")));
    assert_eq!(context.get(&integer_key), Some(&json!(42)));
    assert_eq!(context.get(&bool_true_key), Some(&json!(true)));
    assert_eq!(context.get(&bool_false_key), Some(&json!(false)));
}

#[test]
fn test_environment_variables_unique_keys_work() {
    let mut test = IsolatedEnvTest::new();
    let key_suffix = test.timestamp.clone();

    // Test that our unique key generation strategy works
    test.set_env_var(&format!("SAH_T{}_TEST_VAR", key_suffix), "test_value");

    let context = TemplateContext::load_for_cli().expect("Failed to load env vars");

    let test_key = format!("t{}.test.var", key_suffix);
    assert_eq!(context.get(&test_key), Some(&json!("test_value")));
}
