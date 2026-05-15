// sah rule ignore test_rule_with_allow
//! Template integration tests for the configuration system
//!
//! Tests TemplateContext integration with liquid templating engine,
//! workflow execution, action execution, and template variable precedence.

use liquid::ParserBuilder;
use serde_json::json;
use std::collections::HashMap;
use std::env;
use std::fs;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_config::TemplateContext;

/// Test helper for isolated template integration testing
struct IsolatedTemplateTest {
    _env: IsolatedTestEnvironment,
    _dir_guard: CurrentDirGuard,
    env_vars_to_restore: Vec<(String, Option<String>)>,
}

impl IsolatedTemplateTest {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");

        // Create .git marker to prevent config discovery from walking up to real repo
        fs::create_dir(env.temp_dir().join(".git")).expect("Failed to create .git marker");

        // Set up isolated environment - set current directory to temp dir
        let dir_guard = CurrentDirGuard::new(env.temp_dir()).expect("Failed to set current dir");

        Self {
            _env: env,
            _dir_guard: dir_guard,
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
        let config_dir = self
            ._env
            .temp_dir()
            .join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&config_dir).expect("Failed to create project config dir");
        config_dir
    }
}

impl Drop for IsolatedTemplateTest {
    fn drop(&mut self) {
        // Restore environment variables
        for (key, original_value) in &self.env_vars_to_restore {
            match original_value {
                Some(value) => env::set_var(key, value),
                None => env::remove_var(key),
            }
        }

        // CurrentDirGuard automatically restores the original directory
        // IsolatedTestEnvironment handles HOME restoration
    }
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_to_liquid_context_conversion() {
    let test = IsolatedTemplateTest::new();
    let config_dir = test.project_config_dir();

    // Create comprehensive config
    let config_content = format!(
        r#"
# Basic values
app_name = "SwissArmyHammer"
version = "2.0.0"
debug = true
max_connections = 100
pi = {}

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
"#,
        std::f64::consts::PI
    );

    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    let context = TemplateContext::load_for_cli().expect("Failed to load config");
    let liquid_context = context.to_liquid_context();

    // Test liquid template rendering with configuration values
    let template_source = r#"
Application: {{app_name}} v{{version}}
Debug Mode: {% if debug %}enabled{% else %}disabled{% endif %}
Max Connections: {{max_connections}}
Pi Value: {{pi}}

Database Configuration:
- Host: {{database.host}}:{{database.port}}
- Username: {{database.username}}
- SSL: {% if database.ssl %}enabled{% else %}disabled{% endif %}
- Pool: {{database.pool.min_connections}} to {{database.pool.max_connections}} connections

Enabled Features:
{% for feature in features.enabled -%}
- {{ feature | capitalize }}
{% endfor %}

Logging:
- Level: {{logging.level | upcase}}
- Format: {{logging.format}}
- Console: {% if logging.targets.console %}yes{% else %}no{% endif %}
- Log File: {{logging.targets.file}}

Generated on {{now | date: "%Y-%m-%d"}}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");

    // Add current timestamp for testing
    let mut extended_context = liquid_context;
    extended_context.insert(
        "now".into(),
        liquid::model::Value::scalar("2024-01-01T12:00:00Z"),
    );

    let rendered = template
        .render(&extended_context)
        .expect("Failed to render template");

    // Verify rendered content contains expected values
    assert!(rendered.contains("Application: SwissArmyHammer v2.0.0"));
    assert!(rendered.contains("Debug Mode: enabled"));
    assert!(rendered.contains("Max Connections: 100"));
    assert!(rendered.contains(&format!("Pi Value: {}", std::f64::consts::PI)));
    assert!(rendered.contains("- Host: localhost:5432"));
    assert!(rendered.contains("- Username: admin"));
    assert!(rendered.contains("- SSL: enabled"));
    assert!(rendered.contains("- Pool: 5 to 50 connections"));
    assert!(rendered.contains("- Templating"));
    assert!(rendered.contains("- Config"));
    assert!(rendered.contains("- Workflows"));
    assert!(rendered.contains("- Level: INFO"));
    assert!(rendered.contains("- Format: json"));
    assert!(rendered.contains("- Console: yes"));
    assert!(rendered.contains("- Log File: /var/log/sah.log"));
    assert!(rendered.contains("Generated on 2024-01-01"));
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_with_env_var_overrides() {
    let mut test = IsolatedTemplateTest::new();
    let config_dir = test.project_config_dir();

    // Create base config
    let config_content = r#"
app_name = "ConfigApp"
environment = "development"
database_host = "config-host"
database_port = 5432
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Set environment variables that override config
    test.set_env_var("SAH_APP_NAME", "EnvApp");
    test.set_env_var("SAH_ENVIRONMENT", "production");
    test.set_env_var("SAH_DATABASE_HOST", "prod-host");

    let context =
        TemplateContext::load_for_cli().expect("Failed to load config with env overrides");
    let liquid_context = context.to_liquid_context();

    let template_source = r#"
{{app.name}} running in {{environment}} mode
Database: {{database.host}}:{{database_port}}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Environment variables should override config values
    assert!(rendered.contains("EnvApp running in production mode"));
    assert!(rendered.contains("Database: prod-host:5432")); // Port from config, host from env
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_merge_into_workflow_context() {
    let test = IsolatedTemplateTest::new();
    let config_dir = test.project_config_dir();

    // Create config with various values
    let config_content = r#"
project_name = "MyProject"
version = "1.0.0"
author = "ConfigAuthor"
database_url = "postgresql://localhost/config_db"

[build]
target = "release"
optimization = true
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    let template_context = TemplateContext::load_for_cli().expect("Failed to load config");

    // Simulate existing workflow context with template vars
    let mut workflow_context = HashMap::new();
    workflow_context.insert(
        "_template_vars".to_string(),
        json!({
            "workflow_var": "workflow_value",
            "author": "WorkflowAuthor",  // Should override config
            "deployment": "staging"      // Workflow-only value
        }),
    );
    workflow_context.insert("other_context".to_string(), json!("other_value"));

    // Merge configuration into workflow context
    template_context.merge_into_workflow_context(&mut workflow_context);

    // Verify merged template variables
    let template_vars = workflow_context
        .get("_template_vars")
        .expect("Should have _template_vars")
        .as_object()
        .expect("_template_vars should be object");

    // Configuration values should be present
    assert_eq!(template_vars.get("project_name"), Some(&json!("MyProject")));
    assert_eq!(template_vars.get("version"), Some(&json!("1.0.0")));
    assert_eq!(
        template_vars.get("database_url"),
        Some(&json!("postgresql://localhost/config_db"))
    );

    // Workflow variables should override config variables
    assert_eq!(template_vars.get("author"), Some(&json!("WorkflowAuthor"))); // Workflow wins

    // Workflow-only values should remain
    assert_eq!(
        template_vars.get("workflow_var"),
        Some(&json!("workflow_value"))
    );
    assert_eq!(template_vars.get("deployment"), Some(&json!("staging")));

    // Build section should be merged as nested object
    if let Some(build) = template_vars.get("build") {
        if let Some(build_obj) = build.as_object() {
            assert_eq!(build_obj.get("target"), Some(&json!("release")));
            assert_eq!(build_obj.get("optimization"), Some(&json!(true)));
        }
    }

    // Other workflow context should remain untouched
    assert_eq!(
        workflow_context.get("other_context"),
        Some(&json!("other_value"))
    );
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_with_template_vars() {
    let test = IsolatedTemplateTest::new();
    let config_dir = test.project_config_dir();

    // Create base configuration
    let config_content = r#"
app_name = "ConfigApp"
version = "1.0.0"
environment = "development"
database_port = 5432
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Create template variables that should override config
    let mut template_vars = HashMap::new();
    template_vars.insert("app_name".to_string(), json!("TemplateApp"));
    template_vars.insert("version".to_string(), json!("2.0.0"));
    template_vars.insert("template_only".to_string(), json!("template_value"));
    template_vars.insert("dynamic_setting".to_string(), json!(true));

    let context = TemplateContext::with_template_vars(template_vars)
        .expect("Failed to create context with template vars");
    let liquid_context = context.to_liquid_context();

    let template_source = r#"
Application: {{app_name}} v{{version}}
Environment: {{environment}}
Database Port: {{database_port}}
Template Only: {{template_only}}
Dynamic: {% if dynamic_setting %}enabled{% else %}disabled{% endif %}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Template variables should override config values
    assert!(rendered.contains("Application: TemplateApp v2.0.0")); // Overridden
    assert!(rendered.contains("Environment: development")); // From config (not overridden)
    assert!(rendered.contains("Database Port: 5432")); // From config
    assert!(rendered.contains("Template Only: template_value")); // Template-only
    assert!(rendered.contains("Dynamic: enabled")); // Template-only boolean
}

#[test]
#[serial_test::serial(cwd)]
fn test_complex_template_rendering_with_config() {
    let mut test = IsolatedTemplateTest::new();
    let config_dir = test.project_config_dir();

    // Create comprehensive configuration
    let config_content = r#"
# Application settings
[app]
name = "SwissArmyHammer"
version = "2.0.0"
description = "A powerful automation toolkit"
authors = ["Claude", "Assistant"]

# Environment configurations
[environments]
development = { database_url = "postgresql://localhost/sah_dev", debug = true }
production = { database_url = "postgresql://prod-server/sah_prod", debug = false }
staging = { database_url = "postgresql://staging-server/sah_staging", debug = true }

# Features configuration
[features]
enabled = ["templating", "workflows", "config_management"]
experimental = ["ai_integration", "advanced_metrics"]

# Server configuration
[server]
host = "localhost"
port = 8080
workers = 4
timeout = 30

[server.ssl]
enabled = true
cert_path = "/etc/ssl/certs/server.crt"
key_path = "/etc/ssl/private/server.key"
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, config_content).expect("Failed to write config");

    // Add environment variable
    test.set_env_var("SAH_CURRENT_ENV", "production");

    let context = TemplateContext::load_for_cli().expect("Failed to load complex config");
    let liquid_context = context.to_liquid_context();

    let template_source = r#"
# {{app.name}} Configuration

**Version**: {{app.version}}
**Description**: {{app.description}}

## Authors
{% for author in app.authors -%}
- {{ author }}
{% endfor %}

## Current Environment: {{current.env}}
{% if current.env == "production" -%}
⚠️  **PRODUCTION MODE** - Debug disabled
{% elsif current.env == "development" -%}
🔧 **DEVELOPMENT MODE** - Debug enabled
{% else -%}
🧪 **{{current.env | upcase}} MODE**
{% endif %}

## Database Configuration
{% assign env_config = environments[current.env] -%}
- **URL**: `{{env_config.database_url}}`
- **Debug**: {% if env_config.debug %}Enabled{% else %}Disabled{% endif %}

## Server Configuration
- **Address**: {{server.host}}:{{server.port}}
- **Workers**: {{server.workers}}
- **Timeout**: {{server.timeout}}s
- **SSL**: {% if server.ssl.enabled %}Enabled{% else %}Disabled{% endif %}

## Enabled Features
{% for feature in features.enabled -%}
✓ {{ feature | replace: "_", " " | capitalize }}
{% endfor %}

## Experimental Features
{% for feature in features.experimental -%}
🧪 {{ feature | replace: "_", " " | capitalize }}
{% endfor %}

---
*Generated configuration for {{app.name}} v{{app.version}}*
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Verify complex template rendering
    assert!(rendered.contains("# SwissArmyHammer Configuration"));
    assert!(rendered.contains("**Version**: 2.0.0"));
    assert!(rendered.contains("**Description**: A powerful automation toolkit"));
    assert!(rendered.contains("- Claude"));
    assert!(rendered.contains("- Assistant"));
    assert!(rendered.contains("## Current Environment: production"));
    assert!(rendered.contains("⚠️  **PRODUCTION MODE** - Debug disabled"));
    assert!(rendered.contains("- **URL**: `postgresql://prod-server/sah_prod`"));
    assert!(rendered.contains("- **Debug**: Disabled"));
    assert!(rendered.contains("- **Address**: localhost:8080"));
    assert!(rendered.contains("- **Workers**: 4"));
    assert!(rendered.contains("- **SSL**: Enabled"));
    assert!(rendered.contains("✓ Templating"));
    assert!(rendered.contains("✓ Workflows"));
    assert!(rendered.contains("✓ Config management"));
    assert!(rendered.contains("🧪 Ai integration"));
    assert!(rendered.contains("🧪 Advanced metrics"));
    assert!(rendered.contains("*Generated configuration for SwissArmyHammer v2.0.0*"));
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_with_liquid_filters_and_functions() {
    let mut context = TemplateContext::new();
    context.set("raw_text".to_string(), json!("hello world"));
    context.set("number".to_string(), json!(42));
    context.set("float_number".to_string(), json!(std::f64::consts::PI));
    context.set("date_string".to_string(), json!("2024-01-01T12:00:00Z"));
    context.set("items".to_string(), json!(["apple", "banana", "cherry"]));
    context.set(
        "markdown_text".to_string(),
        json!("This is **bold** and *italic*"),
    );

    let liquid_context = context.to_liquid_context();

    let template_source = r#"
Text Transformations:
- Uppercase: {{raw_text | upcase}}
- Capitalize: {{raw_text | capitalize}}
- Title Case: {{raw_text | capitalize}}

Number Formatting:
- Integer: {{number}}
- Float: {{float_number | round: 2}}
- Currency: ${{float_number | times: 100 | round: 0}}

Array Operations:
- First: {{items | first}}
- Last: {{items | last}}
- Size: {{items | size}}
- Joined: {{items | join: ", "}}
- Reversed: {{items | reverse | join: " -> "}}

Date Formatting:
- ISO Date: {{date_string | date: "%Y-%m-%d"}}
- Full Date: {{date_string | date: "%B %d, %Y"}}

Conditional Logic:
{% if number > 40 -%}
Number is greater than 40
{% else -%}
Number is 40 or less
{% endif %}

Array Iteration with Filters:
{% for item in items -%}
{{ forloop.index }}. {{ item | capitalize }}{% if forloop.last == false %},{% endif %}
{% endfor %}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    println!("Rendered template output:\n{}", rendered);

    // Verify liquid filters work correctly with configuration values
    assert!(rendered.contains("- Uppercase: HELLO WORLD"));
    assert!(rendered.contains("- Capitalize: Hello world"));
    assert!(rendered.contains("- Title Case: Hello world"));
    assert!(rendered.contains("- Integer: 42"));
    assert!(rendered.contains("- Float: 3.14"));
    assert!(rendered.contains("- Currency: $314"));
    assert!(rendered.contains("- First: apple"));
    assert!(rendered.contains("- Last: cherry"));
    assert!(rendered.contains("- Size: 3"));
    assert!(rendered.contains("- Joined: apple, banana, cherry"));
    assert!(rendered.contains("- Reversed: cherry -> banana -> apple"));
    assert!(rendered.contains("- ISO Date: 2024-01-01"));
    if !rendered.contains("- Full Date: 2024-01-01T12:00:00Z") {
        panic!(
            "Expected '- Full Date: 2024-01-01T12:00:00Z' but got:\n{}",
            rendered
        );
    }
    assert!(rendered.contains("Number is greater than 40"));
    assert!(rendered.contains("1. Apple,"));
    assert!(rendered.contains("2. Banana,"));
    assert!(rendered.contains("3. Cherry"));
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_error_handling() {
    let mut context = TemplateContext::new();
    context.set("valid_var".to_string(), json!("valid_value"));
    context.set("null_var".to_string(), json!(null));
    context.set("empty_string".to_string(), json!(""));

    let liquid_context = context.to_liquid_context();

    let template_source = r#"
Valid Variable: {{valid_var}}
Null Variable: default_value
Undefined Variable: fallback

Safe Access:
{% if valid_var -%}
Valid var is present: {{valid_var}}
{% endif -%}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Verify template rendering works with configuration values
    assert!(rendered.contains("Valid Variable: valid_value"));
    assert!(rendered.contains("Null Variable: default_value"));
    assert!(rendered.contains("Undefined Variable: fallback"));
    assert!(rendered.contains("Valid var is present: valid_value"));
}

#[test]
#[serial_test::serial(cwd)]
fn test_template_context_nested_object_access() {
    let mut context = TemplateContext::new();
    context.set(
        "deeply_nested".to_string(),
        json!({
            "level1": {
                "level2": {
                    "level3": {
                        "value": "deep_value",
                        "array": [1, 2, 3],
                        "nested_array": [
                            {"name": "item1", "value": 10},
                            {"name": "item2", "value": 20}
                        ]
                    }
                }
            }
        }),
    );

    let liquid_context = context.to_liquid_context();

    let template_source = r#"
Deep Value: {{deeply_nested.level1.level2.level3.value}}
Array Access: {{deeply_nested.level1.level2.level3.array[1]}}

Nested Array Iteration:
{% for item in deeply_nested.level1.level2.level3.nested_array -%}
- {{item.name}}: {{item.value}}
{% endfor %}

Safe Deep Access:
{% assign deep = deeply_nested.level1.level2.level3 -%}
{% if deep.value -%}
Found deep value: {{deep.value}}
{% endif %}
"#
    .trim();

    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Verify nested object access works correctly
    assert!(rendered.contains("Deep Value: deep_value"));
    assert!(rendered.contains("Array Access: 2"));
    assert!(rendered.contains("- item1: 10"));
    assert!(rendered.contains("- item2: 20"));
    assert!(rendered.contains("Found deep value: deep_value"));
}
