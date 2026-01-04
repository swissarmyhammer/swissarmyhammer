//! Precedence order tests for the configuration system
//!
//! Tests that configuration sources are merged in the correct precedence order:
//! defaults → global config → project config → environment variables → CLI arguments
//! (later sources override earlier ones)

use serde_json::json;
use serial_test::serial;
use std::env;
use std::fs;
use swissarmyhammer_common::test_utils::IsolatedTestEnvironment;
use swissarmyhammer_config::TemplateContext;

/// Test helper for isolated precedence testing
struct IsolatedPrecedenceTest {
    _env: IsolatedTestEnvironment,
    original_cwd: std::path::PathBuf,
    env_vars_to_restore: Vec<(String, Option<String>)>,
}

impl IsolatedPrecedenceTest {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");
        let original_cwd = env::current_dir().expect("Failed to get current dir");

        // Set current directory to temp dir for these tests
        env::set_current_dir(env.temp_dir()).expect("Failed to set current dir");

        Self {
            _env: env,
            original_cwd,
            env_vars_to_restore: Vec::new(),
        }
    }

    fn set_env_var(&mut self, key: &str, value: &str) {
        // Store original value for restoration
        let original = env::var(key).ok();
        self.env_vars_to_restore.push((key.to_string(), original));

        env::set_var(key, value);
    }

    fn project_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self._env.temp_dir().join(".swissarmyhammer");
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }

    fn home_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self._env.swissarmyhammer_dir();
        fs::create_dir_all(&config_dir).expect("Failed to create home config dir");
        config_dir
    }
}

impl Drop for IsolatedPrecedenceTest {
    fn drop(&mut self) {
        // Restore environment variables
        for (key, original_value) in &self.env_vars_to_restore {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }

        // Restore original directory - IsolatedTestEnvironment handles HOME restoration
        let _ = env::set_current_dir(&self.original_cwd);
    }
}

#[test]
#[serial]
fn test_global_overrides_defaults() {
    let test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();

    // Create global config that should override any defaults
    let global_config = r#"
app_name = "GlobalApp"
database_port = 5432
debug_mode = true
global_only = "global_value"
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    let context = TemplateContext::load_for_cli().expect("Failed to load global config");

    // Global config values should be present
    assert_eq!(context.get("app_name"), Some(&json!("GlobalApp")));
    assert_eq!(context.get("database_port"), Some(&json!(5432)));
    assert_eq!(context.get("debug_mode"), Some(&json!(true)));
    assert_eq!(context.get("global_only"), Some(&json!("global_value")));
}

#[test]
#[serial]
fn test_project_overrides_global() {
    let test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();
    let project_config_dir = test.project_config_dir();

    // Create global config
    let global_config = r#"
app_name = "GlobalApp"
database_port = 5432
debug_mode = true
global_only = "global_value"
shared_setting = "from_global"
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    // Create project config that should override global
    let project_config = r#"
app_name = "ProjectApp"
database_port = 3306
project_only = "project_value"
shared_setting = "from_project"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    let context = TemplateContext::load_for_cli().expect("Failed to load merged config");

    // Project values should override global values
    assert_eq!(context.get("app_name"), Some(&json!("ProjectApp")));
    assert_eq!(context.get("database_port"), Some(&json!(3306)));
    assert_eq!(context.get("shared_setting"), Some(&json!("from_project")));

    // Project-only values should be present
    assert_eq!(context.get("project_only"), Some(&json!("project_value")));

    // Global-only values should still be present (not overridden)
    assert_eq!(context.get("global_only"), Some(&json!("global_value")));
    assert_eq!(context.get("debug_mode"), Some(&json!(true)));
}

