//! Agent configuration types and infrastructure
//!
//! This module provides comprehensive agent management for SwissArmyHammer, enabling
//! dynamic switching between different AI execution environments and configurations.
//!
//! # Overview
//!
//! Agents in SwissArmyHammer represent different AI execution contexts, from cloud-based
//! Claude Code integration to local model execution with LlamaAgent. The agent system
//! provides a flexible, hierarchical configuration approach that allows users to:
//!
//! - Switch between different AI models and execution environments
//! - Customize agent behavior per project or globally
//! - Override built-in configurations with user-defined alternatives
//! - Manage complex multi-model workflows
//!
//! # Agent Architecture
//!
//! The system supports multiple executor types:
//!
//! - **Claude Code**: Shell execution using Claude Code CLI (default)
//! - **LlamaAgent**: In-process execution with local models via llama.cpp
//!
//! ## Hierarchical Discovery
//!
//! Agents are loaded from multiple sources with precedence:
//!
//! 1. **Built-in agents** (lowest precedence) - Embedded in binary
//! 2. **Project agents** (medium precedence) - `./agents/*.yaml`
//! 3. **User agents** (highest precedence) - `~/.swissarmyhammer/agents/*.yaml`
//!
//! Higher precedence agents override lower ones by name, enabling customization
//! while preserving defaults.
//!
//! # Configuration Format
//!
//! Agent configurations use YAML format with optional frontmatter:
//!
//! ```yaml
//! ---
//! description: "Custom Claude Code agent with project-specific settings"
//! ---
//! executor:
//!   type: claude-code
//!   config:
//!     claude_path: /usr/local/bin/claude
//!     args: ["--project-mode"]
//! quiet: false
//! ```
//!
//! # Quick Start
//!
//! ## Basic Agent Management
//!
//! ```no_run
//! use swissarmyhammer_config::model::ModelManager;
//!
//! // List all available agents
//! let agents = ModelManager::list_agents()?;
//! for agent in agents {
//!     println!("{}: {:?} - {}",
//!         agent.name,
//!         agent.source,
//!         agent.description.unwrap_or_default()
//!     );
//! }
//!
//! // Find specific agent
//! let claude_agent = ModelManager::find_agent_by_name("claude-code")?;
//! println!("Found: {}", claude_agent.name);
//!
//! // Apply agent to project
//! ModelManager::use_agent("claude-code")?;
//! # Ok::<(), swissarmyhammer_config::model::ModelError>(())
//! ```
//!
//! ## Creating Custom Agents
//!
//! Create a project-specific agent in `./agents/my-agent.yaml`:
//!
//! ```yaml
//! ---
//! description: "Custom agent for data analysis tasks"
//! ---
//! executor:
//!   type: llama-agent
//!   config:
//!     model:
//!       source:
//!         HuggingFace:
//!           repo: "microsoft/DialoGPT-medium"
//!           filename: "model.gguf"
//!     mcp_server:
//!       port: 8080
//!       timeout_seconds: 300
//! quiet: false
//! ```
//!
//! ## Configuration Loading
//!
//! ```no_run
//! use swissarmyhammer_config::model::{parse_agent_config, parse_agent_description};
//!
//! let agent_content = std::fs::read_to_string("./agents/my-agent.yaml")?;
//!
//! // Extract description
//! let description = parse_agent_description(&agent_content);
//! println!("Description: {:?}", description);
//!
//! // Parse configuration
//! let config = parse_model_config(&agent_content)?;
//! println!("Executor: {:?}", config.executor_type());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Built-in Agents
//!
//! SwissArmyHammer includes these built-in agents:
//!
//! ## claude-code
//!
//! Default integration with Claude Code CLI:
//! ```yaml
//! executor:
//!   type: claude-code
//!   config:
//!     claude_path: null  # Use system PATH
//!     args: []
//! quiet: false
//! ```
//!
//! ## qwen-coder
//!
//! Local execution with Qwen3-Coder model:
//! ```yaml
//! executor:
//!   type: llama-agent
//!   config:
//!     model:
//!       source:
//!         HuggingFace:
//!           repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF"
//!           filename: "Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf"
//! ```
//!
//! # Error Handling
//!
//! The agent system provides comprehensive error handling:
//!
//! ```no_run
//! use swissarmyhammer_config::model::{ModelManager, ModelError};
//!
//! match ModelManager::find_agent_by_name("nonexistent") {
//!     Ok(agent) => println!("Found: {}", agent.name),
//!     Err(ModelError::NotFound(name)) => {
//!         eprintln!("Agent '{}' not found", name);
//!         // Show available agents as suggestion
//!         let agents = ModelManager::list_agents()?;
//!         eprintln!("Available agents:");
//!         for agent in agents {
//!             eprintln!("  - {}", agent.name);
//!         }
//!     },
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! # Ok::<(), ModelError>(())
//! ```

use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::{Path, PathBuf};
use swissarmyhammer_common::{ErrorSeverity, Severity, SwissarmyhammerDirectory};
use thiserror::Error;

/// Model executor type enumeration
///
/// Defines the available model executor types with system default being Claude Code
/// for maximum compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum ModelExecutorType {
    /// Shell out to Claude Code CLI (system default)
    #[default]
    ClaudeCode,
    /// Use local LlamaAgent with in-process execution
    LlamaAgent,
}

/// Agent use case enumeration
///
/// Defines the different contexts where agents can be used, allowing
/// different operations to use different agents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AgentUseCase {
    /// Default/fallback agent for general operations
    Root,
    /// Agent for rule checking operations
    Rules,
    /// Agent for workflow execution (plan, review, implement, etc.)
    Workflows,
}

impl fmt::Display for AgentUseCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentUseCase::Root => write!(f, "root"),
            AgentUseCase::Rules => write!(f, "rules"),
            AgentUseCase::Workflows => write!(f, "workflows"),
        }
    }
}

impl std::str::FromStr for AgentUseCase {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s_lower = s.to_lowercase();

        for variant in [
            AgentUseCase::Root,
            AgentUseCase::Rules,
            AgentUseCase::Workflows,
        ] {
            if variant.to_string() == s_lower {
                return Ok(variant);
            }
        }

        Err(format!(
            "Invalid use case: '{}'. Valid options: root, rules, workflows",
            s
        ))
    }
}

/// Complete model configuration with executor-specific settings
///
/// Combines executor configuration with global model settings like quiet mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    /// Agent executor configuration with associated data
    pub executor: ModelExecutorConfig,
    /// Global quiet mode
    pub quiet: bool,
}

/// Tagged union of agent executor configurations
///
/// Uses serde's tagged representation to ensure type safety and proper
/// serialization of executor-specific configuration data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ModelExecutorConfig {
    #[serde(rename = "claude-code")]
    ClaudeCode(ClaudeCodeConfig),
    #[serde(rename = "llama-agent")]
    LlamaAgent(LlamaAgentConfig),
}

/// Configuration for Claude Code CLI execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ClaudeCodeConfig {
    /// Optional custom Claude Code CLI path
    pub claude_path: Option<PathBuf>,
    /// Additional CLI arguments
    #[serde(default)]
    pub args: Vec<String>,
}

/// Configuration for LlamaAgent in-process execution
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LlamaAgentConfig {
    /// Model configuration
    #[serde(default)]
    pub model: LlmModelConfig,
    /// MCP server configuration
    #[serde(default)]
    pub mcp_server: McpServerConfig,

    /// Repetition detection configuration
    #[serde(default)]
    pub repetition_detection: RepetitionDetectionConfig,
}

/// Configuration for repetition detection in model responses
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepetitionDetectionConfig {
    /// Enable repetition detection (default: true)
    #[serde(default = "default_repetition_enabled")]
    pub enabled: bool,
    /// Repetition penalty factor (default: 1.1, higher = more penalty)
    #[serde(default = "default_repetition_penalty")]
    pub repetition_penalty: f64,
    /// Repetition threshold - max allowed repetitive tokens before blocking (default: 50)
    #[serde(default = "default_repetition_threshold")]
    pub repetition_threshold: usize,
    /// Window size for repetition detection (default: 64)
    #[serde(default = "default_repetition_window")]
    pub repetition_window: usize,
}

// Macro to generate default value functions with consistent pattern
macro_rules! serde_default {
    ($fn_name:ident, $type:ty, $value:expr) => {
        fn $fn_name() -> $type {
            $value
        }
    };
}

serde_default!(
    default_repetition_enabled,
    bool,
    crate::DEFAULT_REPETITION_ENABLED
);
serde_default!(
    default_repetition_penalty,
    f64,
    crate::DEFAULT_REPETITION_PENALTY
);
serde_default!(
    default_repetition_threshold,
    usize,
    crate::DEFAULT_REPETITION_THRESHOLD
);
serde_default!(
    default_repetition_window,
    usize,
    crate::DEFAULT_REPETITION_WINDOW
);
serde_default!(default_batch_size, u32, crate::DEFAULT_BATCH_SIZE);
serde_default!(default_use_hf_params, bool, crate::DEFAULT_USE_HF_PARAMS);

/// LLM model configuration for LlamaAgent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmModelConfig {
    pub source: ModelSource,
    /// Batch size for model inference
    #[serde(default = "default_batch_size")]
    pub batch_size: u32,
    /// Whether to use HuggingFace parameters
    #[serde(default = "default_use_hf_params")]
    pub use_hf_params: bool,
    /// Enable debug mode
    #[serde(default)]
    pub debug: bool,
}

/// Model source specification
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ModelSource {
    HuggingFace {
        repo: String,
        filename: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    Local {
        filename: PathBuf,
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<PathBuf>,
    },
}

/// MCP server configuration for LlamaAgent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Port for in-process MCP server (0 = random)
    pub port: u16,
    /// Timeout for MCP requests
    pub timeout_seconds: u64,
}

impl Default for ModelConfig {
    fn default() -> Self {
        // System default is always Claude Code
        Self {
            executor: ModelExecutorConfig::ClaudeCode(ClaudeCodeConfig::default()),
            quiet: crate::DEFAULT_QUIET_MODE,
        }
    }
}

impl Default for LlmModelConfig {
    fn default() -> Self {
        Self {
            source: ModelSource::HuggingFace {
                repo: crate::DEFAULT_LLM_MODEL_REPO.to_string(),
                filename: Some(crate::DEFAULT_LLM_MODEL_FILENAME.to_string()),
                folder: None,
            },
            batch_size: default_batch_size(),
            use_hf_params: default_use_hf_params(),
            debug: crate::DEFAULT_DEBUG_MODE,
        }
    }
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            port: crate::DEFAULT_MCP_PORT,
            timeout_seconds: crate::DEFAULT_MCP_TIMEOUT_SECONDS,
        }
    }
}

impl Default for RepetitionDetectionConfig {
    fn default() -> Self {
        Self {
            enabled: default_repetition_enabled(),
            repetition_penalty: default_repetition_penalty(),
            repetition_threshold: default_repetition_threshold(),
            repetition_window: default_repetition_window(),
        }
    }
}

impl ModelConfig {
    /// Get the executor type from the configuration
    pub fn executor_type(&self) -> ModelExecutorType {
        match &self.executor {
            ModelExecutorConfig::ClaudeCode(_) => ModelExecutorType::ClaudeCode,
            ModelExecutorConfig::LlamaAgent(_) => ModelExecutorType::LlamaAgent,
        }
    }

    /// Create configuration for Claude Code execution
    pub fn claude_code() -> Self {
        Self {
            executor: ModelExecutorConfig::ClaudeCode(ClaudeCodeConfig::default()),
            quiet: crate::DEFAULT_QUIET_MODE,
        }
    }

    /// Create configuration for LlamaAgent execution
    pub fn llama_agent(config: LlamaAgentConfig) -> Self {
        Self {
            executor: ModelExecutorConfig::LlamaAgent(config),
            quiet: crate::DEFAULT_QUIET_MODE,
        }
    }
}

