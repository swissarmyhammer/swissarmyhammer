//! SwissArmyHammer Configuration Management using Figment
//!
//! This crate provides comprehensive configuration management for SwissArmyHammer applications
//! using the figment library. It supports multiple configuration file formats with proper
//! precedence handling, environment variable substitution, and seamless template integration.
//!
//! # Features
//!
//! - **Multiple file formats**: TOML, YAML, JSON with automatic format detection
//! - **File discovery**: Automatic discovery in standard `.swissarmyhammer/` directories  
//! - **Environment integration**: Full environment variable support with `SAH_` and `SWISSARMYHAMMER_` prefixes
//! - **Variable substitution**: Shell-style `${VAR}` and `${VAR:-default}` syntax in config files
//! - **Proper precedence**: Clear precedence ordering: defaults → global → project → env → CLI
//! - **Template context**: Direct integration with liquid templating via `TemplateContext`
//! - **No caching**: Fresh configuration loaded on each access for edit-friendly development
//! - **Type safety**: Strongly typed configuration with comprehensive error handling
//!
//! # Quick Start
//!
//! The simplest way to get started is to use the main entry point:
//!
//! ```no_run
//! use swissarmyhammer_config::load_configuration;
//!
//! // Load configuration from all available sources
//! let context = load_configuration()?;
//!
//! // Access configuration values
//! if let Some(app_name) = context.get("app.name") {
//!     println!("Application: {}", app_name);
//! }
//!
//! // Use with liquid templates
//! let liquid_context = context.to_liquid_context();
//! # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
//! ```
//!
//! # Configuration Files
//!
//! SwissArmyHammer discovers configuration files in these locations:
//!
//! - Global: `~/.swissarmyhammer/sah.{toml,yaml,yml,json}`
//! - Project: `./.swissarmyhammer/sah.{toml,yaml,yml,json}`
//!
//! ## Example TOML Configuration
//!
//! ```toml
//! [app]
//! name = "MyProject"
//! version = "1.0.0"
//! debug = false
//!
//! [database]
//! host = "localhost"
//! port = 5432
//! url = "${DATABASE_URL:-postgresql://localhost:5432/mydb}"
//!
//! [features]
//! experimental = false
//! telemetry = true
//! ```
//!
//! ## Example YAML Configuration
//!
//! ```yaml
//! app:
//!   name: MyProject
//!   version: "1.0.0"
//!   debug: false
//!
//! database:
//!   host: localhost
//!   port: 5432
//!   url: "${DATABASE_URL:-postgresql://localhost:5432/mydb}"
//!
//! features:
//!   experimental: false
//!   telemetry: true
//! ```
//!
//! # Environment Variables
//!
//! Environment variables are automatically mapped to configuration keys:
//!
//! ```bash
//! export SAH_APP_NAME="MyProject"          # → app.name
//! export SAH_DATABASE_HOST="localhost"     # → database.host  
//! export SAH_DATABASE_PORT="5432"          # → database.port
//! export SAH_DEBUG="true"                  # → debug
//! ```
//!
//! # Template Integration
//!
//! Configuration values are automatically available in liquid templates:
//!
//! ```liquid
//! # {{app.name}} Configuration
//!
//! **Version:** {{app.version}}
//! **Debug:** {% if debug %}enabled{% else %}disabled{% endif %}
//! **Database:** {{database.host}}:{{database.port}}
//!
//! ## Features
//! {% for feature in features -%}
//! - {{feature[0] | capitalize}}: {% if feature[1] %}✓{% else %}✗{% endif %}
//! {% endfor %}
//! ```
//!
//! # Advanced Usage
//!
//! ## Custom Template Variables
//!
//! Combine configuration with runtime template variables:
//!
//! ```no_run
//! use swissarmyhammer_config::TemplateContext;
//! use std::collections::HashMap;
//! use serde_json::json;
//!
//! let mut template_vars = HashMap::new();
//! template_vars.insert("task".to_string(), json!("deploy"));
//! template_vars.insert("user".to_string(), json!("admin"));
//!
//! // Template variables override configuration values
//! let context = TemplateContext::with_template_vars(template_vars)?;
//! # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
//! ```
//!
//! ## CLI Integration
//!
//! For CLI applications, use the specialized CLI loader:
//!
//! ```no_run
//! use swissarmyhammer_config::load_configuration_for_cli;
//!
//! // Load configuration for CLI usage (bypasses path validation)
//! let context = load_configuration_for_cli()?;
//! # Ok::<(), swissarmyhammer_config::ConfigurationError>(())
//! ```
//!
//! ## Programmatic Configuration
//!
//! Build configuration programmatically when needed:
//!
//! ```
//! use swissarmyhammer_config::TemplateContext;
//! use serde_json::json;
//!
//! let mut context = TemplateContext::new();
//! context.set("app.name".to_string(), json!("MyApp"));
//! context.set("app.debug".to_string(), json!(true));
//! context.set("database.host".to_string(), json!("localhost"));
//!
//! assert_eq!(context.get("app.name"), Some(&json!("MyApp")));
//! assert_eq!(context.get("database.host"), Some(&json!("localhost")));
//! ```
//!
//! # Error Handling
//!
//! The crate provides comprehensive error handling for common configuration issues:
//!
//! ```no_run
//! use swissarmyhammer_config::{load_configuration, ConfigurationError};
//!
//! match load_configuration() {
//!     Ok(context) => {
//!         println!("Loaded {} configuration values", context.len());
//!     },
//!     Err(ConfigurationError::FileNotFound { path, .. }) => {
//!         eprintln!("Configuration file not found: {}", path);
//!     },
//!     Err(ConfigurationError::ParseError { source, .. }) => {
//!         eprintln!("Configuration parsing failed: {}", source);
//!     },
//!     Err(err) => {
//!         eprintln!("Configuration error: {}", err);
//!     }
//! }
//! ```

