use crate::sah_config::{
    env_vars::{EnvVarError, EnvVarProcessor},
    types::{CacheMetadata, ConfigValue, Configuration},
    validation::{ValidationError, Validator},
};
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use thiserror::Error;

/// Errors that can occur when loading configuration files
#[derive(Error, Debug)]
pub enum ConfigurationError {
    /// IO error occurred while reading the configuration file
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error occurred while parsing the configuration
    #[error("TOML parsing error: {0}")]
    TomlParse(#[from] toml::de::Error),

    /// Configuration file exceeds maximum allowed size
    #[error("File is too large: {size} bytes (max: {max_size} bytes)")]
    FileTooLarge {
        /// Actual file size in bytes
        size: u64,
        /// Maximum allowed size in bytes
        max_size: u64,
    },

    /// Configuration file was not found at the specified path
    #[error("Configuration file not found: {path}")]
    FileNotFound {
        /// Path where the file was expected
        path: String,
    },

    /// File contains invalid UTF-8 characters
    #[error("Invalid UTF-8 in configuration file")]
    InvalidUtf8,

    /// Error occurred during environment variable substitution
    #[error("Environment variable substitution error: {0}")]
    EnvVarSubstitution(#[from] EnvVarError),

    /// Validation error occurred during configuration validation
    #[error("Configuration validation error: {0}")]
    Validation(#[from] ValidationError),
}

/// Configuration loader that handles file loading and TOML parsing
pub struct ConfigurationLoader {
    max_file_size: u64,
    validator: Validator,
    env_processor: EnvVarProcessor,
    enable_env_substitution: bool,
    enable_validation: bool,
}

impl ConfigurationLoader {
    /// Create a new configuration loader with default settings
    pub fn new() -> Result<Self, ConfigurationError> {
        Ok(Self {
            max_file_size: 1024 * 1024, // 1MB default limit
            validator: Validator::new(),
            env_processor: EnvVarProcessor::new()?,
            enable_env_substitution: true,
            enable_validation: true,
        })
    }

    /// Create a configuration loader with custom file size limit
    pub fn with_max_file_size(max_file_size: u64) -> Result<Self, ConfigurationError> {
        Ok(Self {
            max_file_size,
            validator: Validator::new(),
            env_processor: EnvVarProcessor::new()?,
            enable_env_substitution: true,
            enable_validation: true,
        })
    }

    /// Enable or disable environment variable substitution
    pub fn with_env_substitution(mut self, enable: bool) -> Self {
        self.enable_env_substitution = enable;
        self
    }

    /// Enable or disable validation
    pub fn with_validation(mut self, enable: bool) -> Self {
        self.enable_validation = enable;
        self
    }

    /// Use a custom validator
    pub fn with_validator(mut self, validator: Validator) -> Self {
        self.validator = validator;
        self
    }

    /// Load configuration from a specific file path
    pub fn load_from_file(&self, file_path: &Path) -> Result<Configuration, ConfigurationError> {
        // Security validation - check file path and permissions
        if self.enable_validation {
            self.validator.validate_file_security(file_path)?;
        }

        if !file_path.exists() {
            return Err(ConfigurationError::FileNotFound {
                path: file_path.to_string_lossy().to_string(),
            });
        }

        // Check file size
        let metadata = fs::metadata(file_path)?;
        if metadata.len() > self.max_file_size {
            return Err(ConfigurationError::FileTooLarge {
                size: metadata.len(),
                max_size: self.max_file_size,
            });
        }

        // Read file contents with UTF-8 validation
        let contents = self.read_file_contents_securely(file_path)?;

        // Parse TOML
        let toml_value: toml::Value = toml::from_str(&contents)?;

        // Convert to our ConfigValue representation
        let mut config_values = self.toml_value_to_config_map(toml_value)?;

        // Process environment variable substitution
        if self.enable_env_substitution {
            self.process_env_substitution(&mut config_values)?;
        }

        // Create cache metadata
        let cache_metadata = CacheMetadata::from_file(file_path).ok();

        // Create configuration
        let config = Configuration::with_cache_metadata(
            config_values,
            Some(file_path.to_path_buf()),
            cache_metadata,
        );

        // Validate configuration
        if self.enable_validation {
            self.validator.validate(&config)?;
        }

        Ok(config)
    }

    /// Load configuration from repository root
    ///
    /// Searches for sah.toml starting from current directory and walking up
    /// to find the repository root (indicated by .git directory or filesystem root)
    pub fn load_from_repo_root(&self) -> Result<Option<Configuration>, ConfigurationError> {
        let current_dir = std::env::current_dir().map_err(ConfigurationError::Io)?;

        let mut search_dir = current_dir.as_path();

        loop {
            let sah_toml_path = search_dir.join("sah.toml");

            if sah_toml_path.exists() {
                return Ok(Some(self.load_from_file(&sah_toml_path)?));
            }

            // Check if we've reached a git repository root
            if search_dir.join(".git").exists() {
                break;
            }

            // Move to parent directory
            match search_dir.parent() {
                Some(parent) => search_dir = parent,
                None => break, // Reached filesystem root
            }
        }

        Ok(None)
    }

    /// Convert a TOML value to our ConfigValue representation
    fn toml_value_to_config_value(
        value: toml::Value,
    ) -> Result<ConfigValue, ConfigurationError> {
        match value {
            toml::Value::String(s) => Ok(ConfigValue::String(s)),
            toml::Value::Integer(i) => Ok(ConfigValue::Integer(i)),
            toml::Value::Float(f) => Ok(ConfigValue::Float(f)),
            toml::Value::Boolean(b) => Ok(ConfigValue::Boolean(b)),
            toml::Value::Array(arr) => {
                let mut config_array = Vec::new();
                for item in arr {
                    config_array.push(Self::toml_value_to_config_value(item)?);
                }
                Ok(ConfigValue::Array(config_array))
            }
            toml::Value::Table(table) => {
                let mut config_table = HashMap::new();
                for (key, value) in table {
                    config_table.insert(key, Self::toml_value_to_config_value(value)?);
                }
                Ok(ConfigValue::Table(config_table))
            }
            toml::Value::Datetime(_) => {
                // Convert datetime to string representation
                Ok(ConfigValue::String(value.to_string()))
            }
        }
    }

    /// Convert a TOML value to a map of configuration values
    fn toml_value_to_config_map(
        &self,
        value: toml::Value,
    ) -> Result<HashMap<String, ConfigValue>, ConfigurationError> {
        match value {
            toml::Value::Table(table) => {
                let mut config_map = HashMap::new();
                for (key, value) in table {
                    config_map.insert(key, Self::toml_value_to_config_value(value)?);
                }
                Ok(config_map)
            }
            _ => {
                // If the root is not a table, create a single entry
                let mut config_map = HashMap::new();
                config_map.insert("value".to_string(), Self::toml_value_to_config_value(value)?);
                Ok(config_map)
            }
        }
    }

    /// Securely read file contents with UTF-8 validation
    fn read_file_contents_securely(&self, file_path: &Path) -> Result<String, ConfigurationError> {
        // Read as bytes first to provide better error information
        let bytes = fs::read(file_path)?;

        // Validate UTF-8 encoding
        match String::from_utf8(bytes) {
            Ok(content) => Ok(content),
            Err(_) => Err(ConfigurationError::InvalidUtf8),
        }
    }

    /// Process environment variable substitution in configuration values
    fn process_env_substitution(&self, values: &mut HashMap<String, ConfigValue>) -> Result<(), ConfigurationError> {
        for value in values.values_mut() {
            self.process_env_substitution_recursive(value)?;
        }
        Ok(())
    }

    /// Recursively process environment variable substitution in nested structures
    fn process_env_substitution_recursive(&self, value: &mut ConfigValue) -> Result<(), ConfigurationError> {
        match value {
            ConfigValue::String(s) => {
                *s = self.env_processor.substitute_variables(s)?;
            }
            ConfigValue::Array(arr) => {
                for item in arr {
                    self.process_env_substitution_recursive(item)?;
                }
            }
            ConfigValue::Table(table) => {
                for table_value in table.values_mut() {
                    self.process_env_substitution_recursive(table_value)?;
                }
            }
            _ => {} // Other types don't need environment variable substitution
        }
        Ok(())
    }
}

impl Default for ConfigurationLoader {
    fn default() -> Self {
        Self::new().expect("Failed to create default ConfigurationLoader")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_valid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
project_name = "Test Project"
version = 123
debug = true
servers = ["server1", "server2"]

[database]
host = "localhost"
port = 5432
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        assert_eq!(config.len(), 5);
        assert_eq!(
            config.get("project_name"),
            Some(&ConfigValue::String("Test Project".to_string()))
        );
        assert_eq!(config.get("version"), Some(&ConfigValue::Integer(123)));
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));

        // Check nested table
        if let Some(ConfigValue::Table(db_table)) = config.get("database") {
            assert_eq!(
                db_table.get("host"),
                Some(&ConfigValue::String("localhost".to_string()))
            );
            assert_eq!(db_table.get("port"), Some(&ConfigValue::Integer(5432)));
        } else {
            panic!("Expected database table");
        }

        Ok(())
    }

    #[test]
    fn test_file_not_found() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(Path::new("nonexistent.toml"));

        assert!(matches!(
            result,
            Err(ConfigurationError::FileNotFound { .. })
        ));
        Ok(())
    }

