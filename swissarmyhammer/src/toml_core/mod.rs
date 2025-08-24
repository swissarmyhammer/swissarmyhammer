//! Core data structures for sah.toml configuration support
//!
//! This module provides the fundamental building blocks for SwissArmyHammer's configuration
//! system, including TOML parsing, value representation, nested table access, and
//! environment variable substitution.
//!
//! # Key Features
//!
//! - **Complete TOML Type Support**: All TOML data types including tables, arrays, strings,
//!   integers, floats, booleans, and datetimes
//! - **Dot Notation Access**: Navigate nested configuration with paths like "database.host"
//! - **Environment Variable Substitution**: Support for `${VAR:-default}` patterns
//! - **Comprehensive Validation**: File size limits, nesting depth limits, UTF-8 validation
//! - **Type Coercion**: Flexible conversion between configuration value types
//! - **Template Integration**: Direct conversion to Liquid template values and JSON
//!
//! # Core Types
//!
//! - [`ConfigValue`] - Represents any value from sah.toml with type-safe operations
//! - [`Configuration`] - Main container for configuration data with nested access
//! - [`ConfigParser`] - TOML file parser with validation and error handling
//! - [`ConfigError`] - Comprehensive error types with detailed context
//!
//! # Examples
//!
//! ## Basic Configuration Loading
//!
//! ```no_run
//! use swissarmyhammer::toml_core::{ConfigParser, ConfigValue};
//! use std::path::Path;
//!
//! let parser = ConfigParser::new();
//! let config = parser.parse_file(Path::new("sah.toml"))?;
//!
//! // Access simple values
//! if let Some(ConfigValue::String(name)) = config.get("project_name") {
//!     println!("Project: {}", name);
//! }
//!
//! // Access nested values with dot notation
//! let host = config.get_nested("database.host")?;
//! if let Some(ConfigValue::String(hostname)) = host {
//!     println!("Database host: {}", hostname);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Environment Variable Substitution
//!
//! ```no_run
//! use swissarmyhammer::toml_core::{ConfigParser, ConfigValue};
//!
//! let toml_content = r#"
//! database_url = "${DATABASE_URL:-postgresql://localhost:5432/myapp}"
//! api_key = "${API_KEY}"
//! debug = "${DEBUG:-false}"
//! "#;
//!
//! let parser = ConfigParser::new();
//! let mut config = parser.parse_string(toml_content)?;
//!
//! // Process environment variable substitution
//! config.substitute_env_vars()?;
//!
//! // Values now contain actual environment variable values or defaults
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Creating and Modifying Configuration
//!
//! ```
//! use swissarmyhammer::toml_core::{Configuration, ConfigValue};
//! use std::collections::HashMap;
//!
//! let mut config = Configuration::new();
//!
//! // Insert simple values
//! config.insert("name".to_string(), ConfigValue::String("MyProject".to_string()));
//! config.insert("version".to_string(), ConfigValue::Integer(1));
//!
//! // Insert nested values using dot notation
//! config.insert_nested("database.host", ConfigValue::String("localhost".to_string()))?;
//! config.insert_nested("database.port", ConfigValue::Integer(5432))?;
//!
//! // Access nested values
//! let port = config.get_nested("database.port")?;
//! assert_eq!(port, Some(&ConfigValue::Integer(5432)));
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! ## Type Coercion
//!
//! ```
//! use swissarmyhammer::toml_core::ConfigValue;
//!
//! let value = ConfigValue::String("42".to_string());
//!
//! // Coerce to different types
//! let as_int = value.coerce_to_integer()?;
//! assert_eq!(as_int, 42);
//!
//! let as_float = value.coerce_to_float()?;
//! assert_eq!(as_float, 42.0);
//!
//! let as_string = value.coerce_to_string()?;
//! assert_eq!(as_string, "42");
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Validation and Limits
//!
//! The configuration system enforces several limits for security and performance:
//!
//! - **File Size**: Maximum 1MB for sah.toml files
//! - **Nesting Depth**: Maximum 10 levels of nested tables
//! - **UTF-8 Encoding**: All files must be valid UTF-8
//! - **String Length**: Individual strings limited to 10KB
//! - **Array Size**: Arrays limited to 1000 elements
//!
//! These limits can be customized when creating a [`ConfigParser`]:
//!
//! ```
//! use swissarmyhammer::toml_core::ConfigParser;
//!
//! // Configure parser with file size limit
//! let parser = ConfigParser::with_max_file_size(2 * 1024 * 1024);  // 2MB limit
//!
//! // Or with depth limit
//! let parser = ConfigParser::with_max_depth(5);  // 5 levels max
//! ```

