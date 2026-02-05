//! Comprehensive integration tests for the configuration system
//!
//! End-to-end tests that combine multiple aspects of the configuration system
//! to test realistic user scenarios and full system integration.

use liquid::ParserBuilder;
use serde_json::json;
use std::env;
use std::fs;
use swissarmyhammer_common::test_utils::{CurrentDirGuard, IsolatedTestEnvironment};
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_config::TemplateContext;

/// Test helper for comprehensive integration testing
struct IntegrationTestEnvironment {
    _env: IsolatedTestEnvironment,
    _dir_guard: CurrentDirGuard,
    env_vars_to_restore: Vec<(String, Option<String>)>,
}

impl IntegrationTestEnvironment {
    fn new() -> Self {
        let env = IsolatedTestEnvironment::new().expect("Failed to create test environment");

        // Create .git marker to prevent config discovery from walking up to real repo
        fs::create_dir(env.temp_dir().join(".git")).expect("Failed to create .git marker");

        // Set current directory to temp dir for these tests
        let dir_guard = CurrentDirGuard::new(env.temp_dir()).expect("Failed to set current dir");

        Self {
            _env: env,
            _dir_guard: dir_guard,
            env_vars_to_restore: Vec::new(),
        }
    }

    fn set_env_var(&mut self, key: &str, value: &str) {
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

    fn home_config_dir(&self) -> std::path::PathBuf {
        let config_dir = self._env.swissarmyhammer_dir();
        fs::create_dir_all(&config_dir).expect("Failed to create home config dir");
        config_dir
    }

    fn create_nested_project_structure(&self) -> std::path::PathBuf {
        // Create a nested project structure
        let workspace_dir = self._env.temp_dir().join("workspace");
        let project_dir = workspace_dir.join("my-project");
        let subdir = project_dir.join("src").join("components");
        fs::create_dir_all(&subdir).expect("Failed to create nested structure");

        // Create config at workspace level
        let workspace_config_dir = workspace_dir.join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&workspace_config_dir).expect("Failed to create workspace config");

        // Create config at project level
        let project_config_dir = project_dir.join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&project_config_dir).expect("Failed to create project config");

        subdir
    }
}