    #[test]
    fn test_file_too_large() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let loader = ConfigurationLoader::with_max_file_size(5)?.with_validation(false); // Very small limit
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(
            result,
            Err(ConfigurationError::FileTooLarge { .. })
        ));
        Ok(())
    }

    #[test]
    fn test_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "invalid toml content [")?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(result, Err(ConfigurationError::TomlParse(_))));
        Ok(())
    }

    #[test]
    fn test_toml_value_conversion() -> Result<(), Box<dyn std::error::Error>> {
        let _loader = ConfigurationLoader::new()?;

        // Test string conversion
        let string_val = toml::Value::String("test".to_string());
        let config_val = ConfigurationLoader::toml_value_to_config_value(string_val)?;
        assert_eq!(config_val, ConfigValue::String("test".to_string()));

        // Test integer conversion
        let int_val = toml::Value::Integer(42);
        let config_val = ConfigurationLoader::toml_value_to_config_value(int_val)?;
        assert_eq!(config_val, ConfigValue::Integer(42));

        // Test boolean conversion
        let bool_val = toml::Value::Boolean(true);
        let config_val = ConfigurationLoader::toml_value_to_config_value(bool_val)?;
        assert_eq!(config_val, ConfigValue::Boolean(true));

        Ok(())
    }

    #[test]
    #[serial_test::serial]
    fn test_environment_variable_substitution() -> Result<(), Box<dyn std::error::Error>> {
        use std::env;
        
        // Set up test environment variables
        env::set_var("TEST_DB_HOST", "production.example.com");
        env::set_var("TEST_DB_PORT", "5432");

        let mut temp_file = NamedTempFile::new()?;
        write!(
            temp_file,
            r#"
database_url = "postgresql://${{TEST_DB_HOST}}:${{TEST_DB_PORT}}/myapp"
api_key = "${{TEST_API_KEY:-default_key}}"
debug = "${{TEST_DEBUG:-false}}"
"#
        )?;

        let loader = ConfigurationLoader::new()?.with_env_substitution(true).with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        // Check that environment variables were substituted
        if let Some(ConfigValue::String(db_url)) = config.get("database_url") {
            assert_eq!(db_url, "postgresql://production.example.com:5432/myapp");
        } else {
            panic!("Expected database_url to be a string");
        }

        if let Some(ConfigValue::String(api_key)) = config.get("api_key") {
            assert_eq!(api_key, "default_key"); // Should use default since TEST_API_KEY is not set
        } else {
            panic!("Expected api_key to be a string");
        }

        // Clean up
        env::remove_var("TEST_DB_HOST");
        env::remove_var("TEST_DB_PORT");

        Ok(())
    }

    #[test]
    fn test_validation_disabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
