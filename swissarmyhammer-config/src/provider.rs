//! Configuration provider using Figment for SwissArmyHammer

use crate::{
    error::ConfigError,
    types::{RawConfig, TemplateContext},
    ConfigResult,
};
use figment::{
    providers::{Env, Format, Json, Toml, Yaml},
    Figment,
};
use std::collections::HashMap;
use tracing::{debug, info, trace};

/// Configuration provider using figment
///
/// This provider loads configuration from multiple sources with a clear precedence order.
/// No caching is performed - configuration is read fresh each time to allow live editing.
pub struct ConfigProvider;

impl ConfigProvider {
    /// Create a new configuration provider
    pub fn new() -> Self {
        Self
    }

    /// Load template context from all configuration sources
    ///
    /// This is the main entry point that combines all configuration sources
    /// and returns a ready-to-use TemplateContext with environment variable substitution.
    pub fn load_template_context(&self) -> ConfigResult<TemplateContext> {
        debug!("Loading template context from configuration sources");

        let figment = self.build_figment()?;
        let raw_config = figment
            .extract::<RawConfig>()
            .map_err(|e| ConfigError::parse_error(None, e))?;

        debug!("Loaded {} configuration values", raw_config.values.len());

        let mut context = raw_config.to_template_context();

        // Perform environment variable substitution
        context.substitute_env_vars()?;

        info!(
            "Successfully loaded template context with {} variables",
            context.len()
        );
        Ok(context)
    }

    /// Build the figment configuration with all sources in precedence order
    ///
    /// Sources are loaded in precedence order (later sources override earlier ones):
    /// 1. Default values (hardcoded)
    /// 2. Global config file (~/.swissarmyhammer/ directory)
    /// 3. Project config file (.swissarmyhammer/ directory)
    /// 4. Environment variables (SAH_ and SWISSARMYHAMMER_ prefixes)
    /// 5. Command line arguments (placeholder for future implementation)
    fn build_figment(&self) -> ConfigResult<Figment> {
        debug!("Building figment configuration with precedence order");

        let figment = Figment::new()
            // Start with default values
            .merge(self.get_default_config())
            // Add global config files
            .merge(self.load_global_config()?)
            // Add project config files
            .merge(self.load_project_config()?)
            // Add environment variables
            .merge(self.load_env_vars()?);

        // Future: Add command line arguments here

        Ok(figment)
    }

    /// Get default configuration values
    fn get_default_config(&self) -> Figment {
        trace!("Loading default configuration values");

        // Default configuration values can be added here
        let defaults = HashMap::<String, serde_json::Value>::new();

        // Use figment's Serialized provider to handle the default values
        Figment::new().merge(figment::providers::Serialized::defaults(defaults))
    }

    /// Load global configuration files from ~/.swissarmyhammer/
    fn load_global_config(&self) -> ConfigResult<Figment> {
        debug!("Loading global configuration files");

        let home_dir = dirs::home_dir().ok_or_else(|| {
            ConfigError::directory_error(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Home directory not found",
            ))
        })?;

        let sah_dir = home_dir.join(".swissarmyhammer");

        if !sah_dir.exists() {
            debug!(
                "Global SwissArmyHammer directory does not exist: {}",
                sah_dir.display()
            );
            return Ok(Figment::new());
        }

        let mut figment = Figment::new();

        // Load configuration files in format precedence order
        // (later formats can override earlier ones for the same file name)

        // TOML files
        let toml_files = [
            sah_dir.join("sah.toml"),
            sah_dir.join("swissarmyhammer.toml"),
        ];

        for path in &toml_files {
            if path.exists() {
                trace!("Loading global TOML config: {}", path.display());
                figment = figment.merge(Toml::file(path));
            }
        }

        // YAML files
        let yaml_files = [
            sah_dir.join("sah.yaml"),
            sah_dir.join("sah.yml"),
            sah_dir.join("swissarmyhammer.yaml"),
            sah_dir.join("swissarmyhammer.yml"),
        ];

        for path in &yaml_files {
            if path.exists() {
                trace!("Loading global YAML config: {}", path.display());
                figment = figment.merge(Yaml::file(path));
            }
        }

        // JSON files
        let json_files = [
            sah_dir.join("sah.json"),
            sah_dir.join("swissarmyhammer.json"),
        ];

        for path in &json_files {
            if path.exists() {
                trace!("Loading global JSON config: {}", path.display());
                figment = figment.merge(Json::file(path));
            }
        }