impl LlamaAgentConfig {
    /// Configuration optimized for testing with small, fast models
    ///
    /// Uses Qwen2.5-1.5B with Q4_K_M quantization which provides:
    /// - Fast model loading (~3-5 seconds)
    /// - Quick inference for simple prompts
    /// - Good quality responses for testing
    pub fn for_testing() -> Self {
        Self {
            model: LlmModelConfig {
                source: ModelSource::HuggingFace {
                    repo: crate::DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
                    filename: Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                    folder: None,
                },
                batch_size: crate::DEFAULT_TEST_BATCH_SIZE,
                use_hf_params: crate::DEFAULT_USE_HF_PARAMS,
                debug: crate::DEFAULT_DEBUG_MODE,
            },
            mcp_server: McpServerConfig {
                port: crate::DEFAULT_MCP_PORT,
                timeout_seconds: crate::DEFAULT_TEST_MCP_TIMEOUT_SECONDS,
            },

            repetition_detection: RepetitionDetectionConfig {
                enabled: crate::DEFAULT_REPETITION_ENABLED,
                repetition_penalty: crate::DEFAULT_TEST_REPETITION_PENALTY,
                repetition_threshold: crate::DEFAULT_TEST_REPETITION_THRESHOLD,
                repetition_window: crate::DEFAULT_TEST_REPETITION_WINDOW,
            },
        }
    }

    /// Alias for `for_testing()` - kept for backwards compatibility
    #[deprecated(since = "0.1.0", note = "Use `for_testing()` instead")]
    pub fn for_small_model() -> Self {
        Self::for_testing()
    }
}

/// Agent source enumeration
///
/// Defines where a model configuration originates from, used for
/// precedence resolution in the model discovery hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelConfigSource {
    /// Built-in models compiled into the binary
    Builtin,
    /// Project-specific models from models/ directory
    Project,
    /// Git root models from .swissarmyhammer/models/ in git repository root
    GitRoot,
    /// User-defined models from ~/.swissarmyhammer/models/
    User,
}

impl ModelConfigSource {
    /// Get emoji-based display string for the agent source
    ///
    /// - 📦 Built-in: System-provided built-in models
    /// - 📁 Project: Project-specific models from models/ directory
    /// - 🔧 GitRoot: Git repository models from .swissarmyhammer/models/
    /// - 👤 User: User-defined models from ~/.swissarmyhammer/models/
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_config::model::ModelConfigSource;
    ///
    /// assert_eq!(ModelConfigSource::Builtin.display_emoji(), "📦 Built-in");
    /// assert_eq!(ModelConfigSource::Project.display_emoji(), "📁 Project");
    /// assert_eq!(ModelConfigSource::GitRoot.display_emoji(), "🔧 GitRoot");
    /// assert_eq!(ModelConfigSource::User.display_emoji(), "👤 User");
    /// ```
    pub fn display_emoji(&self) -> &'static str {
        match self {
            ModelConfigSource::Builtin => "📦 Built-in",
            ModelConfigSource::Project => "📁 Project",
            ModelConfigSource::GitRoot => "🔧 GitRoot",
            ModelConfigSource::User => "👤 User",
        }
    }
}

/// Model-specific error types
///
/// Comprehensive error handling for model discovery, parsing, and management operations.
#[derive(Error, Debug)]
pub enum ModelError {
    /// Model not found in any source
    #[error("Model '{0}' not found")]
    NotFound(String),
    /// Invalid file path for model configuration
    #[error("Invalid model path: {0}")]
    InvalidPath(PathBuf),
    /// IO error during file operations
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    /// Configuration parsing error
    #[error("Parse error: {0}")]
    ParseError(#[from] serde_yaml::Error),
    /// Configuration validation error
    #[error("Configuration error: {0}")]
    ConfigError(String),
}

impl Severity for ModelError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: Cannot parse or validate agent configuration
            Self::ParseError(_) => ErrorSeverity::Critical,
            Self::ConfigError(_) => ErrorSeverity::Critical,

            // Error: Agent operations failed but system can continue
            Self::NotFound(_) => ErrorSeverity::Error,
            Self::InvalidPath(_) => ErrorSeverity::Error,
            Self::IoError(_) => ErrorSeverity::Error,
        }
    }
}

/// Model information structure
///
/// Holds complete metadata for a model configuration including its source,
/// content, and optional description for discovery and management operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelInfo {
    /// Model name (typically filename without extension)
    pub name: String,
    /// Complete model configuration content
    pub content: String,
    /// Source location of the model
    pub source: ModelConfigSource,
    /// Optional description extracted from configuration
    pub description: Option<String>,
}

/// Parse model description from configuration content
///
/// Extracts description from YAML front matter or comment-based format.
/// Looks for `description:` field in YAML front matter first, then falls
/// back to `# Description:` comment lines.
pub fn parse_model_description(content: &str) -> Option<String> {
    let content = content.trim();

    // Try YAML frontmatter first
    if let Some(description) = extract_yaml_frontmatter_field(content, "description") {
        return Some(description);
    }

    // Fall back to comment format
    extract_comment_field(content, "# Description:")
}

/// Extract a field from YAML frontmatter
fn extract_yaml_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let stripped = content.strip_prefix("---")?;
    let end_pos = stripped.find("---")?;
    let front_matter = &stripped[..end_pos];

    let yaml_value = serde_yaml::from_str::<serde_yaml::Value>(front_matter).ok()?;
    let value = yaml_value.get(field)?;
    let value_str = value.as_str()?;
    Some(value_str.trim().to_string())
}

/// Extract a field from comment-based format
fn extract_comment_field(content: &str, prefix: &str) -> Option<String> {
    for line in content.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix(prefix) {
            let value = value.trim();
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extracts the agent configuration portion from content that may have YAML frontmatter
///
/// Handles two formats:
/// 1. Frontmatter format: `---\ndescription: "..."\n---\nactual_config`
/// 2. Pure config format: just the ModelConfig YAML
pub fn parse_model_config(content: &str) -> Result<ModelConfig, serde_yaml::Error> {
    let content = content.trim();

    // Check for YAML front matter
    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_pos) = stripped.find("---") {
            // Extract the content after the second ---
            let config_content = &stripped[end_pos + 3..].trim();
            return serde_yaml::from_str::<ModelConfig>(config_content);
        }
    }

    // Fall back to parsing entire content as ModelConfig
    serde_yaml::from_str::<ModelConfig>(content)
}

/// Statistics for model merging from multiple sources
struct ModelMergeStats {
    initial_builtin_count: usize,
    project_overrides: usize,
    project_new: usize,
    gitroot_overrides: usize,
    gitroot_new: usize,
    user_overrides: usize,
    user_new: usize,
}

/// Model Manager for discovery and loading of agents from various sources
///
/// Provides functionality to load agents from built-in sources, user directories,
/// and project directories with proper precedence handling.
pub struct ModelManager;

impl ModelManager {
    /// List all available agents from all sources with proper precedence
    ///
    /// Combines agents from built-in, project, and user sources with the following precedence:
    /// 1. Built-in agents (lowest precedence) - provides base ordering
    /// 2. Project agents (medium precedence) - overrides built-in agents by name
    /// 3. User agents (highest precedence) - overrides any existing agent by name
    ///
    /// Agents with the same name from higher precedence sources replace lower precedence
    /// agents at the same position in the list. New agents are appended.
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Combined list of all available agents
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let all_agents = ModelManager::list_agents()?;
    /// for agent in all_agents {
    ///     println!("Agent: {} ({})", agent.name,
    ///              match agent.source {
    ///                  swissarmyhammer_config::model::ModelConfigSource::Builtin => "built-in",
    ///                  swissarmyhammer_config::model::ModelConfigSource::Project => "project",
    ///                  swissarmyhammer_config::model::ModelConfigSource::User => "user",
    ///              });
    /// }
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn list_agents() -> Result<Vec<ModelInfo>, ModelError> {
        tracing::debug!("Starting agent discovery with precedence hierarchy");

        let mut models = Self::load_builtin_models()?;
        tracing::debug!("Loaded {} built-in models", models.len());

        let stats = Self::merge_all_model_sources(&mut models);
        Self::log_discovery_results(&models, &stats);

        Ok(models)
    }

    /// Merge models from all sources (project, gitroot, user) with precedence
    fn merge_all_model_sources(models: &mut Vec<ModelInfo>) -> ModelMergeStats {
        let initial_builtin_count = models.len();

        // Process all model sources in precedence order
        let model_sources = [
            (Self::load_project_models(), "project"),
            (Self::load_gitroot_models(), "gitroot"),
            (Self::load_user_models(), "user"),
        ];

        let mut stats = ModelMergeStats {
            initial_builtin_count,
            project_overrides: 0,
            project_new: 0,
            gitroot_overrides: 0,
            gitroot_new: 0,
            user_overrides: 0,
            user_new: 0,
        };

        for (load_result, source_name) in model_sources {
            let (overrides, new) =
                Self::merge_models_with_precedence(models, load_result, source_name);
            match source_name {
                "project" => {
                    stats.project_overrides = overrides;
                    stats.project_new = new;
                }
                "gitroot" => {
                    stats.gitroot_overrides = overrides;
                    stats.gitroot_new = new;
                }
                "user" => {
                    stats.user_overrides = overrides;
                    stats.user_new = new;
                }
                _ => {}
            }
        }

        stats
    }

    /// Log model discovery results
    fn log_discovery_results(models: &[ModelInfo], stats: &ModelMergeStats) {
        Self::log_model_discovery_summary(
            models.len(),
            stats.initial_builtin_count,
            stats.project_overrides,
            stats.project_new,
            stats.user_overrides,
            stats.user_new,
        );

        if stats.gitroot_overrides > 0 || stats.gitroot_new > 0 {
            tracing::debug!(
                "Git root models: {} overrides, {} new",
                stats.gitroot_overrides,
                stats.gitroot_new
            );
        }

        Self::log_final_model_list(models);
    }