impl Drop for IntegrationTestEnvironment {
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
fn test_complete_development_workflow_scenario() {
    let mut test = IntegrationTestEnvironment::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Scenario: Developer working on a SwissArmyHammer project with multiple environments

    // 1. Global configuration (personal developer settings)
    let global_config = r#"
# Personal developer configuration
[developer]
name = "Jane Developer"
email = "jane@example.com"
preferred_editor = "vscode"

[defaults]
database_port = 5432
log_level = "info"
debug = false

[templates]
author = "Jane Developer"
license = "MIT"

[tools]
formatter = "prettier"
linter = "eslint"
"#;
    let global_file = home_config_dir.join("sah.toml");
    fs::write(&global_file, global_config).expect("Failed to write global config");

    // 2. Project configuration (project-specific settings)
    let project_config = r#"
# Project-specific configuration
[project]
name = "awesome-web-app"
version = "1.2.0"
description = "An awesome web application built with SwissArmyHammer"

[environments]
development = { database_url = "postgresql://localhost/awesome_dev", debug = true }
staging = { database_url = "postgresql://staging-server/awesome_staging", debug = false }
production = { database_url = "postgresql://prod-server/awesome_prod", debug = false }

[build]
target_dir = "dist"
minify = true
source_maps = true

[server]
port = 3000
host = "localhost"
workers = 4

[features]
enabled = ["authentication", "notifications", "analytics"]
experimental = ["real_time_updates", "ai_recommendations"]

# Override global defaults for this project
[defaults]
log_level = "debug"  # More verbose logging for development
"#;
    let project_file = project_config_dir.join("sah.toml");
    fs::write(&project_file, project_config).expect("Failed to write project config");

    // 3. Environment variables (runtime environment)
    test.set_env_var("NODE_ENV", "development");
    test.set_env_var("SAH_CURRENT_ENV", "development");
    test.set_env_var("SAH_SERVER_PORT", "8080"); // Override config port
    test.set_env_var("SWISSARMYHAMMER_BUILD_MINIFY", "false"); // Disable minify for development

    // 4. Load configuration
    let context =
        TemplateContext::load_for_cli().expect("Failed to load development configuration");

    // 5. Verify precedence and merging

    // Personal settings from global config
    assert_eq!(
        context.get("developer.name"),
        Some(&json!("Jane Developer"))
    );
    assert_eq!(
        context.get("developer.email"),
        Some(&json!("jane@example.com"))
    );
    assert_eq!(
        context.get("templates.author"),
        Some(&json!("Jane Developer"))
    );

    // Project settings
    assert_eq!(context.get("project.name"), Some(&json!("awesome-web-app")));
    assert_eq!(context.get("project.version"), Some(&json!("1.2.0")));

    // Environment variable overrides
    assert_eq!(context.get("current.env"), Some(&json!("development")));
    assert_eq!(context.get("server.port"), Some(&json!(8080))); // Overridden by env var (env vars are parsed as numbers)
    assert_eq!(context.get("build.minify"), Some(&json!(false))); // Overridden by env var (env vars are parsed as booleans)

    // Default value overrides (project overrides global)
    assert_eq!(context.get("defaults.log_level"), Some(&json!("debug"))); // Project wins
    assert_eq!(context.get("defaults.database_port"), Some(&json!(5432))); // From global (not overridden)

    // 6. Test liquid template integration for practical use case
    let template_source = r#"
# {{project.name}} Development Configuration

**Version**: {{project.version}}
**Description**: {{project.description}}

## Developer Information
- **Name**: {{developer.name}}
- **Email**: {{developer.email}}
- **Editor**: {{developer.preferred_editor}}

## Current Environment: {{current.env}}
{% assign env_config = environments[current.env] -%}
- **Database**: {{env_config.database_url}}
- **Debug Mode**: {% if env_config.debug %}Enabled{% else %}Disabled{% endif %}

## Server Configuration
- **Host**: {{server.host}}
- **Port**: {{server.port}}
- **Workers**: {{server.workers}}

## Build Configuration
- **Target Directory**: {{build.target_dir}}
- **Minify**: {% if build.minify %}Enabled{% else %}Disabled (Dev Mode){% endif %}
- **Source Maps**: {% if build.source_maps %}Enabled{% else %}Disabled{% endif %}

## Features
### Enabled
{% for feature in features.enabled -%}
- {{feature | replace: "_", " " | capitalize}}
{% endfor %}

### Experimental
{% for feature in features.experimental -%}
- {{feature | replace: "_", " " | capitalize}} (🧪 Experimental)
{% endfor %}

## Default Settings
- **Log Level**: {{defaults.log_level}}
- **Database Port**: {{defaults.database_port}}
- **Debug**: {% if defaults.debug %}Enabled{% else %}Disabled{% endif %}

---
*Configuration generated for {{developer.name}} working on {{project.name}} v{{project.version}}*
*Template Author: {{templates.author}} | License: {{templates.license}}*
"#
    .trim();

    let liquid_context = context.to_liquid_context();
    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(template_source)
        .expect("Failed to parse template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render template");

    // Verify complete template rendering
    assert!(rendered.contains("# awesome-web-app Development Configuration"));
    assert!(rendered.contains("**Version**: 1.2.0"));
    assert!(rendered.contains("- **Name**: Jane Developer"));
    assert!(rendered.contains("## Current Environment: development"));
    assert!(rendered.contains("- **Database**: postgresql://localhost/awesome_dev"));
    assert!(rendered.contains("- **Debug Mode**: Enabled"));
    assert!(rendered.contains("- **Port**: 8080"));
    assert!(rendered.contains("- **Minify**: Disabled (Dev Mode)"));
    assert!(rendered.contains("- Authentication"));
    assert!(rendered.contains("- Real time updates (🧪 Experimental)"));
    assert!(rendered.contains("- **Log Level**: debug"));

    println!("Development workflow template rendered successfully");
}

#[test]
#[serial_test::serial(cwd)]
fn test_production_deployment_scenario() {
    let mut test = IntegrationTestEnvironment::new();
    let project_config_dir = test.project_config_dir();
    let home_config_dir = test.home_config_dir();

    // Scenario: Production deployment with security-focused configuration

    // Global production defaults
    let global_config = r#"
[security]
ssl_required = true
session_timeout = 3600
rate_limiting = true

[monitoring]
enabled = true
metrics_endpoint = "/metrics"
health_check_endpoint = "/health"

[logging]
level = "warn"
format = "json"
audit = true
"#;
    let global_file = home_config_dir.join("swissarmyhammer.toml");
    fs::write(&global_file, global_config).expect("Failed to write global TOML config");

    // Project production config
    let project_config = r#"{
  "application": {
    "name": "production-app",
    "version": "2.1.0",
    "environment": "production"
  },
  "database": {
    "host": "db-cluster.internal",
    "port": 5432,
    "pool_size": 50,
    "ssl_mode": "require"
  },
  "server": {
    "bind_address": "0.0.0.0",
    "port": 80,
    "workers": 16,
    "keepalive_timeout": 65
  },
  "features": {
    "experimental": [],
    "production": ["caching", "compression", "monitoring", "security"]
  }
}"#;
    let project_file = project_config_dir.join("sah.json");
    fs::write(&project_file, project_config).expect("Failed to write project JSON config");

