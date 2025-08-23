//! Test environment utilities for comprehensive integration testing

#![allow(dead_code)] // Test utilities may not all be used in current tests

use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use swissarmyhammer_config::{ConfigFormat, ConfigProvider, ConfigResult, TemplateContext};
use swissarmyhammer_test_utils::IsolatedTestEnvironment;

/// Comprehensive test environment for integration testing
///
/// Provides isolated test environments with proper cleanup and realistic configuration scenarios
/// This extends the base IsolatedTestEnvironment with config-specific functionality.
pub struct TestEnvironment {
    // Use the shared isolated test environment for HOME isolation
    isolated_env: IsolatedTestEnvironment,
    project_dir: PathBuf,
    global_config_dir: PathBuf,
    project_config_dir: PathBuf,
    original_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Global,
    Project,
}

impl TestEnvironment {
    /// Create a new isolated test environment
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // Use the shared isolated test environment for HOME isolation
        let isolated_env = IsolatedTestEnvironment::new()?;
        let original_dir = std::env::current_dir()?;

        // Create realistic directory structure in the isolated home
        let home_path = isolated_env.home_path();
        let project_dir = home_path.join("test-project");
        let global_config_dir = isolated_env.swissarmyhammer_dir();
        let project_config_dir = project_dir.join(".swissarmyhammer");

        fs::create_dir_all(&project_dir)?;
        fs::create_dir_all(&project_config_dir)?;

        // Set current directory to the project directory for config-specific tests
        std::env::set_current_dir(&project_dir)?;

