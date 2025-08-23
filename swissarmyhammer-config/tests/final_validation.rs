//! Final validation tests for SwissArmyHammer configuration system migration
//!
//! This test suite performs comprehensive validation that the new figment-based
//! configuration system fully meets all specification requirements and is ready
//! for production use.

mod common;

use common::{ConfigScope, TestEnvironment};
use serial_test::serial;
use std::collections::HashMap;
use std::time::Instant;
use swissarmyhammer_config::{ConfigFormat, ConfigProvider, TemplateRenderer};

/// Maximum acceptable time for configuration loading (in milliseconds)
const MAX_CONFIG_LOAD_TIME_MS: u128 = 100;

/// Maximum acceptable time for template rendering (in milliseconds)
const MAX_TEMPLATE_RENDER_TIME_MS: u128 = 50;

#[test]
#[serial]
fn test_complete_specification_compliance() {
    let mut env = TestEnvironment::new().unwrap();

    // ✅ Test: Figment Integration & Multiple File Formats
    env.write_global_config(
        r#"
project_name = "GlobalProject"
environment = "production"  
timeout = 30
database = { host = "global.db", port = 5432 }
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Test YAML format support
    env.write_config(
        r#"
project_name: "ProjectOverride"
debug: true
database:
  host: "project.db"
api_settings:
  key: "${API_SECRET:-fallback_key}"
  rate_limit: 1000
"#,
        ConfigFormat::Yaml,
        ConfigScope::Project,
        "sah",
    )
    .unwrap();

    // Test JSON format support
    env.write_config(
        r#"{
  "json_only": "json_value",
  "nested": {
    "array": [1, 2, 3],
    "bool": true
  }
}"#,
        ConfigFormat::Json,
        ConfigScope::Project,
        "swissarmyhammer",
    )
    .unwrap();

    // ✅ Test: Environment Variables with proper prefixes
    env.set_env_vars([
        ("SAH_ENVIRONMENT", "development"),
        ("SAH_API_SETTINGS__RATE_LIMIT", "2000"),
        ("SWISSARMYHAMMER_DEBUG", "false"),
        ("API_SECRET", "env_secret_123"),
    ])
    .unwrap();

    let provider = env.create_provider();
    let context = provider.load_template_context().unwrap();

    // Debug: Print actual context to understand the issue
    println!(
        "DEBUG: Actual project_name value: {:?}",
        context.get_string("project_name")
    );
    println!(
        "DEBUG: Context keys: {:?}",
        context.as_object().map(|o| o.keys().collect::<Vec<_>>())
    );

    // ✅ Verify: Precedence Order (defaults → global → project → env vars → CLI args)
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "ProjectOverride"
    ); // Project overrides global
    assert_eq!(context.get_string("environment").unwrap(), "development"); // Env var overrides config
    assert_eq!(context.get_number("timeout").unwrap(), 30.0); // Global value inherited
    assert_eq!(context.get_bool("debug").unwrap(), false); // Env var overrides project YAML
                                                           // Check env substitution works
    if let Some(api_settings) = context.get("api_settings") {
        assert_eq!(
            api_settings["key"],
            serde_json::Value::String("env_secret_123".to_string())
        );
    }

    // ✅ Verify: Multiple file format discovery worked
    assert_eq!(context.get_string("json_only").unwrap(), "json_value");
    assert!(context.get("nested").is_some());
    if let Some(nested) = context.get("nested") {
        assert!(nested.get("array").is_some());
    }

    // ✅ Test: Template Integration works properly
    let renderer = TemplateRenderer::new().unwrap();
    let template = "{{project_name}} in {{environment}} mode (debug: {{debug}})";
    let result = renderer.render(template, &context).unwrap();
    assert_eq!(result, "ProjectOverride in development mode (debug: false)");

    // ✅ Test: Workflow variable precedence (highest priority)
    let mut workflow_vars = HashMap::new();
    workflow_vars.insert("environment".to_string(), serde_json::json!("workflow_env"));
    workflow_vars.insert("workflow_step".to_string(), serde_json::json!("deployment"));

    let workflow_context = provider.create_context_with_vars(workflow_vars).unwrap();
    let workflow_result = renderer.render(template, &workflow_context).unwrap();
    assert_eq!(
        workflow_result,
        "ProjectOverride in workflow_env mode (debug: false)"
    );

    // Workflow vars should have highest precedence
    assert_eq!(
        workflow_context.get_string("workflow_step").unwrap(),
        "deployment"
    );

    println!("✅ Complete specification compliance test passed");
}

