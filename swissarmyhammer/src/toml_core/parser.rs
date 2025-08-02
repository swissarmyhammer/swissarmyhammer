use crate::toml_core::{configuration::Configuration, error::ConfigError, value::ConfigValue};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// Configuration parser that handles TOML file loading with validation
pub struct ConfigParser {
    /// Maximum file size in bytes (default: 1MB)
    max_file_size: u64,
    /// Maximum nesting depth for tables (default: 10 levels)
    max_depth: usize,
    /// Enable UTF-8 validation (default: true)
    validate_utf8: bool,
}

impl ConfigParser {
    /// Create a new parser with default settings
    pub fn new() -> Self {
        Self {
            max_file_size: 1024 * 1024, // 1MB
            max_depth: 10,
            validate_utf8: true,
        }
    }

    /// Create a parser with custom file size limit
    pub fn with_max_file_size(max_file_size: u64) -> Self {
        Self {
            max_file_size,
            ..Self::new()
        }
    }

    /// Create a parser with custom maximum nesting depth
    pub fn with_max_depth(max_depth: usize) -> Self {
        Self {
            max_depth,
            ..Self::new()
        }
    }

    /// Create a parser with UTF-8 validation setting
    pub fn with_utf8_validation(validate_utf8: bool) -> Self {
        Self {
            validate_utf8,
            ..Self::new()
        }
    }

    /// Parse configuration from a file path
    ///
    /// This method performs comprehensive validation including:
    /// - File existence and size limits
    /// - UTF-8 encoding validation
    /// - TOML syntax validation
    /// - Nesting depth validation
    ///
    /// # Arguments
    /// * `file_path` - Path to the sah.toml file
    ///
    /// # Returns
    /// * `Ok(Configuration)` - Successfully parsed configuration
    /// * `Err(ConfigError)` - Parsing or validation error
    pub fn parse_file(&self, file_path: &Path) -> Result<Configuration, ConfigError> {
        // Check file existence
        if !file_path.exists() {
            return Err(ConfigError::FileNotFound {
                path: file_path.to_string_lossy().to_string(),
            });
        }

        // Validate file size
        let metadata = fs::metadata(file_path)?;
        if metadata.len() > self.max_file_size {
            return Err(ConfigError::FileTooLarge {
                size: metadata.len(),
                max_size: self.max_file_size,
            });
        }

        // Read file contents
        let contents = self.read_file_contents(file_path)?;

        // Parse TOML
        let config = self.parse_toml_string(&contents)?;

        // Set file path in configuration
        Ok(Configuration::with_values(
            config.values().clone(),
            Some(file_path.to_path_buf()),
        ))
    }

    /// Parse configuration from a TOML string
    ///
    /// # Arguments
    /// * `toml_content` - TOML content as string
    ///
    /// # Returns
    /// * `Ok(Configuration)` - Successfully parsed configuration
    /// * `Err(ConfigError)` - Parsing or validation error
    pub fn parse_string(&self, toml_content: &str) -> Result<Configuration, ConfigError> {
        self.parse_toml_string(toml_content)
    }

