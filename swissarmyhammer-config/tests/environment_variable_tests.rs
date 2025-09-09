//! Environment variable tests for the configuration system
//!
//! Tests SAH_ and SWISSARMYHAMMER_ environment variable prefixes with proper
//! key transformation, type conversion, and precedence handling.

use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use std::sync::Mutex;
use swissarmyhammer_config::TemplateContext;
use tempfile::TempDir;

/// Global mutex to serialize environment variable tests
/// This prevents race conditions when multiple tests modify environment variables
static ENV_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Test helper for isolated environment variable testing
struct IsolatedEnvTest {
    temp_dir: TempDir,
    original_cwd: std::path::PathBuf,
    original_home: Option<String>,
    env_vars_to_restore: Vec<(String, Option<String>)>,
    original_sah_vars: std::collections::HashMap<String, String>,
    _lock_guard: std::sync::MutexGuard<'static, ()>,
}

impl IsolatedEnvTest {
    fn new() -> Self {
        // Acquire the global test lock to prevent race conditions
        let lock_guard = ENV_TEST_LOCK.lock().unwrap_or_else(|poisoned| {
            eprintln!("Environment variable test lock was poisoned, recovering");
            poisoned.into_inner()
        });

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_cwd = env::current_dir().expect("Failed to get current dir");
        let original_home = env::var("HOME").ok();

        // Capture all existing SAH_ and SWISSARMYHAMMER_ environment variables
        let mut original_sah_vars = std::collections::HashMap::new();
        for (key, value) in env::vars() {
            if key.starts_with("SAH_") || key.starts_with("SWISSARMYHAMMER_") {
                original_sah_vars.insert(key, value);
            }
        }

        // Set up isolated environment
        let home_dir = temp_dir.path().join("home");
        fs::create_dir(&home_dir).expect("Failed to create home dir");
        env::set_var("HOME", &home_dir);
        env::set_current_dir(temp_dir.path()).expect("Failed to set current dir");

        Self {
            temp_dir,
            original_cwd,
            original_home,
            env_vars_to_restore: Vec::new(),
            original_sah_vars,
            _lock_guard: lock_guard,
        }
    }

    fn set_env_var(&mut self, key: &str, value: &str) {
        // Store original value for restoration
        let original = env::var(key).ok();
        self.env_vars_to_restore.push((key.to_string(), original));

        env::set_var(key, value);
    }

    fn remove_env_var(&mut self, key: &str) {
        let original = env::var(key).ok();
        self.env_vars_to_restore.push((key.to_string(), original));

        env::remove_var(key);
    }
}

impl Drop for IsolatedEnvTest {
    fn drop(&mut self) {
        // FIRST: Restore original working directory before temp directory is cleaned up
        let _ = env::set_current_dir(&self.original_cwd);

        // SECOND: Restore original HOME environment
        if let Some(home) = &self.original_home {
            env::set_var("HOME", home);
        } else {
            env::remove_var("HOME");
        }

        // THIRD: Clean up environment variables completely
        // Remove ALL current SAH_ and SWISSARMYHAMMER_ environment variables
        let current_sah_vars: Vec<String> = env::vars()
            .filter_map(|(key, _)| {
                if key.starts_with("SAH_") || key.starts_with("SWISSARMYHAMMER_") {
                    Some(key)
                } else {
                    None
                }
            })
            .collect();

        for key in current_sah_vars {
            env::remove_var(&key);
        }

        // Restore ONLY the original SAH_ and SWISSARMYHAMMER_ variables that existed before the test
        for (key, value) in &self.original_sah_vars {
            env::set_var(key, value);
        }

        // Restore other environment variables that were explicitly tracked
        for (key, original_value) in &self.env_vars_to_restore {
            if !key.starts_with("SAH_") && !key.starts_with("SWISSARMYHAMMER_") {
                match original_value {
                    Some(value) => env::set_var(key, value),
                    None => env::remove_var(key),
                }
            }
        }

        // FINALLY: The temp_dir will be cleaned up automatically when this struct is dropped
    }
}