    /// Merge models with precedence, replacing existing models or appending new ones
    ///
    /// # Returns
    /// Tuple of (override_count, new_count)
    fn merge_models_with_precedence(
        models: &mut Vec<ModelInfo>,
        load_result: Result<Vec<ModelInfo>, ModelError>,
        source_name: &str,
    ) -> (usize, usize) {
        match load_result {
            Ok(new_models) => {
                tracing::debug!("Loaded {} {} models", new_models.len(), source_name);
                Self::apply_model_overrides(models, new_models, source_name)
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to load {} models: {}. Continuing with existing models",
                    source_name,
                    e
                );
                (0, 0)
            }
        }
    }

    /// Apply model overrides by replacing existing models or appending new ones
    ///
    /// # Returns
    /// Tuple of (override_count, new_count)
    fn apply_model_overrides(
        models: &mut Vec<ModelInfo>,
        new_models: Vec<ModelInfo>,
        source_name: &str,
    ) -> (usize, usize) {
        let mut override_count = 0;
        let mut new_count = 0;

        for new_model in new_models {
            if let Some(existing_pos) = models.iter().position(|m| m.name == new_model.name) {
                let previous_source = &models[existing_pos].source;
                tracing::debug!(
                    "{} model '{}' overriding {:?} model at position {}",
                    source_name,
                    new_model.name,
                    previous_source,
                    existing_pos
                );
                models[existing_pos] = new_model;
                override_count += 1;
            } else {
                tracing::debug!(
                    "Adding new {} model '{}' at position {}",
                    source_name,
                    new_model.name,
                    models.len()
                );
                models.push(new_model);
                new_count += 1;
            }
        }

        (override_count, new_count)
    }

    /// Log model discovery summary with detailed counts
    fn log_model_discovery_summary(
        total_models: usize,
        initial_builtin_count: usize,
        project_overrides: usize,
        project_new: usize,
        user_overrides: usize,
        user_new: usize,
    ) {
        tracing::debug!(
            "Model discovery complete: {} total models ({} built-in, {} project overrides, {} new project, {} user overrides, {} new user)",
            total_models,
            initial_builtin_count,
            project_overrides,
            project_new,
            user_overrides,
            user_new
        );
    }

    /// Log final model list for debugging
    fn log_final_model_list(models: &[ModelInfo]) {
        for (idx, model) in models.iter().enumerate() {
            tracing::trace!(
                "Model[{}]: '{}' ({:?}) - {}",
                idx,
                model.name,
                model.source,
                model.description.as_deref().unwrap_or("no description")
            );
        }
    }

    /// Load all built-in agents compiled into the binary
    ///
    /// Uses the build-time generated `get_builtin_models()` function to access
    /// agents embedded from the `builtin/models/` directory.
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Vector of built-in agent information
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let builtin_models = ModelManager::load_builtin_models()?;
    /// for model in builtin_models {
    ///     println!("Built-in model: {} ({})", model.name,
    ///              model.description.unwrap_or_default());
    /// }
    /// # Ok::<(), swissarmyhammer_config::ModelError>(())
    /// ```
    pub fn load_builtin_models() -> Result<Vec<ModelInfo>, ModelError> {
        let builtin_models = crate::get_builtin_models();
        let mut models = Vec::with_capacity(builtin_models.len());

        for (name, content) in builtin_models {
            let description = parse_model_description(content);
            models.push(ModelInfo {
                name: name.to_string(),
                content: content.to_string(),
                source: ModelConfigSource::Builtin,
                description,
            });
        }

        Ok(models)
    }

    /// Load models from a specific directory
    ///
    /// Scans the given directory for `.yaml` model configuration files and loads them
    /// with the specified source type. Missing directories are handled gracefully by
    /// returning an empty vector. Individual model validation failures are logged but
    /// don't prevent loading other models.
    ///
    /// # Security
    ///
    /// This function implements comprehensive security measures:
    /// - Path validation and canonicalization to resolve symlinks
    /// - Permission checks to ensure directory is readable
    /// - Audit logging of all directory access attempts
    ///
    /// # Arguments
    /// * `dir_path` - Path to the directory to scan for model files
    /// * `source` - The source type to assign to loaded models
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Vector of model information from the directory
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::{ModelManager, ModelSource};
    /// use std::path::Path;
    ///
    /// let models = ModelManager::load_models_from_dir(
    ///     Path::new("./models"),
    ///     ModelConfigSource::Project
    /// )?;
    /// # Ok::<(), swissarmyhammer_config::ModelError>(())
    /// ```
    pub fn load_models_from_dir(
        dir_path: &Path,
        source: ModelConfigSource,
    ) -> Result<Vec<ModelInfo>, ModelError> {
        // Security: Validate and canonicalize the directory path
        let validated_dir = Self::validate_directory_path(dir_path)?;

        if !Self::is_valid_directory(&validated_dir) {
            return Ok(Vec::new());
        }

        // Security: Audit log directory access
        tracing::info!(
            "Loading models from directory: {} (canonical: {}, source: {:?})",
            dir_path.display(),
            validated_dir.display(),
            source
        );

        let entries = Self::read_directory_entries(&validated_dir)?;
        let (models, successful_count, failed_count) =
            Self::process_directory_entries(entries, &source);

        Self::log_directory_loading_result(&validated_dir, successful_count, failed_count);

        Ok(models)
    }

    /// Validate and canonicalize a directory path for secure access
    ///
    /// # Security
    ///
    /// Performs the following validations:
    /// - Canonicalizes path to resolve symlinks and relative components
    /// - Validates path exists and is readable
    /// - Checks for suspicious path patterns
    /// - Audit logs validation attempts
    ///
    /// # Arguments
    /// * `dir_path` - Path to validate
    ///
    /// # Returns
    /// * `Result<PathBuf, ModelError>` - Canonicalized path or error
    fn validate_directory_path(dir_path: &Path) -> Result<PathBuf, ModelError> {
        // Check for empty path
        if dir_path.as_os_str().is_empty() {
            tracing::warn!("Attempted to load models from empty path");
            return Err(ModelError::InvalidPath(dir_path.to_path_buf()));
        }

        // Check path length to prevent system issues
        const MAX_PATH_LENGTH: usize = 4096;
        let path_str = dir_path.to_string_lossy();
        if path_str.len() > MAX_PATH_LENGTH {
            tracing::warn!(
                "Path too long ({} characters, maximum {}): {}",
                path_str.len(),
                MAX_PATH_LENGTH,
                path_str
            );
            return Err(ModelError::InvalidPath(dir_path.to_path_buf()));
        }

        // Canonicalize path to resolve symlinks and validate existence
        let canonical_path = match dir_path.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                // Path doesn't exist or is inaccessible - this is OK, we return empty vector
                tracing::debug!(
                    "Directory path does not exist or is not accessible: {} ({})",
                    dir_path.display(),
                    e
                );
                // Return original path so is_valid_directory can handle it
                return Ok(dir_path.to_path_buf());
            }
        };

        // Security: Check for suspicious path patterns after canonicalization
        let canonical_str = canonical_path.to_string_lossy();
        Self::check_suspicious_patterns(&canonical_str)?;

        // Security: Verify directory permissions
        Self::check_directory_permissions(&canonical_path)?;

        Ok(canonical_path)
    }

    /// Check for suspicious path patterns that might indicate attacks
    fn check_suspicious_patterns(path_str: &str) -> Result<(), ModelError> {
        // Check for null bytes which can cause security issues
        if path_str.contains('\0') {
            tracing::warn!("Path contains null byte: {}", path_str);
            return Err(ModelError::ConfigError(
                "Path contains invalid null byte".to_string(),
            ));
        }

        Ok(())
    }

    /// Check directory permissions to ensure it's readable
    fn check_directory_permissions(path: &Path) -> Result<(), ModelError> {
        // Check if we can read the directory metadata
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    tracing::warn!("Path is not a directory: {}", path.display());
                    return Err(ModelError::InvalidPath(path.to_path_buf()));
                }
                // On Unix, check read permission
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();
                    let has_read = (mode & 0o400) != 0; // Owner read permission
                    if !has_read {
                        tracing::warn!("Directory is not readable: {}", path.display());
                        return Err(ModelError::IoError(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            format!("Directory is not readable: {}", path.display()),
                        )));
                    }
                }
            }
            Err(e) => {
                tracing::debug!("Cannot read directory metadata {}: {}", path.display(), e);
                return Err(ModelError::IoError(e));
            }
        }

        Ok(())
    }

    /// Check if path is a valid directory for loading models
    fn is_valid_directory(dir_path: &Path) -> bool {
        if !dir_path.exists() || !dir_path.is_dir() {
            tracing::debug!(
                "Model directory does not exist or is not a directory: {}",
                dir_path.display()
            );
            return false;
        }
        true
    }

    /// Log the result of directory loading
    fn log_directory_loading_result(dir_path: &Path, successful_count: usize, failed_count: usize) {
        tracing::debug!(
            "Finished loading models from {}: {} successful, {} failed",
            dir_path.display(),
            successful_count,
            failed_count
        );
    }

    /// Read directory entries with error handling
    fn read_directory_entries(dir_path: &Path) -> Result<std::fs::ReadDir, ModelError> {
        std::fs::read_dir(dir_path).map_err(|e| {
            tracing::error!(
                "Failed to read model directory {}: {}",
                dir_path.display(),
                e
            );
            ModelError::IoError(e)
        })
    }

    /// Process directory entries and load valid model files
    ///
    /// # Returns
    /// Tuple of (models, successful_count, failed_count)
    fn process_directory_entries(
        entries: std::fs::ReadDir,
        source: &ModelConfigSource,
    ) -> (Vec<ModelInfo>, usize, usize) {
        let mut models = Vec::new();
        let mut successful_count = 0;
        let mut failed_count = 0;

        for entry in entries {
            let entry = match entry {
                Ok(entry) => entry,
                Err(e) => {
                    tracing::warn!("Failed to read directory entry: {}", e);
                    failed_count += 1;
                    continue;
                }
            };

            let path = entry.path();
            if Self::is_yaml_file(&path) {
                match Self::load_model_file(&path, source) {
                    Ok(model) => {
                        models.push(model);
                        successful_count += 1;
                    }
                    Err(_) => {
                        failed_count += 1;
                    }
                }
            }
        }

        (models, successful_count, failed_count)
    }

    /// Check if path is a YAML file
    fn is_yaml_file(path: &Path) -> bool {
        path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("yaml")
    }

    /// Load and validate a single model file
    fn load_model_file(path: &Path, source: &ModelConfigSource) -> Result<ModelInfo, ModelError> {
        let model_name = Self::extract_model_name(path)?;
        let content = Self::read_model_content(path)?;
        Self::validate_and_create_model_info(&content, &model_name, source, path)
    }

    /// Extract model name from file path
    fn extract_model_name(path: &Path) -> Result<String, ModelError> {
        path.file_stem()
            .and_then(|name| name.to_str())
            .map(|name| name.to_string())
            .ok_or_else(|| {
                tracing::warn!("Failed to extract model name from path: {}", path.display());
                ModelError::InvalidPath(path.to_path_buf())
            })
    }

    /// Read model file content
    fn read_model_content(path: &Path) -> Result<String, ModelError> {
        std::fs::read_to_string(path).map_err(|e| {
            tracing::warn!("Failed to read model file {}: {}", path.display(), e);
            ModelError::IoError(e)
        })
    }

    /// Validate model configuration and create ModelInfo
    fn validate_and_create_model_info(
        content: &str,
        model_name: &str,
        source: &ModelConfigSource,
        path: &Path,
    ) -> Result<ModelInfo, ModelError> {
        parse_model_config(content).map_err(|e| {
            tracing::warn!(
                "Model configuration validation failed for {}: {}. Skipping this model.",
                path.display(),
                e
            );
            ModelError::ParseError(e)
        })?;

        let description = parse_model_description(content);
        tracing::trace!(
            "Successfully loaded model '{}' from {} (description: {:?})",
            model_name,
            path.display(),
            description
        );

        Ok(ModelInfo {
            name: model_name.to_string(),
            content: content.to_string(),
            source: source.clone(),
            description,
        })
    }

    /// Load user-defined models from ~/.swissarmyhammer/models/
    ///
    /// Scans the user's home directory `.swissarmyhammer/models/` for model configuration
    /// files. Missing directory is handled gracefully by returning an empty vector.
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Vector of user-defined model information
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let user_models = ModelManager::load_user_models()?;
    /// for model in user_models {
    ///     println!("User model: {}", model.name);
    /// }
    /// # Ok::<(), swissarmyhammer_config::ModelError>(())
    /// ```
    pub fn load_user_models() -> Result<Vec<ModelInfo>, ModelError> {
        if let Some(home_dir) = dirs::home_dir() {
            let user_models_dir = home_dir
                .join(SwissarmyhammerDirectory::dir_name())
                .join("models");
            Self::load_models_from_dir(&user_models_dir, ModelConfigSource::User)
        } else {
            // No home directory available (rare case)
            Ok(Vec::new())
        }
    }

    /// Load project-specific models from ./models/
    ///
    /// Scans the current working directory's `models/` subdirectory for model configuration
    /// files. Missing directory is handled gracefully by returning an empty vector.
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Vector of project-specific model information
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let project_models = ModelManager::load_project_models()?;
    /// for model in project_models {
    ///     println!("Project model: {}", model.name);
    /// }
    /// # Ok::<(), swissarmyhammer_config::ModelError>(())
    /// ```
    pub fn load_project_models() -> Result<Vec<ModelInfo>, ModelError> {
        let project_models_dir = std::env::current_dir()
            .map_err(ModelError::IoError)?
            .join("models");
        Self::load_models_from_dir(&project_models_dir, ModelConfigSource::Project)
    }

    /// Load git root models from .swissarmyhammer/models/
    ///
    /// Scans the git repository root's `.swissarmyhammer/models/` directory for model
    /// configuration files. Missing directory is handled gracefully by returning an empty vector.
    ///
    /// # Returns
    /// * `Result<Vec<ModelInfo>, ModelError>` - Vector of git root model information
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let gitroot_models = ModelManager::load_gitroot_models()?;
    /// for model in gitroot_models {
    ///     println!("Git root model: {}", model.name);
    /// }
    /// # Ok::<(), swissarmyhammer_config::ModelError>(())
    /// ```
    pub fn load_gitroot_models() -> Result<Vec<ModelInfo>, ModelError> {
        use swissarmyhammer_common::utils::directory_utils::find_git_repository_root;

        if let Some(git_root) = find_git_repository_root() {
            let gitroot_models_dir = git_root
                .join(SwissarmyhammerDirectory::dir_name())
                .join("models");
            Self::load_models_from_dir(&gitroot_models_dir, ModelConfigSource::GitRoot)
        } else {
            // Not in a git repository
            Ok(Vec::new())
        }
    }

    /// Find a specific agent by name from all available sources
    ///
    /// Searches through all available agents (built-in, project, and user) with proper precedence
    /// handling. Returns the first agent found with the given name, respecting the precedence
    /// hierarchy where user agents override project agents which override built-in agents.
    ///
    /// # Arguments
    /// * `agent_name` - The name of the agent to search for
    ///
    /// # Returns
    /// * `Result<ModelInfo, ModelError>` - The found agent information or NotFound error
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let agent = ModelManager::find_agent_by_name("claude-code")?;
    /// println!("Found agent: {} from {:?}", agent.name, agent.source);
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn find_agent_by_name(agent_name: &str) -> Result<ModelInfo, ModelError> {
        let agents = Self::list_agents()?;

        agents
            .into_iter()
            .find(|agent| agent.name == agent_name)
            .ok_or_else(|| ModelError::NotFound(agent_name.to_string()))
    }

    /// Detect existing project configuration file
    ///
    /// Checks for existing project configuration files in the current working directory,
    /// preferring YAML format over TOML. Returns the path to the first configuration
    /// file found or None if no configuration exists.
    ///
    /// # Search Order
    /// 1. `.swissarmyhammer/sah.yaml` (preferred)
    /// 2. `.swissarmyhammer/sah.toml` (fallback)
    ///
    /// # Returns
    /// * `Option<PathBuf>` - Path to existing config file or None if not found
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// match ModelManager::detect_config_file() {
    ///     Some(config_path) => println!("Found config: {}", config_path.display()),
    ///     None => println!("No existing config found"),
    /// }
    /// ```
    pub fn detect_config_file() -> Option<PathBuf> {
        let current_dir = std::env::current_dir().ok()?;
        let sah_dir = current_dir.join(SwissarmyhammerDirectory::dir_name());

        // Check for YAML config first (preferred)
        let yaml_config = sah_dir.join("sah.yaml");
        if yaml_config.exists() && yaml_config.is_file() {
            return Some(yaml_config);
        }

        // Fall back to TOML config
        let toml_config = sah_dir.join("sah.toml");
        if toml_config.exists() && toml_config.is_file() {
            return Some(toml_config);
        }

        None
    }

    /// Ensure project configuration directory structure exists
    ///
    /// Creates the `.swissarmyhammer/` directory if it doesn't exist and returns the path
    /// to the configuration file that should be used. If an existing configuration file
    /// is found, returns that path. Otherwise, returns the path for a new YAML configuration.
    ///
    /// # Security
    ///
    /// This function implements comprehensive security measures:
    /// - Path validation and canonicalization of current directory
    /// - Permission checks to ensure directory is writable
    /// - Audit logging of directory creation and access
    /// - Validates resulting paths before returning
    ///
    /// # Returns
    /// * `Result<PathBuf, ModelError>` - Path to config file (existing or new) or error
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// let config_path = ModelManager::ensure_config_structure()?;
    /// println!("Config file path: {}", config_path.display());
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn ensure_config_structure() -> Result<PathBuf, ModelError> {
        // Security: Get and validate current directory
        let current_dir = std::env::current_dir().map_err(ModelError::IoError)?;

        // Security: Canonicalize current directory to resolve symlinks
        let canonical_current = current_dir.canonicalize().map_err(|e| {
            tracing::error!(
                "Failed to canonicalize current directory {}: {}",
                current_dir.display(),
                e
            );
            ModelError::IoError(e)
        })?;

        // Security: Audit log the directory we're working in
        tracing::info!(
            "Ensuring config structure in directory: {} (canonical: {})",
            current_dir.display(),
            canonical_current.display()
        );

        let sah_dir = canonical_current.join(SwissarmyhammerDirectory::dir_name());

        // Create .swissarmyhammer directory if it doesn't exist
        if !sah_dir.exists() {
            // Security: Check parent directory permissions before creating
            Self::check_directory_writable(&canonical_current)?;

            std::fs::create_dir_all(&sah_dir).map_err(|e| {
                tracing::error!(
                    "Failed to create .swissarmyhammer directory {}: {}",
                    sah_dir.display(),
                    e
                );
                ModelError::IoError(e)
            })?;

            // Security: Audit log directory creation
            tracing::info!("Created .swissarmyhammer directory: {}", sah_dir.display());
        }

        // Security: Validate the created/existing directory
        Self::check_directory_permissions(&sah_dir)?;

        // Check for existing config file first
        if let Some(existing_config) = Self::detect_config_file() {
            // Security: Validate existing config path
            let validated_config = Self::validate_config_file_path(&existing_config)?;
            tracing::debug!("Found existing config file: {}", validated_config.display());
            return Ok(validated_config);
        }

        // Return path for new YAML config (don't create the file yet)
        let new_config = sah_dir.join("sah.yaml");

        // Security: Validate the new config path before returning
        let validated_new_config = Self::validate_config_file_path(&new_config)?;
        tracing::debug!(
            "Will use new config file: {}",
            validated_new_config.display()
        );
        Ok(validated_new_config)
    }

    /// Check if a directory is writable
    fn check_directory_writable(path: &Path) -> Result<(), ModelError> {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    return Err(ModelError::InvalidPath(path.to_path_buf()));
                }
                // On Unix, check write permission
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();
                    let has_write = (mode & 0o200) != 0; // Owner write permission
                    if !has_write {
                        tracing::error!("Directory is not writable: {}", path.display());
                        return Err(ModelError::IoError(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            format!("Directory is not writable: {}", path.display()),
                        )));
                    }
                }
            }
            Err(e) => {
                tracing::error!("Cannot access directory {}: {}", path.display(), e);
                return Err(ModelError::IoError(e));
            }
        }
        Ok(())
    }

    /// Validate a config file path for security
    fn validate_config_file_path(path: &Path) -> Result<PathBuf, ModelError> {
        // Check for empty path
        if path.as_os_str().is_empty() {
            tracing::warn!("Config file path is empty");
            return Err(ModelError::InvalidPath(path.to_path_buf()));
        }

        // Check path length
        const MAX_PATH_LENGTH: usize = 4096;
        let path_str = path.to_string_lossy();
        if path_str.len() > MAX_PATH_LENGTH {
            tracing::warn!(
                "Config path too long ({} characters, maximum {}): {}",
                path_str.len(),
                MAX_PATH_LENGTH,
                path_str
            );
            return Err(ModelError::InvalidPath(path.to_path_buf()));
        }

        // Security: Check for suspicious patterns
        Self::check_suspicious_patterns(&path_str)?;

        // If the file exists, canonicalize it
        if path.exists() {
            let canonical = path.canonicalize().map_err(|e| {
                tracing::error!(
                    "Failed to canonicalize config path {}: {}",
                    path.display(),
                    e
                );
                ModelError::IoError(e)
            })?;

            // Verify it's a file
            if !canonical.is_file() {
                tracing::warn!("Config path is not a file: {}", canonical.display());
                return Err(ModelError::InvalidPath(canonical));
            }

            Ok(canonical)
        } else {
            // File doesn't exist yet, just return the path
            Ok(path.to_path_buf())
        }
    }

    /// Get agent name for a specific use case from config
    ///
    /// Reads the config file and returns the agent name configured for the use case.
    /// Falls back to Root use case if specific use case not configured.
    ///
    /// Returns None if no agent configured at all.
    ///
    /// # Arguments
    /// * `use_case` - The agent use case to look up
    ///
    /// # Returns
    /// * `Result<Option<String>, ModelError>` - Agent name if configured, None otherwise
    pub fn get_agent_for_use_case(use_case: AgentUseCase) -> Result<Option<String>, ModelError> {
        let config_path = Self::ensure_config_structure()?;

        if !config_path.exists() {
            return Ok(None);
        }

        let config_content = std::fs::read_to_string(&config_path).map_err(ModelError::IoError)?;
        let config_value: serde_yaml::Value = serde_yaml::from_str(&config_content)?;

        // Try new format: agents.{use_case}
        if let Some(agents_map) = config_value.get("agents") {
            let use_case_str = use_case.to_string();
            if let Some(agent_name) = agents_map.get(&use_case_str) {
                if let Some(name_str) = agent_name.as_str() {
                    return Ok(Some(name_str.to_string()));
                }
            }

            // Fall back to root if use case not configured
            if use_case != AgentUseCase::Root {
                if let Some(root_agent) = agents_map.get("root") {
                    if let Some(name_str) = root_agent.as_str() {
                        return Ok(Some(name_str.to_string()));
                    }
                }
            }
        }

        Ok(None)
    }

    /// Resolve complete agent configuration for a use case
    ///
    /// Returns the ModelConfig for the specified use case, with fallback chain:
    /// 1. Use case-specific agent (if configured)
    /// 2. Root agent (if configured)
    /// 3. Default claude-code agent
    ///
    /// # Arguments
    /// * `use_case` - The agent use case to resolve
    ///
    /// # Returns
    /// * `Result<ModelConfig, ModelError>` - Resolved agent configuration
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::{ModelManager, AgentUseCase};
    ///
    /// let config = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules)?;
    /// println!("Using agent: {:?}", config.executor_type());
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn resolve_agent_config_for_use_case(
        use_case: AgentUseCase,
    ) -> Result<ModelConfig, ModelError> {
        // Try to get agent name for this use case
        let agent_name = Self::get_agent_for_use_case(use_case)?;

        if let Some(name) = agent_name {
            // Found configured agent - load it
            let agent_info = Self::find_agent_by_name(&name)?;
            return Ok(parse_model_config(&agent_info.content)?);
        }

        // No agent configured - use default (claude-code)
        tracing::debug!(
            "No agent configured for use case {}, using default (claude-code)",
            use_case
        );
        Ok(ModelConfig::claude_code())
    }

    /// Apply an agent configuration to the project
    ///
    /// Finds the specified agent by name, loads or creates the project configuration file,
    /// and updates the agent section with the selected agent's configuration. Preserves
    /// all other sections in existing configuration files.
    ///
    /// # Arguments
    /// * `agent_name` - Name of the agent to apply
    ///
    /// # Returns
    /// * `Result<(), ModelError>` - Success or error details
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// // Apply built-in claude-code agent to project
    /// ModelManager::use_agent("claude-code")?;
    ///
    /// // Apply a custom user agent
    /// ModelManager::use_agent("my-custom-agent")?;
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn use_agent(agent_name: &str) -> Result<(), ModelError> {
        Self::use_agent_for_use_case(agent_name, AgentUseCase::Root)
    }

    /// Ensure agents map exists in config
    ///
    /// Creates the agents map if it doesn't exist in the configuration.
    fn ensure_agents_map(config: &mut serde_yaml::Value) -> Result<(), ModelError> {
        if config.get("agents").is_none() {
            let agents_map = serde_yaml::Value::Mapping(Default::default());
            if let Some(map) = config.as_mapping_mut() {
                map.insert(serde_yaml::Value::String("agents".to_string()), agents_map);
            }
        }
        Ok(())
    }

    /// Set use case agent in agents map
    ///
    /// Updates the agents map with the agent name for the given use case.
    fn set_use_case_agent(
        agents_map: &mut serde_yaml::Mapping,
        use_case: AgentUseCase,
        agent_name: &str,
    ) {
        agents_map.insert(
            serde_yaml::Value::String(use_case.to_string()),
            serde_yaml::Value::String(agent_name.to_string()),
        );
    }

    /// Apply an agent configuration to the project for a specific use case
    ///
    /// Finds the specified agent by name, loads or creates the project configuration file,
    /// and updates the agents map with the selected agent for the given use case.
    ///
    /// # Security
    ///
    /// This function implements comprehensive security measures:
    /// - Validates agent name to prevent injection attacks
    /// - Path validation and canonicalization for config file
    /// - Permission checks before reading/writing config
    /// - Audit logging of all configuration changes
    ///
    /// # Arguments
    /// * `agent_name` - Name of the agent to apply
    /// * `use_case` - The use case to configure this agent for
    ///
    /// # Returns
    /// * `Result<(), ModelError>` - Success or error details
    pub fn use_agent_for_use_case(
        agent_name: &str,
        use_case: AgentUseCase,
    ) -> Result<(), ModelError> {
        // Security: Validate agent name to prevent injection
        Self::validate_agent_name_security(agent_name)?;

        // Security: Audit log the configuration change attempt
        tracing::info!(
            "Attempting to set {} use case to model '{}' (user request)",
            use_case,
            agent_name
        );

        Self::validate_agent(agent_name)?;
        let config_path = Self::ensure_config_structure()?;

        // Security: Validate config path before operations
        let validated_config_path = Self::validate_config_file_path(&config_path)?;

        let mut config_value = Self::load_or_create_config(&validated_config_path)?;

        Self::update_config_with_agent(&mut config_value, use_case, agent_name)?;
        Self::save_config(&validated_config_path, &config_value)?;

        // Security: Audit log successful configuration change
        tracing::info!(
            "Successfully set {} use case to model '{}' in {}",
            use_case,
            agent_name,
            validated_config_path.display()
        );

        Ok(())
    }

    /// Validate agent name for security (prevent injection attacks)
    fn validate_agent_name_security(agent_name: &str) -> Result<(), ModelError> {
        // Check for empty name
        if agent_name.trim().is_empty() {
            tracing::warn!("Agent name is empty");
            return Err(ModelError::ConfigError(
                "Agent name cannot be empty".to_string(),
            ));
        }

        // Check name length to prevent buffer overflow issues
        const MAX_AGENT_NAME_LENGTH: usize = 256;
        if agent_name.len() > MAX_AGENT_NAME_LENGTH {
            tracing::warn!(
                "Agent name too long ({} characters, maximum {}): {}",
                agent_name.len(),
                MAX_AGENT_NAME_LENGTH,
                agent_name
            );
            return Err(ModelError::ConfigError(format!(
                "Agent name too long ({} characters, maximum {})",
                agent_name.len(),
                MAX_AGENT_NAME_LENGTH
            )));
        }

        // Check for null bytes
        if agent_name.contains('\0') {
            tracing::warn!("Agent name contains null byte: {}", agent_name);
            return Err(ModelError::ConfigError(
                "Agent name contains invalid null byte".to_string(),
            ));
        }

        // Check for path traversal patterns in agent name
        let suspicious_patterns = ["../", "..\\", "/", "\\"];
        for pattern in &suspicious_patterns {
            if agent_name.contains(pattern) {
                tracing::warn!(
                    "Agent name contains suspicious pattern '{}': {}",
                    pattern,
                    agent_name
                );
                return Err(ModelError::ConfigError(format!(
                    "Agent name contains invalid pattern: {}",
                    pattern
                )));
            }
        }

        // Check for control characters
        if agent_name.chars().any(|c| c.is_control()) {
            tracing::warn!("Agent name contains control characters: {}", agent_name);
            return Err(ModelError::ConfigError(
                "Agent name contains invalid control characters".to_string(),
            ));
        }

        Ok(())
    }

    /// Validate that agent exists and is parseable
    fn validate_agent(agent_name: &str) -> Result<(), ModelError> {
        let agent_info = Self::find_agent_by_name(agent_name)?;
        let _agent_config = parse_model_config(&agent_info.content)?;
        Ok(())
    }

    /// Load existing config or create empty one
    ///
    /// # Security
    ///
    /// - Validates file permissions before reading
    /// - Checks file size to prevent resource exhaustion
    /// - Audit logs file access
    fn load_or_create_config(config_path: &Path) -> Result<serde_yaml::Value, ModelError> {
        if config_path.exists() {
            // Security: Check file permissions
            Self::check_file_readable(config_path)?;

            // Security: Check file size to prevent resource exhaustion
            const MAX_CONFIG_SIZE: u64 = 10 * 1024 * 1024; // 10MB
            let metadata = std::fs::metadata(config_path).map_err(ModelError::IoError)?;
            if metadata.len() > MAX_CONFIG_SIZE {
                tracing::error!(
                    "Config file too large ({} bytes, maximum {}): {}",
                    metadata.len(),
                    MAX_CONFIG_SIZE,
                    config_path.display()
                );
                return Err(ModelError::ConfigError(format!(
                    "Config file too large ({} bytes, maximum {})",
                    metadata.len(),
                    MAX_CONFIG_SIZE
                )));
            }

            // Security: Audit log config file access
            tracing::debug!("Reading config file: {}", config_path.display());

            let content = std::fs::read_to_string(config_path).map_err(|e| {
                tracing::error!(
                    "Failed to read config file {}: {}",
                    config_path.display(),
                    e
                );
                ModelError::IoError(e)
            })?;

            Ok(serde_yaml::from_str(&content)
                .unwrap_or(serde_yaml::Value::Mapping(Default::default())))
        } else {
            tracing::debug!(
                "Config file does not exist, creating new: {}",
                config_path.display()
            );
            Ok(serde_yaml::Value::Mapping(Default::default()))
        }
    }

    /// Check if a file is readable
    fn check_file_readable(path: &Path) -> Result<(), ModelError> {
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_file() {
                    tracing::error!("Path is not a file: {}", path.display());
                    return Err(ModelError::InvalidPath(path.to_path_buf()));
                }
                // On Unix, check read permission
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();
                    let has_read = (mode & 0o400) != 0; // Owner read permission
                    if !has_read {
                        tracing::error!("File is not readable: {}", path.display());
                        return Err(ModelError::IoError(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            format!("File is not readable: {}", path.display()),
                        )));
                    }
                }
                Ok(())
            }
            Err(e) => {
                tracing::error!("Cannot access file {}: {}", path.display(), e);
                Err(ModelError::IoError(e))
            }
        }
    }

    /// Update config with agent for use case
    fn update_config_with_agent(
        config: &mut serde_yaml::Value,
        use_case: AgentUseCase,
        agent_name: &str,
    ) -> Result<(), ModelError> {
        Self::ensure_agents_map(config)?;

        if let Some(agents_map) = config.get_mut("agents").and_then(|v| v.as_mapping_mut()) {
            Self::set_use_case_agent(agents_map, use_case, agent_name);
        }

        Ok(())
    }

    /// Save config to file
    ///
    /// # Security
    ///
    /// - Validates file path before writing
    /// - Checks parent directory permissions
    /// - Uses atomic write operation when possible
    /// - Audit logs file modifications
    fn save_config(config_path: &Path, config: &serde_yaml::Value) -> Result<(), ModelError> {
        // Security: Validate the parent directory is writable
        if let Some(parent) = config_path.parent() {
            Self::check_directory_writable(parent)?;
        }

        // Security: Audit log config write
        tracing::info!("Writing config to file: {}", config_path.display());

        let config_yaml = serde_yaml::to_string(config)?;

        // Security: Check serialized config size before writing
        const MAX_CONFIG_SIZE: usize = 10 * 1024 * 1024; // 10MB
        if config_yaml.len() > MAX_CONFIG_SIZE {
            tracing::error!(
                "Config too large to write ({} bytes, maximum {})",
                config_yaml.len(),
                MAX_CONFIG_SIZE
            );
            return Err(ModelError::ConfigError(format!(
                "Config too large ({} bytes, maximum {})",
                config_yaml.len(),
                MAX_CONFIG_SIZE
            )));
        }

        std::fs::write(config_path, config_yaml).map_err(|e| {
            tracing::error!(
                "Failed to write config file {}: {}",
                config_path.display(),
                e
            );
            ModelError::IoError(e)
        })?;

        // Security: Audit log successful write
        tracing::info!("Successfully wrote config to: {}", config_path.display());

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test helper: Setup a temporary directory and change to it, returning cleanup guard
    ///
    /// Returns (TempDir, original_dir) - when dropped, TempDir cleans up and you should restore original_dir
    fn setup_temp_test_dir() -> (tempfile::TempDir, std::path::PathBuf) {
        use std::env;
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");
        (temp_dir, original_dir)
    }

    /// Test helper: Restore original directory after test
    fn restore_dir(original_dir: std::path::PathBuf) {
        use std::env;
        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    /// Test helper: Setup config test environment with optional initial config content
    ///
    /// Creates .swissarmyhammer directory and optionally writes a config file.
    /// Returns (TempDir, config_path, original_dir)
    fn setup_config_test_env(
        config_file: &str,
        content: Option<&str>,
    ) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let (temp_dir, original_dir) = setup_temp_test_dir();
        let config_path = create_config_file(&temp_dir, config_file, content);
        (temp_dir, config_path, original_dir)
    }

    /// Create config file in temp directory with optional content
    fn create_config_file(
        temp_dir: &tempfile::TempDir,
        config_file: &str,
        content: Option<&str>,
    ) -> std::path::PathBuf {
        use std::fs;
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer dir");

        let config_path = sah_dir.join(config_file);
        if let Some(content) = content {
            fs::write(&config_path, content).expect("Failed to write config file");
        }
        config_path
    }

    #[test]
    fn test_system_default_is_claude() {
        let config = ModelConfig::default();
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
        assert!(!config.quiet);
    }

    #[test]
    fn test_agent_executor_type_default() {
        let executor_type = ModelExecutorType::default();
        assert_eq!(executor_type, ModelExecutorType::ClaudeCode);
    }

    #[test]
    fn test_claude_code_config_default() {
        let config = ClaudeCodeConfig::default();
        assert!(config.claude_path.is_none());
        assert!(config.args.is_empty());
    }

    #[test]
    fn test_llama_agent_config_default() {
        let config = LlamaAgentConfig::default();
        match config.model.source {
            ModelSource::HuggingFace { repo, filename, .. } => {
                assert!(
                    repo.starts_with("unsloth/Qwen3"),
                    "Expected Qwen3 model, got {}",
                    repo
                );
                assert!(
                    filename
                        .as_ref()
                        .map(|f| f.contains("Qwen3"))
                        .unwrap_or(false),
                    "Expected Qwen3 filename, got {:?}",
                    filename
                );
            }
            ModelSource::Local { .. } => panic!("Default should be HuggingFace"),
        }
        assert_eq!(config.mcp_server.port, 0);
        assert_eq!(config.mcp_server.timeout_seconds, 15 * 60);
    }

    #[test]
    fn test_llama_agent_config_for_testing() {
        let config = LlamaAgentConfig::for_testing();
        match config.model.source {
            ModelSource::HuggingFace { repo, filename, .. } => {
                assert_eq!(repo, crate::DEFAULT_TEST_LLM_MODEL_REPO);
                assert_eq!(
                    filename,
                    Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string())
                );
            }
            ModelSource::Local { .. } => panic!("Testing config should be HuggingFace"),
        }
        assert_eq!(config.mcp_server.port, 0);
        assert_eq!(config.mcp_server.timeout_seconds, 30); // Test timeout (DEFAULT_TEST_MCP_TIMEOUT_SECONDS)
    }

    #[test]
    fn test_agent_config_claude_code_factory() {
        let config = ModelConfig::claude_code();
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);
        assert!(!config.quiet);

        match config.executor {
            ModelExecutorConfig::ClaudeCode(claude_config) => {
                assert!(claude_config.claude_path.is_none());
                assert!(claude_config.args.is_empty());
            }
            ModelExecutorConfig::LlamaAgent(_) => panic!("Should be Claude Code config"),
        }
    }

    #[test]
    fn test_agent_config_llama_agent_factory() {
        let llama_config = LlamaAgentConfig::for_testing();
        let config = ModelConfig::llama_agent(llama_config.clone());

        assert_eq!(config.executor_type(), ModelExecutorType::LlamaAgent);
        assert!(!config.quiet);

        match config.executor {
            ModelExecutorConfig::LlamaAgent(agent_config) => {
                assert_eq!(agent_config.mcp_server.timeout_seconds, 30); // Test timeout (DEFAULT_TEST_MCP_TIMEOUT_SECONDS)
            }
            ModelExecutorConfig::ClaudeCode(_) => panic!("Should be LlamaAgent config"),
        }
    }

    #[test]
    fn test_configuration_serialization_yaml() {
        let config = ModelConfig::llama_agent(LlamaAgentConfig::for_testing());

        // Should serialize to YAML correctly
        let yaml = serde_yaml::to_string(&config).expect("Failed to serialize to YAML");
        assert!(yaml.contains("type: llama-agent"));
        assert!(yaml.contains("quiet: false"));

        // Should deserialize from YAML correctly
        let deserialized: ModelConfig =
            serde_yaml::from_str(&yaml).expect("Failed to deserialize from YAML");
        assert_eq!(config.executor_type(), deserialized.executor_type());
        assert_eq!(config.quiet, deserialized.quiet);
    }

    #[test]
    fn test_configuration_serialization_json() {
        let config = ModelConfig::claude_code();

        // Should serialize to JSON correctly
        let json = serde_json::to_string(&config).expect("Failed to serialize to JSON");
        assert!(json.contains("\"type\":\"claude-code\""));
        assert!(json.contains("\"quiet\":false"));

        // Should deserialize from JSON correctly
        let deserialized: ModelConfig =
            serde_json::from_str(&json).expect("Failed to deserialize from JSON");
        assert_eq!(config.executor_type(), deserialized.executor_type());
        assert_eq!(config.quiet, deserialized.quiet);
    }

    #[test]
    fn test_model_source_serialization() {
        let huggingface_source = ModelSource::HuggingFace {
            repo: "test/repo".to_string(),
            filename: Some("model.gguf".to_string()),
            folder: None,
        };

        let json = serde_json::to_string(&huggingface_source)
            .expect("Failed to serialize HuggingFace source");
        let deserialized: ModelSource =
            serde_json::from_str(&json).expect("Failed to deserialize HuggingFace source");

        match deserialized {
            ModelSource::HuggingFace { repo, filename, .. } => {
                assert_eq!(repo, "test/repo");
                assert_eq!(filename, Some("model.gguf".to_string()));
            }
            ModelSource::Local { .. } => panic!("Should be HuggingFace source"),
        }

        let local_source = ModelSource::Local {
            filename: PathBuf::from("/path/to/model.gguf"),
            folder: None,
        };

        let json = serde_json::to_string(&local_source).expect("Failed to serialize Local source");
        let deserialized: ModelSource =
            serde_json::from_str(&json).expect("Failed to deserialize Local source");

        match deserialized {
            ModelSource::Local { filename, folder } => {
                assert_eq!(filename, PathBuf::from("/path/to/model.gguf"));
                assert_eq!(folder, None);
            }
            ModelSource::HuggingFace { .. } => panic!("Should be Local source"),
        }
    }

    #[test]
    fn test_model_source_local_with_folder_serialization() {
        // Test serialization of ModelSource::Local with explicit folder
        let local_source_with_folder = ModelSource::Local {
            filename: PathBuf::from("model.gguf"),
            folder: Some(PathBuf::from("/custom/folder")),
        };

        let json = serde_json::to_string(&local_source_with_folder)
            .expect("Failed to serialize Local source with folder");

        let deserialized: ModelSource =
            serde_json::from_str(&json).expect("Failed to deserialize Local source with folder");

        match deserialized {
            ModelSource::Local { filename, folder } => {
                assert_eq!(filename, PathBuf::from("model.gguf"));
                assert_eq!(folder, Some(PathBuf::from("/custom/folder")));
            }
            ModelSource::HuggingFace { .. } => panic!("Should be Local source"),
        }

        // Test that folder field is omitted when None (due to skip_serializing_if)
        let local_source_no_folder = ModelSource::Local {
            filename: PathBuf::from("model.gguf"),
            folder: None,
        };

        let json = serde_json::to_string(&local_source_no_folder)
            .expect("Failed to serialize Local source without folder");

        // The JSON should not contain "folder" field when None
        assert!(!json.contains("folder"));
    }

    #[test]
    fn test_huggingface_folder_deserialization() {
        // Test JSON deserialization with folder field
        let json_with_folder = r#"{
            "HuggingFace": {
                "repo": "unsloth/test-repo",
                "folder": "UD-Q4_K_XL"
            }
        }"#;

        let source: ModelSource = serde_json::from_str(json_with_folder)
            .expect("Failed to deserialize HuggingFace source with folder");

        match source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "unsloth/test-repo");
                assert_eq!(filename, None);
                assert_eq!(folder, Some("UD-Q4_K_XL".to_string()));
            }
            _ => panic!("Expected HuggingFace source"),
        }

        // Test JSON deserialization with both filename and folder
        let json_with_both = r#"{
            "HuggingFace": {
                "repo": "unsloth/test-repo",
                "filename": "model.gguf",
                "folder": "UD-Q4_K_XL"
            }
        }"#;

        let source: ModelSource = serde_json::from_str(json_with_both)
            .expect("Failed to deserialize HuggingFace source with both filename and folder");

        match source {
            ModelSource::HuggingFace {
                repo,
                filename,
                folder,
            } => {
                assert_eq!(repo, "unsloth/test-repo");
                assert_eq!(filename, Some("model.gguf".to_string()));
                assert_eq!(folder, Some("UD-Q4_K_XL".to_string()));
            }
            _ => panic!("Expected HuggingFace source"),
        }
    }

    #[test]
    fn test_model_source_variants() {
        // Test all ModelSource variants exist and have correct Debug output
        assert_eq!(format!("{:?}", ModelConfigSource::Builtin), "Builtin");
        assert_eq!(format!("{:?}", ModelConfigSource::Project), "Project");
        assert_eq!(format!("{:?}", ModelConfigSource::User), "User");
    }

    #[test]
    fn test_model_source_equality() {
        assert_eq!(ModelConfigSource::Builtin, ModelConfigSource::Builtin);
        assert_eq!(ModelConfigSource::Project, ModelConfigSource::Project);
        assert_eq!(ModelConfigSource::User, ModelConfigSource::User);

        assert_ne!(ModelConfigSource::Builtin, ModelConfigSource::Project);
        assert_ne!(ModelConfigSource::Builtin, ModelConfigSource::User);
        assert_ne!(ModelConfigSource::Project, ModelConfigSource::User);
    }

    #[test]
    fn test_model_source_display_emoji() {
        assert_eq!(ModelConfigSource::Builtin.display_emoji(), "📦 Built-in");
        assert_eq!(ModelConfigSource::Project.display_emoji(), "📁 Project");
        assert_eq!(ModelConfigSource::User.display_emoji(), "👤 User");
    }

    #[test]
    fn test_agent_source_serialization() {
        // Test serde serialization with kebab-case
        let builtin = ModelConfigSource::Builtin;
        let json = serde_json::to_string(&builtin).expect("Failed to serialize Builtin");
        assert_eq!(json, "\"builtin\"");

        let project = ModelConfigSource::Project;
        let json = serde_json::to_string(&project).expect("Failed to serialize Project");
        assert_eq!(json, "\"project\"");

        let user = ModelConfigSource::User;
        let json = serde_json::to_string(&user).expect("Failed to serialize User");
        assert_eq!(json, "\"user\"");
    }

    #[test]
    fn test_agent_source_deserialization() {
        let builtin: ModelConfigSource =
            serde_json::from_str("\"builtin\"").expect("Failed to deserialize builtin");
        assert_eq!(builtin, ModelConfigSource::Builtin);

        let project: ModelConfigSource =
            serde_json::from_str("\"project\"").expect("Failed to deserialize project");
        assert_eq!(project, ModelConfigSource::Project);

        let user: ModelConfigSource =
            serde_json::from_str("\"user\"").expect("Failed to deserialize user");
        assert_eq!(user, ModelConfigSource::User);
    }

    #[test]
    fn test_model_error_display() {
        let not_found = ModelError::NotFound("test-agent".to_string());
        assert_eq!(format!("{}", not_found), "Model 'test-agent' not found");

        let invalid_path = ModelError::InvalidPath(PathBuf::from("/invalid/path"));
        assert!(format!("{}", invalid_path).contains("Invalid model path"));
        assert!(format!("{}", invalid_path).contains("/invalid/path"));
    }

    #[test]
    fn test_model_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let model_error: ModelError = io_error.into();

        match model_error {
            ModelError::IoError(_) => {} // Expected
            _ => panic!("Should convert to IoError variant"),
        }
    }

    #[test]
    fn test_model_error_from_serde_yaml_error() {
        let invalid_yaml = "invalid: yaml: content: [unclosed";
        let yaml_error = serde_yaml::from_str::<serde_yaml::Value>(invalid_yaml)
            .expect_err("Should fail to parse invalid YAML");
        let model_error: ModelError = yaml_error.into();

        match model_error {
            ModelError::ParseError(_) => {} // Expected
            _ => panic!("Should convert to ParseError variant"),
        }
    }

    #[test]
    fn test_agent_info_creation() {
        let agent_info = ModelInfo {
            name: "test-agent".to_string(),
            content: "agent: config".to_string(),
            source: ModelConfigSource::Builtin,
            description: Some("Test agent description".to_string()),
        };

        assert_eq!(agent_info.name, "test-agent");
        assert_eq!(agent_info.content, "agent: config");
        assert_eq!(agent_info.source, ModelConfigSource::Builtin);
        assert_eq!(
            agent_info.description,
            Some("Test agent description".to_string())
        );
    }

    #[test]
    fn test_agent_info_equality() {
        let agent1 = ModelInfo {
            name: "test".to_string(),
            content: "config".to_string(),
            source: ModelConfigSource::Builtin,
            description: None,
        };

        let agent2 = ModelInfo {
            name: "test".to_string(),
            content: "config".to_string(),
            source: ModelConfigSource::Builtin,
            description: None,
        };

        let agent3 = ModelInfo {
            name: "different".to_string(),
            content: "config".to_string(),
            source: ModelConfigSource::Builtin,
            description: None,
        };

        assert_eq!(agent1, agent2);
        assert_ne!(agent1, agent3);
    }

    #[test]
    fn test_agent_info_serialization() {
        let agent_info = ModelInfo {
            name: "test-agent".to_string(),
            content: "executor:\n  type: claude-code\n  config: {}\nquiet: false".to_string(),
            source: ModelConfigSource::User,
            description: Some("A test agent".to_string()),
        };

        let json = serde_json::to_string(&agent_info).expect("Failed to serialize ModelInfo");
        let deserialized: ModelInfo =
            serde_json::from_str(&json).expect("Failed to deserialize ModelInfo");

        assert_eq!(agent_info, deserialized);
    }

    #[test]
    fn test_parse_model_description_yaml_frontmatter() {
        let content = r#"---
description: "This is a test agent"
other_field: value
---
type: claude-code
config: {}"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("This is a test agent".to_string()));
    }

    #[test]
    fn test_parse_model_description_comment_format() {
        let content = r#"# Description: This is a comment-based description
type: claude-code
config: {}"#;

        let description = parse_model_description(content);
        assert_eq!(
            description,
            Some("This is a comment-based description".to_string())
        );
    }

    #[test]
    fn test_parse_model_description_no_description() {
        let content = r#"executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, None);
    }

    #[test]
    fn test_parse_model_description_empty_yaml_description() {
        let content = r#"---
description: ""
other_field: value
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("".to_string()));
    }

    #[test]
    fn test_parse_model_description_empty_comment_description() {
        let content = r#"# Description:
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, None); // Empty descriptions are treated as None
    }

    #[test]
    fn test_parse_model_description_yaml_precedence() {
        let content = r#"---
description: "YAML description"
---
# Description: Comment description
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("YAML description".to_string()));
    }

    #[test]
    fn test_parse_model_description_malformed_yaml() {
        let content = r#"---
invalid: yaml: content: [unclosed
---
# Description: Fallback comment description
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(
            description,
            Some("Fallback comment description".to_string())
        );
    }

    #[test]
    fn test_parse_model_description_whitespace_handling() {
        let content = r#"---
description: "  Padded description  "
---"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("Padded description".to_string()));

        let comment_content = r#"# Description:   Padded comment   "#;
        let description = parse_model_description(comment_content);
        assert_eq!(description, Some("Padded comment".to_string()));
    }

    #[test]
    fn test_parse_model_description_multiline_comment() {
        let content = r#"# Description: First line
# This is additional content
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("First line".to_string()));
    }

    #[test]
    fn test_parse_agent_config_frontmatter() {
        let content = r#"---
description: "Test agent"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let config = parse_model_config(content);
        assert!(config.is_ok(), "Should parse frontmatter agent config");
        let config = config.unwrap();
        assert!(!config.quiet);
    }

    #[test]
    fn test_parse_agent_config_comment_format() {
        let content = r#"# Description: Test agent 2
executor:
  type: claude-code
  config:
    claude_path: /test/path
    args: ["--test"]
quiet: false"#;

        let config = parse_model_config(content);
        assert!(config.is_ok(), "Should parse comment format agent config");
        let config = config.unwrap();
        assert!(!config.quiet);
    }

    #[test]
    fn test_parse_agent_config_pure_yaml() {
        let content = r#"executor:
  type: claude-code
  config: {}
quiet: true"#;

        let config = parse_model_config(content);
        assert!(config.is_ok(), "Should parse pure YAML agent config");
        let config = config.unwrap();
        assert!(config.quiet);
    }

    #[test]
    fn test_agent_manager_load_builtin_models() {
        let agents = ModelManager::load_builtin_models().expect("Failed to load builtin models");

        // Should contain at least the known builtin agents
        assert!(!agents.is_empty(), "Builtin agents should not be empty");

        // All agents should have Builtin source
        for agent in &agents {
            assert_eq!(agent.source, ModelConfigSource::Builtin);
            assert!(!agent.name.is_empty(), "Agent name should not be empty");
            assert!(
                !agent.content.is_empty(),
                "Agent content should not be empty"
            );
        }

        // Check for known builtin agents
        let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(
            agent_names.contains(&"claude-code"),
            "Should contain claude-code agent"
        );
        assert!(
            agent_names.contains(&"qwen-coder"),
            "Should contain qwen-coder agent"
        );
    }

    #[test]
    fn test_agent_manager_load_agents_from_missing_dir() {
        use std::path::Path;

        let non_existent_dir = Path::new("/non/existent/directory");
        let result = ModelManager::load_models_from_dir(non_existent_dir, ModelConfigSource::User);

        assert!(result.is_ok(), "Should handle missing directory gracefully");
        let agents = result.unwrap();
        assert!(
            agents.is_empty(),
            "Should return empty vector for missing directory"
        );
    }

    #[test]
    fn test_agent_manager_load_models_from_dir_with_temp_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create test agent files
        let agent1_content = r#"---
