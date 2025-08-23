//! End-to-end integration tests for comprehensive workflow scenarios

mod common;

use common::{ConfigScope, TestEnvironment};
use serial_test::serial;
use swissarmyhammer_config::{ConfigFormat, TemplateRenderer};

#[test]
#[serial]
fn test_complete_template_rendering_workflow() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up comprehensive configuration
    env.write_project_config(
        r#"
project_name = "Template Test Project"
author = "SwissArmyHammer"
version = "1.2.3"
environment = "development"

[deployment]
target = "kubernetes"
namespace = "default"
replicas = 3

[database]
host = "db.example.com"
port = 5432
name = "appdb"
user = "app_user"

[features]
auth_enabled = true
metrics_enabled = false
debug_mode = true
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Set environment variables for dynamic values
    env.set_env_vars([
        ("SAH_BUILD_NUMBER", "1234"),
        ("SAH_GIT_SHA", "abc123def456"),
        ("SAH_DEPLOYMENT__NAMESPACE", "production"),
    ])
    .unwrap();

    let provider = env.create_provider();
    let renderer = TemplateRenderer::new().unwrap();

    // Load the test configuration context
    let context = provider.load_template_context().unwrap();

    // Test basic template rendering using the test context
    let template = "Welcome to {{ project_name }} v{{ version }} by {{ author }}!";
    let result = renderer.render(template, &context).unwrap();
    assert_eq!(
        result,
        "Welcome to Template Test Project v1.2.3 by SwissArmyHammer!"
    );

    // Test nested configuration templating
    let db_template =
        "Database: {{ database.user }}@{{ database.host }}:{{ database.port }}/{{ database.name }}";
    let db_result = renderer.render(db_template, &context).unwrap();
    assert_eq!(db_result, "Database: app_user@db.example.com:5432/appdb");

    // Test environment variable overrides in templates
    let deploy_template =
        "Deploying to {{ deployment.namespace }} with {{ deployment.replicas }} replicas";
    let deploy_result = renderer.render(deploy_template, &context).unwrap();
    assert_eq!(deploy_result, "Deploying to production with 3 replicas");

    // Test conditional rendering based on configuration
    let conditional_template = r#"
{%- if features.auth_enabled -%}
Authentication is enabled
{%- else -%}
Authentication is disabled
{%- endif -%}
"#;
    let conditional_result = renderer
        .render(conditional_template.trim(), &context)
        .unwrap();
    assert_eq!(conditional_result, "Authentication is enabled");

    // Test workflow variable override
    let mut workflow_vars = std::collections::HashMap::new();
    workflow_vars.insert("step".to_string(), serde_json::json!("deployment"));
    workflow_vars.insert("target_env".to_string(), serde_json::json!("staging"));

    // Create context with workflow variables (workflow vars should override config)
    let workflow_context = provider.create_context_with_vars(workflow_vars).unwrap();
    let workflow_template =
        "Step: {{ step }} | Target: {{ target_env }} | Project: {{ project_name }}";
    let workflow_result = renderer
        .render(workflow_template, &workflow_context)
        .unwrap();
    assert_eq!(
        workflow_result,
        "Step: deployment | Target: staging | Project: Template Test Project"
    );
}

#[test]
#[serial]
fn test_configuration_discovery_workflow() {
    let env = TestEnvironment::new().unwrap();

    // Test discovery of multiple configuration files
    env.write_config(
        r#"shared_key = "from_sah_toml""#,
        ConfigFormat::Toml,
        ConfigScope::Project,
        "sah",
    )
    .unwrap();

    env.write_config(
        r#"shared_key: "from_swissarmyhammer_yaml""#,
        ConfigFormat::Yaml,
        common::ConfigScope::Project,
        "swissarmyhammer",
    )
    .unwrap();

    env.write_config(
        r#"{"shared_key": "from_sah_json", "json_only": "json_value"}"#,
        ConfigFormat::Json,
        common::ConfigScope::Project,
        "sah",
    )
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Should discover and merge all configuration files
    assert!(context.get("shared_key").is_some());
    assert!(context.get("json_only").is_some());

    // Verify that configuration system can handle multiple file formats
    let shared_value = context.get("shared_key").unwrap();
    let shared_str = shared_value.as_str().unwrap();
    assert!(
        shared_str == "from_sah_toml"
            || shared_str == "from_swissarmyhammer_yaml"
            || shared_str == "from_sah_json"
    );
}