    // Production environment variables (from container/k8s)
    test.set_env_var("NODE_ENV", "production");
    test.set_env_var("SAH_DATABASE_PASSWORD", "super_secret_prod_password");
    test.set_env_var("SAH_SERVER_PORT", "443");
    test.set_env_var("SAH_SSL_CERT_PATH", "/etc/ssl/certs/app.crt");
    test.set_env_var("SAH_SSL_KEY_PATH", "/etc/ssl/private/app.key");
    test.set_env_var("SWISSARMYHAMMER_LOGGING_LEVEL", "error"); // Even more restrictive in prod

    // Production CLI overrides (deployment script)
    let cli_args = json!({
        "deployment": {
            "id": "deploy-20240101-123456",
            "timestamp": "2024-01-01T12:34:56Z",
            "version": "2.1.0-build.789"
        },
        "monitoring": {
            "datadog_api_key": "dd_api_key_from_vault",
            "sentry_dsn": "sentry_dsn_from_vault"
        }
    });

    let context =
        TemplateContext::load_with_cli_args(cli_args).expect("Failed to load production config");

    // Verify production configuration
    assert_eq!(
        context.get("application.name"),
        Some(&json!("production-app"))
    );
    assert_eq!(
        context.get("application.environment"),
        Some(&json!("production"))
    );

    // Security settings from global config
    assert_eq!(context.get("security.ssl_required"), Some(&json!(true)));
    assert_eq!(context.get("security.rate_limiting"), Some(&json!(true)));

    // Environment variable overrides
    assert_eq!(
        context.get("database.password"),
        Some(&json!("super_secret_prod_password"))
    );
    assert_eq!(context.get("server.port"), Some(&json!(443)));
    assert_eq!(
        context.get("ssl.cert.path"),
        Some(&json!("/etc/ssl/certs/app.crt"))
    );

    // CLI overrides (highest precedence)
    assert_eq!(
        context.get("deployment.id"),
        Some(&json!("deploy-20240101-123456"))
    );
    assert_eq!(
        context.get("monitoring.datadog_api_key"),
        Some(&json!("dd_api_key_from_vault"))
    );

    // Logging level precedence (SWISSARMYHAMMER_ env var wins)
    assert_eq!(context.get("logging.level"), Some(&json!("error")));