#[test]
#[serial]
fn test_env_vars_override_config_files() {
    let mut test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();
    let project_config_dir = test.project_config_dir();

    // Create global config
    let global_config = r#"
app_name = "GlobalApp"
database_port = 5432
shared_value = "from_global"
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    // Create project config
    let project_config = r#"
app_name = "ProjectApp"
database_port = 3306
shared_value = "from_project"
project_setting = "project_value"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set environment variables that should override config files
    test.set_env_var("SAH_APP_NAME", "EnvApp");
    test.set_env_var("SAH_DATABASE_PORT", "9999");
    test.set_env_var("SAH_SHARED_VALUE", "from_env");
    test.set_env_var("SAH_ENV_ONLY", "env_value");

    let context = TemplateContext::load_for_cli().expect("Failed to load config with env vars");

    // Environment variables should override config file values
    assert_eq!(context.get("app.name"), Some(&json!("EnvApp")));
    assert_eq!(context.get("database.port"), Some(&json!(9999)));
    assert_eq!(context.get("shared.value"), Some(&json!("from_env")));

    // Environment-only values should be present
    assert_eq!(context.get("env.only"), Some(&json!("env_value")));

    // Non-overridden config values should still be present
    assert_eq!(
        context.get("project_setting"),
        Some(&json!("project_value"))
    );
}

#[test]
#[serial]
fn test_swissarmyhammer_prefix_env_vars() {
    let mut test = IsolatedPrecedenceTest::new();
    let project_config_dir = test.project_config_dir();

    // Create project config
    let project_config = r#"
app_name = "ProjectApp"
database_port = 3306
shared_value = "from_project"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set SWISSARMYHAMMER_ prefixed environment variables
    test.set_env_var("SWISSARMYHAMMER_APP_NAME", "SwissArmyApp");
    test.set_env_var("SWISSARMYHAMMER_DATABASE_PORT", "8888");
    test.set_env_var("SWISSARMYHAMMER_SHARED_VALUE", "from_swissarmyhammer_env");
    test.set_env_var("SWISSARMYHAMMER_LONG_PREFIX_ONLY", "long_prefix_value");

    let context = TemplateContext::load_for_cli()
        .expect("Failed to load config with SWISSARMYHAMMER env vars");

    // SWISSARMYHAMMER_ environment variables should override config file values
    assert_eq!(context.get("app.name"), Some(&json!("SwissArmyApp")));
    assert_eq!(context.get("database.port"), Some(&json!(8888)));
    assert_eq!(
        context.get("shared.value"),
        Some(&json!("from_swissarmyhammer_env"))
    );

    // SWISSARMYHAMMER-only values should be present
    assert_eq!(
        context.get("long.prefix.only"),
        Some(&json!("long_prefix_value"))
    );
}

#[test]
#[serial]
fn test_both_env_prefixes_simultaneously() {
    let mut test = IsolatedPrecedenceTest::new();
    let project_config_dir = test.project_config_dir();

    // Create project config
    let project_config = r#"
app_name = "ProjectApp"
database_port = 3306
sah_specific = "from_project"
swissarmyhammer_specific = "from_project"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set both SAH_ and SWISSARMYHAMMER_ environment variables
    test.set_env_var("SAH_APP_NAME", "SahApp");
    test.set_env_var("SAH_SAH_SPECIFIC", "from_sah_env");
    test.set_env_var("SWISSARMYHAMMER_DATABASE_PORT", "7777");
    test.set_env_var(
        "SWISSARMYHAMMER_SWISSARMYHAMMER_SPECIFIC",
        "from_swissarmyhammer_env",
    );

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with both env prefixes");

    // Both prefixes should work
    assert_eq!(context.get("app.name"), Some(&json!("SahApp")));
    assert_eq!(context.get("database.port"), Some(&json!(7777)));
    assert_eq!(context.get("sah.specific"), Some(&json!("from_sah_env")));
    assert_eq!(
        context.get("swissarmyhammer.specific"),
        Some(&json!("from_swissarmyhammer_env"))
    );
}