# This would normally fail validation due to reserved name
for = "invalid_reserved_name"
"#
        )?;

        // With validation disabled, this should work
        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        assert_eq!(config.len(), 1);
        assert_eq!(
            config.get("for"),
            Some(&ConfigValue::String("invalid_reserved_name".to_string()))
        );

        Ok(())
    }

    #[test]
    fn test_validation_enabled() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
# This should fail validation due to reserved name
for = "invalid_reserved_name"
"#
        )?;

        // With validation enabled (default), this should fail
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(result, Err(ConfigurationError::Validation(_))));

        Ok(())
    }

    #[test]
    fn test_cache_metadata_creation() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let loader = ConfigurationLoader::new()?.with_validation(false);
        let config = loader.load_from_file(temp_file.path())?;

        // Should have cache metadata
        assert!(config.cache_metadata().is_some());
        
        // Should have file path
        assert!(config.file_path().is_some());
        assert_eq!(config.file_path().unwrap(), temp_file.path());

        Ok(())
    }

    #[test]
    fn test_security_validation() -> Result<(), Box<dyn std::error::Error>> {
        // Test with path traversal attempt (should fail with validation enabled)
        let loader = ConfigurationLoader::new()?;
        let result = loader.load_from_file(Path::new("../../../etc/passwd"));

        // Should fail due to security validation
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_loader_configuration_options() -> Result<(), Box<dyn std::error::Error>> {
        // Test chaining configuration methods
        let loader = ConfigurationLoader::with_max_file_size(2048)?
            .with_env_substitution(false)
            .with_validation(false);

        // Create a simple test file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let config = loader.load_from_file(temp_file.path())?;
        assert_eq!(config.len(), 1);

        Ok(())
    }
}
