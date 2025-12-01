use crate::error::{ConfigurationError, ConfigurationResult};
use figment::providers::{Format, Json, Serialized, Toml, Yaml};
use figment::{Figment, Metadata};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;
use tracing::debug;

/// Trait for configuration providers
pub trait ConfigurationProvider {
    /// Load configuration and merge with the provided figment
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment>;

    /// Get metadata about this provider
    fn metadata(&self) -> Metadata;
}

/// File-based configuration provider
pub struct FileProvider {
    path: PathBuf,
}

impl FileProvider {
    /// Create a new file provider
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl ConfigurationProvider for FileProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        if !self.path.exists() {
            debug!("Configuration file does not exist: {}", self.path.display());
            return Ok(figment);
        }

        debug!("Loading configuration from: {}", self.path.display());

        // Read file to ensure it's readable (unused but validates file access)
        let _content = fs::read_to_string(&self.path).map_err(|e| {
            ConfigurationError::load(
                self.path.clone(),
                figment::Error::from(format!("Failed to read file: {}", e)),
            )
        })?;

        // Validate parsing based on file extension before merging
        let figment = match self.path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => {
                // Validate TOML parsing by attempting to merge and extract immediately
                let test_figment = Figment::new().merge(Toml::file(&self.path));
                let _: Value = test_figment
                    .extract()
                    .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
                figment.merge(Toml::file(&self.path))
            }
            Some("yaml") | Some("yml") => {
                // Check if YAML file contains only null before attempting to merge
                let content = fs::read_to_string(&self.path).map_err(|e| {
                    ConfigurationError::load(
                        self.path.clone(),
                        figment::Error::from(format!("Failed to read YAML file: {}", e)),
                    )
                })?;

                // Parse YAML to check if it's null
                let yaml_value: serde_yaml::Value =
                    serde_yaml::from_str(&content).map_err(|e| {
                        ConfigurationError::load(
                            self.path.clone(),
                            figment::Error::from(format!("Failed to parse YAML: {}", e)),
                        )
                    })?;

                if yaml_value.is_null() {
                    // Skip null/empty YAML files
                    debug!(
                        "Skipping null/empty YAML configuration file: {}",
                        self.path.display()
                    );
                    return Ok(figment);
                }

                // Validate YAML parsing by attempting to merge and extract immediately
                let test_figment = Figment::new().merge(Yaml::file(&self.path));
                let _: Value = test_figment
                    .extract()
                    .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
                figment.merge(Yaml::file(&self.path))
            }
            Some("json") => {
                // Validate JSON parsing by attempting to merge and extract immediately
                let test_figment = Figment::new().merge(Json::file(&self.path));
                let _: Value = test_figment
                    .extract()
                    .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
                figment.merge(Json::file(&self.path))
            }
            _ => {
                return Err(ConfigurationError::load(
                    self.path.clone(),
                    figment::Error::from("Unsupported file extension".to_string()),
                ));
            }
        };

        Ok(figment)
    }

    fn metadata(&self) -> Metadata {
        Metadata::named(format!("file: {}", self.path.display()))
    }
}

/// Environment variable configuration provider
pub struct EnvProvider {
    prefix: String,
}

impl EnvProvider {
    /// Create a new environment provider with SAH_ prefix
    pub fn sah() -> Self {
        Self {
            prefix: "SAH_".to_string(),
        }
    }

    /// Create a new environment provider with SWISSARMYHAMMER_ prefix
    pub fn swissarmyhammer() -> Self {
        Self {
            prefix: "SWISSARMYHAMMER_".to_string(),
        }
    }
}

impl EnvProvider {
    fn insert_nested_value(
        map: &mut serde_json::Map<String, serde_json::Value>,
        path: &[String],
        value: String,
    ) {
        if path.is_empty() {
            return;
        }

        if path.len() == 1 {
            // Base case: insert the value
            let parsed_value = Self::parse_env_value(&value);
            map.insert(path[0].clone(), parsed_value);
        } else {
            // Recursive case: create or get the nested object
            let key = &path[0];
            let nested_map = map
                .entry(key.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            if let serde_json::Value::Object(ref mut nested) = nested_map {
                Self::insert_nested_value(nested, &path[1..], value);
            }
        }
    }

    fn parse_env_value(value: &str) -> serde_json::Value {
        // Try to parse as different types
        if let Ok(bool_val) = value.parse::<bool>() {
            serde_json::Value::Bool(bool_val)
        } else if let Ok(int_val) = value.parse::<i64>() {
            serde_json::Value::Number(serde_json::Number::from(int_val))
        } else if let Ok(float_val) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(float_val) {
                serde_json::Value::Number(num)
            } else {
                serde_json::Value::String(value.to_string())
            }
        } else {
            serde_json::Value::String(value.to_string())
        }
    }
}

impl ConfigurationProvider for EnvProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        debug!("Loading environment variables with prefix: {}", self.prefix);

        let mut env_config = serde_json::Map::new();

        for (key, value) in std::env::vars() {
            if let Some(stripped_key) = key.strip_prefix(&self.prefix) {
                // Convert UPPER_CASE to lower case and split by underscores
                let path_parts: Vec<String> = stripped_key
                    .to_lowercase()
                    .split('_')
                    .map(|s| s.to_string())
                    .collect();

                // Insert the value into the nested structure
                Self::insert_nested_value(&mut env_config, &path_parts, value);
            }
        }

        if !env_config.is_empty() {
            let config_value = serde_json::Value::Object(env_config);
            Ok(figment.merge(Serialized::defaults(config_value)))
        } else {
            Ok(figment)
        }
    }

    fn metadata(&self) -> Metadata {
        Metadata::named(format!("env: {}", self.prefix))
    }
}