    // Production template for deployment verification
    let deployment_template = r#"
# Production Deployment Verification

## Application
- **Name**: {{application.name}}
- **Version**: {{deployment.version}}
- **Environment**: {{application.environment}}
- **Deployment ID**: {{deployment.id}}
- **Deployed At**: {{deployment.timestamp}}

## Security Configuration
- **SSL Required**: {% if security.ssl_required %}✓ Enabled{% else %}✗ Disabled{% endif %}
- **Rate Limiting**: {% if security.rate_limiting %}✓ Enabled{% else %}✗ Disabled{% endif %}
- **Session Timeout**: {{security.session_timeout}}s
- **SSL Certificate**: {{ssl.cert.path}}

## Database Configuration
- **Host**: {{database.host}}:{{database.port}}
- **Pool Size**: {{database.pool_size}}
- **SSL Mode**: {{database.ssl_mode}}
- **Password**: {% if database.password %}✓ Set{% else %}✗ Not Set{% endif %}

## Server Configuration
- **Bind Address**: {{server.bind_address}}
- **Port**: {{server.port}}
- **Workers**: {{server.workers}}
- **Keepalive Timeout**: {{server.keepalive_timeout}}s

## Monitoring
- **Enabled**: {% if monitoring.enabled %}✓ Yes{% else %}✗ No{% endif %}
- **Metrics**: {{monitoring.metrics_endpoint}}
- **Health Check**: {{monitoring.health_check_endpoint}}
- **Datadog**: {% if monitoring.datadog_api_key %}✓ Configured{% else %}✗ Not Configured{% endif %}
- **Sentry**: {% if monitoring.sentry_dsn %}✓ Configured{% else %}✗ Not Configured{% endif %}

## Logging
- **Level**: {{logging.level}}
- **Format**: {{logging.format}}
- **Audit**: {% if logging.audit %}✓ Enabled{% else %}✗ Disabled{% endif %}

## Production Features
{% for feature in features.production -%}
✓ {{feature | capitalize}}
{% endfor %}

---
**⚠️ PRODUCTION ENVIRONMENT ⚠️**
*Deployment verified at {{deployment.timestamp}}*
"#
    .trim();

    let liquid_context = context.to_liquid_context();
    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(deployment_template)
        .expect("Failed to parse deployment template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render deployment template");

    // Verify production deployment template
    assert!(rendered.contains("# Production Deployment Verification"));
    assert!(rendered.contains("- **Name**: production-app"));
    assert!(rendered.contains("- **Version**: 2.1.0-build.789"));
    assert!(rendered.contains("- **Environment**: production"));
    assert!(rendered.contains("- **SSL Required**: ✓ Enabled"));
    assert!(rendered.contains("- **Rate Limiting**: ✓ Enabled"));
    assert!(rendered.contains("- **Port**: 443"));
    assert!(rendered.contains("- **Password**: ✓ Set"));
    assert!(rendered.contains("- **Datadog**: ✓ Configured"));
    assert!(rendered.contains("- **Level**: error"));
    assert!(rendered.contains("✓ Caching"));
    assert!(rendered.contains("✓ Security"));
    assert!(rendered.contains("**⚠️ PRODUCTION ENVIRONMENT ⚠️**"));

    println!("Production deployment template rendered successfully");
}