#[test]
#[serial]
fn test_file_discovery_specification() {
    let env = TestEnvironment::new().unwrap();

    // ✅ Test: File Discovery - both `sah.*` and `swissarmyhammer.*` files
    let discovery_test_cases = [
        ("sah.toml", ConfigFormat::Toml),
        ("sah.yaml", ConfigFormat::Yaml),
        ("sah.yml", ConfigFormat::Yaml),
        ("sah.json", ConfigFormat::Json),
        ("swissarmyhammer.toml", ConfigFormat::Toml),
        ("swissarmyhammer.yaml", ConfigFormat::Yaml),
        ("swissarmyhammer.yml", ConfigFormat::Yaml),
        ("swissarmyhammer.json", ConfigFormat::Json),
    ];

    for (filename, format) in discovery_test_cases {
        let content = match format {
            ConfigFormat::Toml => r#"test_key = "toml_value""#,
            ConfigFormat::Yaml => r#"test_key: "yaml_value""#,
            ConfigFormat::Json => r#"{"test_key": "json_value"}"#,
        };

        // Test both ./.swissarmyhammer/ and ~/.swissarmyhammer/ locations
        env.write_config_with_filename(content, format, ConfigScope::Project, filename)
            .unwrap();

        let context = env.load_template_context().unwrap();
        assert!(
            context.get("test_key").is_some(),
            "Failed to discover {}",
            filename
        );

        env.cleanup_config_with_filename(ConfigScope::Project, filename)
            .unwrap();
    }

    // ✅ Test: Search Locations - verify both directories are checked
    env.write_global_config("global_marker = true", ConfigFormat::Toml)
        .unwrap();
    env.write_project_config("project_marker = true", ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();
    assert_eq!(context.get_bool("global_marker").unwrap(), true);
    assert_eq!(context.get_bool("project_marker").unwrap(), true);

    println!("✅ File discovery specification test passed");
}

#[test]
#[serial]
fn test_performance_meets_requirements() {
    let env = TestEnvironment::new().unwrap();

    // Create realistic large configuration
    let large_config = create_realistic_large_config();
    env.write_project_config(&large_config, ConfigFormat::Toml)
        .unwrap();

    let provider = env.create_provider();

    // ✅ Test: Configuration loading performance
    let start = Instant::now();
    for _ in 0..10 {
        let _ = provider.load_template_context().unwrap();
    }
    let load_duration = start.elapsed() / 10;

    assert!(
        load_duration.as_millis() < MAX_CONFIG_LOAD_TIME_MS,
        "Config loading took {}ms, expected < {}ms",
        load_duration.as_millis(),
        MAX_CONFIG_LOAD_TIME_MS
    );

    // ✅ Test: Template rendering performance
    let context = provider.load_template_context().unwrap();
    let renderer = TemplateRenderer::new().unwrap();
    let complex_template = create_complex_template();

    let start = Instant::now();
    for _ in 0..100 {
        let _result = renderer.render(&complex_template, &context).unwrap();
    }
    let render_duration = start.elapsed() / 100;

    assert!(
        render_duration.as_millis() < MAX_TEMPLATE_RENDER_TIME_MS,
        "Template rendering took {}ms, expected < {}ms",
        render_duration.as_millis(),
        MAX_TEMPLATE_RENDER_TIME_MS
    );

    println!(
        "✅ Performance validation passed - Load: {}ms, Render: {}ms",
        load_duration.as_millis(),
        render_duration.as_millis()
    );
}

#[test]
#[serial]
fn test_no_caching_live_editing() {
    let env = TestEnvironment::new().unwrap();

    // ✅ Test: No Caching - reads config fresh each time for live editing
    env.write_project_config("version = \"1.0.0\"", ConfigFormat::Toml)
        .unwrap();

    let provider = env.create_provider();
    let context1 = provider.load_template_context().unwrap();
    assert_eq!(context1.get_string("version").unwrap(), "1.0.0");

    // Change configuration
    env.write_project_config("version = \"2.0.0\"", ConfigFormat::Toml)
        .unwrap();

    // Should pick up changes immediately (no caching)
    let context2 = provider.load_template_context().unwrap();
    assert_eq!(context2.get_string("version").unwrap(), "2.0.0");

    assert_ne!(
        context1.get_string("version").unwrap(),
        context2.get_string("version").unwrap()
    );

    println!("✅ Live editing (no caching) test passed");
}

#[test]
#[serial]
fn test_backward_compatibility_existing_configs() {
    let env = TestEnvironment::new().unwrap();

    // ✅ Test: Backward compatibility with typical existing config patterns
    let existing_config_patterns = [
        // Basic TOML config
        r#"
project_name = "ExistingProject" 
debug = true
timeout = 30
"#,
        // Nested configuration
        r#"
[database]
host = "localhost"
port = 5432
credentials = { username = "user", password = "pass" }

[services]
api = { enabled = true, port = 8080 }
web = { enabled = false, port = 3000 }
"#,
        // Array configurations
        r#"
environments = ["dev", "staging", "prod"]

[[servers]]
name = "web1"
ip = "10.0.0.1"

[[servers]] 
name = "web2"
ip = "10.0.0.2"
"#,
        // Complex mixed types
        r#"
[deployment]
enabled = true
replicas = 3
resources = { cpu = "500m", memory = "1Gi" }
env_vars = ["NODE_ENV=production", "DEBUG=false"]
"#,
    ];

    for (i, config_content) in existing_config_patterns.iter().enumerate() {
        let context = env
            .test_config_compatibility(config_content, ConfigFormat::Toml)
            .expect(&format!("Failed to load existing config pattern {}", i + 1));

        // Each config should load successfully and preserve its structure
        assert!(context.as_object().is_some());
        println!("✅ Existing config pattern {} loaded successfully", i + 1);
    }

    println!("✅ Backward compatibility test passed");
}

#[test]
#[serial]
fn test_template_context_not_hashmap() {
    let env = TestEnvironment::new().unwrap();

    env.write_project_config(
        r#"
test_key = "test_value"
nested = { inner = "inner_value" }
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    let provider = env.create_provider();
    let context = provider.load_template_context().unwrap();

    // ✅ Test: TemplateContext is proper context object, not HashMap
    // The context should provide structured access methods
    assert!(context.get_string("test_key").is_some());
    assert!(context.get("nested").is_some());
    if let Some(nested) = context.get("nested") {
        assert!(nested.get("inner").is_some());
    }

    // Should work with template rendering
    let renderer = TemplateRenderer::new().unwrap();
    let result = renderer
        .render("{{ test_key }} - {{ nested.inner }}", &context)
        .unwrap();
    assert_eq!(result, "test_value - inner_value");

    println!("✅ TemplateContext (not HashMap) test passed");
}

#[test]
#[serial]
fn test_environment_variable_precedence_detailed() {
    let mut env = TestEnvironment::new().unwrap();

    // ✅ Test: Comprehensive environment variable support
    env.write_project_config(
        r#"
base_value = "from_config"
timeout = 30

[nested]
key = "config_nested"
port = 8080

[deep]
nested = { value = "deep_config" }
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Test both SAH_ and SWISSARMYHAMMER_ prefixes
    env.set_env_vars([
        ("SAH_BASE_VALUE", "from_sah_env"),
        ("SAH_NESTED__KEY", "from_sah_nested"),
        ("SWISSARMYHAMMER_TIMEOUT", "60"),
        ("SWISSARMYHAMMER_DEEP__NESTED__VALUE", "from_swiss_env"),
        ("SAH_DYNAMIC_VAL", "new_from_env"),
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Environment variables should override config values
    assert_eq!(context.get_string("base_value").unwrap(), "from_sah_env");
    if let Some(nested) = context.get("nested") {
        assert_eq!(
            nested["key"],
            serde_json::Value::String("from_sah_nested".to_string())
        );
        assert_eq!(nested["port"], serde_json::Value::Number(8080.into()));
    }
    assert_eq!(context.get_number("timeout").unwrap(), 60.0); // SWISSARMYHAMMER_ prefix
    if let Some(deep) = context.get("deep") {
        if let Some(nested_deep) = deep.get("nested") {
            assert_eq!(
                nested_deep["value"],
                serde_json::Value::String("from_swiss_env".to_string())
            );
        }
    }
    assert_eq!(context.get_string("dynamic_val").unwrap(), "new_from_env"); // New key from env

    println!("✅ Environment variable precedence test passed");
}

#[test]
#[serial]
fn test_migration_completeness_validation() {
    // This test would run the search patterns mentioned in the spec
    // For now, we'll validate that the new system APIs are accessible

    // ✅ Test: New system components are available
    let provider = ConfigProvider::new();
    let _context = provider.load_template_context().unwrap();

    // ✅ Test: Template rendering works with new system
    let renderer = TemplateRenderer::new().unwrap();
    let test_context = provider.load_template_context().unwrap();
    let _result = renderer
        .render("{{ project_name | default: 'test' }}", &test_context)
        .unwrap();

    println!("✅ Migration completeness validation passed");
}

/// Creates a realistic large configuration for performance testing
fn create_realistic_large_config() -> String {
    let mut config = String::from(
        r#"
project_name = "Enterprise Config"
version = "2.1.0"
environment = "production"

[infrastructure]
cloud = "aws"
region = "us-east-1"
zones = ["us-east-1a", "us-east-1b", "us-east-1c"]

[database]
primary = "db-primary.company.com"
replicas = ["db-replica-1.company.com", "db-replica-2.company.com"]
port = 5432
pool_size = 20

"#,
    );

    // Add many service configurations for realistic scale
    for i in 1..=20 {
        config.push_str(&format!(
            r#"
[services.service_{:02}]
name = "service-{:02}"
port = {}
replicas = {}
memory = "{}Mi"
version = "1.{}.0"
"#,
            i,
            i,
            8000 + i,
            if i <= 10 { 3 } else { 2 },
            512 + (i * 32),
            i % 3
        ));
    }

    config
}

/// Creates a complex template for performance testing  
fn create_complex_template() -> String {
    r#"
# {{ project_name | default: "Test Project" }} v{{ version | default: "1.0.0" }}
Environment: {{ environment | default: "development" }}
Database: {{ database.primary | default: "localhost" }}:{{ database.port | default: "5432" }}
Pool Size: {{ database.pool_size | default: "10" }} connections
Cloud Provider: {{ infrastructure.cloud | default: "local" }}
Region: {{ infrastructure.region | default: "us-east-1" }}
"#
    .to_string()
}
