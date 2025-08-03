use crate::toml_config::configuration::Configuration;
use crate::toml_config::error::{ConfigError, ValidationLimits};
use crate::toml_config::value::ConfigValue;
use std::fs;
use std::path::Path;

/// TOML configuration parser with comprehensive validation and error handling
pub struct ConfigParser {
    /// Maximum file size allowed
    max_file_size: u64,
    /// Maximum nesting depth allowed
    max_nesting_depth: usize,
}

impl ConfigParser {
    /// Create a new parser with default limits
    pub fn new() -> Self {
        Self {
            max_file_size: ValidationLimits::MAX_FILE_SIZE,
            max_nesting_depth: ValidationLimits::MAX_NESTING_DEPTH,
        }
    }

    /// Create a parser with custom limits
    pub fn with_limits(max_file_size: u64, max_nesting_depth: usize) -> Self {
        Self {
            max_file_size,
            max_nesting_depth,
        }
    }

    /// Parse a TOML configuration from a file
    ///
    /// # Arguments
    /// * `file_path` - Path to the TOML file to parse
    ///
    /// # Returns
    /// * `Result<Configuration, ConfigError>` - The parsed configuration or an error
    ///
    /// # Errors
    /// * `ConfigError::Io` - If the file cannot be read
    /// * `ConfigError::FileTooLarge` - If the file exceeds the size limit
    /// * `ConfigError::InvalidUtf8` - If the file is not valid UTF-8
    /// * `ConfigError::TomlParse*` - If the TOML syntax is invalid
    /// * `ConfigError::NestingTooDeep` - If the nesting exceeds the depth limit
    pub fn parse_file<P: AsRef<Path>>(&self, file_path: P) -> Result<Configuration, ConfigError> {
        let path = file_path.as_ref();

        // Check if file exists and get metadata
        let metadata = fs::metadata(path)?;

        // Validate file size
        if metadata.len() > self.max_file_size {
            return Err(ConfigError::file_too_large(
                metadata.len(),
                self.max_file_size,
            ));
        }

        // Read file contents
        let contents = fs::read(path)?;

        // Validate UTF-8 encoding
        let contents_str = String::from_utf8(contents)?;

        // Parse TOML content
        self.parse_string(&contents_str, Some(path.to_path_buf()))
    }

    /// Parse a TOML configuration from a string
    ///
    /// # Arguments
    /// * `contents` - The TOML content as a string
    /// * `file_path` - Optional file path for error reporting
    ///
    /// # Returns
    /// * `Result<Configuration, ConfigError>` - The parsed configuration or an error
    pub fn parse_string(
        &self,
        contents: &str,
        file_path: Option<std::path::PathBuf>,
    ) -> Result<Configuration, ConfigError> {
        // Parse TOML
        let toml_value: toml::Value = contents.parse().map_err(|e| self.convert_toml_error(e))?;

        // Convert to our ConfigValue structure
        let config_value = ConfigValue::from(toml_value);

        // Validate nesting depth
        config_value.validate(0)?;

        // Extract values from the root table
        let values = match config_value {
            ConfigValue::Table(table) => table,
            _ => {
                return Err(ConfigError::validation(
                    "Root of TOML file must be a table".to_string(),
                ));
            }
        };

        // Create configuration
        let mut config = Configuration::with_values(values, file_path);

        // Validate the configuration
        config.validate()?;

        // Process environment variable substitution
        config.substitute_env_vars()?;

        Ok(config)
    }

    /// Convert TOML parsing error to our ConfigError with better context
    fn convert_toml_error(&self, error: toml::de::Error) -> ConfigError {
        // Try to extract line and column information
        if let Some(range) = error.span() {
            // TOML errors include span information
            ConfigError::toml_parse(range.start, range.end, error.message().to_string())
        } else {
            // Fall back to generic error
            ConfigError::TomlParseGeneric(error)
        }
    }

    /// Validate file exists and is readable
    pub fn validate_file<P: AsRef<Path>>(&self, file_path: P) -> Result<(), ConfigError> {
        let path = file_path.as_ref();

        // Check if file exists
        if !path.exists() {
            return Err(ConfigError::validation(format!(
                "Configuration file does not exist: {}",
                path.display()
            )));
        }

        // Check if it's a file (not a directory)
        if !path.is_file() {
            return Err(ConfigError::validation(format!(
                "Path is not a file: {}",
                path.display()
            )));
        }

        // Check file size
        let metadata = fs::metadata(path)?;
        if metadata.len() > self.max_file_size {
            return Err(ConfigError::file_too_large(
                metadata.len(),
                self.max_file_size,
            ));
        }

        Ok(())
    }