#[test]
#[serial]
fn test_cli_args_highest_precedence() {
    let mut test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();
    let project_config_dir = test.project_config_dir();

    // Create global config
    let global_config = r#"
app_name = "GlobalApp"
database_port = 5432
shared_value = "from_global"
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    // Create project config
    let project_config = r#"
app_name = "ProjectApp"
database_port = 3306
shared_value = "from_project"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set environment variables
    test.set_env_var("SAH_APP_NAME", "EnvApp");
    test.set_env_var("SAH_DATABASE_PORT", "9999");
    test.set_env_var("SAH_SHARED_VALUE", "from_env");

    // Create CLI arguments (highest precedence)
    let cli_args = json!({
        "app_name": "CliApp",
        "database_port": 1234,
        "shared_value": "from_cli",
        "cli_only": "cli_value"
    });

    let context =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load config with CLI args");

    // CLI arguments should override everything
    assert_eq!(context.get("app_name"), Some(&json!("CliApp")));
    assert_eq!(context.get("database_port"), Some(&json!(1234)));
    assert_eq!(context.get("shared_value"), Some(&json!("from_cli")));

    // CLI-only values should be present
    assert_eq!(context.get("cli_only"), Some(&json!("cli_value")));
}

#[test]
#[serial]
fn test_complete_precedence_chain() {
    let mut test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();
    let project_config_dir = test.project_config_dir();

    // Create global config
    let global_config = r#"
# Global config values (precedence 2)
source = "global"
global_only = "global_value"
overridden_by_project = "global_version"
overridden_by_env = "global_version"
overridden_by_cli = "global_version"
shared_across_all = "global_version"
database_timeout = 30
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    // Create project config
    let project_config = r#"
# Project config values (precedence 3)
source = "project"
project_only = "project_value"
overridden_by_project = "project_version"
overridden_by_env = "project_version"
overridden_by_cli = "project_version"
shared_across_all = "project_version"
database_timeout = 60
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set environment variables (precedence 4)
    test.set_env_var("SAH_OVERRIDDEN_BY_ENV", "env_version");
    test.set_env_var("SAH_OVERRIDDEN_BY_CLI", "env_version");
    test.set_env_var("SAH_SHARED_ACROSS_ALL", "env_version");
    test.set_env_var("SAH_ENV_ONLY", "env_value");
    test.set_env_var("SAH_DATABASE_TIMEOUT", "90");

    // Create CLI arguments (precedence 5 - highest)
    let cli_args = json!({
        "overridden_by_cli": "cli_version",
        "shared_across_all": "cli_version",
        "cli_only": "cli_value",
        "database_timeout": 120
    });

    let context = TemplateContext::load_with_cli_args(cli_args)
        .expect("Failed to load complete precedence chain");

    // Test precedence order:

    // 1. Values only from global config should have global values
    assert_eq!(context.get("global_only"), Some(&json!("global_value")));

    // 2. Values overridden by project should have project values
    assert_eq!(
        context.get("overridden_by_project"),
        Some(&json!("project_version"))
    );
    assert_eq!(context.get("source"), Some(&json!("project")));
    assert_eq!(context.get("project_only"), Some(&json!("project_value")));

    // 3. Values overridden by env should have env values
    assert_eq!(
        context.get("overridden.by.env"),
        Some(&json!("env_version"))
    );
    assert_eq!(context.get("env.only"), Some(&json!("env_value")));

    // 4. Values overridden by CLI should have CLI values (highest precedence)
    assert_eq!(
        context.get("overridden_by_cli"),
        Some(&json!("cli_version"))
    );
    assert_eq!(
        context.get("shared_across_all"),
        Some(&json!("cli_version"))
    );
    assert_eq!(context.get("cli_only"), Some(&json!("cli_value")));
    assert_eq!(context.get("database_timeout"), Some(&json!(120)));
}