#[test]
#[serial_test::serial(cwd)]
fn test_multi_environment_project_with_dynamic_switching() {
    let mut test = IntegrationTestEnvironment::new();
    let project_config_dir = test.project_config_dir();

    // Multi-environment project configuration
    let project_config = r#"
[project]
name = "multi-env-app"
version = "1.0.0"

# Environment-specific configurations
[environments.development]
database_url = "postgresql://localhost/app_dev"
api_base_url = "http://localhost:3000"
debug = true
log_level = "debug"
features = ["hot_reload", "dev_tools", "debug_ui"]

[environments.staging]
database_url = "postgresql://staging-db/app_staging"
api_base_url = "https://api-staging.example.com"
debug = false
log_level = "info"
features = ["performance_monitoring", "error_tracking"]

[environments.production]
database_url = "postgresql://prod-db-cluster/app_prod"
api_base_url = "https://api.example.com"
debug = false
log_level = "error"
features = ["performance_monitoring", "error_tracking", "analytics", "security_headers"]

# Common configuration that applies to all environments
[common]
app_name = "Multi-Environment Application"
timeout = 30
retry_attempts = 3

[common.security]
csrf_protection = true
rate_limiting = true
"#;
    let config_file = project_config_dir.join("sah.toml");
    fs::write(&config_file, project_config).expect("Failed to write multi-env config");

    // Test each environment dynamically
    let environments = ["development", "staging", "production"];

    for env_name in &environments {
        // Set environment via environment variable
        test.set_env_var("SAH_CURRENT_ENVIRONMENT", env_name);

        let context = TemplateContext::load_for_cli().expect("Failed to load multi-env config");

        // Verify basic project settings are always present
        assert_eq!(context.get("project.name"), Some(&json!("multi-env-app")));
        assert_eq!(context.get("project.version"), Some(&json!("1.0.0")));
        assert_eq!(
            context.get("common.app_name"),
            Some(&json!("Multi-Environment Application"))
        );
        assert_eq!(context.get("common.timeout"), Some(&json!(30)));

        // Verify environment-specific settings
        assert_eq!(context.get("current.environment"), Some(&json!(env_name)));

        // Check environment-specific values
        match *env_name {
            "development" => {
                if let Some(envs) = context.get("environments") {
                    if let Some(envs_obj) = envs.as_object() {
                        if let Some(dev) = envs_obj.get("development") {
                            if let Some(dev_obj) = dev.as_object() {
                                assert_eq!(
                                    dev_obj.get("database_url"),
                                    Some(&json!("postgresql://localhost/app_dev"))
                                );
                                assert_eq!(dev_obj.get("debug"), Some(&json!(true)));
                                assert_eq!(dev_obj.get("log_level"), Some(&json!("debug")));
                            }
                        }
                    }
                }
            }
            "staging" => {
                if let Some(envs) = context.get("environments") {
                    if let Some(envs_obj) = envs.as_object() {
                        if let Some(staging) = envs_obj.get("staging") {
                            if let Some(staging_obj) = staging.as_object() {
                                assert_eq!(
                                    staging_obj.get("api_base_url"),
                                    Some(&json!("https://api-staging.example.com"))
                                );
                                assert_eq!(staging_obj.get("debug"), Some(&json!(false)));
                                assert_eq!(staging_obj.get("log_level"), Some(&json!("info")));
                            }
                        }
                    }
                }
            }
            "production" => {
                if let Some(envs) = context.get("environments") {
                    if let Some(envs_obj) = envs.as_object() {
                        if let Some(prod) = envs_obj.get("production") {
                            if let Some(prod_obj) = prod.as_object() {
                                assert_eq!(
                                    prod_obj.get("database_url"),
                                    Some(&json!("postgresql://prod-db-cluster/app_prod"))
                                );
                                assert_eq!(prod_obj.get("log_level"), Some(&json!("error")));
                            }
                        }
                    }
                }
            }
            _ => panic!("Unexpected environment: {}", env_name),
        }

        // Test environment-aware template rendering
        let env_template = r#"
# {{common.app_name}} - {{current.environment | upcase}} Environment

**Project**: {{project.name}} v{{project.version}}
**Environment**: {{current.environment}}

{% assign current_env_config = environments[current.environment] -%}
## Current Environment Configuration
- **Database**: {{current_env_config.database_url}}
- **API Base URL**: {{current_env_config.api_base_url}}
- **Debug Mode**: {% if current_env_config.debug %}Enabled{% else %}Disabled{% endif %}
- **Log Level**: {{current_env_config.log_level}}

## Environment Features
{% for feature in current_env_config.features -%}
- {{feature | replace: "_", " " | capitalize}}
{% endfor %}

## Common Settings
- **Timeout**: {{common.timeout}}s
- **Retry Attempts**: {{common.retry_attempts}}
- **CSRF Protection**: {% if common.security.csrf_protection %}Enabled{% else %}Disabled{% endif %}

{% if current.environment == "production" -%}
⚠️ **PRODUCTION ENVIRONMENT** - Extra care required!
{% elsif current.environment == "staging" -%}
🧪 **STAGING ENVIRONMENT** - Pre-production testing
{% else -%}
🔧 **DEVELOPMENT ENVIRONMENT** - Development mode active
{% endif %}
"#
        .trim();

        let liquid_context = context.to_liquid_context();
        let parser = ParserBuilder::with_stdlib()
            .build()
            .expect("Failed to create parser");
        let template = parser
            .parse(env_template)
            .expect("Failed to parse env template");
        let rendered = template
            .render(&liquid_context)
            .expect("Failed to render env template");

        // Verify environment-specific rendering
        assert!(rendered.contains(&format!(
            "# Multi-Environment Application - {} Environment",
            env_name.to_uppercase()
        )));
        assert!(rendered.contains(&format!("**Environment**: {}", env_name)));

        match *env_name {
            "development" => {
                assert!(rendered.contains("- **Debug Mode**: Enabled"));
                assert!(rendered.contains("- **Log Level**: debug"));
                assert!(rendered.contains("- Hot reload"));
                assert!(rendered.contains("🔧 **DEVELOPMENT ENVIRONMENT**"));
            }
            "staging" => {
                assert!(rendered.contains("- **API Base URL**: https://api-staging.example.com"));
                assert!(rendered.contains("- **Log Level**: info"));
                assert!(rendered.contains("🧪 **STAGING ENVIRONMENT**"));
            }
            "production" => {
                assert!(rendered.contains("- **Database**: postgresql://prod-db-cluster/app_prod"));
                assert!(rendered.contains("- **Log Level**: error"));
                assert!(rendered.contains("- Analytics"));
                assert!(rendered.contains("⚠️ **PRODUCTION ENVIRONMENT**"));
            }
            _ => {}
        }

        println!("Environment '{}' configuration verified", env_name);
    }
}

