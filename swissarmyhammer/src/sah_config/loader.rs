use crate::sah_config::types::{ConfigValue, Configuration};
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
    EnvVarSubstitution(String),
}

/// Configuration loader that handles file loading and TOML parsing
pub struct ConfigurationLoader {
    max_file_size: u64,
}

impl ConfigurationLoader {
    /// Create a new configuration loader with default settings
    pub fn new() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB default limit
        }
    }

    /// Create a configuration loader with custom file size limit
    pub fn with_max_file_size(max_file_size: u64) -> Self {
        Self { max_file_size }
    }

    /// Load configuration from a specific file path
    pub fn load_from_file(&self, file_path: &Path) -> Result<Configuration, ConfigurationError> {
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

        // Read file contents
        let contents =
            fs::read_to_string(file_path).map_err(|_| ConfigurationError::InvalidUtf8)?;

        // Parse TOML
        let toml_value: toml::Value = toml::from_str(&contents)?;

        // Convert to our ConfigValue representation
        let config_values = self.toml_value_to_config_map(toml_value)?;

        Ok(Configuration::with_values(
            config_values,
            Some(file_path.to_path_buf()),
        ))
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
}

impl Default for ConfigurationLoader {
    fn default() -> Self {
        Self::new()
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

        let loader = ConfigurationLoader::new();
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
    fn test_file_not_found() {
        let loader = ConfigurationLoader::new();
        let result = loader.load_from_file(Path::new("nonexistent.toml"));

        assert!(matches!(
            result,
            Err(ConfigurationError::FileNotFound { .. })
        ));
    }

    #[test]
    fn test_file_too_large() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let loader = ConfigurationLoader::with_max_file_size(5); // Very small limit
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

        let loader = ConfigurationLoader::new();
        let result = loader.load_from_file(temp_file.path());

        assert!(matches!(result, Err(ConfigurationError::TomlParse(_))));
        Ok(())
    }

    #[test]
    fn test_toml_value_conversion() -> Result<(), Box<dyn std::error::Error>> {
        let loader = ConfigurationLoader::new();

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
}
