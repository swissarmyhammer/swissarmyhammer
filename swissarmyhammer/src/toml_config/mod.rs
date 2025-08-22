//! sah.toml configuration support for SwissArmyHammer
//!
//! This module provides core data structures and functionality for loading, parsing,
//! and validating sah.toml configuration files. It includes comprehensive error handling,
//! environment variable substitution, and support for all TOML data types.
//!
//! # Features
//!
//! - **Full TOML Support**: All TOML data types (String, Integer, Float, Boolean, Array, Table)
//! - **Environment Variable Substitution**: `${VAR:-default}` syntax support
//! - **Dot Notation Access**: Access nested values with `config.get("database.host")`
//! - **Comprehensive Validation**: File size, nesting depth, and syntax validation
//! - **Liquid Template Integration**: Convert values to liquid template objects
//! - **Detailed Error Reporting**: Context-aware error messages with line numbers
//!
//! # Quick Start
//!
//! ```no_run
//! use swissarmyhammer::toml_config::{load_config, ConfigError};
//! use std::path::Path;
//!
//! // Load configuration from file
//! let config = load_config(Path::new("sah.toml"))?;
//!
//! // Access simple values
//! if let Some(name) = config.get("name") {
//!     println!("Project name: {}", name.coerce_to_string()?);
//! }
//!
//! // Access nested values with dot notation
//! if let Some(host) = config.get("database.host") {
//!     println!("Database host: {}", host.coerce_to_string()?);
//! }
//!
//! # Ok::<(), ConfigError>(())
//! ```
//!
//! # Configuration File Format
//!
//! The sah.toml file uses standard TOML syntax:
//!
//! ```toml
//! # Project metadata
//! name = "MyProject"
//! version = "1.0.0"
//! debug = true
//!
//! # Environment variable substitution
//! database_url = "${DATABASE_URL:-postgresql://localhost:5432/myapp}"
//!
//! # Nested configuration
//! [database]
//! host = "localhost"
//! port = 5432
//!
//! [database.credentials]
//! username = "user"
//! password = "${DB_PASSWORD}"
//!
//! # Arrays
//! features = ["feature1", "feature2"]
//! ports = [8080, 8081, 8082]
//! ```

/// Configuration error types and validation limits
pub mod error;

/// ConfigValue enum and type conversion functionality
pub mod value;

/// Configuration struct with dot notation support
pub mod configuration;

/// TOML parser with validation and error handling
pub mod parser;

/// Comprehensive test suite for configuration system
#[cfg(test)]
pub mod tests;

// Re-export main types for easier access
pub use configuration::Configuration;
pub use error::{ConfigError, ValidationLimits};
pub use parser::{load_repo_config, parse_config_file, parse_config_string, ConfigParser};
pub use value::ConfigValue;

/// Load and parse a sah.toml configuration file
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file
///
/// # Returns
/// * `Result<Configuration, ConfigError>` - The parsed configuration or an error
///
/// # Example
/// ```no_run
/// use swissarmyhammer::toml_config::load_config;
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
/// * `Result<Option<Configuration>, ConfigError>` - The configuration if found, None if no file exists
///
/// # Example
/// ```no_run
/// use swissarmyhammer::toml_config::load_repo_config_wrapper;
///
/// match load_repo_config_wrapper()? {
///     Some(config) => println!("Found configuration with {} values", config.len()),
///     None => println!("No sah.toml found in repository"),
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_repo_config_wrapper() -> Result<Option<Configuration>, ConfigError> {
    ConfigParser::new().load_from_repo_root()
}

/// Validate a sah.toml configuration file
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file to validate
///
/// # Returns
/// * `Result<(), ConfigError>` - Ok if valid, ConfigError if invalid
///
/// # Example
/// ```no_run
/// use swissarmyhammer::toml_config::validate_config_file;
/// use std::path::Path;
///
/// validate_config_file(Path::new("sah.toml"))?;
/// println!("Configuration file is valid");
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn validate_config_file(file_path: &std::path::Path) -> Result<(), ConfigError> {
    let parser = ConfigParser::new();
    parser.validate_file(file_path)?;
    let config = parser.parse_file(file_path)?;
    config.validate()?;
    Ok(())
}

/// Parse TOML content from a string
///
/// # Arguments
/// * `contents` - The TOML content as a string
///
/// # Returns
/// * `Result<Configuration, ConfigError>` - The parsed configuration or an error
///
/// # Example
/// ```
/// use swissarmyhammer::toml_config::parse_toml_string;
///
/// let toml_content = r#"
///     name = "Test Project"
///     version = "1.0.0"
///     
///     [database]
///     host = "localhost"
///     port = 5432
/// "#;
///
/// let config = parse_toml_string(toml_content)?;
/// assert_eq!(config.get("name").unwrap().coerce_to_string()?, "Test Project");
/// assert_eq!(config.get("database.port").unwrap().coerce_to_integer()?, 5432);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn parse_toml_string(contents: &str) -> Result<Configuration, ConfigError> {
    ConfigParser::new().parse_string(contents, None)
}

