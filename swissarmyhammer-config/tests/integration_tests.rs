//! Integration tests for SwissArmyHammer configuration system
//!
//! These tests validate the complete configuration system end-to-end with realistic scenarios

mod common;

use common::TestEnvironment;
use serial_test::serial;
use std::time::Instant;
use swissarmyhammer_config::{ConfigFormat, TemplateContext};

#[test]
#[serial]
fn test_basic_integration() {
    let env = TestEnvironment::new().unwrap();

    let config_content = TestEnvironment::create_sample_toml_config();
    env.write_project_config(&config_content, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Verify basic configuration loading
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Integration Test Project"
    );
    assert_eq!(context.get_string("environment").unwrap(), "test");
    assert!(context.get_bool("debug").unwrap());

    // Verify nested configuration
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("localhost".to_string())
        );
        assert_eq!(database["port"], serde_json::Value::Number(5432.into()));
    }
}

#[test]
#[serial]
fn test_multi_format_integration() {
    let env = TestEnvironment::new().unwrap();

    // Create configurations in all supported formats
    let toml_config = TestEnvironment::create_sample_toml_config();
    let yaml_config = TestEnvironment::create_sample_yaml_config();
    let json_config = TestEnvironment::create_sample_json_config();

    env.write_project_config(&toml_config, ConfigFormat::Toml)
        .unwrap();
    env.write_project_config(&yaml_config, ConfigFormat::Yaml)
        .unwrap();
    env.write_project_config(&json_config, ConfigFormat::Json)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Should have values from all formats merged together
    // Exact precedence depends on figment's behavior
    assert!(context.get("project_name").is_some());
    assert!(context.get("environment").is_some());
    assert!(context.get("database").is_some());

    // Format-specific values should be present
    let has_toml_specific = context.get("logging").is_some();
    let has_yaml_specific = context.get("api").is_some();
    let has_json_specific = context.get("cache").is_some();

    assert!(has_toml_specific || has_yaml_specific || has_json_specific);
}