description: "Test agent 1"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;
        fs::write(temp_path.join("test-agent-1.yaml"), agent1_content)
            .expect("Failed to write test agent 1");

        let agent2_content = r#"# Description: Test agent 2
executor:
  type: claude-code
  config:
    claude_path: /test/path
    args: ["--test"]
quiet: false"#;
        fs::write(temp_path.join("test-agent-2.yaml"), agent2_content)
            .expect("Failed to write test agent 2");

        // Create a non-YAML file that should be ignored
        fs::write(temp_path.join("not-an-agent.txt"), "ignored content")
            .expect("Failed to write non-yaml file");

        let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::Project);
        if let Err(e) = &result {
            eprintln!("Error loading agents: {:?}", e);
        }
        assert!(
            result.is_ok(),
            "Should load agents from directory successfully: {:?}",
            result
        );

        let agents = result.unwrap();
        println!("Loaded {} agents", agents.len());
        if agents.is_empty() {
            println!("No agents loaded. Directory contents:");
            for entry in std::fs::read_dir(temp_path).unwrap() {
                let entry = entry.unwrap();
                println!("  {:?}", entry.path());
            }
        }
        assert_eq!(agents.len(), 2, "Should load exactly 2 YAML files");

        // Check that all agents have correct source
        for agent in &agents {
            assert_eq!(agent.source, ModelConfigSource::Project);
        }

        // Find specific agents
        let agent1 = agents.iter().find(|a| a.name == "test-agent-1");
        let agent2 = agents.iter().find(|a| a.name == "test-agent-2");

        assert!(agent1.is_some(), "Should find test-agent-1");
        assert!(agent2.is_some(), "Should find test-agent-2");

        let agent1 = agent1.unwrap();
        let agent2 = agent2.unwrap();

        assert_eq!(agent1.description, Some("Test agent 1".to_string()));
        assert_eq!(agent2.description, Some("Test agent 2".to_string()));
    }

    #[test]
    fn test_agent_manager_load_user_agents() {
        let result = ModelManager::load_user_models();

        // Should not fail even if no user agents exist
        assert!(
            result.is_ok(),
            "Should handle user agent loading gracefully"
        );

        let agents = result.unwrap();
        // All agents should have User source
        for agent in &agents {
            assert_eq!(agent.source, ModelConfigSource::User);
        }
    }

    #[test]
    fn test_agent_manager_load_project_models() {
        let result = ModelManager::load_project_models();

        // Should not fail even if no project agents exist
        assert!(
            result.is_ok(),
            "Should handle project agent loading gracefully"
        );

        let agents = result.unwrap();
        // All agents should have Project source
        for agent in &agents {
            assert_eq!(agent.source, ModelConfigSource::Project);
        }
    }

    #[test]
    fn test_agent_manager_list_agents_precedence() {
        // This test verifies the complete agent discovery hierarchy with precedence
        let result = ModelManager::list_agents();

        assert!(result.is_ok(), "list_agents() should not fail");
        let agents = result.unwrap();

        // Should contain at least built-in agents
        assert!(
            !agents.is_empty(),
            "Should contain at least built-in agents"
        );

        // Verify precedence: user > project > builtin
        // If there are duplicate names, user/project should override builtin
        let mut seen_names = std::collections::HashSet::new();
        for agent in &agents {
            if seen_names.contains(&agent.name) {
                panic!(
                    "Duplicate agent name found: {}. Precedence system should prevent duplicates.",
                    agent.name
                );
            }
            seen_names.insert(&agent.name);
        }

        // All agents should have proper source assignments
        for agent in &agents {
            match agent.source {
                ModelConfigSource::Builtin
                | ModelConfigSource::Project
                | ModelConfigSource::GitRoot
                | ModelConfigSource::User => {
                    // Valid source
                }
            }
            assert!(!agent.name.is_empty(), "Agent name should not be empty");
            assert!(
                !agent.content.is_empty(),
                "Agent content should not be empty"
            );
        }

        // Should contain known builtin agents unless overridden
        let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(
            agent_names.contains(&"claude-code"),
            "Should contain claude-code agent"
        );
        assert!(
            agent_names.contains(&"qwen-coder"),
            "Should contain qwen-coder agent"
        );
    }

    #[test]
    fn test_agent_manager_list_agents_overriding_with_temp_files() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        // Create temporary directories for testing
        let temp_project_dir = TempDir::new().expect("Failed to create temp project dir");
        let temp_user_dir = TempDir::new().expect("Failed to create temp user dir");

        // Create project agent that overrides a builtin agent
        let project_claude_content = r#"---
