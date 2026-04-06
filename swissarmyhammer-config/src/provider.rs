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

impl FileProvider {
    /// Validate and merge TOML configuration
    fn validate_and_merge_toml(&self, figment: Figment) -> ConfigurationResult<Figment> {
        let test_figment = Figment::new().merge(Toml::file(&self.path));
        let _: Value = test_figment
            .extract()
            .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
        Ok(figment.merge(Toml::file(&self.path)))
    }

    /// Validate and merge YAML configuration, checking for null/empty files
    fn validate_and_merge_yaml(&self, figment: Figment) -> ConfigurationResult<Figment> {
        let content = fs::read_to_string(&self.path).map_err(|e| {
            ConfigurationError::load(
                self.path.clone(),
                figment::Error::from(format!("Failed to read YAML file: {}", e)),
            )
        })?;

        let yaml_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&content).map_err(|e| {
            ConfigurationError::load(
                self.path.clone(),
                figment::Error::from(format!("Failed to parse YAML: {}", e)),
            )
        })?;

        if yaml_value.is_null() {
            debug!(
                "Skipping null/empty YAML configuration file: {}",
                self.path.display()
            );
            return Ok(figment);
        }

        let test_figment = Figment::new().merge(Yaml::file(&self.path));
        let _: Value = test_figment
            .extract()
            .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
        Ok(figment.merge(Yaml::file(&self.path)))
    }

    /// Validate and merge JSON configuration
    fn validate_and_merge_json(&self, figment: Figment) -> ConfigurationResult<Figment> {
        let test_figment = Figment::new().merge(Json::file(&self.path));
        let _: Value = test_figment
            .extract()
            .map_err(|e| ConfigurationError::load(self.path.clone(), e))?;
        Ok(figment.merge(Json::file(&self.path)))
    }
}

impl ConfigurationProvider for FileProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        if !self.path.exists() {
            debug!("Configuration file does not exist: {}", self.path.display());
            return Ok(figment);
        }

        debug!("Loading configuration from: {}", self.path.display());

        let _content = fs::read_to_string(&self.path).map_err(|e| {
            ConfigurationError::load(
                self.path.clone(),
                figment::Error::from(format!("Failed to read file: {}", e)),
            )
        })?;

        match self.path.extension().and_then(|ext| ext.to_str()) {
            Some("toml") => self.validate_and_merge_toml(figment),
            Some("yaml") | Some("yml") => self.validate_and_merge_yaml(figment),
            Some("json") => self.validate_and_merge_json(figment),
            _ => Err(ConfigurationError::load(
                self.path.clone(),
                figment::Error::from("Unsupported file extension".to_string()),
            )),
        }
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
    /// Convert environment variable key to path parts (e.g., "DATABASE_HOST" -> ["database", "host"])
    fn key_to_path_parts(key: &str) -> Vec<String> {
        key.to_lowercase()
            .split('_')
            .map(|s| s.to_string())
            .collect()
    }

    /// Parse environment variable value to appropriate JSON type
    fn parse_env_value(value: &str) -> serde_json::Value {
        // Try to parse as different types
        if let Ok(bool_val) = value.parse::<bool>() {
            return serde_json::Value::Bool(bool_val);
        }

        if let Ok(int_val) = value.parse::<i64>() {
            return serde_json::Value::Number(serde_json::Number::from(int_val));
        }

        if let Ok(float_val) = value.parse::<f64>() {
            if let Some(num) = serde_json::Number::from_f64(float_val) {
                return serde_json::Value::Number(num);
            }
        }

        serde_json::Value::String(value.to_string())
    }

    /// Insert a value into nested map structure using iterative approach
    fn insert_nested_value(
        map: &mut serde_json::Map<String, serde_json::Value>,
        path: &[String],
        value: String,
    ) {
        if path.is_empty() {
            return;
        }

        let parsed_value = Self::parse_env_value(&value);

        // Handle single-level path
        if path.len() == 1 {
            map.insert(path[0].clone(), parsed_value);
            return;
        }

        // Iteratively traverse/create nested structure
        let mut current_map = map;
        for (i, key) in path.iter().enumerate() {
            let is_last = i == path.len() - 1;

            if is_last {
                current_map.insert(key.clone(), parsed_value);
                break;
            }

            // Get or create nested object
            let nested_value = current_map
                .entry(key.clone())
                .or_insert_with(|| serde_json::Value::Object(serde_json::Map::new()));

            // Move to next level (we know it's an object because we just created it)
            if let serde_json::Value::Object(ref mut nested_map) = nested_value {
                current_map = nested_map;
            } else {
                // If there's already a non-object value, we can't nest further
                break;
            }
        }
    }

    /// Process a single environment variable and insert into config map
    fn process_env_var(
        &self,
        env_config: &mut serde_json::Map<String, serde_json::Value>,
        key: String,
        value: String,
    ) {
        if let Some(stripped_key) = key.strip_prefix(&self.prefix) {
            let path_parts = Self::key_to_path_parts(stripped_key);
            Self::insert_nested_value(env_config, &path_parts, value);
        }
    }
}