#[test]
#[serial_test::serial(cwd)]
fn test_complex_nested_project_structure_with_inheritance() {
    let mut test = IntegrationTestEnvironment::new();
    let nested_subdir = test.create_nested_project_structure();

    // Create workspace-level configuration
    let workspace_config_dir = test
        ._env
        .temp_dir()
        .join("workspace")
        .join(SwissarmyhammerDirectory::dir_name());
    let workspace_config = r#"
# Workspace-level configuration
[workspace]
name = "my-awesome-workspace"
version = "1.0.0"
type = "monorepo"

[defaults]
language = "rust"
license = "MIT"
author = "Workspace Team"

[tools]
formatter = "rustfmt"
linter = "clippy"
test_runner = "cargo-nextest"

[ci]
provider = "github-actions"
rust_version = "1.70"
"#;
    let workspace_file = workspace_config_dir.join("sah.toml");
    fs::write(&workspace_file, workspace_config).expect("Failed to write workspace config");

    // Create project-level configuration
    let project_config_dir = test
        ._env
        .temp_dir()
        .join("workspace/my-project")
        .join(SwissarmyhammerDirectory::dir_name());
    let project_config = r#"
# Project-level configuration
[project]
name = "my-project"
version = "0.2.0"
description = "A component of the awesome workspace"

# Override some workspace defaults
[defaults]
author = "Project Team"  # Override workspace author
license = "Apache-2.0"   # Override workspace license

# Project-specific settings
[dependencies]
serde = "1.0"
tokio = "1.0"
anyhow = "1.0"

[features]
default = ["async"]
async = ["tokio"]
serialization = ["serde"]
"#;
    let project_file = project_config_dir.join("sah.toml");
    fs::write(&project_file, project_config).expect("Failed to write project config");

    // Set some environment variables
    test.set_env_var("SAH_CI_BRANCH", "feature/awesome-feature");
    test.set_env_var("SAH_BUILD_NUMBER", "42");

    // Change to nested subdirectory and load config
    env::set_current_dir(&nested_subdir).expect("Failed to change to nested subdir");

    let context = TemplateContext::load_for_cli().expect("Failed to load nested project config");

    // Verify inheritance and overrides work correctly

    // Workspace-level settings should be inherited
    assert_eq!(
        context.get("workspace.name"),
        Some(&json!("my-awesome-workspace"))
    );
    assert_eq!(context.get("workspace.type"), Some(&json!("monorepo")));
    assert_eq!(context.get("tools.formatter"), Some(&json!("rustfmt")));
    assert_eq!(
        context.get("tools.test_runner"),
        Some(&json!("cargo-nextest"))
    );
    assert_eq!(context.get("ci.provider"), Some(&json!("github-actions")));

    // Project-level settings
    assert_eq!(context.get("project.name"), Some(&json!("my-project")));
    assert_eq!(context.get("project.version"), Some(&json!("0.2.0")));
    assert_eq!(
        context.get("project.description"),
        Some(&json!("A component of the awesome workspace"))
    );

    // Project should override workspace defaults
    assert_eq!(context.get("defaults.author"), Some(&json!("Project Team"))); // Project wins
    assert_eq!(context.get("defaults.license"), Some(&json!("Apache-2.0"))); // Project wins
    assert_eq!(context.get("defaults.language"), Some(&json!("rust"))); // From workspace (not overridden)

    // Environment variables
    assert_eq!(
        context.get("ci.branch"),
        Some(&json!("feature/awesome-feature"))
    );
    assert_eq!(context.get("build.number"), Some(&json!(42))); // Environment variables are parsed as numbers when numeric

    // Test comprehensive template that uses inherited and overridden values
    let inheritance_template = r#"
# {{project.name}} - Development Overview

## Workspace Information
- **Workspace**: {{workspace.name}} v{{workspace.version}}
- **Type**: {{workspace.type}}

## Project Information  
- **Name**: {{project.name}}
- **Version**: {{project.version}}

## Configuration
- **Language**: {{defaults.language}}
- **Author**: {{defaults.author}}
- **License**: {{defaults.license}}
"#
    .trim();

    let liquid_context = context.to_liquid_context();
    let parser = ParserBuilder::with_stdlib()
        .build()
        .expect("Failed to create parser");
    let template = parser
        .parse(inheritance_template)
        .expect("Failed to parse inheritance template");
    let rendered = template
        .render(&liquid_context)
        .expect("Failed to render inheritance template");

    // Verify inheritance template rendering
    assert!(rendered.contains("# my-project - Development Overview"));
    assert!(rendered.contains("- **Workspace**: my-awesome-workspace v1.0.0"));
    assert!(rendered.contains("- **Type**: monorepo"));
    assert!(rendered.contains("- **Version**: 0.2.0"));
    assert!(rendered.contains("- **Language**: rust"));
    assert!(rendered.contains("- **Author**: Project Team"));
    assert!(rendered.contains("- **License**: Apache-2.0"));

    println!("Complex nested project inheritance verified");
}