#[test]
#[serial]
fn test_complex_environment_substitution_workflow() {
    let mut env = TestEnvironment::new().unwrap();

    // Set up complex environment for substitution
    env.set_env_vars([
        ("DEPLOY_ENV", "production"),
        ("DB_HOST", "prod-db.company.com"),
        ("DB_PORT", "5432"),
        ("DB_USER", "prod_user"),
        ("DB_PASS", "secure_password_123"),
        ("API_VERSION", "v2"),
        ("FEATURE_FLAGS", "auth,metrics,logging"),
        ("REPLICA_COUNT", "5"),
        ("MEMORY_LIMIT", "2Gi"),
        ("CPU_LIMIT", "1000m"),
    ])
    .unwrap();

    // Create configuration with comprehensive environment substitution
    let config = r#"
# Environment Configuration
environment = "${DEPLOY_ENV:-development}"
version = "1.0.0"

[database]
host = "${DB_HOST:-localhost}"
port = "${DB_PORT:-5432}"
user = "${DB_USER:-app}"
password = "${DB_PASS}"
connection_string = "postgresql://${DB_USER}:${DB_PASS}@${DB_HOST}:${DB_PORT}/appdb"

[api]
version = "${API_VERSION:-v1}"
base_url = "https://api-${DEPLOY_ENV}.company.com/${API_VERSION}"
features = "${FEATURE_FLAGS}"

[deployment]
replicas = "${REPLICA_COUNT:-3}"
resources = { memory = "${MEMORY_LIMIT:-1Gi}", cpu = "${CPU_LIMIT:-500m}" }

[monitoring]
enabled = true
namespace = "monitoring-${DEPLOY_ENV}"
endpoints = [
    "https://metrics-${DEPLOY_ENV}.company.com",
    "https://logs-${DEPLOY_ENV}.company.com"
]

# Test fallback values
[fallbacks]
missing_with_fallback = "${MISSING_VAR:-fallback_value}"
debug_mode = "${DEBUG:-false}"
log_level = "${LOG_LEVEL:-info}"
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();

    let context = env.load_template_context().unwrap();

    // Verify environment substitution worked correctly
    assert_eq!(context.get_string("environment").unwrap(), "production");

    // Verify complex nested substitution
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("prod-db.company.com".to_string())
        );
        assert_eq!(
            database["user"],
            serde_json::Value::String("prod_user".to_string())
        );
        assert_eq!(
            database["connection_string"],
            serde_json::Value::String(
                "postgresql://prod_user:secure_password_123@prod-db.company.com:5432/appdb"
                    .to_string()
            )
        );
    }

    // Verify API configuration
    if let Some(api) = context.get("api") {
        assert_eq!(
            api["base_url"],
            serde_json::Value::String("https://api-production.company.com/v2".to_string())
        );
        assert_eq!(
            api["features"],
            serde_json::Value::String("auth,metrics,logging".to_string())
        );
    }

    // Verify fallback values work
    if let Some(fallbacks) = context.get("fallbacks") {
        assert_eq!(
            fallbacks["missing_with_fallback"],
            serde_json::Value::String("fallback_value".to_string())
        );
        assert_eq!(
            fallbacks["debug_mode"],
            serde_json::Value::String("false".to_string())
        );
    }
}

#[test]
#[serial]
fn test_configuration_validation_workflow() {
    let env = TestEnvironment::new().unwrap();

    // Test that the configuration system can handle various data types correctly
    let config = r#"
[strings]
simple_string = "hello world"
empty_string = ""
quoted_string = "this has \"quotes\" inside"
multiline_string = """
This is a multiline string
with multiple lines
and special characters: !@#$%^&*()
"""

[numbers]
integer = 42
negative_integer = -17
float_number = 3.14159
scientific_notation = 1.23e-4
zero = 0

[booleans]
true_value = true
false_value = false

[arrays]
string_array = ["one", "two", "three"]
number_array = [1, 2, 3, 4, 5]
mixed_array = ["string", 42, true]
empty_array = []

[nested_objects]
level1 = { key1 = "value1", key2 = { nested_key = "nested_value" } }

[[array_of_objects]]
name = "object1"
value = 100

[[array_of_objects]]
name = "object2"
value = 200

[special_chars]
unicode_string = "Hello 世界 🌍"
symbols = "~!@#$%^&*()_+-={}[]|\\:;\"'<>,.?/"
"#;

    env.write_project_config(config, ConfigFormat::Toml)
        .unwrap();
    let context = env.load_template_context().unwrap();

    // Verify all data types are handled correctly
    if let Some(strings) = context.get("strings") {
        assert_eq!(
            strings["simple_string"],
            serde_json::Value::String("hello world".to_string())
        );
        assert_eq!(
            strings["empty_string"],
            serde_json::Value::String("".to_string())
        );
        // Multiline string should be preserved
        assert!(strings["multiline_string"].is_string());
    }

    if let Some(numbers) = context.get("numbers") {
        assert_eq!(numbers["integer"], serde_json::Value::Number(42.into()));
        assert_eq!(numbers["zero"], serde_json::Value::Number(0.into()));
        assert!(numbers["float_number"].is_number());
    }

    if let Some(booleans) = context.get("booleans") {
        assert_eq!(booleans["true_value"], serde_json::Value::Bool(true));
        assert_eq!(booleans["false_value"], serde_json::Value::Bool(false));
    }

    if let Some(arrays) = context.get("arrays") {
        if let serde_json::Value::Array(string_arr) = &arrays["string_array"] {
            assert_eq!(string_arr.len(), 3);
        }
        if let serde_json::Value::Array(empty_arr) = &arrays["empty_array"] {
            assert_eq!(empty_arr.len(), 0);
        }
    }

    // Verify special characters are handled
    if let Some(special) = context.get("special_chars") {
        assert!(special["unicode_string"].is_string());
        assert!(special["symbols"].is_string());
    }
}