/// Default values configuration provider
pub struct DefaultProvider {
    values: Value,
}

impl DefaultProvider {
    /// Create a new default provider with the given values
    pub fn new(values: Value) -> Self {
        Self { values }
    }

    /// Create a default provider with empty configuration
    pub fn empty() -> Self {
        Self {
            values: Value::Object(serde_json::Map::new()),
        }
    }
}

impl ConfigurationProvider for DefaultProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        debug!("Loading default configuration values");

        // Use figment's Serialized provider to merge default values
        Ok(figment.merge(Serialized::defaults(&self.values)))
    }

    fn metadata(&self) -> Metadata {
        Metadata::named("defaults")
    }
}

/// CLI arguments configuration provider
pub struct CliProvider {
    values: Value,
}

impl CliProvider {
    /// Create a new CLI provider with the given values
    pub fn new(values: Value) -> Self {
        Self { values }
    }

    /// Create a CLI provider with empty configuration
    pub fn empty() -> Self {
        Self {
            values: Value::Object(serde_json::Map::new()),
        }
    }
}

impl ConfigurationProvider for CliProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        debug!("Loading CLI argument overrides");

        // CLI args have the highest priority, so they come last
        Ok(figment.merge(Serialized::defaults(&self.values)))
    }

    fn metadata(&self) -> Metadata {
        Metadata::named("cli")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_file_provider_toml() {
        let temp_dir = TempDir::new().unwrap();
        let toml_file = temp_dir.path().join("config.toml");
        fs::write(
            &toml_file,
            r#"
[database]
host = "localhost"
port = 5432
        "#,
        )
        .unwrap();

        let provider = FileProvider::new(toml_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "localhost");
        assert_eq!(config["database"]["port"], 5432);
    }

    #[test]
    fn test_file_provider_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("config.yaml");
        fs::write(
            &yaml_file,
            r#"
database:
  host: localhost
  port: 5432
        "#,
        )
        .unwrap();

        let provider = FileProvider::new(yaml_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "localhost");
        assert_eq!(config["database"]["port"], 5432);
    }

    #[test]
    fn test_file_provider_json() {
        let temp_dir = TempDir::new().unwrap();
        let json_file = temp_dir.path().join("config.json");
        fs::write(
            &json_file,
            r#"
{
  "database": {
    "host": "localhost",
    "port": 5432
  }
}
        "#,
        )
        .unwrap();

        let provider = FileProvider::new(json_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "localhost");
        assert_eq!(config["database"]["port"], 5432);
    }

    #[test]
    fn test_file_provider_nonexistent() {
        let provider = FileProvider::new(PathBuf::from("nonexistent.toml"));
        let figment = provider.load_into(Figment::new()).unwrap();

        // Should succeed but not add any configuration
        let config: Value = figment.extract().unwrap_or(json!({}));
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_env_provider_sah() {
        env::set_var("SAH_DATABASE_HOST", "env-host");
        env::set_var("SAH_DATABASE_PORT", "3306");

        let provider = EnvProvider::sah();
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "env-host");
        assert_eq!(config["database"]["port"], 3306);

        env::remove_var("SAH_DATABASE_HOST");
        env::remove_var("SAH_DATABASE_PORT");
    }

    #[test]
    fn test_env_provider_swissarmyhammer() {
        env::set_var("SWISSARMYHAMMER_API_KEY", "secret-key");
        env::set_var("SWISSARMYHAMMER_DEBUG_MODE", "true");

        let provider = EnvProvider::swissarmyhammer();
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["api"]["key"], "secret-key");
        assert_eq!(config["debug"]["mode"], true);

        env::remove_var("SWISSARMYHAMMER_API_KEY");
        env::remove_var("SWISSARMYHAMMER_DEBUG_MODE");
    }

    #[test]
    fn test_default_provider() {
        let defaults = json!({
            "database": {
                "host": "localhost",
                "port": 5432
            },
            "debug": false
        });

        let provider = DefaultProvider::new(defaults);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "localhost");
        assert_eq!(config["database"]["port"], 5432);
        assert_eq!(config["debug"], false);
    }

    #[test]
    fn test_cli_provider() {
        let cli_overrides = json!({
            "database": {
                "host": "cli-host"
            },
            "debug": true
        });

        let provider = CliProvider::new(cli_overrides);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "cli-host");
        assert_eq!(config["debug"], true);
    }

    #[test]
    fn test_provider_precedence() {
        // Test that providers merge correctly with proper precedence
        let defaults = json!({"key": "default", "only_default": "default_value"});
        let cli_override = json!({"key": "cli"});

        let figment = Figment::new();
        let figment = DefaultProvider::new(defaults).load_into(figment).unwrap();
        let figment = CliProvider::new(cli_override).load_into(figment).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["key"], "cli"); // CLI should override default
        assert_eq!(config["only_default"], "default_value"); // Default should remain
    }
}