#[cfg(test)]
mod module_tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_load_config() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let config_path = temp_dir.path().join("test_config.toml");

        let toml_content = r#"
            name = "IntegrationTest"
            version = "1.0.0"
            
            [database]
            host = "localhost"
            port = 5432
        "#;

        fs::write(&config_path, toml_content).unwrap();

        let config = load_config(&config_path).unwrap();
        assert_eq!(
            config.get("name").unwrap().coerce_to_string().unwrap(),
            "IntegrationTest"
        );
        assert_eq!(
            config
                .get("database.port")
                .unwrap()
                .coerce_to_integer()
                .unwrap(),
            5432
        );
    }

    #[test]
    fn test_parse_toml_string() {
        let toml_content = "name = \"test\""; // Simplest possible TOML

        let config = parse_toml_string(toml_content).unwrap();
        assert_eq!(
            config.get("name").unwrap().coerce_to_string().unwrap(),
            "test"
        );
    }

    #[test]
    fn test_validate_config_file() {
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let config_path = temp_dir.path().join("valid_config.toml");

        let valid_toml = r#"
            name = "ValidConfig"
            valid_key = "valid_value"
            number = 42
        "#;

        fs::write(&config_path, valid_toml).unwrap();

        let result = validate_config_file(&config_path);
        assert!(result.is_ok());

        // Test invalid config
        let invalid_config_path = temp_dir.path().join("invalid_config.toml");
        let invalid_toml = r#"
            123invalid = "value"  # Invalid variable name
        "#;

        fs::write(&invalid_config_path, invalid_toml).unwrap();

        let result = validate_config_file(&invalid_config_path);
        assert!(result.is_err());
    }

    #[test]
    fn test_load_repo_config_wrapper() {
        use std::panic;

        // This test creates a temporary directory structure and tests repo config loading
        let temp_dir = crate::test_utils::create_temp_dir_with_retry();
        let git_dir = temp_dir.path().join(".git");
        fs::create_dir(&git_dir).unwrap();

        let config_path = temp_dir.path().join("sah.toml");
        fs::write(&config_path, "name = \"repo_config_test\"").unwrap();

        // Test from subdirectory with proper RAII cleanup
        let sub_dir = temp_dir.path().join("subdir");
        fs::create_dir(&sub_dir).unwrap();

        let original_dir = std::env::current_dir().expect("Failed to get current directory");
        std::env::set_current_dir(&sub_dir).expect("Failed to change to sub directory");

        // Use panic::catch_unwind to ensure directory is restored even on panic
        let result = panic::catch_unwind(load_repo_config_wrapper);

        // Always restore original directory
        std::env::set_current_dir(original_dir).expect("Failed to restore original directory");

        let config_result = result.unwrap();
        assert!(config_result.is_ok());
        let config = config_result.unwrap();
        assert!(config.is_some());

        let config = config.unwrap();
        assert_eq!(
            config.get("name").unwrap().coerce_to_string().unwrap(),
            "repo_config_test"
        );
    }

    #[test]
    fn test_comprehensive_config_example() {
        let toml_content = r#"
            # Project metadata
            name = "SwissArmyHammer"
            version = "2.0.0"
            description = "A flexible prompt and workflow management tool"
            license = "MIT"
            
            # Arrays
            keywords = ["cli", "automation", "templates"]
            maintainers = ["alice@example.com", "bob@example.com"]
            
            # Team information
            [team]
            lead = "alice@example.com"
            size = 5
            
            # Build configuration
            [build]
            language = "rust"
            features = ["async", "cli", "templates"]
            optimized = true
            
            # Nested database configuration
            [database.primary]
            host = "localhost"
            port = 5432
            name = "swissarmyhammer"
            
            [database.replica]
            host = "replica.example.com"
            port = 5432
            readonly = true
        "#;

        let config = parse_toml_string(toml_content).unwrap();

        // Test basic values
        assert_eq!(
            config.get("name").unwrap().coerce_to_string().unwrap(),
            "SwissArmyHammer"
        );
        assert_eq!(
            config.get("version").unwrap().coerce_to_string().unwrap(),
            "2.0.0"
        );

        // Test arrays
        if let Some(ConfigValue::Array(keywords)) = config.get("keywords") {
            assert_eq!(keywords.len(), 3);
            assert_eq!(keywords[0], ConfigValue::String("cli".to_string()));
        }

        // Test nested values
        assert_eq!(
            config.get("team.lead").unwrap().coerce_to_string().unwrap(),
            "alice@example.com"
        );
        assert_eq!(
            config
                .get("team.size")
                .unwrap()
                .coerce_to_integer()
                .unwrap(),
            5
        );
        assert!(config
            .get("build.optimized")
            .unwrap()
            .coerce_to_boolean()
            .unwrap());

        // Test deeply nested values
        assert_eq!(
            config
                .get("database.primary.host")
                .unwrap()
                .coerce_to_string()
                .unwrap(),
            "localhost"
        );
        assert_eq!(
            config
                .get("database.primary.port")
                .unwrap()
                .coerce_to_integer()
                .unwrap(),
            5432
        );
        assert!(config
            .get("database.replica.readonly")
            .unwrap()
            .coerce_to_boolean()
            .unwrap());

        // Test liquid conversion
        let liquid_object = config.to_liquid_object();
        assert!(!liquid_object.is_empty());
    }
}