    /// Load configuration from repository root, returning None if file doesn't exist
    ///
    /// Searches for sah.toml in the current directory and parent directories
    /// up to the repository root (indicated by .git directory).
    pub fn load_from_repo_root(&self) -> Result<Option<Configuration>, ConfigError> {
        let current_dir = std::env::current_dir()?;
        let mut search_dir = current_dir.as_path();

        loop {
            let config_path = search_dir.join("sah.toml");

            if config_path.exists() {
                return Ok(Some(self.parse_file(&config_path)?));
            }

            // Check if we've reached the repository root (has .git directory)
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

    /// Get the maximum file size limit
    pub fn max_file_size(&self) -> u64 {
        self.max_file_size
    }

    /// Get the maximum nesting depth limit
    pub fn max_nesting_depth(&self) -> usize {
        self.max_nesting_depth
    }
}

impl Default for ConfigParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Convenience function to parse a TOML file with default settings
pub fn parse_config_file<P: AsRef<Path>>(file_path: P) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_file(file_path)
}

/// Convenience function to parse TOML content from a string with default settings
pub fn parse_config_string(contents: &str) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_string(contents, None)
}

/// Load configuration from repository root with default settings
pub fn load_repo_config() -> Result<Option<Configuration>, ConfigError> {
    ConfigParser::new().load_from_repo_root()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_parse_valid_toml_string() {
        let toml_content = r#"
            name = "TestProject"
            version = "1.0.0"
            debug = true
            
            [database]
            host = "localhost"
            port = 5432
            
            [database.credentials]
            username = "user"
            password = "pass"
        "#;

        let config = parse_config_string(toml_content).unwrap();

        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("TestProject".to_string()))
        );
        assert_eq!(
            config.get("version"),
            Some(&ConfigValue::String("1.0.0".to_string()))
        );
        assert_eq!(config.get("debug"), Some(&ConfigValue::Boolean(true)));
        assert_eq!(
            config.get("database.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            config.get("database.port"),
            Some(&ConfigValue::Integer(5432))
        );
        assert_eq!(
            config.get("database.credentials.username"),
            Some(&ConfigValue::String("user".to_string()))
        );
    }

    #[test]
    fn test_parse_invalid_toml_string() {
        let invalid_toml = r#"
            name = "TestProject
            invalid syntax here
        "#;

        let result = parse_config_string(invalid_toml);
        assert!(result.is_err());
        let error = result.unwrap_err();
        println!("Actual error: {error:?}");
        assert!(error.is_parse_error());
    }

    #[test]
    fn test_parse_config_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let toml_content = r#"
            name = "FileTest"
            value = 42
            
            [section]
            key = "value"
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let config = parse_config_file(&config_path).unwrap();
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("FileTest".to_string()))
        );
        assert_eq!(config.get("value"), Some(&ConfigValue::Integer(42)));
        assert_eq!(
            config.get("section.key"),
            Some(&ConfigValue::String("value".to_string()))
        );
        assert_eq!(config.file_path(), Some(&config_path));
    }

    #[test]
    fn test_file_size_validation() {
        let parser = ConfigParser::with_limits(100, 10); // 100 byte limit
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("large_config.toml");

        // Create a file larger than the limit
        let large_content = "key = \"".to_string() + &"x".repeat(200) + "\"";
        fs::write(&config_path, &large_content).unwrap();

        let result = parser.parse_file(&config_path);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::FileTooLarge { .. }
        ));
    }

    #[test]
    fn test_nesting_depth_validation() {
        // Test the validation directly since TOML structure doesn't create deep nesting
        let deep_config_value = create_deep_nested_value(12); // 12 > 10 ValidationLimit::MAX_NESTING_DEPTH
        let validation_result = deep_config_value.validate(0);
        assert!(validation_result.is_err());
        assert!(matches!(
            validation_result.unwrap_err(),
            ConfigError::NestingTooDeep { .. }
        ));
    }

    fn create_deep_nested_value(depth: usize) -> ConfigValue {
        use std::collections::HashMap;
        if depth == 0 {
            ConfigValue::String("value".to_string())
        } else {
            let mut table = HashMap::new();
            table.insert("nested".to_string(), create_deep_nested_value(depth - 1));
            ConfigValue::Table(table)
        }
    }

    #[test]
    fn test_validate_file() {
        let parser = ConfigParser::new();
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("valid_config.toml");

        // Test non-existent file
        let result = parser.validate_file(&config_path);
        assert!(result.is_err());

        // Create valid file
        fs::write(&config_path, "name = \"test\"").unwrap();
        let result = parser.validate_file(&config_path);
        assert!(result.is_ok());

        // Test directory instead of file
        let dir_path = temp_dir.path().join("directory");
        fs::create_dir(&dir_path).unwrap();
        let result = parser.validate_file(&dir_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_from_repo_root() {
        let temp_dir = TempDir::new().unwrap();
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let config_path = temp_dir.path().join("sah.toml");
        fs::write(&config_path, "name = \"repo_test\"").unwrap();

        // Change to subdirectory
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        // Temporarily change directory for test
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(&sub_dir).unwrap();

        let parser = ConfigParser::new();
        let result = parser.load_from_repo_root();

        // Restore original directory
        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config = result.unwrap();
        assert!(config.is_some());
        let config = config.unwrap();
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("repo_test".to_string()))
        );
    }

    #[test]
    fn test_env_var_substitution_in_parsing() {
        std::env::set_var("PARSER_TEST_CONFIG_VAR", "substituted_value");
        std::env::set_var("PARSER_TEST_PORT", "8080");

        let toml_content = r#"
            name = "${PARSER_TEST_CONFIG_VAR}"
            port = "${PARSER_TEST_PORT:-3000}"
            fallback = "${NONEXISTENT_VAR:-default_value}"
        "#;

        let config = parse_config_string(toml_content).unwrap();

        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("substituted_value".to_string()))
        );
        assert_eq!(
            config.get("port"),
            Some(&ConfigValue::String("8080".to_string()))
        );
        assert_eq!(
            config.get("fallback"),
            Some(&ConfigValue::String("default_value".to_string()))
        );

        std::env::remove_var("PARSER_TEST_CONFIG_VAR");
        std::env::remove_var("PARSER_TEST_PORT");
    }

    #[test]
    fn test_parser_with_custom_limits() {
        let parser = ConfigParser::with_limits(500, 5);
        assert_eq!(parser.max_file_size(), 500);
        assert_eq!(parser.max_nesting_depth(), 5);
    }

    #[test]
    fn test_utf8_validation() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid_utf8.toml");

        // Write invalid UTF-8 bytes
        let invalid_utf8 = vec![0xFF, 0xFE, 0xFD];
        fs::write(&config_path, invalid_utf8).unwrap();

        let result = parse_config_file(&config_path);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::InvalidUtf8(_)));
    }

    #[test]
    fn test_empty_toml_file() {
        let config = parse_config_string("").unwrap();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);
    }

    #[test]
    fn test_toml_array_parsing() {
        let toml_content = r#"
            simple_array = ["item1", "item2", "item3"]
            mixed_array = ["string", 42, true, 3.15]
            
            [[table_array]]
            name = "first"
            value = 1
            
            [[table_array]]
            name = "second"
            value = 2
        "#;

        let config = parse_config_string(toml_content).unwrap();

        // Test simple array
        if let Some(ConfigValue::Array(array)) = config.get("simple_array") {
            assert_eq!(array.len(), 3);
            assert_eq!(array[0], ConfigValue::String("item1".to_string()));
        } else {
            panic!("Expected array for simple_array");
        }

        // Test mixed array
        if let Some(ConfigValue::Array(array)) = config.get("mixed_array") {
            assert_eq!(array.len(), 4);
            assert_eq!(array[0], ConfigValue::String("string".to_string()));
            assert_eq!(array[1], ConfigValue::Integer(42));
            assert_eq!(array[2], ConfigValue::Boolean(true));
            assert_eq!(array[3], ConfigValue::Float(3.15));
        } else {
            panic!("Expected array for mixed_array");
        }
    }
}