        Ok(TestEnvironment {
            isolated_env,
            project_dir,
            global_config_dir,
            project_config_dir,
            original_dir,
        })
    }

    /// Write a configuration file to the specified scope with the given content and format
    pub fn write_config<S: AsRef<str>>(
        &self,
        content: S,
        format: ConfigFormat,
        scope: ConfigScope,
        name_variant: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = match scope {
            ConfigScope::Global => &self.global_config_dir,
            ConfigScope::Project => &self.project_config_dir,
        };

        let filename = match format {
            ConfigFormat::Toml => format!("{}.toml", name_variant),
            ConfigFormat::Yaml => format!("{}.yaml", name_variant),
            ConfigFormat::Json => format!("{}.json", name_variant),
        };

        let config_path = config_dir.join(filename);
        fs::write(&config_path, content.as_ref())?;
        Ok(config_path)
    }

    /// Write a global configuration file (convenience method)
    #[allow(dead_code)]
    pub fn write_global_config<S: AsRef<str>>(
        &self,
        content: S,
        format: ConfigFormat,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.write_config(content, format, ConfigScope::Global, "sah")
    }

    /// Write a project configuration file (convenience method)
    pub fn write_project_config<S: AsRef<str>>(
        &self,
        content: S,
        format: ConfigFormat,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        self.write_config(content, format, ConfigScope::Project, "sah")
    }

    /// Set an environment variable and remember original value for restoration
    pub fn set_env_var<K: AsRef<str>, V: AsRef<str>>(
        &mut self,
        key: K,
        value: V,
    ) -> Result<(), Box<dyn std::error::Error>> {
        (&mut self.isolated_env).set_env_var(key, value)
    }

    /// Set multiple environment variables at once
    pub fn set_env_vars<I, K, V>(&mut self, vars: I) -> Result<(), Box<dyn std::error::Error>>
    where
        I: IntoIterator<Item = (K, V)>,
        K: AsRef<str>,
        V: AsRef<str>,
    {
        (&mut self.isolated_env).set_env_vars(vars)
    }

    /// Create a ConfigProvider using the test environment
    pub fn create_provider(&self) -> ConfigProvider {
        ConfigProvider::new()
    }

    /// Load template context using the test environment
    pub fn load_template_context(&self) -> ConfigResult<TemplateContext> {
        let provider = self.create_provider();
        provider.load_template_context()
    }

    /// Load template context with strict error handling
    #[allow(dead_code)]
    pub fn load_template_context_strict(&self) -> ConfigResult<TemplateContext> {
        let provider = self.create_provider();
        provider.load_template_context_strict()
    }

    /// Create sample configuration data for testing
    pub fn create_sample_toml_config() -> String {
        r#"
# SwissArmyHammer Test Configuration
project_name = "Integration Test Project"
environment = "test"
debug = true
version = "1.0.0"

[database]
host = "localhost"
port = 5432
name = "testdb"
ssl = true

[database.pool]
min_connections = 5
max_connections = 20
timeout_seconds = 30

[logging]
level = "debug"
format = "json"
file = "/tmp/test.log"

[features]
workflows = true
prompts = true
mcp = false

[[services]]
name = "api"
port = 8080
enabled = true

[[services]]
name = "worker"
port = 8081
enabled = false
"#
        .to_string()
    }

    /// Create sample YAML configuration data
    #[allow(dead_code)]
    pub fn create_sample_yaml_config() -> String {
        r#"
# SwissArmyHammer YAML Test Configuration
project_name: "YAML Integration Test"
environment: "yaml_test"
debug: false
version: "2.0.0"

database:
  host: "yaml-db.example.com"
  port: 3306
  name: "yamldb"
  options:
    - "charset=utf8mb4"
    - "timeout=30"

api:
  version: "v2"
  base_url: "https://api.example.com"
  endpoints:
    users: "/users"
    auth: "/auth"
    health: "/health"
  rate_limits:
    per_minute: 100
    per_hour: 1000

features:
  - "workflows"
  - "templates"
  - "yaml_support"

environments:
  development:
    debug: true
    log_level: "debug"
  production:
    debug: false
    log_level: "info"
"#
        .to_string()
    }

    /// Create sample JSON configuration data
    #[allow(dead_code)]
    pub fn create_sample_json_config() -> String {
        serde_json::to_string_pretty(&json!({
            "project_name": "JSON Integration Test",
            "environment": "json_test",
            "debug": true,
            "version": "3.0.0",
            "database": {
                "host": "json-db.example.com",
                "port": 5432,
                "credentials": {
                    "username": "app_user",
                    "password_env": "${DB_PASSWORD:-default_password}"
                },
                "pools": [
                    {
                        "name": "read",
                        "size": 5,
                        "timeout": 30
                    },
                    {
                        "name": "write",
                        "size": 2,
                        "timeout": 10
                    }
                ]
            },
            "cache": {
                "type": "redis",
                "host": "localhost",
                "port": 6379,
                "ttl": 3600
            },
            "features": [
                "json_support",
                "caching",
                "authentication"
            ],
            "metadata": {
                "created_at": "2024-01-01T00:00:00Z",
                "config_version": 1.2
            }
        }))
        .unwrap()
    }

    /// Create complex nested configuration for precedence testing
    #[allow(dead_code)]
    pub fn create_complex_nested_config() -> String {
        r#"
[server]
host = "0.0.0.0"
port = 8080
timeout = 30

[server.ssl]
enabled = true
cert_path = "/etc/ssl/cert.pem"
key_path = "/etc/ssl/key.pem"

[server.ssl.options]
protocols = ["TLSv1.2", "TLSv1.3"]
ciphers = "HIGH:!aNULL:!MD5"

[database]
primary_url = "postgresql://localhost:5432/main"
replica_url = "postgresql://localhost:5433/main"

[database.pool]
min_connections = 5
max_connections = 20
acquire_timeout = 30
idle_timeout = 300

[database.migrations]
enabled = true
directory = "./migrations"
table = "schema_migrations"

[cache]
type = "redis"
url = "redis://localhost:6379"

[cache.settings]
default_ttl = 3600
max_memory = "1gb"
eviction_policy = "allkeys-lru"

[features]
feature_a = true
feature_b = false

[features.experimental]
new_ui = false
beta_api = true
advanced_caching = true

[monitoring]
enabled = true
metrics_port = 9090

[monitoring.tracing]
enabled = true
endpoint = "http://jaeger:14268"
sample_rate = 0.1
"#
        .to_string()
    }

    /// Get the path to the temporary directory
    #[allow(dead_code)]
    pub fn temp_path(&self) -> &Path {
        self.isolated_env.temp_dir()
    }

    /// Get the path to the project directory
    pub fn project_path(&self) -> &Path {
        &self.project_dir
    }

    /// Get the path to the global config directory
    pub fn global_config_path(&self) -> &Path {
        &self.global_config_dir
    }

    /// Get the path to the project config directory
    pub fn project_config_path(&self) -> &Path {
        &self.project_config_dir
    }

    /// Write a configuration file with a custom filename
    #[allow(dead_code)]
    pub fn write_config_with_filename<S: AsRef<str>>(
        &self,
        content: S,
        _format: ConfigFormat,
        scope: ConfigScope,
        filename: &str,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let config_dir = match scope {
            ConfigScope::Global => &self.global_config_dir,
            ConfigScope::Project => &self.project_config_dir,
        };

        let config_path = config_dir.join(filename);
        fs::write(&config_path, content.as_ref())?;
        Ok(config_path)
    }

    /// Clean up a configuration file with a custom filename
    #[allow(dead_code)]
    pub fn cleanup_config_with_filename(
        &self,
        scope: ConfigScope,
        filename: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_dir = match scope {
            ConfigScope::Global => &self.global_config_dir,
            ConfigScope::Project => &self.project_config_dir,
        };

        let config_path = config_dir.join(filename);
        if config_path.exists() {
            fs::remove_file(config_path)?;
        }
        Ok(())
    }

    /// Test configuration compatibility by loading and validating a config string
    #[allow(dead_code)]
    pub fn test_config_compatibility<S: AsRef<str>>(
        &self,
        config_content: S,
        format: ConfigFormat,
    ) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
        // Write temporary config
        let temp_path = self.write_project_config(config_content, format)?;

        // Load and validate
        let context = self.load_template_context()?;
        let json_value = context.to_json();

        // Clean up temp file
        fs::remove_file(temp_path)?;

        Ok(json_value)
    }
}

impl Drop for TestEnvironment {
    /// Clean up the test environment by restoring original directory
    /// (HOME and environment variables are handled by the IsolatedTestEnvironment)
    fn drop(&mut self) {
        // Restore original working directory
        let _ = std::env::set_current_dir(&self.original_dir);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_creation() {
        let env = TestEnvironment::new().unwrap();
        assert!(env.project_path().exists());
        assert!(env.global_config_path().exists());
        assert!(env.project_config_path().exists());
    }

    #[test]
    fn test_config_file_creation() {
        let env = TestEnvironment::new().unwrap();

        let config_content = TestEnvironment::create_sample_toml_config();
        let path = env
            .write_project_config(&config_content, ConfigFormat::Toml)
            .unwrap();

        assert!(path.exists());
        let written_content = fs::read_to_string(path).unwrap();
        assert!(written_content.contains("Integration Test Project"));
    }

    #[test]
    fn test_environment_variables() {
        let mut env = TestEnvironment::new().unwrap();

        env.set_env_var("TEST_VAR", "test_value").unwrap();
        assert_eq!(std::env::var("TEST_VAR").unwrap(), "test_value");

        // Variable should be restored when TestEnvironment is dropped
    }
}