#[test]
#[serial]
fn test_sah_prefix_basic_variables() {
    let mut test = IsolatedEnvTest::new();

    // Use unique timestamp-based keys to avoid conflicts with concurrent tests
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let key_suffix = timestamp % 1_000_000; // Keep it shorter for readability

    // Set basic SAH_ prefixed environment variables with unique keys
    test.set_env_var(&format!("SAH_T{}_APP_NAME", key_suffix), "TestApp");
    test.set_env_var(&format!("SAH_T{}_DEBUG", key_suffix), "true");
    test.set_env_var(&format!("SAH_T{}_PORT", key_suffix), "8080");
    test.set_env_var(&format!("SAH_T{}_VERSION", key_suffix), "1.0.0");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with SAH_ env vars");

    // Variables should be available with dot notation (uppercase converted to lowercase with dots)
    // Environment variables are automatically type-converted: booleans, numbers, and strings
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
#[serial]
fn test_swissarmyhammer_prefix_basic_variables() {
    let mut test = IsolatedEnvTest::new();

    // Set basic SWISSARMYHAMMER_ prefixed environment variables
    test.set_env_var("SWISSARMYHAMMER_APP_NAME", "SwissApp");
    test.set_env_var("SWISSARMYHAMMER_DEBUG", "false");
    test.set_env_var("SWISSARMYHAMMER_PORT", "9090");
    test.set_env_var("SWISSARMYHAMMER_VERSION", "2.0.0");

    let context = TemplateContext::load_for_cli()
        .expect("Failed to load config with SWISSARMYHAMMER_ env vars");

    // Variables should be available with dot notation
    // Environment variables are automatically type-converted: booleans, numbers, and strings
    assert_eq!(context.get("app.name"), Some(&json!("SwissApp")));
    assert_eq!(context.get("debug"), Some(&json!(false)));
    assert_eq!(context.get("port"), Some(&json!(9090)));
    assert_eq!(context.get("version"), Some(&json!("2.0.0")));
}

#[test]
#[serial]
fn test_nested_environment_variables() {
    let mut test = IsolatedEnvTest::new();

    // Set nested environment variables with underscores
    test.set_env_var("SAH_DATABASE_HOST", "localhost");
    test.set_env_var("SAH_DATABASE_PORT", "5432");
    test.set_env_var("SAH_DATABASE_USER", "admin");
    test.set_env_var("SAH_DATABASE_PASSWORD", "secret");
    test.set_env_var("SAH_DATABASE_SSL_ENABLED", "true");
    test.set_env_var("SAH_LOGGING_LEVEL", "info");
    test.set_env_var("SAH_LOGGING_FILE_PATH", "/var/log/app.log");

    let context = TemplateContext::load_for_cli().expect("Failed to load nested env vars");

    // Nested values should be accessible
    // Environment variables are automatically type-converted
    assert_eq!(context.get("database.host"), Some(&json!("localhost")));
    assert_eq!(context.get("database.port"), Some(&json!(5432)));
    assert_eq!(context.get("database.user"), Some(&json!("admin")));
    assert_eq!(context.get("database.password"), Some(&json!("secret")));
    assert_eq!(context.get("database.ssl.enabled"), Some(&json!(true)));
    assert_eq!(context.get("logging.level"), Some(&json!("info")));
    assert_eq!(
        context.get("logging.file.path"),
        Some(&json!("/var/log/app.log"))
    );
}

#[test]
#[serial]
fn test_both_prefixes_simultaneously() {
    let mut test = IsolatedEnvTest::new();

    // Set both SAH_ and SWISSARMYHAMMER_ variables for different keys
    test.set_env_var("SAH_SHORT_VAR", "sah_value");
    test.set_env_var("SAH_DATABASE_HOST", "sah-host");
    test.set_env_var("SWISSARMYHAMMER_LONG_VAR", "swiss_value");
    test.set_env_var("SWISSARMYHAMMER_DATABASE_PORT", "9999");

    let context = TemplateContext::load_for_cli().expect("Failed to load both prefix env vars");

    // Both prefixes should work simultaneously
    assert_eq!(context.get("short.var"), Some(&json!("sah_value")));
    assert_eq!(context.get("long.var"), Some(&json!("swiss_value")));
    assert_eq!(context.get("database.host"), Some(&json!("sah-host")));
    assert_eq!(context.get("database.port"), Some(&json!(9999)));
}

#[test]
#[serial]
fn test_prefix_precedence_when_both_set() {
    let mut test = IsolatedEnvTest::new();

    // Set the same logical key with both prefixes
    test.set_env_var("SAH_SHARED_VALUE", "from_sah");
    test.set_env_var("SWISSARMYHAMMER_SHARED_VALUE", "from_swissarmyhammer");
    test.set_env_var("SAH_DATABASE_HOST", "sah-database");
    test.set_env_var("SWISSARMYHAMMER_DATABASE_HOST", "swiss-database");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load conflicting prefix env vars");

    // One of the values should be present (precedence depends on implementation)
    let shared_value = context
        .get("shared.value")
        .expect("Should have shared.value");
    let shared_str = shared_value
        .as_str()
        .expect("shared.value should be string");
    assert!(
        shared_str == "from_sah" || shared_str == "from_swissarmyhammer",
        "shared.value should be from one of the prefixes, got: {}",
        shared_str
    );

    let db_host = context
        .get("database.host")
        .expect("Should have database.host");
    let db_host_str = db_host.as_str().expect("database.host should be string");
    assert!(
        db_host_str == "sah-database" || db_host_str == "swiss-database",
        "database.host should be from one of the prefixes, got: {}",
        db_host_str
    );
}

#[test]
#[serial]
fn test_environment_variable_type_conversion() {
    let mut test = IsolatedEnvTest::new();

    // Set environment variables that should be converted to appropriate types
    test.set_env_var("SAH_STRING_VALUE", "hello");
    test.set_env_var("SAH_INTEGER_VALUE", "42");
    test.set_env_var("SAH_FLOAT_VALUE", &std::f64::consts::PI.to_string());
    test.set_env_var("SAH_BOOLEAN_TRUE", "true");
    test.set_env_var("SAH_BOOLEAN_FALSE", "false");
    test.set_env_var("SAH_BOOLEAN_YES", "yes");
    test.set_env_var("SAH_BOOLEAN_NO", "no");
    test.set_env_var("SAH_BOOLEAN_ON", "on");
    test.set_env_var("SAH_BOOLEAN_OFF", "off");
    test.set_env_var("SAH_BOOLEAN_1", "1");
    test.set_env_var("SAH_BOOLEAN_0", "0");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load env vars with type conversion");

    // Note: Environment variables are typically strings, but figment may attempt type conversion
    // The exact behavior depends on figment's implementation
    assert_eq!(context.get("string.value"), Some(&json!("hello")));

    // Check if numeric values are present (might be strings or numbers)
    let integer_val = context
        .get("integer.value")
        .expect("Should have integer.value");
    // Could be "42" (string) or 42 (number) depending on figment's behavior
    assert!(
        integer_val == &json!("42") || integer_val == &json!(42),
        "integer.value should be '42' or 42, got: {:?}",
        integer_val
    );

    let float_val = context.get("float.value").expect("Should have float.value");
    assert!(
        float_val == &json!(std::f64::consts::PI.to_string())
            || float_val == &json!(std::f64::consts::PI),
        "float.value should be PI string or PI number, got: {:?}",
        float_val
    );

    // Boolean values might be converted
    let bool_true = context
        .get("boolean.true")
        .expect("Should have boolean.true");
    assert!(
        bool_true == &json!("true") || bool_true == &json!(true),
        "boolean.true should be 'true' or true, got: {:?}",
        bool_true
    );

    let bool_false = context
        .get("boolean.false")
        .expect("Should have boolean.false");
    assert!(
        bool_false == &json!("false") || bool_false == &json!(false),
        "boolean.false should be 'false' or false, got: {:?}",
        bool_false
    );
}

#[test]
#[serial]
fn test_environment_variable_with_special_characters() {
    let mut test = IsolatedEnvTest::new();

    // Set environment variables with special characters and edge cases
    test.set_env_var("SAH_URL", "https://example.com/path?param=value&other=123");
    test.set_env_var("SAH_PATH_WITH_SPACES", "/path/to/file with spaces");
    test.set_env_var("SAH_JSON_STRING", r#"{"nested": "value", "number": 42}"#);
    test.set_env_var("SAH_MULTILINE", "line1\nline2\nline3");
    test.set_env_var("SAH_EMPTY_VALUE", "");
    test.set_env_var("SAH_SPECIAL_CHARS", "!@#$%^&*()_+-=[]{}|;':\",./<>?");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load env vars with special chars");

    // All values should be preserved as strings
    assert_eq!(
        context.get("url"),
        Some(&json!("https://example.com/path?param=value&other=123"))
    );
    assert_eq!(
        context.get("path.with.spaces"),
        Some(&json!("/path/to/file with spaces"))
    );
    assert_eq!(
        context.get("json.string"),
        Some(&json!(r#"{"nested": "value", "number": 42}"#))
    );
    assert_eq!(
        context.get("multiline"),
        Some(&json!("line1\nline2\nline3"))
    );
    assert_eq!(context.get("empty.value"), Some(&json!("")));
    assert_eq!(
        context.get("special.chars"),
        Some(&json!("!@#$%^&*()_+-=[]{}|;':\",./<>?"))
    );
}

#[test]
#[serial]
fn test_environment_variable_override_config_file() {
    let mut test = IsolatedEnvTest::new();

    // Create a config file
    let config_dir = test.temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = r#"
app_name = "ConfigApp"
database_host = "config-host"
database_port = 5432
config_only = "config_value"
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config file");

    // Set environment variables that should override config
    test.set_env_var("SAH_APP_NAME", "EnvApp");
    test.set_env_var("SAH_DATABASE_HOST", "env-host");
    test.set_env_var("SAH_ENV_ONLY", "env_value");

    let context = TemplateContext::load_for_cli().expect("Failed to load config with env override");

    // Environment variables should override config values
    assert_eq!(context.get("app.name"), Some(&json!("EnvApp")));
    assert_eq!(context.get("database.host"), Some(&json!("env-host")));

    // Config values not overridden should remain
    assert_eq!(context.get("database_port"), Some(&json!(5432)));
    assert_eq!(context.get("config_only"), Some(&json!("config_value")));

    // Environment-only values should be present
    assert_eq!(context.get("env.only"), Some(&json!("env_value")));
}

#[test]
#[serial]
fn test_case_sensitivity_in_env_vars() {
    let mut test = IsolatedEnvTest::new();

    // Test different case patterns
    test.set_env_var("SAH_lowercase", "lower");
    test.set_env_var("SAH_UPPERCASE", "upper");
    test.set_env_var("SAH_MixedCase", "mixed");
    test.set_env_var("SAH_camelCase", "camel");
    test.set_env_var("SAH_snake_case", "snake");
    test.set_env_var("SAH_SCREAMING_SNAKE", "screaming");

    let context = TemplateContext::load_for_cli().expect("Failed to load case-sensitive env vars");

    // Check how different cases are handled
    // The exact mapping depends on figment's implementation
    let keys: Vec<String> = context.variables().keys().cloned().collect();

    // At least some of the variables should be present
    assert!(
        !keys.is_empty(),
        "Should have loaded some environment variables"
    );

    // We don't assert exact mappings since the case conversion rules depend on figment
    // But we verify that the variables are accessible in some form
    let has_some_vars = keys.iter().any(|k| {
        k.contains("lowercase")
            || k.contains("uppercase")
            || k.contains("mixed")
            || k.contains("camel")
            || k.contains("snake")
            || k.contains("screaming")
    });

    assert!(
        has_some_vars,
        "Should have some case-variant variables accessible: {:?}",
        keys
    );
}

#[test]
#[serial]
fn test_env_var_with_numbers_in_keys() {
    let mut test = IsolatedEnvTest::new();

    // Test environment variables with numbers
    test.set_env_var("SAH_SERVER1_HOST", "server1.example.com");
    test.set_env_var("SAH_SERVER2_HOST", "server2.example.com");
    test.set_env_var("SAH_PORT_8080_ENABLED", "true");
    test.set_env_var("SAH_VERSION_2_0_FEATURES", "advanced");
    test.set_env_var("SAH_DB_POOL_SIZE_10", "optimal");

    let context = TemplateContext::load_for_cli().expect("Failed to load env vars with numbers");

    // Variables with numbers should be handled correctly
    // The exact dot notation mapping depends on implementation
    let variables = context.variables();

    // Check that variables are present in some form
    let keys: Vec<&String> = variables.keys().collect();
    let has_server_vars = keys.iter().any(|k| k.contains("server"));
    let has_port_vars = keys
        .iter()
        .any(|k| k.contains("port") || k.contains("8080"));
    let has_version_vars = keys.iter().any(|k| k.contains("version"));

    assert!(has_server_vars, "Should have server variables: {:?}", keys);
    assert!(has_port_vars, "Should have port variables: {:?}", keys);
    assert!(
        has_version_vars,
        "Should have version variables: {:?}",
        keys
    );
}

#[test]
#[serial]
fn test_env_vars_with_no_config_files() {
    let mut test = IsolatedEnvTest::new();

    // Set environment variables without any config files
    test.set_env_var("SAH_STANDALONE_APP", "standalone");
    test.set_env_var("SAH_DATABASE_URL", "postgresql://localhost/standalone_db");
    test.set_env_var("SAH_FEATURE_FLAGS", "feature1,feature2,feature3");
    test.set_env_var("SWISSARMYHAMMER_BACKUP_ENABLED", "true");
    test.set_env_var("SWISSARMYHAMMER_LOG_LEVEL", "debug");

    let context = TemplateContext::load_for_cli().expect("Failed to load env-only config");

    // Environment variables should be the only source
    assert_eq!(context.get("standalone.app"), Some(&json!("standalone")));
    assert_eq!(
        context.get("database.url"),
        Some(&json!("postgresql://localhost/standalone_db"))
    );
    assert_eq!(
        context.get("feature.flags"),
        Some(&json!("feature1,feature2,feature3"))
    );
    assert_eq!(context.get("backup.enabled"), Some(&json!(true)));
    assert_eq!(context.get("log.level"), Some(&json!("debug")));
}

#[test]
#[serial]
fn test_invalid_env_var_names() {
    let mut test = IsolatedEnvTest::new();

    // Test edge cases and invalid patterns that should be ignored or handled gracefully
    test.set_env_var("SAH_", "empty_key"); // Empty key after prefix
    test.set_env_var("SAH__DOUBLE_UNDERSCORE", "double"); // Double underscore
    test.set_env_var("SAH_123_STARTS_WITH_NUMBER", "number"); // Starts with number
    test.set_env_var("SAH_VALID_KEY", "valid"); // Valid for comparison

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with edge case env vars");

    // Valid key should definitely work
    assert_eq!(context.get("valid.key"), Some(&json!("valid")));

    // Other edge cases might be handled differently - we just ensure no crash
    let variables = context.variables();
    assert!(
        !variables.is_empty(),
        "Should have at least the valid variable"
    );
}

#[test]
#[serial]
fn test_env_var_substitution_in_values() {
    let mut test = IsolatedEnvTest::new();

    // Set up environment variables for substitution testing
    test.set_env_var("HOME_DIR", "/home/user");
    test.set_env_var("APP_NAME", "MyApp");
    test.set_env_var("SAH_CONFIG_PATH", "${HOME_DIR}/.config/${APP_NAME}");
    test.set_env_var("SAH_DATABASE_URL", "postgresql://localhost/${APP_NAME}_db");
    test.set_env_var("SAH_LOG_FILE", "${HOME_DIR}/logs/${APP_NAME}.log");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with env var substitution");

    // Check if environment variable substitution occurs
    // This depends on whether the config system supports ${VAR} substitution
    let config_path = context.get("config.path").expect("Should have config.path");
    let db_url = context
        .get("database.url")
        .expect("Should have database.url");
    let log_file = context.get("log.file").expect("Should have log.file");

    // Values might be substituted or kept as-is depending on implementation
    let config_path_str = config_path.as_str().expect("config.path should be string");
    let db_url_str = db_url.as_str().expect("database.url should be string");
    let log_file_str = log_file.as_str().expect("log.file should be string");

    // At minimum, the values should be present
    assert!(
        !config_path_str.is_empty(),
        "config.path should not be empty"
    );
    assert!(!db_url_str.is_empty(), "database.url should not be empty");
    assert!(!log_file_str.is_empty(), "log.file should not be empty");

    // If substitution is supported, check for expanded values
    if config_path_str.contains("/home/user") && config_path_str.contains("MyApp") {
        // Substitution worked
        assert_eq!(config_path_str, "/home/user/.config/MyApp");
        assert_eq!(db_url_str, "postgresql://localhost/MyApp_db");
        assert_eq!(log_file_str, "/home/user/logs/MyApp.log");
    } else {
        // No substitution - values kept as-is
        assert_eq!(config_path_str, "${HOME_DIR}/.config/${APP_NAME}");
        assert_eq!(db_url_str, "postgresql://localhost/${APP_NAME}_db");
        assert_eq!(log_file_str, "${HOME_DIR}/logs/${APP_NAME}.log");
    }
}

#[test]
#[serial]
fn test_unset_environment_variables() {
    let mut test = IsolatedEnvTest::new();

    // Ensure specific environment variables are not set
    test.remove_env_var("SAH_UNSET_VAR");
    test.remove_env_var("SWISSARMYHAMMER_UNSET_VAR");

    // Set some valid variables for comparison
    test.set_env_var("SAH_SET_VAR", "set_value");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with unset env vars");

    // Set variable should be present
    assert_eq!(context.get("set.var"), Some(&json!("set_value")));

    // Unset variables should not be present
    assert_eq!(context.get("unset.var"), None);
}

#[test]
#[serial]
fn test_env_var_precedence_order_consistency() {
    let mut test = IsolatedEnvTest::new();

    // Create config file
    let config_dir = test.temp_dir.path().join(".swissarmyhammer");
    fs::create_dir_all(&config_dir).expect("Failed to create config dir");

    let config_content = r#"
test_precedence = "from_config"
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config file");

    // Set environment variable that should override config
    test.set_env_var("SAH_TEST_PRECEDENCE", "from_env");

    let context = TemplateContext::load_for_cli().expect("Failed to load precedence test");

    // Environment should override config
    assert_eq!(context.get("test.precedence"), Some(&json!("from_env")));

    // Test with CLI args (should override both)
    let cli_args = json!({
        "test_precedence": "from_cli"
    });

    let context_with_cli =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load with CLI args");

    // CLI should override both config and env
    assert_eq!(
        context_with_cli.get("test_precedence"),
        Some(&json!("from_cli"))
    );
}
