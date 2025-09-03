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

/// Agent configuration types and infrastructure
pub mod agent;
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
pub use agent::{
    AgentConfig, AgentExecutorConfig, AgentExecutorType, ClaudeCodeConfig, LlamaAgentConfig,
    McpServerConfig, ModelConfig, ModelSource,
};
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

/// Default LLM model repository for testing
///
/// This constant specifies the Hugging Face repository for the default test LLM model.
/// Qwen3-1.7B is chosen as the test model because it provides:
/// - Small size (suitable for CI/CD environments)
/// - Fast inference (minimizes test execution time)
/// - High quality instruction following (reliable test behavior)
/// - Local execution capability (no API dependencies)
///
/// Used in conjunction with [`DEFAULT_TEST_LLM_MODEL_FILENAME`] to configure
/// test LlamaAgent instances across all packages.
pub const DEFAULT_TEST_LLM_MODEL_REPO: &str = "unsloth/Qwen3-1.7B-GGUF";

/// Default LLM model filename for testing
///
/// This constant specifies the specific GGUF file within the repository
/// defined by [`DEFAULT_TEST_LLM_MODEL_REPO`]. The Q6_K_XL quantization
/// provides an optimal balance between:
/// - Model quality (maintains instruction following capability)  
/// - File size (~1.2GB - efficient downloads and storage)
/// - Inference speed (fast enough for test suites with 1.7B parameters)
/// - Memory usage (runs on typical development machines with ~2-3GB RAM)
///
/// This file will be automatically downloaded by llama.cpp when first accessed.
pub const DEFAULT_TEST_LLM_MODEL_FILENAME: &str = "Qwen3-1.7B-UD-Q6_K_XL.gguf";

/// Default embedding model for testing
///
/// This constant specifies the embedding model used for all semantic search
/// and embedding-related tests. BGE-small-en-v1.5 is selected because it:
/// - Generates 384-dimensional embeddings (manageable size for tests)
/// - Provides good semantic understanding for English text
/// - Has fast inference speed suitable for test environments
/// - Is well-supported by the fastembed library
/// - Maintains consistent behavior across different platforms
///
/// All embedding tests use this model to ensure consistent vector dimensions
/// and semantic behavior across the test suite.
pub const DEFAULT_TEST_EMBEDDING_MODEL: &str = "BAAI/bge-small-en-v1.5";

/// Test configuration utilities for LlamaAgent testing
pub mod test_config {
    use crate::agent::{
        AgentConfig, AgentExecutorType, LlamaAgentConfig, McpServerConfig, ModelConfig, ModelSource,
    };
    use std::env;

    /// Test configuration for different environments
    #[derive(Debug, Clone)]
    pub struct TestConfig {
        pub enable_llama_tests: bool,
        pub enable_claude_tests: bool,
        pub test_timeout_seconds: u64,
        pub llama_model_repo: String,
        pub llama_model_filename: String,
    }

    impl TestConfig {
        pub fn from_environment() -> Self {
            Self {
                enable_llama_tests: env::var("SAH_TEST_LLAMA")
                    .map(|v| v.to_lowercase() == "true" || v == "1")
                    .unwrap_or(false),
                enable_claude_tests: env::var("SAH_TEST_CLAUDE")
                    .map(|v| v.to_lowercase() == "true" || v == "1")
                    .unwrap_or(true),
                test_timeout_seconds: env::var("SAH_TEST_TIMEOUT")
                    .ok()
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(120),
                llama_model_repo: env::var("SAH_TEST_MODEL_REPO")
                    .unwrap_or_else(|_| crate::DEFAULT_TEST_LLM_MODEL_REPO.to_string()),
                llama_model_filename: env::var("SAH_TEST_MODEL_FILENAME")
                    .unwrap_or_else(|_| crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
            }
        }

        pub fn create_llama_config(&self) -> LlamaAgentConfig {
            LlamaAgentConfig {
                model: ModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: self.llama_model_repo.clone(),
                        filename: Some(self.llama_model_filename.clone()),
                        folder: None,
                    },
                    batch_size: 256, // Smaller batch size for testing
                    use_hf_params: true,
                    debug: true, // Enable debug for testing
                },
                mcp_server: McpServerConfig {
                    port: 0,
                    timeout_seconds: 30,
                },

                repetition_detection: Default::default(),
            }
        }

        pub fn create_claude_config() -> AgentConfig {
            AgentConfig::claude_code()
        }

        pub fn create_llama_agent_config(&self) -> AgentConfig {
            AgentConfig::llama_agent(self.create_llama_config())
        }
    }

    /// Skip test if LlamaAgent testing is disabled
    pub fn skip_if_llama_disabled() {
        let config = TestConfig::from_environment();
        if !config.enable_llama_tests {
            println!("Skipping LlamaAgent test (set SAH_TEST_LLAMA=true to enable)");
        }
    }

    /// Skip test if Claude testing is disabled  
    pub fn skip_if_claude_disabled() {
        let config = TestConfig::from_environment();
        if !config.enable_claude_tests {
            println!("Skipping Claude test (set SAH_TEST_CLAUDE=false to disable)");
        }
    }

    /// Check if LlamaAgent tests are enabled
    pub fn is_llama_enabled() -> bool {
        let config = TestConfig::from_environment();
        config.enable_llama_tests
    }

    /// Check if Claude tests are enabled
    pub fn is_claude_enabled() -> bool {
        let config = TestConfig::from_environment();
        config.enable_claude_tests
    }

    /// Get enabled executor types for testing
    pub fn get_enabled_executors() -> Vec<AgentExecutorType> {
        let mut executors = Vec::new();

        if is_claude_enabled() {
            executors.push(AgentExecutorType::ClaudeCode);
        }

        if is_llama_enabled() {
            executors.push(AgentExecutorType::LlamaAgent);
        }

        executors
    }
}