#[test]
#[serial_test::serial(cwd)]
fn test_real_time_configuration_updates_workflow() {
    let test = IntegrationTestEnvironment::new();
    let config_dir = test.project_config_dir();

    // Simulate a real-time development workflow with configuration changes

    // Initial configuration
    let initial_config = r#"
[app]
name = "live-app"
version = "0.1.0"
debug = true

[server]
port = 3000
workers = 2

[database]
url = "sqlite:app.db"
pool_size = 5
"#;
    let config_file = config_dir.join("sah.toml");
    fs::write(&config_file, initial_config).expect("Failed to write initial config");

    // Load initial configuration
    let context1 = TemplateContext::load_for_cli().expect("Failed to load initial config");
    assert_eq!(context1.get("app.version"), Some(&json!("0.1.0")));
    assert_eq!(context1.get("server.port"), Some(&json!(3000)));
    assert_eq!(context1.get("database.pool_size"), Some(&json!(5)));

    // Simulate development: update version and add new feature
    let updated_config_v2 = r#"
[app]
name = "live-app"
version = "0.2.0"
debug = true

[server]
port = 3000
workers = 4  # Increased workers

[database]
url = "postgresql://localhost/app_dev"  # Switched to PostgreSQL
pool_size = 10  # Increased pool size

[features]
new_feature = true
experimental_ui = false
"#;
    fs::write(&config_file, updated_config_v2).expect("Failed to write updated config");

    // Load updated configuration (should reflect changes immediately)
    let context2 = TemplateContext::load_for_cli().expect("Failed to load updated config");
    assert_eq!(context2.get("app.version"), Some(&json!("0.2.0")));
    assert_eq!(context2.get("server.workers"), Some(&json!(4)));
    assert_eq!(
        context2.get("database.url"),
        Some(&json!("postgresql://localhost/app_dev"))
    );
    assert_eq!(context2.get("database.pool_size"), Some(&json!(10)));
    assert_eq!(context2.get("features.new_feature"), Some(&json!(true)));

    // Simulate production preparation: disable debug, update version
    let production_config = r#"
[app]
name = "live-app"
version = "1.0.0"
debug = false

[server]
port = 80
workers = 8

[database]
url = "postgresql://prod-db/app_prod"
pool_size = 50

[features]
new_feature = true
experimental_ui = false
performance_monitoring = true
security_headers = true

[security]
ssl_required = true
csrf_protection = true
"#;
    fs::write(&config_file, production_config).expect("Failed to write production config");

    // Load production configuration
    let context3 = TemplateContext::load_for_cli().expect("Failed to load production config");
    assert_eq!(context3.get("app.version"), Some(&json!("1.0.0")));
    assert_eq!(context3.get("app.debug"), Some(&json!(false)));
    assert_eq!(context3.get("server.port"), Some(&json!(80)));
    assert_eq!(context3.get("server.workers"), Some(&json!(8)));
    assert_eq!(context3.get("security.ssl_required"), Some(&json!(true)));
    assert_eq!(
        context3.get("features.performance_monitoring"),
        Some(&json!(true))
    );

    // Test that each configuration change was picked up immediately (fresh loading)
    // and that template rendering works with each version

    let version_contexts = vec![
        ("v0.1.0", &context1),
        ("v0.2.0", &context2),
        ("v1.0.0", &context3),
    ];

    for (version_name, context) in version_contexts {
        let status_template = r#"
{{app.name}} {{app.version}} Status
{% if app.debug -%}
🔧 DEBUG MODE ACTIVE
{% else -%}
✓ PRODUCTION MODE
{% endif %}

Server: {{server.workers}} workers on port {{server.port}}
Database: {{database.url}} (pool: {{database.pool_size}})

{% if features -%}
Features:
{% for feature in features -%}
- {{feature[0] | replace: "_", " " | capitalize}}: {% if feature[1] %}✓{% else %}✗{% endif %}
{% endfor %}
{% endif %}

{% if security -%}
Security:
{% for setting in security -%}
- {{setting[0] | replace: "_", " " | capitalize}}: {% if setting[1] %}✓{% else %}✗{% endif %}
{% endfor %}
{% endif %}
"#
        .trim();

        let liquid_context = context.to_liquid_context();
        let parser = ParserBuilder::with_stdlib()
            .build()
            .expect("Failed to create parser");
        let template = parser
            .parse(status_template)
            .expect("Failed to parse status template");
        let rendered = template
            .render(&liquid_context)
            .expect("Failed to render status template");

        // Verify version-specific rendering
        assert!(rendered.contains(&format!(
            "live-app {} Status",
            context.get("app.version").unwrap().as_str().unwrap()
        )));

        match version_name {
            "v0.1.0" => {
                assert!(rendered.contains("🔧 DEBUG MODE ACTIVE"));
                assert!(rendered.contains("Server: 2 workers on port 3000"));
                assert!(rendered.contains("Database: sqlite:app.db (pool: 5)"));
                assert!(!rendered.contains("Features:")); // No features in v0.1.0
            }
            "v0.2.0" => {
                assert!(rendered.contains("🔧 DEBUG MODE ACTIVE"));
                assert!(rendered.contains("Server: 4 workers on port 3000"));
                assert!(rendered.contains("Database: postgresql://localhost/app_dev (pool: 10)"));
                assert!(rendered.contains("- New feature: ✓"));
                assert!(rendered.contains("- Experimental ui: ✗"));
            }
            "v1.0.0" => {
                assert!(rendered.contains("✓ PRODUCTION MODE"));
                assert!(rendered.contains("Server: 8 workers on port 80"));
                assert!(rendered.contains("Database: postgresql://prod-db/app_prod (pool: 50)"));
                assert!(rendered.contains("- Performance monitoring: ✓"));
                assert!(rendered.contains("- Ssl required: ✓"));
            }
            _ => panic!("Unexpected version: {}", version_name),
        }

        println!("Real-time configuration update {} verified", version_name);
    }

    // Verify that no caching occurred by ensuring all three contexts are different
    assert_ne!(context1.get("app.version"), context2.get("app.version"));
    assert_ne!(context2.get("app.version"), context3.get("app.version"));
    assert_ne!(
        context1.get("server.workers"),
        context3.get("server.workers")
    );

    println!("Real-time configuration updates workflow completed successfully");
}