#[test]
#[serial]
fn test_global_and_project_precedence_integration() {
    let env = TestEnvironment::new().unwrap();

    // Create global configuration
    env.write_global_config(
        r#"
project_name = "Global Project"
environment = "global"
global_only = "global_value"
shared_setting = "from_global"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Create project configuration that should override global
    env.write_project_config(
        r#"
project_name = "Project Override"
project_only = "project_value"
shared_setting = "from_project"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Project should override global
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Project Override"
    );
    assert_eq!(
        context.get_string("shared_setting").unwrap(),
        "from_project"
    );

    // Global-only value should still be present
    assert_eq!(context.get_string("global_only").unwrap(), "global_value");

    // Project-only value should be present
    assert_eq!(context.get_string("project_only").unwrap(), "project_value");

    // Global environment should still be present since not overridden
    assert_eq!(context.get_string("environment").unwrap(), "global");
}

#[test]
#[serial]
fn test_environment_override_integration() {
    let mut env = TestEnvironment::new().unwrap();

    // Create base configuration
    env.write_project_config(
        r#"
app_name = "Config App"
debug = false
database_host = "config-db"
nested_value = "from_config"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Set environment variables that should override config
    env.set_env_vars([
        ("SAH_APP_NAME", "Env Override App"),
        ("SAH_DEBUG", "true"),
        ("SAH_DATABASE_HOST", "env-db"),
        ("SAH_ENV_ONLY", "env_only_value"),
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Environment variables should override file values
    assert_eq!(context.get_string("app_name").unwrap(), "Env Override App");
    assert!(context.get_bool("debug").unwrap());
    assert_eq!(context.get_string("database_host").unwrap(), "env-db");

    // Environment-only value should be present
    assert_eq!(context.get_string("env_only").unwrap(), "env_only_value");

    // Unoverridden config value should remain
    assert_eq!(context.get_string("nested_value").unwrap(), "from_config");
}

#[test]
#[serial]
fn test_nested_environment_variables_integration() {
    let mut env = TestEnvironment::new().unwrap();

    // Create base configuration with nested structure
    let config = TestEnvironment::create_complex_nested_config();
    env.write_project_config(&config, ConfigFormat::Toml)
        .unwrap();

    // Set nested environment variables
    env.set_env_vars([
        ("SAH_SERVER__PORT", "9090"),
        ("SAH_SERVER__SSL__ENABLED", "false"),
        ("SAH_DATABASE__POOL__MAX_CONNECTIONS", "50"),
        ("SAH_FEATURES__EXPERIMENTAL__NEW_UI", "true"),
        ("SAH_MONITORING__TRACING__SAMPLE_RATE", "0.5"),
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Check that nested overrides work
    if let Some(server) = context.get("server") {
        // Port should be overridden
        let port_value = &server["port"];
        let port_str = match port_value {
            serde_json::Value::String(s) => s.clone(),
            serde_json::Value::Number(n) => n.to_string(),
            _ => panic!("Unexpected port type"),
        };
        assert_eq!(port_str, "9090");

        // Nested SSL setting should be overridden
        if let Some(ssl) = server.get("ssl") {
            let enabled_value = &ssl["enabled"];
            match enabled_value {
                serde_json::Value::String(s) => assert_eq!(s, "false"),
                serde_json::Value::Bool(b) => assert!(!*b),
                _ => panic!("Unexpected enabled type"),
            }
        }
    }

    // Deep nesting should work
    if let Some(features) = context.get("features") {
        if let Some(experimental) = features.get("experimental") {
            let new_ui_value = &experimental["new_ui"];
            match new_ui_value {
                serde_json::Value::String(s) => assert_eq!(s, "true"),
                serde_json::Value::Bool(b) => assert!(*b),
                _ => panic!("Unexpected new_ui type"),
            }
        }
    }
}

#[test]
#[serial]
fn test_environment_variable_substitution_integration() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up environment variables for substitution
    env.set_env_vars([
        ("PROJECT_NAME", "Substituted Project"),
        ("APP_ENVIRONMENT", "production"),
        ("API_KEY", "api_key_abc123"),
    ])
    .unwrap();

    // Use a simpler configuration for substitution testing
    let config = r#"
project_name = "${PROJECT_NAME:-Default Project}"
environment = "${APP_ENVIRONMENT}"
api_key = "${API_KEY}"
fallback_config = "${MISSING_VAR:-default_fallback}"
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Environment variable substitution test - verify the key values are properly substituted

    // Verify environment variable substitution worked correctly
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Substituted Project"
    );
    assert_eq!(context.get_string("environment").unwrap(), "production");
    assert_eq!(context.get_string("api_key").unwrap(), "api_key_abc123");
    assert_eq!(
        context.get_string("fallback_config").unwrap(),
        "default_fallback"
    );

    // At minimum, the context should not be empty
    assert!(!context.is_empty(), "Context should not be empty");

    // This demonstrates the integration test is working, even if env substitution details differ
    println!("Environment variable substitution integration test completed successfully");
}

#[test]
#[serial]
fn test_complete_precedence_integration() {
    let mut env = TestEnvironment::new().unwrap();

    // 1. Create global config (lowest precedence)
    env.write_global_config(
        r#"
app_name = "Global App"
environment = "global"
global_only = "global_value"
shared_key = "from_global"
will_be_overridden = "global_default"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // 2. Create project config (higher precedence)
    env.write_project_config(
        r#"
app_name = "Project App"
environment = "project" 
project_only = "project_value"
shared_key = "from_project"
will_be_overridden = "project_override"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // 3. Set environment variables (highest precedence)
    env.set_env_vars([
        ("SAH_APP_NAME", "Env App"),
        ("SAH_WILL_BE_OVERRIDDEN", "env_final"),
        ("SAH_ENV_ONLY", "env_exclusive"),
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Environment should have highest precedence
    assert_eq!(context.get_string("app_name").unwrap(), "Env App");
    assert_eq!(
        context.get_string("will_be_overridden").unwrap(),
        "env_final"
    );
    assert_eq!(context.get_string("env_only").unwrap(), "env_exclusive");

    // Project should override global
    assert_eq!(context.get_string("environment").unwrap(), "project");
    assert_eq!(context.get_string("shared_key").unwrap(), "from_project");

    // Layer-specific values should be preserved
    assert_eq!(context.get_string("global_only").unwrap(), "global_value");
    assert_eq!(context.get_string("project_only").unwrap(), "project_value");
}

#[test]
#[serial]
fn test_template_context_operations_integration() {
    let env = TestEnvironment::new().unwrap();

    let config = TestEnvironment::create_sample_toml_config();
    env.write_project_config(&config, ConfigFormat::Toml)
        .unwrap();

    let mut context = env.load_template_context().unwrap();

    // Test TemplateContext operations
    let original_project_name = context.get_string("project_name").unwrap();
    assert_eq!(original_project_name, "Integration Test Project");

    // Test adding new values
    context.set(
        "new_runtime_value".to_string(),
        serde_json::Value::String("runtime_added".to_string()),
    );
    assert_eq!(
        context.get_string("new_runtime_value").unwrap(),
        "runtime_added"
    );

    // Test merging with another context
    let mut other_context = TemplateContext::new();
    other_context.set(
        "merged_value".to_string(),
        serde_json::Value::String("merged_content".to_string()),
    );

    context.merge(&other_context);
    assert_eq!(
        context.get_string("merged_value").unwrap(),
        "merged_content"
    );

    // Original values should still be present
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Integration Test Project"
    );
}

#[test]
#[serial]
fn test_error_handling_integration() {
    let env = TestEnvironment::new().unwrap();

    // Test 1: Config with missing environment variables (no fallback)
    env.write_project_config(
        r#"
app_name = "Test App"
database_url = "${MISSING_DATABASE_URL}"
api_key = "${MISSING_API_KEY}"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Legacy mode should succeed with empty strings
    let context = env.load_template_context().unwrap();
    assert_eq!(context.get_string("app_name").unwrap(), "Test App");
    assert_eq!(context.get_string("database_url").unwrap(), "");

    // Strict mode should fail
    let strict_result = env.load_template_context_strict();
    assert!(strict_result.is_err());
}

#[test]
#[serial]
fn test_real_world_workflow_integration() {
    let mut env = TestEnvironment::new().unwrap();

    // Simulate a real-world configuration scenario
    env.write_global_config(
        r#"
# Global SwissArmyHammer config
default_timeout = 30
global_api_endpoint = "https://api.example.com"
debug = false
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    env.write_project_config(
        r#"
# Project-specific config
project_name = "My SwissArmyHammer Project"
debug = true
workflow_timeout = 60

[database]
host = "localhost"
port = 5432
name = "project_db"

[features]
enable_workflows = true
enable_prompts = true
enable_mcp = false

[custom_actions]
deploy_command = "kubectl apply -f deployment.yaml"
test_command = "cargo nextest run"
build_command = "cargo build --release"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Set environment variables as a user might
    env.set_env_vars([
        ("SAH_DATABASE__HOST", "prod-db.company.com"),
        ("SAH_DATABASE__PORT", "5432"),
        ("SAH_FEATURES__ENABLE_MCP", "true"), // Override project config
        (
            "SAH_CUSTOM_ACTIONS__DEPLOY_COMMAND",
            "helm upgrade myapp ./chart",
        ),
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Verify project-specific settings
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "My SwissArmyHammer Project"
    );
    assert!(context.get_bool("debug").unwrap()); // Project overrides global

    // Verify global settings are inherited
    assert_eq!(context.get_number("default_timeout").unwrap(), 30.0);
    assert_eq!(
        context.get_string("global_api_endpoint").unwrap(),
        "https://api.example.com"
    );

    // Verify environment overrides
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("prod-db.company.com".to_string())
        );
    }

    if let Some(features) = context.get("features") {
        // Should be overridden by environment variable
        let enable_mcp = &features["enable_mcp"];
        match enable_mcp {
            serde_json::Value::String(s) => assert_eq!(s, "true"),
            serde_json::Value::Bool(b) => assert!(*b),
            _ => panic!("Unexpected enable_mcp type"),
        }
    }

    // Verify all expected sections are present
    assert!(context.get("database").is_some());
    assert!(context.get("features").is_some());
    assert!(context.get("custom_actions").is_some());
}

#[test]
#[serial]
fn test_configuration_loading_performance() {
    let env = TestEnvironment::new().unwrap();

    // Create a reasonably large configuration
    let mut large_config = TestEnvironment::create_complex_nested_config();

    // Add more sections to make it larger
    for i in 0..50 {
        large_config.push_str(&format!(
            r#"
[section_{}]
key_a = "value_a_{}"
key_b = "value_b_{}"
key_c = {}
"#,
            i, i, i, i
        ));
    }

    env.write_project_config(&large_config, ConfigFormat::Toml)
        .unwrap();

    // Measure performance of configuration loading
    let start = Instant::now();
    let iterations = 100;

    for _ in 0..iterations {
        let _context = env.load_template_context().unwrap();
    }

    let duration = start.elapsed();
    let avg_duration = duration / iterations;

    // Configuration loading should be reasonably fast
    // Adjust threshold as needed based on system performance
    assert!(
        avg_duration.as_millis() < 50,
        "Configuration loading took {}ms on average, expected < 50ms",
        avg_duration.as_millis()
    );

    println!(
        "Configuration loading performance: {}ms average over {} iterations",
        avg_duration.as_millis(),
        iterations
    );
}

#[test]
#[serial]
fn test_cross_platform_paths_integration() {
    let env = TestEnvironment::new().unwrap();

    // Test configuration with various path formats
    let config = r#"
[paths]
unix_style = "/var/log/app.log"
relative_path = "./config/settings.conf"
windows_style = "C:\\Program Files\\MyApp\\config.ini"
mixed_separators = "./data\\files/output.txt"

[build]
output_dir = "./target/release"
source_dir = "./src"
test_dir = "./tests"
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();
    let context = env.load_template_context().unwrap();

    // All path configurations should be loaded successfully
    if let Some(paths) = context.get("paths") {
        assert!(paths.get("unix_style").is_some());
        assert!(paths.get("relative_path").is_some());
        assert!(paths.get("windows_style").is_some());
        assert!(paths.get("mixed_separators").is_some());
    }

    if let Some(build) = context.get("build") {
        assert!(build.get("output_dir").is_some());
        assert!(build.get("source_dir").is_some());
        assert!(build.get("test_dir").is_some());
    }
}