/// File discovery logic for configuration files
pub mod discovery;
/// Environment variable processing and substitution
pub mod env_vars;
/// Error types and handling
pub mod error;
/// Core configuration provider trait and implementations
pub mod provider;
/// Template context integration
pub mod template_context;

// Re-export main types for easier access
pub use discovery::{ConfigurationDiscovery, DiscoveryPaths};
pub use env_vars::EnvVarSubstitution;
pub use error::{ConfigurationError, ConfigurationResult};
pub use provider::ConfigurationProvider;
pub use template_context::TemplateContext;

/// Load configuration from discovered sources with proper precedence
///
/// This is the main entry point for loading configuration. It discovers configuration
/// files in standard locations (global and project `.swissarmyhammer/` directories)
/// and merges them according to the precedence rules:
///
/// 1. Default values (lowest precedence)
/// 2. Global config files (`~/.swissarmyhammer/sah.*`)
/// 3. Project config files (`./.swissarmyhammer/sah.*`)  
/// 4. Environment variables (`SAH_*` and `SWISSARMYHAMMER_*`)
/// 5. CLI arguments (highest precedence, if provided)
///
/// The function supports multiple file formats (TOML, YAML, JSON) and performs
/// environment variable substitution using `${VAR}` and `${VAR:-default}` syntax.
///
/// # Returns
/// * `ConfigurationResult<TemplateContext>` - The merged template context or an error
///
/// # Errors
///
/// Returns `ConfigurationError` for issues such as:
/// - Invalid file syntax (malformed TOML, YAML, or JSON)
/// - Environment variable substitution failures  
/// - File permission or access issues
/// - Type conversion errors
///
/// # Examples
///
/// ## Basic Usage
/// ```no_run
/// use swissarmyhammer_config::load_configuration;
///
/// // Load all available configuration
/// let context = load_configuration()?;
/// println!("Loaded configuration with {} variables", context.len());
///
/// // Access specific values
/// if let Some(app_name) = context.get("app.name") {
///     println!("Application: {}", app_name);
/// }
///
/// // Access nested values  
/// if let Some(db_host) = context.get("database.host") {
///     println!("Database host: {}", db_host);
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## Template Integration
/// ```no_run
/// use swissarmyhammer_config::load_configuration;
///
/// let context = load_configuration()?;
/// let liquid_context = context.to_liquid_context();
///
/// // Use with liquid template
/// let template_source = "Welcome to {{app.name}} v{{app.version}}!";
/// let parser = liquid::ParserBuilder::with_stdlib().build()?;
/// let template = parser.parse(template_source)?;
/// let output = template.render(&liquid_context)?;
///
/// println!("{}", output); // "Welcome to MyApp v1.0.0!"
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## Error Handling
/// ```no_run
/// use swissarmyhammer_config::{load_configuration, ConfigurationError};
///
/// match load_configuration() {
///     Ok(context) => {
///         println!("Configuration loaded successfully");
///         for (key, value) in context.variables() {
///             println!("{}: {}", key, value);
///         }
///     },
///     Err(ConfigurationError::ParseError { source, path, .. }) => {
///         eprintln!("Failed to parse {}: {}", path.display(), source);
///     },
///     Err(ConfigurationError::EnvironmentVariableError { variable, source, .. }) => {
///         eprintln!("Environment variable {} error: {}", variable, source);
///     },
///     Err(err) => {
///         eprintln!("Configuration error: {}", err);
///     }
/// }
/// ```
pub fn load_configuration() -> ConfigurationResult<TemplateContext> {
    TemplateContext::load()
}

/// Load configuration for CLI usage (disables path validation)
///
/// Similar to `load_configuration` but designed for CLI tools that may need to
/// load configuration from any location. This function disables security-focused
/// path validation that's appropriate for server contexts but restrictive for
/// CLI usage.
///
/// Use this function when:
/// - Building CLI tools that need flexible configuration loading
/// - Working in development environments with non-standard paths
/// - Creating utilities that process configuration files from arbitrary locations
///
/// # Security Note
///
/// This function bypasses path validation for flexibility. Only use it in CLI
/// contexts where the security implications are acceptable.
///
/// # Returns
/// * `ConfigurationResult<TemplateContext>` - The merged template context or an error
///
/// # Examples
///
/// ## CLI Application Usage
/// ```no_run
/// use swissarmyhammer_config::load_configuration_for_cli;
///
/// // Load configuration with relaxed path restrictions
/// let context = load_configuration_for_cli()?;
///
/// // Perfect for CLI tools that need configuration access
/// if let Some(debug) = context.get("debug") {
///     if debug.as_bool().unwrap_or(false) {
///         println!("Debug mode enabled");
///     }
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// ## Development Tool Usage
/// ```no_run
/// use swissarmyhammer_config::load_configuration_for_cli;
/// use std::env;
///
/// // CLI tool that respects configuration
/// fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let context = load_configuration_for_cli()?;
///     
///     // Get configuration or use CLI args as fallback
///     let output_format = env::args()
///         .find(|arg| arg.starts_with("--format="))
///         .map(|arg| arg.split('=').nth(1).unwrap_or("json").to_string())
///         .or_else(|| context.get("output.format").and_then(|v| v.as_str().map(String::from)))
///         .unwrap_or_else(|| "json".to_string());
///         
///     println!("Using output format: {}", output_format);
///     Ok(())
/// }
/// ```
pub fn load_configuration_for_cli() -> ConfigurationResult<TemplateContext> {
    TemplateContext::load_for_cli()
}