#[test]
#[serial]
fn test_large_scale_configuration_workflow() {
    let env = TestEnvironment::new().unwrap();

    // Create a large-scale configuration that might be used in enterprise scenarios
    let mut large_config = String::from(
        r#"
# Large Scale SwissArmyHammer Configuration
project_name = "Enterprise Application"
version = "2.1.0"
environment = "production"

[infrastructure]
cloud_provider = "aws"
region = "us-east-1"
availability_zones = ["us-east-1a", "us-east-1b", "us-east-1c"]

[database]
primary_host = "prod-db-primary.company.com"
replica_hosts = [
    "prod-db-replica-1.company.com",
    "prod-db-replica-2.company.com",
    "prod-db-replica-3.company.com"
]
port = 5432
name = "enterprise_app"
ssl_mode = "require"

[redis_clusters]
primary = { host = "redis-primary.company.com", port = 6379 }
sessions = { host = "redis-sessions.company.com", port = 6379 }
cache = { host = "redis-cache.company.com", port = 6379 }

"#,
    );

    // Add many service configurations
    for i in 1..=25 {
        large_config.push_str(&format!(
            r#"
[services.service_{:02}]
name = "microservice-{:02}"
port = {}
replicas = {}
memory = "{}Mi"
cpu = "{}m"
health_check = "/health"
version = "1.{}.0"
"#,
            i,
            i,
            8000 + i,
            if i <= 10 { 3 } else { 2 },
            512 + (i * 64),
            200 + (i * 50),
            i % 5
        ));
    }

    // Add feature flags for all services
    large_config.push_str(
        r#"
[feature_flags]
auth_service = true
payment_service = true
notification_service = true
analytics_service = false
experimental_ui = false
new_checkout_flow = true
advanced_search = true
machine_learning = false
"#,
    );

    // Add monitoring configuration
    for env_name in ["production", "staging", "development"] {
        large_config.push_str(&format!(
            r#"
[monitoring.{}]
enabled = true
metrics_endpoint = "https://metrics-{}.company.com"
logs_endpoint = "https://logs-{}.company.com"
alerts_endpoint = "https://alerts-{}.company.com"
retention_days = {}
"#,
            env_name,
            env_name,
            env_name,
            env_name,
            if env_name == "production" { 90 } else { 30 }
        ));
    }

    env.write_project_config(&large_config, ConfigFormat::Toml)
        .unwrap();

    let start_time = std::time::Instant::now();
    let context = env.load_template_context().unwrap();
    let load_time = start_time.elapsed();

    // Verify large configuration loads within reasonable time
    assert!(
        load_time.as_millis() < 1000,
        "Large configuration took {}ms to load, expected < 1000ms",
        load_time.as_millis()
    );

    // Verify all sections are present
    assert!(context.get("infrastructure").is_some());
    assert!(context.get("database").is_some());
    assert!(context.get("redis_clusters").is_some());
    assert!(context.get("services").is_some());
    assert!(context.get("feature_flags").is_some());
    assert!(context.get("monitoring").is_some());

    // Verify specific values from different sections
    assert_eq!(
        context.get_string("project_name").unwrap(),
        "Enterprise Application"
    );

    if let Some(services) = context.get("services") {
        // Should have all 25 services
        if let serde_json::Value::Object(services_map) = services {
            assert_eq!(services_map.len(), 25);
        }
    }

    if let Some(database) = context.get("database") {
        if let serde_json::Value::Array(replicas) = &database["replica_hosts"] {
            assert_eq!(replicas.len(), 3);
        }
    }

    println!(
        "Large configuration (estimated ~{} lines) loaded in {}ms",
        large_config.lines().count(),
        load_time.as_millis()
    );
}