    /// Load configuration from repository root
    ///
    /// Searches for sah.toml starting from current directory and walking up
    /// to find the repository root (indicated by .git directory or filesystem root)
    ///
    /// # Returns
    /// * `Ok(Some(Configuration))` - Configuration found and loaded
    /// * `Ok(None)` - No sah.toml file found in repository
    /// * `Err(ConfigError)` - Error loading or parsing configuration
    pub fn load_from_repo_root(&self) -> Result<Option<Configuration>, ConfigError> {
        let current_dir = std::env::current_dir()?;
        let mut search_dir = current_dir.as_path();

        loop {
            let sah_toml_path = search_dir.join("sah.toml");

            if sah_toml_path.exists() {
                return Ok(Some(self.parse_file(&sah_toml_path)?));
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

    /// Read file contents with UTF-8 validation
    fn read_file_contents(&self, file_path: &Path) -> Result<String, ConfigError> {
        if self.validate_utf8 {
            // Read as bytes first to provide better error information
            let bytes = fs::read(file_path)?;

            // Find the position of invalid UTF-8 if any
            match String::from_utf8(bytes) {
                Ok(content) => Ok(content),
                Err(utf8_error) => {
                    let position = utf8_error.utf8_error().valid_up_to();
                    Err(ConfigError::invalid_utf8_at_position(Some(position)))
                }
            }
        } else {
            // Use standard file reading (will return IO error for invalid UTF-8)
            fs::read_to_string(file_path).map_err(|e| match e.kind() {
                std::io::ErrorKind::InvalidData => ConfigError::invalid_utf8_at_position(None),
                _ => ConfigError::Io(e),
            })
        }
    }

    /// Parse TOML string content
    fn parse_toml_string(&self, content: &str) -> Result<Configuration, ConfigError> {
        // Parse TOML
        let toml_value: toml::Value =
            toml::from_str(content).map_err(ConfigError::from_toml_error)?;

        // Convert to our ConfigValue representation
        let config_values = self.toml_value_to_config_map(toml_value)?;

        // Validate nesting depth
        self.validate_nesting_depth(&config_values)?;

        Ok(Configuration::with_values(config_values, None))
    }

    /// Convert a TOML value to a map of configuration values
    fn toml_value_to_config_map(
        &self,
        value: toml::Value,
    ) -> Result<HashMap<String, ConfigValue>, ConfigError> {
        match value {
            toml::Value::Table(table) => {
                let mut config_map = HashMap::new();
                for (key, value) in table {
                    config_map.insert(key, ConfigValue::from(value));
                }
                Ok(config_map)
            }
            _ => {
                // If the root is not a table, create a single entry
                let mut config_map = HashMap::new();
                config_map.insert("value".to_string(), ConfigValue::from(value));
                Ok(config_map)
            }
        }
    }

    /// Validate that nesting depth doesn't exceed limits
    fn validate_nesting_depth(
        &self,
        values: &HashMap<String, ConfigValue>,
    ) -> Result<(), ConfigError> {
        for value in values.values() {
            let depth = Self::calculate_nesting_depth(value);
            if depth > self.max_depth {
                return Err(ConfigError::NestingTooDeep {
                    depth,
                    max_depth: self.max_depth,
                });
            }
        }
        Ok(())
    }

    /// Calculate the maximum nesting depth of a ConfigValue
    fn calculate_nesting_depth(value: &ConfigValue) -> usize {
        match value {
            ConfigValue::Table(table) => {
                let max_child_depth = table
                    .values()
                    .map(Self::calculate_nesting_depth)
                    .max()
                    .unwrap_or(0);
                1 + max_child_depth
            }
            ConfigValue::Array(arr) => {
                let max_child_depth = arr
                    .iter()
                    .map(Self::calculate_nesting_depth)
                    .max()
                    .unwrap_or(0);
                max_child_depth // Arrays don't add to table nesting depth
            }
            _ => 0, // Scalar values have no nesting
        }
    }
}

impl Default for ConfigParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to parse a sah.toml file with default settings
pub fn parse_config_file(file_path: &Path) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_file(file_path)
}

/// Convenience function to parse TOML content with default settings
pub fn parse_config_string(content: &str) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_string(content)
}

/// Convenience function to load configuration from repository root
pub fn load_repo_config() -> Result<Option<Configuration>, ConfigError> {
    ConfigParser::new().load_from_repo_root()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_parse_valid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
project_name = "Test Project"
version = 123
debug = true
servers = ["server1", "server2"]
created_at = 2023-01-01T00:00:00Z

[database]
host = "localhost"
port = 5432

[database.connection]
timeout = 30
pool_size = 10
"#
        )?;

        let parser = ConfigParser::new();
        let config = parser.parse_file(temp_file.path())?;

        assert_eq!(config.len(), 6); // Top-level keys: project_name, version, debug, servers, created_at, database