#[test]
#[serial]
fn test_nested_value_precedence() {
    let mut test = IsolatedPrecedenceTest::new();
    let home_config_dir = test.home_config_dir();
    let project_config_dir = test.project_config_dir();

    // Create global config with nested values
    let global_config = r#"
[database]
host = "global-host"
port = 5432
timeout = 30

[database.pool]
min_connections = 5
max_connections = 100

[logging]
level = "info"
file = "global.log"
"#;
    let global_config_file = home_config_dir.join("sah.toml");
    fs::write(&global_config_file, global_config).expect("Failed to write global config");

    // Create project config with some nested overrides
    let project_config = r#"
[database]
host = "project-host"
port = 3306

[database.pool]
max_connections = 50

[logging]
level = "debug"
"#;
    let project_config_file = project_config_dir.join("sah.toml");
    fs::write(&project_config_file, project_config).expect("Failed to write project config");

    // Set environment variables for nested values
    test.set_env_var("SAH_DATABASE_HOST", "env-host");
    test.set_env_var("SAH_LOGGING_LEVEL", "warn");

    // CLI args for nested values
    let cli_args = json!({
        "database": {
            "host": "cli-host",
            "timeout": 45
        },
        "logging": {
            "level": "error",
            "format": "json"
        }
    });

    let context =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load nested precedence");

    // Test nested value precedence
    assert_eq!(context.get("database.host"), Some(&json!("cli-host"))); // CLI wins
    assert_eq!(context.get("database.port"), Some(&json!(3306))); // Project wins (no env/CLI override)
    assert_eq!(context.get("database.timeout"), Some(&json!(45))); // CLI wins
    assert_eq!(context.get("logging.level"), Some(&json!("error"))); // CLI wins
    assert_eq!(context.get("logging.format"), Some(&json!("json"))); // CLI only

    // Values not overridden should come from appropriate level
    if let Some(database) = context.get("database") {
        if let Some(db_obj) = database.as_object() {
            if let Some(pool) = db_obj.get("pool") {
                if let Some(pool_obj) = pool.as_object() {
                    // min_connections only in global, max_connections overridden by project
                    assert_eq!(pool_obj.get("min_connections"), Some(&json!(5))); // From global
                    assert_eq!(pool_obj.get("max_connections"), Some(&json!(50)));
                    // From project
                }
            }
        }
    }

    // Global-only nested values should remain
    assert_eq!(context.get("logging.file"), Some(&json!("global.log")));
}

#[test]
#[serial]
fn test_precedence_with_missing_layers() {
    let mut test = IsolatedPrecedenceTest::new();

    // Test with only environment variables and CLI args (no config files)
    test.set_env_var("SAH_FROM_ENV", "env_value");
    test.set_env_var("SAH_SHARED", "env_version");

    let cli_args = json!({
        "from_cli": "cli_value",
        "shared": "cli_version"
    });

    let context =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load with missing layers");

    // Environment and CLI values should both be present
    assert_eq!(context.get("from.env"), Some(&json!("env_value")));
    assert_eq!(context.get("from_cli"), Some(&json!("cli_value")));

    // CLI should win over env for shared values
    assert_eq!(context.get("shared"), Some(&json!("cli_version")));
}

#[test]
#[serial]
fn test_precedence_with_empty_sources() {
    let mut test = IsolatedPrecedenceTest::new();
    let project_config_dir = test.project_config_dir();

    // Create empty config files
    let empty_config = "";
    let empty_config_file = project_config_dir.join("empty.toml");
    fs::write(&empty_config_file, empty_config).expect("Failed to write empty config");

    // Set environment variables
    test.set_env_var("SAH_ENV_VALUE", "from_env");

    let cli_args = json!({
        "cli_value": "from_cli"
    });

    let context =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load with empty sources");

    // Should still get env and CLI values
    assert_eq!(context.get("env.value"), Some(&json!("from_env")));
    assert_eq!(context.get("cli_value"), Some(&json!("from_cli")));
}
