//! # SwissArmyHammer Configuration System
//!
//! This crate provides a configuration system for SwissArmyHammer using the `figment` library.
//! It supports multiple configuration file formats (TOML, YAML, JSON) with a clear precedence
//! order and environment variable integration.
//!
//! ## Features
//!
//! - **Multiple formats**: TOML, YAML, and JSON configuration files
//! - **File discovery**: Automatic search in project and home directories
//! - **Precedence order**: Clear hierarchy for configuration sources
//! - **Environment variables**: Support for `SAH_` and `SWISSARMYHAMMER_` prefixed variables
//! - **Template integration**: Direct integration with liquid templating system
//!
//! ## Configuration File Discovery
//!
//! The system searches for configuration files in the following order:
//! 1. Project directory: `./.swissarmyhammer/{sah,swissarmyhammer}.{toml,yaml,yml,json}`
//! 2. Home directory: `~/.swissarmyhammer/{sah,swissarmyhammer}.{toml,yaml,yml,json}`
//!
//! ## Configuration Precedence
//!
//! Configuration sources are merged in the following order (later sources override earlier ones):
//! 1. Default values (hardcoded in application)
//! 2. Global config file (`~/.swissarmyhammer/` directory)
//! 3. Project config file (`.swissarmyhammer/` directory in current project)
//! 4. Environment variables (with `SAH_` or `SWISSARMYHAMMER_` prefix)
//! 5. Command line arguments (highest priority)
//!
//! ## Usage
//!
//! This crate is designed to be used by the SwissArmyHammer ecosystem for template variable
//! provisioning. It does not cache configuration data, allowing for immediate updates when
//! configuration files are modified.

/// Configuration error types
pub mod error;

// Re-export main types for easier access
pub use error::ConfigError;

/// The result type used throughout this crate
pub type Result<T> = std::result::Result<T, ConfigError>;
