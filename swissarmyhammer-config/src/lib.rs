//! SwissArmyHammer Configuration System using Figment
//!
//! This crate provides configuration management for SwissArmyHammer using the `figment` library.
//! It supports multiple configuration file formats (TOML, YAML, JSON) with a clear precedence
//! order and environment variable integration.
//!
//! # Features
//!
//! - **Multiple File Formats**: Support for TOML, YAML, and JSON configuration files
//! - **Precedence Order**: Configuration sources are merged with clear precedence rules
//! - **Environment Variables**: Support for environment variable substitution and overrides
//! - **File Discovery**: Automatic discovery of configuration files in standard locations
//! - **Validation**: Comprehensive configuration validation and error reporting
//!
//! # Configuration File Discovery
//!
//! The system searches for configuration files in the following locations and formats:
//!
//! ## Project Configuration
//! - `./.swissarmyhammer/sah.{toml,yaml,yml,json}`
//! - `./.swissarmyhammer/swissarmyhammer.{toml,yaml,yml,json}`
//!
//! ## User Configuration
//! - `~/.swissarmyhammer/sah.{toml,yaml,yml,json}`
//! - `~/.swissarmyhammer/swissarmyhammer.{toml,yaml,yml,json}`
//!
//! # Precedence Order
//!
//! Configuration sources are merged in the following order (later sources override earlier ones):
//!
//! 1. **Default values** (hardcoded in application)
//! 2. **Global config file** (`~/.swissarmyhammer/` directory)
//! 3. **Project config file** (`.swissarmyhammer/` directory in current project)
//! 4. **Environment variables** (with `SAH_` or `SWISSARMYHAMMER_` prefix)
//! 5. **Command line arguments** (highest priority)
//!
//! # Example
//!
//! ```no_run
//! use swissarmyhammer_config::{ConfigError, ConfigResult};
//!
//! // This will be implemented in future iterations
//! // let config = swissarmyhammer_config::load_config()?;
//! # Ok::<(), ConfigError>(())
//! ```

/// Error types and result aliases for configuration operations
pub mod error;

/// Core data structures for configuration system
pub mod types;

/// Configuration file discovery system
pub mod discovery;

/// Configuration provider using Figment
pub mod provider;

/// Default configuration values
pub mod defaults;

/// Integration tests (only compiled in test mode)
#[cfg(test)]
pub mod tests;

/// Template renderer with TemplateContext integration
pub mod renderer;



/// Integration tests
#[cfg(test)]
pub mod integration_test;

/// Environment variable substitution for template contexts
pub mod env_substitution;

// Re-export main types for easier access
pub use defaults::ConfigDefaults;
pub use discovery::{ConfigFile, ConfigFormat, ConfigScope, FileDiscovery};
pub use error::{ConfigError, ConfigResult};
pub use provider::ConfigProvider;
pub use renderer::TemplateRenderer;
pub use types::{RawConfig, TemplateContext};

/// Current version of the configuration system
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
