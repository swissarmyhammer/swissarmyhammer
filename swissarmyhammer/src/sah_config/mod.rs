//! sah.toml configuration support for SwissArmyHammer
//!
//! This module provides support for loading and parsing sah.toml configuration files,
//! integrating configuration variables with the liquid template engine, and validating
//! configuration files.

/// Configuration file loading and parsing functionality
pub mod loader;
/// Template integration for merging configuration with Liquid context
pub mod template_integration;
/// Core types for configuration values and structures
pub mod types;
/// Configuration validation rules and error handling
pub mod validation;

// Re-export the main types for easier access
pub use loader::{ConfigurationError, ConfigurationLoader};
pub use template_integration::{merge_config_into_context, load_and_merge_repo_config, substitute_env_vars};
pub use types::{ConfigValue, Configuration};
pub use validation::{ValidationError, ValidationRule, Validator};

/// Load and parse a sah.toml configuration file
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file
///
/// # Returns
/// * `Result<Configuration, ConfigurationError>` - The parsed configuration or an error
///
/// # Example
/// ```no_run
/// use swissarmyhammer::sah_config::load_config;
/// use std::path::Path;
///
/// let config = load_config(Path::new("sah.toml"))?;
/// println!("Loaded {} configuration values", config.len());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_config(file_path: &std::path::Path) -> Result<Configuration, ConfigurationError> {
    ConfigurationLoader::new().load_from_file(file_path)
}

/// Load configuration from repository root
///
/// Searches for sah.toml in the current directory and parent directories
/// up to the repository root.
///
/// # Returns
/// * `Result<Option<Configuration>, ConfigurationError>` - The configuration if found, None if no file exists
pub fn load_repo_config() -> Result<Option<Configuration>, ConfigurationError> {
    ConfigurationLoader::new().load_from_repo_root()
}

/// Validate a sah.toml configuration file
///
/// # Arguments
/// * `file_path` - Path to the sah.toml file to validate
///
/// # Returns
/// * `Result<(), ValidationError>` - Ok if valid, ValidationError if invalid
pub fn validate_config_file(file_path: &std::path::Path) -> Result<(), ValidationError> {
    let validator = Validator::new();
    let config = load_config(file_path)?;
    validator.validate(&config)
}