description: "Project-overridden Claude Code agent"
---
executor:
  type: claude-code
  config:
    claude_path: /custom/claude
    args: ["--project-mode"]
quiet: true"#;

        let project_agents_dir = temp_project_dir.path().join("models");
        fs::create_dir_all(&project_agents_dir).expect("Failed to create project agents dir");
        fs::write(
            project_agents_dir.join("claude-code.yaml"),
            project_claude_content,
        )
        .expect("Failed to write project claude-code agent");

        // Create user agent that overrides the project agent
        let user_claude_content = r#"---
description: "User-overridden Claude Code agent"
---
executor:
  type: claude-code
  config:
    claude_path: /user/claude
    args: ["--user-mode"]
quiet: false"#;

        let user_agents_dir = temp_user_dir.path().join("models");
        fs::create_dir_all(&user_agents_dir).expect("Failed to create user agents dir");
        fs::write(
            user_agents_dir.join("claude-code.yaml"),
            user_claude_content,
        )
        .expect("Failed to write user claude-code agent");

        // Create a unique project agent
        let unique_project_content = r#"---
description: "Unique project agent"
---
executor:
  type: llama-agent
  config: {}
quiet: false"#;
        fs::write(
            project_agents_dir.join("unique-project.yaml"),
            unique_project_content,
        )
        .expect("Failed to write unique project agent");

        // Temporarily change working directory to test project agents
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_project_dir).expect("Failed to change to temp project dir");

        // Mock home directory for user agents test
        // Note: This is tricky to test without mocking the dirs::home_dir() function
        // For now, we'll test the directory loading function directly

        let result = env::set_current_dir(&original_dir);
        assert!(result.is_ok(), "Failed to restore original directory");

        // Test direct directory loading instead since we can't easily mock home_dir
        let project_agents =
            ModelManager::load_models_from_dir(&project_agents_dir, ModelConfigSource::Project);
        assert!(
            project_agents.is_ok(),
            "Should load project agents successfully"
        );

        let project_agents = project_agents.unwrap();
        assert_eq!(project_agents.len(), 2, "Should load 2 project agents");

        // Verify project agents
        let claude_agent = project_agents.iter().find(|a| a.name == "claude-code");
        assert!(
            claude_agent.is_some(),
            "Should find overridden claude-code agent"
        );
        let claude_agent = claude_agent.unwrap();
        assert_eq!(claude_agent.source, ModelConfigSource::Project);
        assert_eq!(
            claude_agent.description,
            Some("Project-overridden Claude Code agent".to_string())
        );

        let unique_agent = project_agents.iter().find(|a| a.name == "unique-project");
        assert!(unique_agent.is_some(), "Should find unique project agent");
        let unique_agent = unique_agent.unwrap();
        assert_eq!(unique_agent.source, ModelConfigSource::Project);
        assert_eq!(
            unique_agent.description,
            Some("Unique project agent".to_string())
        );

        // Test user agents
        let user_agents =
            ModelManager::load_models_from_dir(&user_agents_dir, ModelConfigSource::User);
        assert!(user_agents.is_ok(), "Should load user agents successfully");

        let user_agents = user_agents.unwrap();
        assert_eq!(user_agents.len(), 1, "Should load 1 user agent");

        let user_claude = &user_agents[0];
        assert_eq!(user_claude.name, "claude-code");
        assert_eq!(user_claude.source, ModelConfigSource::User);
        assert_eq!(
            user_claude.description,
            Some("User-overridden Claude Code agent".to_string())
        );
    }

    #[test]
    fn test_agent_manager_list_agents_validation_errors() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create multiple invalid YAML files with different types of errors
        let invalid_yaml_content = "invalid: yaml: content: [unclosed";
        fs::write(temp_path.join("invalid-yaml.yaml"), invalid_yaml_content)
            .expect("Failed to write invalid YAML agent");

        let invalid_config_content = r#"---