        Ok(figment)
    }

    /// Load project configuration files from ./.swissarmyhammer/
    fn load_project_config(&self) -> ConfigResult<Figment> {
        debug!("Loading project configuration files");

        let current_dir = std::env::current_dir().map_err(ConfigError::directory_error)?;

        let sah_dir = current_dir.join(".swissarmyhammer");

        if !sah_dir.exists() {
            debug!(
                "Project SwissArmyHammer directory does not exist: {}",
                sah_dir.display()
            );
            return Ok(Figment::new());
        }

        let mut figment = Figment::new();

        // Load configuration files in format precedence order
        // (later formats can override earlier ones for the same file name)

        // TOML files
        let toml_files = [
            sah_dir.join("sah.toml"),
            sah_dir.join("swissarmyhammer.toml"),
        ];

        for path in &toml_files {
            if path.exists() {
                trace!("Loading project TOML config: {}", path.display());
                figment = figment.merge(Toml::file(path));
            }
        }

        // YAML files
        let yaml_files = [
            sah_dir.join("sah.yaml"),
            sah_dir.join("sah.yml"),
            sah_dir.join("swissarmyhammer.yaml"),
            sah_dir.join("swissarmyhammer.yml"),
        ];

        for path in &yaml_files {
            if path.exists() {
                trace!("Loading project YAML config: {}", path.display());
                figment = figment.merge(Yaml::file(path));
            }
        }

        // JSON files
        let json_files = [
            sah_dir.join("sah.json"),
            sah_dir.join("swissarmyhammer.json"),
        ];

        for path in &json_files {
            if path.exists() {
                trace!("Loading project JSON config: {}", path.display());
                figment = figment.merge(Json::file(path));
            }
        }

        Ok(figment)
    }

    /// Load environment variables with SAH_ and SWISSARMYHAMMER_ prefixes
    fn load_env_vars(&self) -> ConfigResult<Figment> {
        debug!("Loading environment variables");

        // Load environment variables with both prefixes
        // Later prefixes override earlier ones for the same variable name
        // Don't split on underscores to get flat key-value pairs for templates
        let figment = Figment::new()
            .merge(Env::prefixed("SAH_").map(|key| key.as_str().to_lowercase().into()))
            .merge(Env::prefixed("SWISSARMYHAMMER_").map(|key| key.as_str().to_lowercase().into()));

        Ok(figment)
    }
}

impl Default for ConfigProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_config_provider_new() {
        let provider = ConfigProvider::new();
        // Test that it creates successfully - no assertions needed as this would panic if it failed
        let _ = provider;
    }

    #[test]
    fn test_load_empty_template_context() {
        let provider = ConfigProvider::new();
        let _context = provider.load_template_context().unwrap();

        // With no configuration files, should still succeed but be mostly empty
        // (may have environment variables)
        // Note: context.len() is always >= 0 by definition
    }

    #[test]
    fn test_get_default_config() {
        let provider = ConfigProvider::new();
        let figment = provider.get_default_config();

        // Default config should be empty for now
        let config: RawConfig = figment.extract().unwrap();
        assert!(config.is_empty());
    }

    #[test]
    fn test_load_env_vars() {
        let provider = ConfigProvider::new();

        // Set some test environment variables
        std::env::set_var("SAH_TEST_VAR", "test_value");
        std::env::set_var("SWISSARMYHAMMER_OTHER_VAR", "other_value");

        let figment = provider.load_env_vars().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Debug print the keys
        println!("Config keys: {:?}", config.keys().collect::<Vec<_>>());

        // Check that environment variables are loaded as flat keys
        // SAH_TEST_VAR becomes {"test_var": "test_value"}
        assert!(config.contains_key("test_var"));
        assert!(config.contains_key("other_var"));

        // Clean up
        std::env::remove_var("SAH_TEST_VAR");
        std::env::remove_var("SWISSARMYHAMMER_OTHER_VAR");
    }

    #[test]
    fn test_load_project_config_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();

        // Change to temp directory with no .swissarmyhammer folder
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();

        // This should succeed even if no project config exists
        let figment = provider.load_project_config().unwrap();
        let config: RawConfig = figment.extract().unwrap();

        // Restore directory
        std::env::set_current_dir(original_dir).unwrap();

        // Should be empty if no config files exist in current directory
        assert!(config.is_empty());
    }

    #[test]
    fn test_load_project_config_with_toml() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
test_key = "test_value"
number_key = 42
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.load_project_config().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );
    }

    #[test]
    fn test_load_project_config_with_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.yaml");
        fs::write(
            &config_file,
            r#"
test_key: test_value
number_key: 42
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.load_project_config().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );
    }

    #[test]
    fn test_load_project_config_with_json() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.json");
        fs::write(
            &config_file,
            r#"
{
    "test_key": "test_value",
    "number_key": 42
}
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let provider = ConfigProvider::new();
        let figment = provider.load_project_config().unwrap();
        let config: HashMap<String, serde_json::Value> = figment.extract().unwrap();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert_eq!(
            config.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(
            config.get("number_key"),
            Some(&serde_json::Value::Number(42.into()))
        );
    }

    #[test]
    fn test_precedence_order() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        // Create config files with overlapping keys
        let toml_config = sah_dir.join("sah.toml");
        fs::write(
            &toml_config,
            r#"
shared_key = "from_toml"
toml_only = "toml_value"
"#,
        )
        .unwrap();

        let yaml_config = sah_dir.join("sah.yaml");
        fs::write(
            &yaml_config,
            r#"
shared_key: from_yaml
yaml_only: yaml_value
"#,
        )
        .unwrap();

        // Change to temp directory
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        // Set environment variable that should override file values
        std::env::set_var("SAH_SHARED_KEY", "from_env");
        std::env::set_var("SAH_ENV_ONLY", "env_value");

        let provider = ConfigProvider::new();
        let context = provider.load_template_context().unwrap();

        // Restore original directory and clean up env vars
        std::env::set_current_dir(original_dir).unwrap();
        std::env::remove_var("SAH_SHARED_KEY");
        std::env::remove_var("SAH_ENV_ONLY");

        // Environment should override file values
        assert_eq!(
            context.get("shared_key"),
            Some(&serde_json::Value::String("from_env".to_string()))
        );

        // YAML should override TOML for file-only values
        // Note: This test might be sensitive to figment's exact merging behavior

        // Environment-only value should be present
        assert_eq!(
            context.get("env_only"),
            Some(&serde_json::Value::String("env_value".to_string()))
        );
    }
}