        // Test basic values
        assert_eq!(
            config.get("project_name"),
            Some(&ConfigValue::String("Test Project".to_string()))
        );
        assert_eq!(config.get("version"), Some(&ConfigValue::Integer(123)));
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));

        // Test array
        if let Some(ConfigValue::Array(servers)) = config.get("servers") {
            assert_eq!(servers.len(), 2);
            assert_eq!(servers[0], ConfigValue::String("server1".to_string()));
        } else {
            panic!("Expected servers array");
        }

        // Test datetime conversion
        assert_eq!(
            config.get("created_at"),
            Some(&ConfigValue::DateTime("2023-01-01T00:00:00Z".to_string()))
        );

        // Test nested table access
        let host = config.get_nested("database.host")?;
        assert_eq!(host, Some(&ConfigValue::String("localhost".to_string())));

        let timeout = config.get_nested("database.connection.timeout")?;
        assert_eq!(timeout, Some(&ConfigValue::Integer(30)));

        Ok(())
    }

    #[test]
    fn test_file_not_found() {
        let parser = ConfigParser::new();
        let result = parser.parse_file(Path::new("nonexistent.toml"));

        assert!(matches!(result, Err(ConfigError::FileNotFound { .. })));
    }

    #[test]
    fn test_file_too_large() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "test = 'value'")?;

        let parser = ConfigParser::with_max_file_size(5); // Very small limit
        let result = parser.parse_file(temp_file.path());

        assert!(matches!(result, Err(ConfigError::FileTooLarge { .. })));
        Ok(())
    }

    #[test]
    fn test_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "invalid toml content [")?;

        let parser = ConfigParser::new();
        let result = parser.parse_file(temp_file.path());

        assert!(matches!(result, Err(ConfigError::TomlParse { .. })));
        Ok(())
    }

    #[test]
    fn test_nesting_depth_validation() {
        let parser = ConfigParser::with_max_depth(2);

        // Create TOML with nesting deeper than limit
        let toml_content = r#"
[level1]
[level1.level2]
[level1.level2.level3]
value = "too deep"
"#;

        let result = parser.parse_string(toml_content);
        assert!(matches!(result, Err(ConfigError::NestingTooDeep { .. })));
    }

    #[test]
    fn test_parse_string() -> Result<(), Box<dyn std::error::Error>> {
        let toml_content = r#"
name = "TestProject"
version = 1
debug = true

[database]
host = "localhost"
port = 5432
"#;

        let parser = ConfigParser::new();
        let config = parser.parse_string(toml_content)?;

        assert_eq!(config.len(), 4);
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("TestProject".to_string()))
        );

        let host = config.get_nested("database.host")?;
        assert_eq!(host, Some(&ConfigValue::String("localhost".to_string())));

        Ok(())
    }

    #[test]
    fn test_utf8_validation() -> Result<(), Box<dyn std::error::Error>> {
        // Create a file with invalid UTF-8
        let mut temp_file = NamedTempFile::new()?;
        temp_file.write_all(&[0xFF, 0xFE])?; // Invalid UTF-8 bytes
        temp_file.write_all(b"name = \"test\"")?;

        let parser = ConfigParser::with_utf8_validation(true);
        let result = parser.parse_file(temp_file.path());

        assert!(matches!(result, Err(ConfigError::InvalidUtf8 { .. })));

        // Test with validation disabled
        let parser = ConfigParser::with_utf8_validation(false);
        let result = parser.parse_file(temp_file.path());

        // Should still fail but with IO error instead
        assert!(result.is_err());

        Ok(())
    }

    #[test]
    fn test_convenience_functions() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, r#"name = "test""#)?;

        // Test convenience function
        let config = parse_config_file(temp_file.path())?;
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("test".to_string()))
        );

        // Test string parsing convenience function
        let config = parse_config_string(r#"version = 42"#)?;
        assert_eq!(config.get("version"), Some(&ConfigValue::Integer(42)));

        Ok(())
    }

    #[test]
    fn test_empty_configuration() -> Result<(), Box<dyn std::error::Error>> {
        let temp_file = NamedTempFile::new()?;
        // Create empty file (no content needed)

        let parser = ConfigParser::new();
        let config = parser.parse_file(temp_file.path())?;

        assert!(config.is_empty());
        assert_eq!(config.len(), 0);

        Ok(())
    }

    #[test]
    fn test_simple_key_value() -> Result<(), Box<dyn std::error::Error>> {
        // TOML with a simple key-value pair
        let parser = ConfigParser::new();
        let config = parser.parse_string("answer = 42")?;

        // Should create a single entry
        assert_eq!(config.len(), 1);
        assert_eq!(config.get("answer"), Some(&ConfigValue::Integer(42)));

        Ok(())
    }
}