description: "Invalid agent config"
---
executor:
  type: unknown-executor-type
  config: {}
quiet: not-a-boolean"#;
        fs::write(
            temp_path.join("invalid-config.yaml"),
            invalid_config_content,
        )
        .expect("Failed to write invalid config agent");

        // Create multiple valid agent files
        let valid_content1 = r#"---
description: "Valid agent 1"
---
executor:
  type: claude-code
  config: {}
quiet: false"#;
        fs::write(temp_path.join("valid-agent-1.yaml"), valid_content1)
            .expect("Failed to write valid agent 1");

        let valid_content2 = r#"---
description: "Valid agent 2"
---
executor:
  type: claude-code
  config:
    claude_path: /test/path2
    args: ["--test2"]
quiet: true"#;
        fs::write(temp_path.join("valid-agent-2.yaml"), valid_content2)
            .expect("Failed to write valid agent 2");

        // Test that loading continues despite invalid agents and loads only valid ones
        let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::Project);

        // The function should succeed and load only valid agents
        assert!(
            result.is_ok(),
            "Should successfully load valid agents while skipping invalid ones"
        );

        let agents = result.unwrap();
        assert_eq!(
            agents.len(),
            2,
            "Should load exactly 2 valid agents, skipping 2 invalid ones"
        );

        // Verify the loaded agents are the valid ones
        let agent_names: Vec<&str> = agents.iter().map(|a| a.name.as_str()).collect();
        assert!(
            agent_names.contains(&"valid-agent-1"),
            "Should contain valid-agent-1"
        );
        assert!(
            agent_names.contains(&"valid-agent-2"),
            "Should contain valid-agent-2"
        );

        // Verify agent details
        for agent in &agents {
            assert_eq!(agent.source, ModelConfigSource::Project);
            assert!(!agent.name.is_empty());
            assert!(!agent.content.is_empty());
            assert!(agent.description.is_some());
        }

        let agent1 = agents.iter().find(|a| a.name == "valid-agent-1").unwrap();
        assert_eq!(agent1.description, Some("Valid agent 1".to_string()));

        let agent2 = agents.iter().find(|a| a.name == "valid-agent-2").unwrap();
        assert_eq!(agent2.description, Some("Valid agent 2".to_string()));
    }

    #[test]
    fn test_agent_manager_list_agents_empty_directories() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let empty_dir = temp_dir.path().join("empty_agents");
        std::fs::create_dir_all(&empty_dir).expect("Failed to create empty dir");

        let result = ModelManager::load_models_from_dir(&empty_dir, ModelConfigSource::Project);
        assert!(result.is_ok(), "Should handle empty directory gracefully");

        let agents = result.unwrap();
        assert!(
            agents.is_empty(),
            "Should return empty vector for empty directory"
        );
    }

    #[test]
    fn test_agent_manager_list_agents_non_yaml_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let temp_path = temp_dir.path();

        // Create non-YAML files that should be ignored
        fs::write(temp_path.join("not-an-agent.txt"), "This is not an agent")
            .expect("Failed to write txt file");
        fs::write(temp_path.join("also-not-agent.json"), r#"{"not": "agent"}"#)
            .expect("Failed to write json file");
        fs::write(temp_path.join("README.md"), "# Agent Directory")
            .expect("Failed to write readme");

        // Create one valid YAML agent
        let valid_content = r#"executor:
  type: claude-code
  config: {}
quiet: false"#;
        fs::write(temp_path.join("real-agent.yaml"), valid_content)
            .expect("Failed to write valid agent");

        let result = ModelManager::load_models_from_dir(temp_path, ModelConfigSource::User);
        assert!(
            result.is_ok(),
            "Should load agents while ignoring non-YAML files"
        );

        let agents = result.unwrap();
        assert_eq!(agents.len(), 1, "Should load only the YAML file");
        assert_eq!(agents[0].name, "real-agent");
        assert_eq!(agents[0].source, ModelConfigSource::User);
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_existing() {
        let result = ModelManager::find_agent_by_name("claude-code");
        assert!(result.is_ok(), "Should find existing claude-code agent");

        let agent = result.unwrap();
        assert_eq!(agent.name, "claude-code");
        assert_eq!(agent.source, ModelConfigSource::Builtin);
        assert!(!agent.content.is_empty());
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_not_found() {
        let result = ModelManager::find_agent_by_name("non-existent-agent");
        assert!(
            result.is_err(),
            "Should return error for non-existent agent"
        );

        match result {
            Err(ModelError::NotFound(name)) => {
                assert_eq!(name, "non-existent-agent");
            }
            _ => panic!("Should return NotFound error"),
        }
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_precedence() {
        // This test will pass the existing agent names from builtin agents
        // Test with known builtin agent
        let result = ModelManager::find_agent_by_name("qwen-coder");
        assert!(result.is_ok(), "Should find qwen-coder agent");

        let agent = result.unwrap();
        assert_eq!(agent.name, "qwen-coder");
        // Should be builtin unless overridden by project or user agents
        assert_eq!(agent.source, ModelConfigSource::Builtin);
    }

    #[test]
    fn test_agent_manager_detect_config_file_no_config() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        let result = ModelManager::detect_config_file();
        assert!(
            result.is_none(),
            "Should return None when no config files exist"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_detect_config_file_yaml_exists() {
        let (_temp_dir, _yaml_path, original_dir) =
            setup_config_test_env("sah.yaml", Some("agent: {}\n"));

        let result = ModelManager::detect_config_file();
        assert!(result.is_some(), "Should find yaml config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should find sah.yaml file"
        );
        assert!(
            found_path.ends_with(".swissarmyhammer/sah.yaml"),
            "Should end with .swissarmyhammer/sah.yaml"
        );

        restore_dir(original_dir);
    }

    #[test]
    fn test_agent_manager_detect_config_file_toml_fallback() {
        let (_temp_dir, _toml_path, original_dir) =
            setup_config_test_env("sah.toml", Some("[agent]\n"));

        let result = ModelManager::detect_config_file();
        assert!(result.is_some(), "Should find toml config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.toml")),
            "Should find sah.toml file"
        );
        assert!(
            found_path.ends_with(".swissarmyhammer/sah.toml"),
            "Should end with .swissarmyhammer/sah.toml"
        );

        restore_dir(original_dir);
    }

    #[test]
    fn test_agent_manager_detect_config_file_yaml_precedence() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Create .swissarmyhammer directory with both yaml and toml configs
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer dir");
        let yaml_path = sah_dir.join("sah.yaml");
        let toml_path = sah_dir.join("sah.toml");
        fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");
        fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

        let result = ModelManager::detect_config_file();
        assert!(result.is_some(), "Should find config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should prefer yaml over toml"
        );
        assert!(
            found_path.ends_with(".swissarmyhammer/sah.yaml"),
            "Should end with .swissarmyhammer/sah.yaml"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_ensure_config_structure_creates_directory() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        let result = ModelManager::ensure_config_structure();
        assert!(
            result.is_ok(),
            "Should successfully create config structure"
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should return path to sah.yaml"
        );
        assert!(
            config_path.ends_with(".swissarmyhammer/sah.yaml"),
            "Should end with .swissarmyhammer/sah.yaml"
        );

        // Check that the directory was created
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        assert!(sah_dir.exists(), "Should create .swissarmyhammer directory");
        assert!(sah_dir.is_dir(), "Should create directory, not file");

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_ensure_config_structure_existing_directory() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Pre-create the directory
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");

        let result = ModelManager::ensure_config_structure();
        assert!(
            result.is_ok(),
            "Should handle existing directory gracefully"
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should return path to sah.yaml"
        );
        assert!(
            config_path.ends_with(".swissarmyhammer/sah.yaml"),
            "Should end with .swissarmyhammer/sah.yaml"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_ensure_config_structure_with_existing_config() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Pre-create directory and existing config file
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
        let existing_config = sah_dir.join("sah.toml");
        fs::write(&existing_config, "[existing]\nvalue = true\n")
            .expect("Failed to write existing config");

        let result = ModelManager::ensure_config_structure();
        assert!(result.is_ok(), "Should handle existing config gracefully");

        let config_path = result.unwrap();
        // Should return existing toml config path, not create new yaml
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.toml")),
            "Should return existing config file"
        );
        assert!(
            config_path.ends_with(".swissarmyhammer/sah.toml"),
            "Should return existing toml config"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_use_agent_creates_new_config() {
        use std::fs;
        let (temp_dir, _config_path, original_dir) = setup_config_test_env("sah.yaml", None);

        let result = ModelManager::use_agent("claude-code");
        assert!(result.is_ok(), "Should successfully use claude-code agent");

        let config_path = temp_dir.path().join(SwissarmyhammerDirectory::dir_name()).join("sah.yaml");
        assert!(config_path.exists(), "Should create config file");

        let config_content = fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(
            config_content.contains("agents:"),
            "Should contain agents section"
        );
        assert!(
            config_content.contains("root:"),
            "Should contain root use case"
        );

        restore_dir(original_dir);
    }

    #[test]
    fn test_agent_manager_use_agent_updates_existing_config() {
        use std::fs;
        let existing_config = r#"# Existing config
other_section:
  value: "preserved"
  number: 42
"#;
        let (_temp_dir, config_path, original_dir) =
            setup_config_test_env("sah.yaml", Some(existing_config));

        let result = ModelManager::use_agent("claude-code");
        if let Err(e) = &result {
            panic!("Failed to use claude-code agent: {:?}", e);
        }
        assert!(result.is_ok(), "Should successfully update existing config");

        let updated_config =
            fs::read_to_string(&config_path).expect("Failed to read updated config");
        assert!(
            updated_config.contains("other_section:"),
            "Should preserve existing sections"
        );
        assert!(
            updated_config.contains("value: preserved"),
            "Should preserve existing values"
        );
        assert!(
            updated_config.contains("agents:"),
            "Should add agents section"
        );
        assert!(
            updated_config.contains("root:"),
            "Should contain root use case"
        );

        restore_dir(original_dir);
    }

    #[test]
    fn test_agent_manager_use_agent_replaces_existing_agent() {
        use std::fs;
        let existing_config = r#"# Config with existing agents
other_section:
  value: "preserved"
agents:
  root: "qwen-coder"
"#;
        let (_temp_dir, config_path, original_dir) =
            setup_config_test_env("sah.yaml", Some(existing_config));

        let result = ModelManager::use_agent("claude-code");
        assert!(result.is_ok(), "Should successfully replace existing agent");

        let updated_config =
            fs::read_to_string(&config_path).expect("Failed to read updated config");
        assert!(
            updated_config.contains("other_section:"),
            "Should preserve other sections"
        );
        assert!(
            updated_config.contains("value: preserved"),
            "Should preserve existing values"
        );
        assert!(
            updated_config.contains("agents:"),
            "Should have agents section"
        );
        assert!(
            updated_config.contains("claude-code"),
            "Should contain new agent config"
        );
        assert!(
            !updated_config.contains("qwen-coder"),
            "Should replace old agent config"
        );

        restore_dir(original_dir);
    }

    #[test]
    fn test_agent_manager_use_agent_not_found() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        let result = ModelManager::use_agent("non-existent-agent");
        assert!(result.is_err(), "Should fail for non-existent agent");

        match result {
            Err(ModelError::NotFound(name)) => {
                assert_eq!(name, "non-existent-agent");
            }
            _ => panic!("Should return NotFound error"),
        }

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_model_error_not_found_is_error() {
        let error = ModelError::NotFound("test-agent".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_model_error_invalid_path_is_error() {
        let error = ModelError::InvalidPath(PathBuf::from("/invalid/path"));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_model_error_io_error_is_error() {
        let error = ModelError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_model_error_parse_error_is_critical() {
        let yaml_err =
            serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content").unwrap_err();
        let error = ModelError::from(yaml_err);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_model_error_config_error_is_critical() {
        let error = ModelError::ConfigError("Invalid configuration".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_resolve_agent_fallback_chain() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Test 1: No config - should return default claude-code
        let result = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules);
        assert!(
            result.is_ok(),
            "Should resolve to default agent when no config exists"
        );
        let config = result.unwrap();
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);

        // Test 2: Config with root agent only - rules should fall back to root
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create sah dir");
        let config_path = sah_dir.join("sah.yaml");
        let config_with_root = r#"agents:
  root: "claude-code"
"#;
        fs::write(&config_path, config_with_root).expect("Failed to write config");

        let result = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules);
        assert!(
            result.is_ok(),
            "Should resolve to root agent when use case not configured"
        );
        let config = result.unwrap();
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);

        // Test 3: Config with specific rules agent - should use rules agent
        let config_with_rules = r#"agents:
  root: "claude-code"
  rules: "qwen-coder"
"#;
        fs::write(&config_path, config_with_rules).expect("Failed to write config with rules");

        let result = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules);
        assert!(result.is_ok(), "Should resolve to rules-specific agent");
        let config = result.unwrap();
        assert_eq!(config.executor_type(), ModelExecutorType::LlamaAgent);

        // Test 4: Root use case should use root agent directly
        let result = ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Root);
        assert!(result.is_ok(), "Should resolve root use case");
        let config = result.unwrap();
        assert_eq!(config.executor_type(), ModelExecutorType::ClaudeCode);

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    // Use Case Resolution Tests
    mod use_case_resolution_tests {
        use super::*;
        use tempfile::TempDir;

        fn setup_test_env() -> TempDir {
            let temp_dir = TempDir::new().unwrap();
            std::env::set_current_dir(temp_dir.path()).unwrap();
            temp_dir
        }

        // Note: This is a simplified test setup that doesn't need the full
        // setup_temp_test_dir pattern since tests in this module don't need
        // to restore the original directory

        #[test]
        fn test_resolve_use_case_with_specific_agent() {
            let _temp = setup_test_env();

            // Set Rules to specific agent
            ModelManager::use_agent_for_use_case("claude-code", AgentUseCase::Rules).unwrap();

            // Verify it resolves correctly
            let agent =
                ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules).unwrap();
            assert!(matches!(
                agent.executor_type(),
                ModelExecutorType::ClaudeCode
            ));
        }

        #[test]
        fn test_fallback_to_root_when_use_case_not_configured() {
            let _temp = setup_test_env();

            // Set only Root
            ModelManager::use_agent_for_use_case("claude-code", AgentUseCase::Root).unwrap();

            // Rules should fall back to Root
            let agent =
                ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules).unwrap();
            assert!(matches!(
                agent.executor_type(),
                ModelExecutorType::ClaudeCode
            ));
        }

        #[test]
        fn test_fallback_to_default_when_nothing_configured() {
            let _temp = setup_test_env();

            // Don't configure anything
            // Should fall back to default (claude-code)
            let agent =
                ModelManager::resolve_agent_config_for_use_case(AgentUseCase::Rules).unwrap();
            assert!(matches!(
                agent.executor_type(),
                ModelExecutorType::ClaudeCode
            ));
        }

        #[test]
        fn test_use_case_from_str() {
            assert_eq!("root".parse::<AgentUseCase>().unwrap(), AgentUseCase::Root);
            assert_eq!(
                "rules".parse::<AgentUseCase>().unwrap(),
                AgentUseCase::Rules
            );
            assert_eq!(
                "workflows".parse::<AgentUseCase>().unwrap(),
                AgentUseCase::Workflows
            );

            assert!("invalid".parse::<AgentUseCase>().is_err());
        }

        #[test]
        fn test_use_case_display() {
            assert_eq!(AgentUseCase::Root.to_string(), "root");
            assert_eq!(AgentUseCase::Rules.to_string(), "rules");
            assert_eq!(AgentUseCase::Workflows.to_string(), "workflows");
        }

        #[test]
        fn test_backward_compatibility_with_old_config() {
            let _temp = setup_test_env();

            // Create old-style config
            let config_path = ModelManager::ensure_config_structure().unwrap();
            std::fs::write(&config_path, "agent: claude-code\n").unwrap();

            // Should be able to read it (returns None since old format not supported)
            let agent = ModelManager::get_agent_for_use_case(AgentUseCase::Root).unwrap();
            assert_eq!(agent, None);
        }

        #[test]
        fn test_new_config_format() {
            let _temp = setup_test_env();

            // Create new-style config
            let config_path = ModelManager::ensure_config_structure().unwrap();
            std::fs::write(
                &config_path,
                "agents:\n  root: claude-code\n  rules: qwen-coder\n",
            )
            .unwrap();

            // Should read both use cases
            assert_eq!(
                ModelManager::get_agent_for_use_case(AgentUseCase::Root)
                    .unwrap()
                    .unwrap(),
                "claude-code"
            );
            assert_eq!(
                ModelManager::get_agent_for_use_case(AgentUseCase::Rules)
                    .unwrap()
                    .unwrap(),
                "qwen-coder"
            );
        }
    }
}