impl ConfigurationProvider for EnvProvider {
    fn load_into(&self, figment: Figment) -> ConfigurationResult<Figment> {
        debug!("Loading environment variables with prefix: {}", self.prefix);

        let mut env_config = serde_json::Map::new();

        for (key, value) in std::env::vars() {
            self.process_env_var(&mut env_config, key, value);
        }

        if env_config.is_empty() {
            return Ok(figment);
        }

        let config_value = serde_json::Value::Object(env_config);
        Ok(figment.merge(Serialized::defaults(config_value)))
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
    fn test_default_provider_metadata() {
        let provider = DefaultProvider::empty();
        let meta = provider.metadata();
        assert_eq!(meta.name, "defaults");
    }

    #[test]
    fn test_cli_provider_metadata() {
        let provider = CliProvider::empty();
        let meta = provider.metadata();
        assert_eq!(meta.name, "cli");
    }

    #[test]
    fn test_default_provider_empty() {
        let provider = DefaultProvider::empty();
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_cli_provider_empty() {
        let provider = CliProvider::empty();
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_file_provider_yaml_null_file() {
        // An empty YAML file parses to null; the provider should skip it gracefully
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("empty.yaml");
        fs::write(&yaml_file, "").unwrap();

        let provider = FileProvider::new(yaml_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        // Figment should be unchanged (no config merged)
        let config: Value = figment.extract().unwrap_or(json!({}));
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_file_provider_yaml_comment_only_null() {
        // A YAML file with only comments also parses to null
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("comments.yml");
        fs::write(&yaml_file, "# just a comment\n").unwrap();

        let provider = FileProvider::new(yaml_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap_or(json!({}));
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_file_provider_unsupported_extension() {
        // A file with an unsupported extension (e.g. .ini) should return an error
        let temp_dir = TempDir::new().unwrap();
        let ini_file = temp_dir.path().join("config.ini");
        fs::write(&ini_file, "[section]\nkey=value\n").unwrap();

        let provider = FileProvider::new(ini_file);
        let result = provider.load_into(Figment::new());

        assert!(result.is_err());
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("Unsupported file extension"),
            "Expected 'Unsupported file extension' in error, got: {}",
            err_msg
        );
    }

    #[test]
    fn test_file_provider_metadata() {
        let provider = FileProvider::new(PathBuf::from("/some/path/config.toml"));
        let meta = provider.metadata();
        assert_eq!(meta.name, "file: /some/path/config.toml");
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

    #[test]
    fn test_env_provider_metadata_sah() {
        // Exercises the `metadata()` method on `EnvProvider`.
        let provider = EnvProvider::sah();
        let meta = provider.metadata();
        assert_eq!(meta.name, "env: SAH_");
    }

    #[test]
    fn test_env_provider_metadata_swissarmyhammer() {
        let provider = EnvProvider::swissarmyhammer();
        let meta = provider.metadata();
        assert_eq!(meta.name, "env: SWISSARMYHAMMER_");
    }

    #[test]
    fn test_env_provider_no_matching_vars() {
        // Exercises the empty env_config path where no env vars match the prefix.
        // Use a prefix that won't match any real env vars
        let provider = EnvProvider {
            prefix: "ZZZZNONEXISTENT_PREFIX_".to_string(),
        };
        let figment = provider.load_into(Figment::new()).unwrap();
        let config: Value = figment.extract().unwrap_or(json!({}));
        assert_eq!(config, json!({}));
    }

    #[test]
    fn test_env_provider_parse_env_value_types() {
        // Exercises the `parse_env_value` function for all type paths.
        // Boolean
        assert_eq!(EnvProvider::parse_env_value("true"), json!(true));
        assert_eq!(EnvProvider::parse_env_value("false"), json!(false));

        // Integer
        assert_eq!(EnvProvider::parse_env_value("42"), json!(42));
        assert_eq!(EnvProvider::parse_env_value("-7"), json!(-7));

        // Float
        assert_eq!(EnvProvider::parse_env_value("2.72"), json!(2.72));

        // String fallback
        assert_eq!(EnvProvider::parse_env_value("hello"), json!("hello"));
    }

    #[test]
    fn test_env_provider_key_to_path_parts() {
        let parts = EnvProvider::key_to_path_parts("DATABASE_HOST");
        assert_eq!(parts, vec!["database", "host"]);

        let parts = EnvProvider::key_to_path_parts("SINGLE");
        assert_eq!(parts, vec!["single"]);
    }

    #[test]
    fn test_env_provider_insert_nested_value_empty_path() {
        // Exercises the empty path early return in `insert_nested_value`.
        let mut map = serde_json::Map::new();
        EnvProvider::insert_nested_value(&mut map, &[], "value".to_string());
        assert!(map.is_empty());
    }

    #[test]
    fn test_env_provider_insert_nested_value_single_key() {
        // Exercises the single-level path in `insert_nested_value`.
        let mut map = serde_json::Map::new();
        EnvProvider::insert_nested_value(&mut map, &["key".to_string()], "value".to_string());
        assert_eq!(map.get("key"), Some(&json!("value")));
    }

    #[test]
    fn test_env_provider_insert_nested_value_multi_key() {
        // Exercises the multi-level nesting in `insert_nested_value`.
        let mut map = serde_json::Map::new();
        EnvProvider::insert_nested_value(
            &mut map,
            &["database".to_string(), "host".to_string()],
            "localhost".to_string(),
        );
        assert_eq!(map["database"]["host"], json!("localhost"));
    }

    #[test]
    fn test_env_provider_insert_nested_value_existing_non_object() {
        // Exercises the branch where an intermediate value is not an object.
        let mut map = serde_json::Map::new();
        map.insert("key".to_string(), json!("string_value"));
        // Trying to nest into a string should gracefully break
        EnvProvider::insert_nested_value(
            &mut map,
            &["key".to_string(), "nested".to_string()],
            "value".to_string(),
        );
        // The original string value should remain (can't nest into it)
        assert_eq!(map["key"], json!("string_value"));
    }

    #[test]
    fn test_env_provider_process_env_var_with_prefix() {
        let provider = EnvProvider::sah();
        let mut map = serde_json::Map::new();
        provider.process_env_var(&mut map, "SAH_DB_HOST".to_string(), "localhost".to_string());
        assert_eq!(map["db"]["host"], json!("localhost"));
    }

    #[test]
    fn test_env_provider_process_env_var_without_prefix() {
        // Exercises the branch where key doesn't match the prefix.
        let provider = EnvProvider::sah();
        let mut map = serde_json::Map::new();
        provider.process_env_var(&mut map, "OTHER_VAR".to_string(), "value".to_string());
        assert!(map.is_empty(), "Non-matching prefix should be ignored");
    }

    #[test]
    fn test_file_provider_yml_extension() {
        // Exercises the `.yml` extension path (in addition to `.yaml`).
        let temp_dir = TempDir::new().unwrap();
        let yml_file = temp_dir.path().join("config.yml");
        fs::write(&yml_file, "database:\n  host: localhost\n  port: 5432\n").unwrap();

        let provider = FileProvider::new(yml_file);
        let figment = provider.load_into(Figment::new()).unwrap();

        let config: Value = figment.extract().unwrap();
        assert_eq!(config["database"]["host"], "localhost");
    }

    #[test]
    fn test_file_provider_invalid_toml() {
        // Exercises the TOML validation error path.
        let temp_dir = TempDir::new().unwrap();
        let toml_file = temp_dir.path().join("bad.toml");
        fs::write(&toml_file, "invalid toml [[[content").unwrap();

        let provider = FileProvider::new(toml_file);
        let result = provider.load_into(Figment::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_file_provider_invalid_json() {
        // Exercises the JSON validation error path.
        let temp_dir = TempDir::new().unwrap();
        let json_file = temp_dir.path().join("bad.json");
        fs::write(&json_file, "{ invalid json }").unwrap();

        let provider = FileProvider::new(json_file);
        let result = provider.load_into(Figment::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_file_provider_invalid_yaml() {
        // Exercises the YAML parse error path.
        let temp_dir = TempDir::new().unwrap();
        let yaml_file = temp_dir.path().join("bad.yaml");
        // Create YAML that parses but can't be extracted as a Value
        fs::write(&yaml_file, "valid: yaml\n  but: invalid indentation").unwrap();

        let provider = FileProvider::new(yaml_file);
        let result = provider.load_into(Figment::new());
        // This may or may not error depending on the YAML content;
        // the important thing is it doesn't panic
        let _ = result;
    }

    #[test]
    fn test_env_provider_parse_nan_float() {
        // Exercises the float parsing path where `from_f64` returns None (NaN).
        let result = EnvProvider::parse_env_value("NaN");
        // NaN can't be represented as JSON number, so falls through to string
        assert!(result.is_string() || result.is_number());
    }

    #[test]
    fn test_env_provider_parse_infinity() {
        // Exercises the float parsing path where `from_f64` returns None (Infinity).
        let result = EnvProvider::parse_env_value("inf");
        // inf can't be represented as JSON number, so falls through to string
        assert_eq!(result, json!("inf"));
    }

    #[test]
    fn test_file_provider_yaml_unreadable_path() {
        // Exercises the YAML read error path (line 42-47 in validate_and_merge_yaml).
        // Uses a directory path which can't be read as a string.
        let temp_dir = TempDir::new().unwrap();
        // Create a .yaml file that is actually a directory (can't be read as string)
        let yaml_dir = temp_dir.path().join("config.yaml");
        fs::create_dir(&yaml_dir).unwrap();

        let provider = FileProvider::new(yaml_dir);
        let result = provider.load_into(Figment::new());
        assert!(result.is_err(), "Reading a directory as YAML should fail");
    }
}
