//! Integration tests for environment variable substitution across the entire system

use crate::{ConfigProvider, TemplateContext};
use serial_test::serial;
use std::collections::HashMap;
use std::env;

#[test]
#[serial]
fn test_config_provider_env_substitution_legacy_mode() {
    // Set up test environment variables
    env::set_var("INTEGRATION_HOST", "test-server");
    env::set_var("INTEGRATION_PORT", "8080");
    env::set_var("INTEGRATION_KEY", "secret123");

    let provider = ConfigProvider::new();

    // Create template context with variables that reference env vars
    let mut workflow_vars = HashMap::new();
    workflow_vars.insert(
        "database_url".to_string(),
        serde_json::json!("postgresql://${INTEGRATION_HOST}:${INTEGRATION_PORT}/mydb"),
    );
    workflow_vars.insert(
        "api_key".to_string(),
        serde_json::json!("${INTEGRATION_KEY}"),
    );
    workflow_vars.insert(
        "timeout".to_string(),
        serde_json::json!("${INTEGRATION_TIMEOUT:-30}"), // default value
    );
    workflow_vars.insert(
        "missing_var".to_string(),
        serde_json::json!("${MISSING_INTEGRATION_VAR}"), // should be empty string
    );

    let context = provider.create_context_with_vars(workflow_vars).unwrap();

    // Verify substitution worked correctly
    assert_eq!(
        context.get_string("database_url"),
        Some("postgresql://test-server:8080/mydb".to_string())
    );
    assert_eq!(context.get_string("api_key"), Some("secret123".to_string()));
    assert_eq!(
        context.get_string("timeout"),
        Some("30".to_string()) // default value used
    );
    assert_eq!(
        context.get_string("missing_var"),
        Some("".to_string()) // empty string for missing var in legacy mode
    );

    // Clean up
    env::remove_var("INTEGRATION_HOST");
    env::remove_var("INTEGRATION_PORT");
    env::remove_var("INTEGRATION_KEY");
}

#[test]
#[serial]
fn test_config_provider_env_substitution_strict_mode() {
    // Set up test environment variables
    env::set_var("STRICT_HOST", "strict-server");
    env::set_var("STRICT_PORT", "9090");

    let provider = ConfigProvider::new();

    // Test with valid environment variables
    let mut valid_workflow_vars = HashMap::new();
    valid_workflow_vars.insert(
        "server_url".to_string(),
        serde_json::json!("https://${STRICT_HOST}:${STRICT_PORT}"),
    );
    valid_workflow_vars.insert(
        "timeout".to_string(),
        serde_json::json!("${STRICT_TIMEOUT:-45}"), // with default
    );

    let context = provider
        .create_context_with_vars_strict(valid_workflow_vars)
        .unwrap();

    assert_eq!(
        context.get_string("server_url"),
        Some("https://strict-server:9090".to_string())
    );
    assert_eq!(
        context.get_string("timeout"),
        Some("45".to_string()) // default value used
    );

    // Test with missing environment variable (should fail in strict mode)
    let mut invalid_workflow_vars = HashMap::new();
    invalid_workflow_vars.insert(
        "missing_config".to_string(),
        serde_json::json!("${MISSING_STRICT_VAR}"), // no default, should cause error
    );

    let result = provider.create_context_with_vars_strict(invalid_workflow_vars);
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("MISSING_STRICT_VAR"));

    // Clean up
    env::remove_var("STRICT_HOST");
    env::remove_var("STRICT_PORT");
}

#[test]
#[serial]
fn test_config_provider_raw_context_no_substitution() {
    // Set up test environment variables
    env::set_var("RAW_TEST_VAR", "should_not_substitute");

    let provider = ConfigProvider::new();

    let mut workflow_vars = HashMap::new();
    workflow_vars.insert(
        "raw_value".to_string(),
        serde_json::json!("${RAW_TEST_VAR}"),
    );

    let context = provider
        .create_raw_context_with_vars(workflow_vars)
        .unwrap();

    // Should NOT be substituted in raw mode
    assert_eq!(
        context.get_string("raw_value"),
        Some("${RAW_TEST_VAR}".to_string())
    );

    // Now test selective substitution
    let mut context_copy = context.clone();
    context_copy.substitute_var("raw_value", false).unwrap();

    // Now should be substituted
    assert_eq!(
        context_copy.get_string("raw_value"),
        Some("should_not_substitute".to_string())
    );

    // Clean up
    env::remove_var("RAW_TEST_VAR");
}