/// Configuration error types with detailed context and error chaining
pub mod error;

/// Configuration value types with support for all TOML data types and type coercion
pub mod value;

/// Main configuration container with nested table access and dot notation support  
pub mod configuration;

/// TOML parser with comprehensive validation and error handling
pub mod parser;

// Re-export the main types for easier access
pub use configuration::Configuration;
pub use error::ConfigError;
pub use parser::{parse_config_file, parse_config_string, ConfigParser};
pub use value::ConfigValue;

/// Load and parse a sah.toml configuration file
///
/// This is a convenience function that uses default parsing settings.
/// For more control over parsing behavior, use [`ConfigParser`] directly.
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file
///
/// # Returns
/// * `Ok(Configuration)` - The parsed configuration
/// * `Err(ConfigError)` - Parsing or validation error
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer::toml_core::load_config;
/// use std::path::Path;
///
/// let config = load_config(Path::new("sah.toml"))?;
/// println!("Loaded {} configuration values", config.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_config(file_path: &std::path::Path) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_file(file_path)
}

/// Load configuration from repository root
///
/// Searches for sah.toml in the current directory and parent directories
/// up to the repository root (indicated by .git directory).
///
/// # Returns
/// * `Ok(Some(Configuration))` - Configuration found and loaded
/// * `Ok(None)` - No sah.toml file found
/// * `Err(ConfigError)` - Error loading or parsing configuration
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer::toml_core::load_repo_config;
///
/// match load_repo_config()? {
///     Some(config) => println!("Found sah.toml with {} values", config.len()),
///     None => println!("No sah.toml file found in repository"),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_repo_config() -> Result<Option<Configuration>, ConfigError> {
    ConfigParser::new().load_from_repo_root()
}

/// Validate a sah.toml configuration file
///
/// This function loads and parses a configuration file, performing all
/// validation checks but discarding the result. Useful for validation-only
/// operations like `sah validate`.
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file to validate
///
/// # Returns
/// * `Ok(())` - File is valid
/// * `Err(ConfigError)` - Validation error with details
///
/// # Examples
///
/// ```no_run
/// use swissarmyhammer::toml_core::validate_config_file;
/// use std::path::Path;
///
/// match validate_config_file(Path::new("sah.toml")) {
///     Ok(()) => println!("✅ sah.toml is valid"),
///     Err(e) => eprintln!("❌ Validation failed: {}", e),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn validate_config_file(file_path: &std::path::Path) -> Result<(), ConfigError> {
    // Load and parse the file - this performs all validation
    load_config(file_path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_module_convenience_functions() -> Result<(), Box<dyn std::error::Error>> {
        // Create a test configuration file
        let mut temp_file = NamedTempFile::new()?;
        writeln!(
            temp_file,
            r#"
name = "TestProject"
version = 1
debug = true

[database]
host = "localhost"
port = 5432
"#
        )?;

        // Test load_config function
        let config = load_config(temp_file.path())?;
        assert_eq!(config.len(), 4);
        assert_eq!(
            config.get("name"),
            Some(&ConfigValue::String("TestProject".to_string()))
        );

        // Test validation function
        validate_config_file(temp_file.path())?;

        Ok(())
    }

    #[test]
    fn test_load_repo_config_not_found() -> Result<(), Box<dyn std::error::Error>> {
        use tempfile::TempDir;
        
        let _test_env = crate::test_utils::IsolatedTestEnvironment::new().unwrap();
        let temp_dir = TempDir::new()?;
        
        // Test from a directory that is not a git repository and has no sah.toml
        let parser = crate::toml_core::parser::ConfigParser::new();
        let result = parser.load_from_repo_root_with_start_dir(temp_dir.path())?;
        
        // Should return None when no config file is found
        assert!(result.is_none(), "Expected no config to be found in empty temp directory");
        Ok(())
    }

    #[test]
    fn test_validate_invalid_file() {
        // Test validation of non-existent file
        let result = validate_config_file(std::path::Path::new("nonexistent.toml"));
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::FileNotFound { .. }
        ));
    }

    #[test]
    fn test_validate_invalid_toml() -> Result<(), Box<dyn std::error::Error>> {
        let mut temp_file = NamedTempFile::new()?;
        writeln!(temp_file, "invalid toml content [")?;

        let result = validate_config_file(temp_file.path());
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::TomlParse { .. }));

        Ok(())
    }
}