#[test]
#[serial]
fn test_multi_environment_configuration_workflow() {
    let mut env = TestEnvironment::new().unwrap();

    // Create global configuration with defaults
    env.write_global_config(
        r#"
project_name = "Multi-Env Project"
default_timeout = 30
log_level = "info"

[defaults]
database_pool_size = 10
redis_ttl = 3600
api_rate_limit = 100

[shared_services]
auth_service = "https://auth.company.com"
metrics_service = "https://metrics.company.com"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Create environment-specific project configuration
    env.write_project_config(
        r#"
environment = "production"
debug = false
log_level = "warn"

[database]
host = "prod-db.company.com"
port = 5432
pool_size = 50
ssl_required = true

[redis]
host = "prod-redis.company.com"
port = 6379
ttl = 7200

[api]
rate_limit = 1000
timeout = 60

[scaling]
min_replicas = 5
max_replicas = 20
cpu_threshold = 70
memory_threshold = 80

# Environment-specific overrides
[overrides.production]
enable_debug_endpoints = false
enable_profiling = false
log_sampling_rate = 0.1

[overrides.staging]
enable_debug_endpoints = true
enable_profiling = true
log_sampling_rate = 1.0

[overrides.development]
enable_debug_endpoints = true
enable_profiling = true
log_sampling_rate = 1.0
mock_external_services = true
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Set environment-specific variables
    env.set_env_vars([
        ("SAH_ENVIRONMENT", "production"),
        ("SAH_API__RATE_LIMIT", "2000"), // Override config value
        ("SAH_DATABASE__HOST", "prod-primary.company.com"), // Override config value
        ("SAH_SCALING__MIN_REPLICAS", "10"), // Override scaling
    ])
    .unwrap();

    let context = env.load_template_context().unwrap();

    // Verify environment configuration is applied correctly
    assert_eq!(context.get_string("environment").unwrap(), "production");
    assert_eq!(context.get_string("log_level").unwrap(), "warn"); // Project overrides global

    // Verify environment overrides work
    if let Some(api) = context.get("api") {
        // Environment variable should override config
        let rate_limit = &api["rate_limit"];
        match rate_limit {
            serde_json::Value::String(s) => assert_eq!(s, "2000"),
            serde_json::Value::Number(n) => assert_eq!(n, &2000.into()),
            _ => panic!("Unexpected rate_limit type"),
        }
    }

    // Verify nested environment overrides
    if let Some(database) = context.get("database") {
        assert_eq!(
            database["host"],
            serde_json::Value::String("prod-primary.company.com".to_string())
        );
        // Config value should remain for non-overridden keys
        assert_eq!(database["port"], serde_json::Value::Number(5432.into()));
    }

    // Verify global defaults are inherited where not overridden
    assert_eq!(context.get_number("default_timeout").unwrap(), 30.0);

    if let Some(shared_services) = context.get("shared_services") {
        assert!(shared_services.get("auth_service").is_some());
        assert!(shared_services.get("metrics_service").is_some());
    }
}

#[test]
#[serial]
fn test_configuration_hot_reload_simulation() {
    let env = TestEnvironment::new().unwrap();

    // Initial configuration
    env.write_project_config(
        r#"
version = "1.0.0"
feature_enabled = false
max_connections = 10
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    let initial_context = env.load_template_context().unwrap();
    assert_eq!(initial_context.get_string("version").unwrap(), "1.0.0");
    assert_eq!(initial_context.get_bool("feature_enabled").unwrap(), false);
    assert_eq!(initial_context.get_number("max_connections").unwrap(), 10.0);

    // Simulate configuration change (hot reload)
    env.write_project_config(
        r#"
version = "1.1.0"
feature_enabled = true
max_connections = 20
new_feature = "enabled"
"#,
        ConfigFormat::Toml,
    )
    .unwrap();

    // Load configuration again (simulating hot reload)
    let updated_context = env.load_template_context().unwrap();
    assert_eq!(updated_context.get_string("version").unwrap(), "1.1.0");
    assert_eq!(updated_context.get_bool("feature_enabled").unwrap(), true);
    assert_eq!(updated_context.get_number("max_connections").unwrap(), 20.0);
    assert_eq!(
        updated_context.get_string("new_feature").unwrap(),
        "enabled"
    );

    // Verify that configuration system doesn't cache and picks up changes
    assert_ne!(
        initial_context.get_string("version").unwrap(),
        updated_context.get_string("version").unwrap()
    );
}