#[test]
#[serial]
fn test_template_context_compatibility_with_legacy_system() {
    // Test that the new system produces identical results to the legacy system
    env::set_var("COMPAT_VAR", "compat_value");

    let test_cases = vec![
        ("${COMPAT_VAR}", "compat_value"),
        ("${COMPAT_VAR:-default}", "compat_value"),
        ("${MISSING_COMPAT_VAR:-default}", "default"),
        ("${MISSING_COMPAT_VAR}", ""), // Empty string in legacy mode
        ("prefix_${COMPAT_VAR}_suffix", "prefix_compat_value_suffix"),
        ("${COMPAT_VAR}${COMPAT_VAR}", "compat_valuecompat_value"),
        ("no vars", "no vars"),
        ("${EMPTY_VAR:-}", ""), // Empty default
        ("${SPACE_VAR:-default with spaces}", "default with spaces"),
        ("${COLON_VAR:-default:with:colons}", "default:with:colons"),
    ];

    // Test using legacy function directly
    for (input, expected) in &test_cases {
        let result = crate::env_substitution::substitute_env_vars_legacy(input);
        assert_eq!(
            result, *expected,
            "Legacy function failed for input '{}': expected '{}', got '{}'",
            input, expected, result
        );
    }

    // Test using TemplateContext
    for (input, expected) in &test_cases {
        let mut ctx = TemplateContext::new();
        ctx.set("test_var", serde_json::Value::String(input.to_string()));

        ctx.substitute_env_vars().unwrap();

        let result = ctx.get_string("test_var").unwrap();
        assert_eq!(
            result, *expected,
            "TemplateContext failed for input '{}': expected '{}', got '{}'",
            input, expected, result
        );
    }

    env::remove_var("COMPAT_VAR");
}

#[test]
#[serial]
fn test_nested_structure_compatibility() {
    env::set_var("NESTED_HOST", "nested-server");
    env::set_var("NESTED_PORT", "5432");

    let mut ctx = TemplateContext::new();
    ctx.set(
        "config".to_string(),
        serde_json::json!({
            "database": {
                "url": "postgresql://${NESTED_HOST}:${NESTED_PORT}/app",
                "pool_size": "${NESTED_POOL_SIZE:-10}",
                "timeout": "${NESTED_TIMEOUT}"  // missing, should be empty
            },
            "servers": [
                "${NESTED_HOST}:${NESTED_PORT}",
                "${NESTED_HOST}:8080",
                "static-server:3000"
            ],
            "metadata": {
                "version": "1.0.0",  // no substitution needed
                "build": "${BUILD_ID:-dev}"
            }
        }),
    );

    ctx.substitute_env_vars().unwrap();

    let config = ctx.get("config").unwrap();

    // Check database configuration
    assert_eq!(
        config["database"]["url"],
        serde_json::Value::String("postgresql://nested-server:5432/app".to_string())
    );
    assert_eq!(
        config["database"]["pool_size"],
        serde_json::Value::String("10".to_string())
    );
    assert_eq!(
        config["database"]["timeout"],
        serde_json::Value::String("".to_string()) // empty for missing var
    );

    // Check servers array
    let servers = config["servers"].as_array().unwrap();
    assert_eq!(
        servers[0],
        serde_json::Value::String("nested-server:5432".to_string())
    );
    assert_eq!(
        servers[1],
        serde_json::Value::String("nested-server:8080".to_string())
    );
    assert_eq!(
        servers[2],
        serde_json::Value::String("static-server:3000".to_string())
    );

    // Check metadata
    assert_eq!(
        config["metadata"]["version"],
        serde_json::Value::String("1.0.0".to_string()) // unchanged
    );
    assert_eq!(
        config["metadata"]["build"],
        serde_json::Value::String("dev".to_string()) // default used
    );

    env::remove_var("NESTED_HOST");
    env::remove_var("NESTED_PORT");
}

#[test]
#[serial]
fn test_performance_with_thread_local_caching() {
    env::set_var("PERF_TEST", "performance");

    let start = std::time::Instant::now();

    // Perform many substitutions to test thread-local caching
    for i in 0..1000 {
        let input = format!("iteration_{}_${{{}}}", i, "PERF_TEST");
        let result = crate::env_substitution::substitute_env_vars_legacy(&input);
        assert_eq!(result, format!("iteration_{}_performance", i));
    }

    let duration = start.elapsed();
    println!("1000 substitutions took: {:?}", duration);

    // Should be quite fast with thread-local caching
    assert!(
        duration < std::time::Duration::from_millis(100),
        "Performance test took too long: {:?}",
        duration
    );

    env::remove_var("PERF_TEST");
}

#[test]
#[serial]
fn test_complex_real_world_scenario() {
    // Simulate a complex real-world configuration scenario
    env::set_var("APP_ENV", "production");
    env::set_var("DATABASE_HOST", "prod-db.example.com");
    env::set_var("DATABASE_PORT", "5432");
    env::set_var("DATABASE_NAME", "myapp_prod");
    env::set_var("API_KEY", "prod-api-key-123");
    env::set_var("LOG_LEVEL", "info");

    let provider = ConfigProvider::new();

    let mut workflow_vars = HashMap::new();
    workflow_vars.insert(
        "application".to_string(),
        serde_json::json!({
            "environment": "${APP_ENV}",
            "database": {
                "url": "postgresql://${DATABASE_HOST}:${DATABASE_PORT}/${DATABASE_NAME}",
                "ssl_mode": "${DATABASE_SSL_MODE:-require}",
                "pool_size": "${DATABASE_POOL_SIZE:-20}"
            },
            "api": {
                "key": "${API_KEY}",
                "rate_limit": "${API_RATE_LIMIT:-1000}",
                "timeout": "${API_TIMEOUT:-30}"
            },
            "logging": {
                "level": "${LOG_LEVEL}",
                "format": "${LOG_FORMAT:-json}",
                "output": "${LOG_OUTPUT:-stdout}"
            },
            "features": [
                "${FEATURE_A:-enabled}",
                "${FEATURE_B:-disabled}",
                "always_on"
            ]
        }),
    );

    let context = provider.create_context_with_vars(workflow_vars).unwrap();

    let app_config = context.get("application").unwrap();

    // Verify all substitutions
    assert_eq!(app_config["environment"], "production");
    assert_eq!(
        app_config["database"]["url"],
        "postgresql://prod-db.example.com:5432/myapp_prod"
    );
    assert_eq!(app_config["database"]["ssl_mode"], "require"); // default
    assert_eq!(app_config["database"]["pool_size"], "20"); // default

    assert_eq!(app_config["api"]["key"], "prod-api-key-123");
    assert_eq!(app_config["api"]["rate_limit"], "1000"); // default
    assert_eq!(app_config["api"]["timeout"], "30"); // default

    assert_eq!(app_config["logging"]["level"], "info");
    assert_eq!(app_config["logging"]["format"], "json"); // default
    assert_eq!(app_config["logging"]["output"], "stdout"); // default

    let features = app_config["features"].as_array().unwrap();
    assert_eq!(features[0], "enabled"); // default
    assert_eq!(features[1], "disabled"); // default
    assert_eq!(features[2], "always_on"); // no substitution

    // Clean up
    env::remove_var("APP_ENV");
    env::remove_var("DATABASE_HOST");
    env::remove_var("DATABASE_PORT");
    env::remove_var("DATABASE_NAME");
    env::remove_var("API_KEY");
    env::remove_var("LOG_LEVEL");
}

#[test]
fn test_contains_env_patterns() {
    let test_cases = vec![
        ("${VAR}", true),
        ("${VAR:-default}", true),
        ("prefix ${VAR} suffix", true),
        ("${VAR1} and ${VAR2}", true),
        ("no variables here", false),
        ("$VAR", false),  // Missing braces
        ("{VAR}", false), // Missing dollar sign
        ("${}", false),   // Empty variable name
        ("${ }", false),  // Space in variable name
        ("${VAR", false), // Missing closing brace
        ("VAR}", false),  // Missing opening part
    ];

    for (input, expected) in test_cases {
        let result = crate::env_substitution::contains_env_patterns(input);
        assert_eq!(
            result, expected,
            "contains_env_patterns failed for input '{}': expected {}, got {}",
            input, expected, result
        );
    }
}

#[test]
#[serial]
fn test_error_handling_in_strict_mode() {
    let mut ctx = TemplateContext::new();

    // Test various error conditions in strict mode
    ctx.set(
        "missing_simple".to_string(),
        serde_json::json!("${DEFINITELY_MISSING}"),
    );
    ctx.set(
        "missing_with_text".to_string(),
        serde_json::json!("prefix ${ALSO_MISSING} suffix"),
    );
    ctx.set(
        "nested_missing".to_string(),
        serde_json::json!({
            "config": "${NESTED_MISSING}",
            "other": "static"
        }),
    );

    // All of these should fail in strict mode
    let result1 = ctx.substitute_var("missing_simple", true);
    assert!(result1.is_err());
    assert!(result1
        .unwrap_err()
        .to_string()
        .contains("DEFINITELY_MISSING"));

    let result2 = ctx.substitute_var("missing_with_text", true);
    assert!(result2.is_err());
    assert!(result2.unwrap_err().to_string().contains("ALSO_MISSING"));

    let result3 = ctx.substitute_var("nested_missing", true);
    assert!(result3.is_err());
    assert!(result3.unwrap_err().to_string().contains("NESTED_MISSING"));

    // But should work in legacy mode (empty strings)
    ctx.substitute_var("missing_simple", false).unwrap();
    assert_eq!(ctx.get_string("missing_simple"), Some("".to_string()));

    ctx.substitute_var("missing_with_text", false).unwrap();
    assert_eq!(
        ctx.get_string("missing_with_text"),
        Some("prefix  suffix".to_string())
    );

    ctx.substitute_var("nested_missing", false).unwrap();
    let nested = ctx.get("nested_missing").unwrap();
    assert_eq!(nested["config"], "");
    assert_eq!(nested["other"], "static");
}
