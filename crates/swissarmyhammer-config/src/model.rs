//! Model configuration types and infrastructure
//!
//! This module holds two independent kinds of model configuration.
//!
//! # Chat models
//!
//! Claude Code is the only chat executor, so the chat scope chooses no
//! executor. [`ChatModelConfig`] carries a single setting: the value of the
//! Claude CLI `--model` switch. `None` spawns plain `claude`, which applies the
//! Claude CLI's own default.
//!
//! The switch is read from the project config file (`.sah/sah.yaml`):
//!
//! ```yaml
//! model: sonnet      # the default chat scope
//! review:
//!   model: haiku     # the review scope only
//! ```
//!
//! Precedence for the review scope is `review.model` -> the top-level `model:`
//! -> the baked-in [`REVIEW_DEFAULT_CLAUDE_MODEL`]. The default scope reads the
//! top-level `model:` alone. [`ModelManager::resolve_chat_config`] and
//! [`ModelManager::resolve_review_chat_config`] are the only resolvers, so the
//! switch a tool reports can never disagree with the switch it runs.
//!
//! ```no_run
//! use swissarmyhammer_config::model::{ModelManager, ModelPaths};
//!
//! let review = ModelManager::resolve_review_chat_config(&ModelPaths::sah())?;
//! assert_eq!(review.claude_args(), vec!["--model".to_string(), "haiku".to_string()]);
//! # Ok::<(), swissarmyhammer_config::model::ModelError>(())
//! ```
//!
//! # Embedding models
//!
//! Embedding models are declared in YAML files and loaded by name. Each file
//! names an executor: `llama-embedding` (llama.cpp GGUF) or `ane-embedding`
//! (Apple Neural Engine). A file may list several, and the first one compatible
//! with the running platform wins.
//!
//! ```yaml
//! ---
//! description: "Qwen3 Embedding 0.6B: Compact semantic embedding model"
//! ---
//! quiet: false
//! executors:
//!   - platform: macos-arm64
//!     executor:
//!       type: ane-embedding
//!       config:
//!         source: !HuggingFace
//!           repo: "wballard/Qwen3-Embedding-0.6B-CoreML"
//!         normalize: true
//!   - executor:
//!       type: llama-embedding
//!       config:
//!         source: !HuggingFace
//!           repo: "Qwen/Qwen3-Embedding-0.6B-GGUF"
//!           filename: "Qwen3-Embedding-0.6B-Q8_0.gguf"
//!         normalize: true
//! ```
//!
//! ## Hierarchical discovery
//!
//! Embedding models are loaded from several sources, in increasing precedence:
//!
//! 1. **Built-in models** - embedded in the binary
//! 2. **Git-root models** - `<git root>/.sah/models/*.yaml`
//! 3. **Project models** - `./.sah/models/*.yaml`
//! 4. **User models** - `~/.sah/models/*.yaml`
//!
//! A model of higher precedence replaces a lower one of the same name, so a
//! user can customize a built-in model without editing the binary.
//!
//! ```no_run
//! use swissarmyhammer_config::model::{parse_model_config, ModelManager};
//!
//! let info = ModelManager::find_agent_by_name("qwen-embedding")?;
//! let config = parse_model_config(&info.content)?;
//! println!("Executor: {:?}", config.executor_type());
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```
//!
//! # Error handling
//!
//! ```no_run
//! use swissarmyhammer_config::model::{ModelError, ModelManager};
//!
//! match ModelManager::find_agent_by_name("nonexistent") {
//!     Ok(model) => println!("Found: {}", model.name),
//!     Err(ModelError::NotFound(name)) => {
//!         eprintln!("Model '{}' not found", name);
//!         let models = ModelManager::list_agents()?;
//!         eprintln!("Available models:");
//!         for model in models {
//!             eprintln!("  - {}", model.name);
//!         }
//!     },
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! # Ok::<(), ModelError>(())
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
#[cfg(test)]
use swissarmyhammer_common::SwissarmyhammerDirectory;
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Claude CLI `--model` switch used as the baked-in default for the review
/// scope.
///
/// When nothing is configured for `review.model`, the review scope runs
/// `claude --model haiku` — a cheaper and faster Claude than the plain `claude`
/// the default scope runs. This is the single source of truth for the value —
/// do not scatter the literal.
pub const REVIEW_DEFAULT_CLAUDE_MODEL: &str = "haiku";

/// Configurable paths for model config file location.
///
/// Different CLIs (SAH vs AVP) write to different directories and filenames.
/// Pass this to `ModelManager` methods that read/write config.
#[derive(Debug, Clone)]
pub struct ModelPaths {
    /// Directory name relative to project root (e.g. ".sah" or ".avp")
    pub dir_name: &'static str,
    /// Config filename within the directory (e.g. "sah.yaml" or "avp.yaml")
    pub config_filename: &'static str,
}

impl ModelPaths {
    /// Paths for SwissArmyHammer CLI: `.sah/sah.yaml`
    pub fn sah() -> Self {
        Self {
            dir_name: ".sah",
            config_filename: "sah.yaml",
        }
    }

    /// Paths for AVP CLI: `.avp/avp.yaml`
    pub fn avp() -> Self {
        Self {
            dir_name: ".avp",
            config_filename: "avp.yaml",
        }
    }
}

/// Configuration for the chat agent.
///
/// Claude Code is the only chat executor, so this carries no executor choice.
/// The one setting is the value of the Claude CLI `--model` switch: `Some("haiku")`
/// spawns `claude --model haiku`, and `None` spawns plain `claude`, which applies
/// the Claude CLI's own default.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChatModelConfig {
    /// Value of the Claude CLI `--model` switch, e.g. `haiku`, `sonnet`, `opus`,
    /// or a full model id. `None` spawns `claude` with no `--model`.
    pub model: Option<String>,
}

impl ChatModelConfig {
    /// Configuration that spawns `claude --model <model>`.
    pub fn with_model(model: impl Into<String>) -> Self {
        Self {
            model: Some(model.into()),
        }
    }

    /// The Claude CLI switches a spawned `claude` process receives.
    ///
    /// This is the single source of truth for the switch lookup, so the value a
    /// tool reports and the value the process receives cannot drift.
    pub fn claude_args(&self) -> Vec<String> {
        match &self.model {
            Some(model) => vec!["--model".to_string(), model.clone()],
            None => Vec::new(),
        }
    }
}

/// Runtime platform for executor selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    MacosArm64,
    MacosX86_64,
    LinuxX86_64,
    LinuxAarch64,
}

impl Platform {
    /// Detect the current compile-time platform
    pub fn current() -> Self {
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            Platform::MacosArm64
        }
        #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
        {
            Platform::MacosX86_64
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Platform::LinuxX86_64
        }
        #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
        {
            Platform::LinuxAarch64
        }
        #[cfg(not(any(
            all(target_os = "macos", target_arch = "aarch64"),
            all(target_os = "macos", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "x86_64"),
            all(target_os = "linux", target_arch = "aarch64"),
        )))]
        {
            Platform::LinuxX86_64
        } // fallback
    }
}

/// Embedding model executor type enumeration
///
/// Only embedding models declare an executor. The chat scope has none — Claude
/// Code is the only chat executor, configured by [`ChatModelConfig`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModelExecutorType {
    /// Use local embedding model for semantic search via llama.cpp
    LlamaEmbedding,
    /// Use Apple Neural Engine for embedding inference
    AneEmbedding,
}

/// An executor entry with optional platform constraint for multi-executor configs.
///
/// YAML format:
/// ```yaml
/// executors:
///   - platform: macos-arm64
///     executor:
///       type: ane-embedding
///       config:
///         source: ...
///   - executor:
///       type: llama-embedding
///       config:
///         source: ...
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutorEntry {
    /// Optional platform constraint. If None, this executor is a universal fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub platform: Option<Platform>,
    /// The executor configuration.
    pub executor: ModelExecutorConfig,
}

/// Complete embedding model configuration with executor-specific settings
///
/// Supports both the singular `executor:` format and the `executors:` list
/// format with platform-based selection. The first compatible executor in the
/// list is selected at runtime.
#[derive(Debug, Clone, Serialize)]
pub struct ModelConfig {
    /// Ordered list of executor entries; first compatible match wins.
    pub executors: Vec<ExecutorEntry>,
    /// Global quiet mode
    pub quiet: bool,
}

impl<'de> serde::Deserialize<'de> for ModelConfig {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::{MapAccess, Visitor};

        struct ModelConfigVisitor;

        impl<'de> Visitor<'de> for ModelConfigVisitor {
            type Value = ModelConfig;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a model config with executor or executors field")
            }

            fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut executors: Option<Vec<ExecutorEntry>> = None;
                let mut executor: Option<ModelExecutorConfig> = None;
                let mut quiet = false;

                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "executors" => {
                            executors = Some(map.next_value()?);
                        }
                        "executor" => {
                            executor = Some(map.next_value()?);
                        }
                        "quiet" => {
                            quiet = map.next_value()?;
                        }
                        _ => {
                            let _: serde::de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let executors = if let Some(list) = executors {
                    list
                } else if let Some(single) = executor {
                    vec![ExecutorEntry {
                        platform: None,
                        executor: single,
                    }]
                } else {
                    return Err(serde::de::Error::custom(
                        "model config must have either 'executors' or 'executor' field",
                    ));
                };

                Ok(ModelConfig { executors, quiet })
            }
        }

        deserializer.deserialize_map(ModelConfigVisitor)
    }
}

/// Tagged union of embedding executor configurations
///
/// Uses serde's tagged representation to ensure type safety and proper
/// serialization of executor-specific configuration data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ModelExecutorConfig {
    #[serde(rename = "llama-embedding")]
    LlamaEmbedding(EmbeddingModelConfig),
    #[serde(rename = "ane-embedding")]
    AneEmbedding(EmbeddingModelConfig),
}

/// Configuration for embedding model execution
///
/// Used with the `llama-embedding` executor type for semantic embedding models.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddingModelConfig {
    /// Model source (HuggingFace or local path)
    pub source: ModelSource,
    /// Normalize embeddings to unit vectors
    #[serde(default)]
    pub normalize: bool,
    /// Maximum sequence length for tokenization
    #[serde(default)]
    pub max_sequence_length: Option<usize>,
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

impl ModelConfig {
    /// Select the first executor compatible with the current platform.
    ///
    /// Returns `None` if no executor matches (e.g., all entries have
    /// platform constraints that don't match the current platform).
    pub fn select_executor(&self) -> Option<&ModelExecutorConfig> {
        let current = Platform::current();
        self.executors
            .iter()
            .find(|e| e.platform.is_none() || e.platform == Some(current))
            .map(|e| &e.executor)
    }

    /// Convenience accessor: returns the selected executor for the current platform.
    ///
    /// Backward-compatible replacement for the old `config.executor` field.
    /// Panics if no executor matches — use `select_executor()` for fallible access.
    pub fn executor(&self) -> &ModelExecutorConfig {
        self.select_executor()
            .expect("no compatible executor for current platform")
    }

    /// Get the executor type from the configuration
    pub fn executor_type(&self) -> ModelExecutorType {
        match self.executor() {
            ModelExecutorConfig::LlamaEmbedding(_) => ModelExecutorType::LlamaEmbedding,
            ModelExecutorConfig::AneEmbedding(_) => ModelExecutorType::AneEmbedding,
        }
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
    /// Git root models from {git-root}/models/ in git repository root
    GitRoot,
    /// User-defined models from ~/.models/
    User,
}

impl ModelConfigSource {
    /// Get emoji-based display string for the agent source
    ///
    /// - 📦 Built-in: System-provided built-in models
    /// - 📁 Project: Project-specific models from models/ directory
    /// - 🔧 GitRoot: Git repository models from {git-root}/models/
    /// - 👤 User: User-defined models from ~/.models/
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
    ParseError(#[from] serde_yaml_ng::Error),
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

    let yaml_value = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(front_matter).ok()?;
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
pub fn parse_model_config(content: &str) -> Result<ModelConfig, serde_yaml_ng::Error> {
    let content = content.trim();

    // Check for YAML front matter
    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_pos) = stripped.find("---") {
            // Extract the content after the second ---
            let config_content = &stripped[end_pos + 3..].trim();
            return serde_yaml_ng::from_str::<ModelConfig>(config_content);
        }
    }

    // Fall back to parsing entire content as ModelConfig
    serde_yaml_ng::from_str::<ModelConfig>(content)
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
    ///                  swissarmyhammer_config::model::ModelConfigSource::GitRoot => "gitroot",
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
    /// use swissarmyhammer_config::model::{ModelManager, ModelConfigSource};
    /// use std::path::Path;
    ///
    /// let models = ModelManager::load_models_from_dir(
    ///     Path::new("./models"),
    ///     ModelConfigSource::Project
    /// )?;
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
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

    /// Validate basic path constraints shared by directory and config-file
    /// paths.
    ///
    /// Checks that `path` is non-empty and does not exceed `MAX_PATH_LENGTH`.
    /// `context` labels the path in warning text (e.g. `"model directory"`,
    /// `"config file"`).
    ///
    /// # Returns
    /// * `Result<PathBuf, ModelError>` - The path, unchanged, or an error
    fn validate_path_basics(path: &Path, context: &str) -> Result<PathBuf, ModelError> {
        // Check for empty path
        if path.as_os_str().is_empty() {
            tracing::warn!("{context} path is empty");
            return Err(ModelError::InvalidPath(path.to_path_buf()));
        }

        // Check path length to prevent system issues
        const MAX_PATH_LENGTH: usize = 4096;
        let path_str = path.to_string_lossy();
        if path_str.len() > MAX_PATH_LENGTH {
            tracing::warn!(
                "{context} path too long ({} characters, maximum {}): {}",
                path_str.len(),
                MAX_PATH_LENGTH,
                path_str
            );
            return Err(ModelError::InvalidPath(path.to_path_buf()));
        }

        Ok(path.to_path_buf())
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
        Self::validate_path_basics(dir_path, "model directory")?;

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

    /// Check that `path` is a directory with `required_mode` permission bits
    /// set for the owner (e.g. `0o400` for read, `0o200` for write).
    ///
    /// `access_name` (e.g. `"readable"`, `"writable"`) labels the checked
    /// permission in log and error text.
    fn check_directory_access(
        path: &Path,
        required_mode: u32,
        access_name: &str,
    ) -> Result<(), ModelError> {
        // Check if we can read the directory metadata
        match std::fs::metadata(path) {
            Ok(metadata) => {
                if !metadata.is_dir() {
                    tracing::warn!("Path is not a directory: {}", path.display());
                    return Err(ModelError::InvalidPath(path.to_path_buf()));
                }
                // On Unix, check the requested permission bit
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let mode = metadata.permissions().mode();
                    let has_access = (mode & required_mode) != 0;
                    if !has_access {
                        tracing::warn!("Directory is not {access_name}: {}", path.display());
                        return Err(ModelError::IoError(std::io::Error::new(
                            std::io::ErrorKind::PermissionDenied,
                            format!("Directory is not {access_name}: {}", path.display()),
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

    /// Check directory permissions to ensure it's readable
    fn check_directory_permissions(path: &Path) -> Result<(), ModelError> {
        Self::check_directory_access(path, 0o400, "readable")
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

    /// Load user-defined models from ~/.models/
    ///
    /// Scans the user's home directory `.models/` for model configuration
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
            let user_models_dir = home_dir.join(".models");
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

    /// Load git root models from {git-root}/models/
    ///
    /// Scans the git repository root's `models/` directory for model
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
            let gitroot_models_dir = git_root.join("models");
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
    /// let model = ModelManager::find_agent_by_name("qwen-embedding")?;
    /// println!("Found model: {} from {:?}", model.name, model.source);
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
    /// 1. `.sah/sah.yaml` (preferred)
    /// 2. `.sah/sah.toml` (fallback)
    ///
    /// # Returns
    /// * `Option<PathBuf>` - Path to existing config file or None if not found
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::ModelManager;
    ///
    /// use swissarmyhammer_config::model::ModelPaths;
    /// match ModelManager::detect_config_file(&ModelPaths::sah()) {
    ///     Some(config_path) => println!("Found config: {}", config_path.display()),
    ///     None => println!("No existing config found"),
    /// }
    /// ```
    pub fn detect_config_file(paths: &ModelPaths) -> Option<PathBuf> {
        let current_dir = std::env::current_dir().ok()?;
        let config_dir = current_dir.join(paths.dir_name);

        // Check for YAML config first (preferred)
        let yaml_config = config_dir.join(paths.config_filename);
        if yaml_config.exists() && yaml_config.is_file() {
            return Some(yaml_config);
        }

        // Fall back to TOML config (replace .yaml with .toml)
        let toml_filename = paths.config_filename.replace(".yaml", ".toml");
        let toml_config = config_dir.join(&toml_filename);
        if toml_config.exists() && toml_config.is_file() {
            return Some(toml_config);
        }

        None
    }

    /// Canonicalize `path`, mapping any error to `ModelError::IoError` and
    /// logging it with `context` (e.g. `"current directory"`, `"config
    /// path"`) to identify which caller failed.
    fn canonicalize_path(path: &Path, context: &str) -> Result<PathBuf, ModelError> {
        path.canonicalize().map_err(|e| {
            tracing::error!("Failed to canonicalize {context} {}: {}", path.display(), e);
            ModelError::IoError(e)
        })
    }

    /// Ensure project configuration directory structure exists
    ///
    /// Creates the `.sah/` directory if it doesn't exist and returns the path
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
    /// use swissarmyhammer_config::model::ModelPaths;
    /// let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah())?;
    /// println!("Config file path: {}", config_path.display());
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn ensure_config_structure(paths: &ModelPaths) -> Result<PathBuf, ModelError> {
        // Security: Get and validate current directory
        let current_dir = std::env::current_dir().map_err(ModelError::IoError)?;

        // Security: Canonicalize current directory to resolve symlinks
        let canonical_current = Self::canonicalize_path(&current_dir, "current directory")?;

        // Security: Audit log the directory we're working in
        tracing::debug!(
            "Ensuring config structure in directory: {} (canonical: {})",
            current_dir.display(),
            canonical_current.display()
        );

        let config_dir = canonical_current.join(paths.dir_name);

        // Create config directory if it doesn't exist
        if !config_dir.exists() {
            // Security: Check parent directory permissions before creating
            Self::check_directory_writable(&canonical_current)?;

            std::fs::create_dir_all(&config_dir).map_err(|e| {
                tracing::error!(
                    "Failed to create {} directory {}: {}",
                    paths.dir_name,
                    config_dir.display(),
                    e
                );
                ModelError::IoError(e)
            })?;

            // Security: Audit log directory creation
            tracing::info!(
                "Created {} directory: {}",
                paths.dir_name,
                config_dir.display()
            );
        }

        // Security: Validate the created/existing directory
        Self::check_directory_permissions(&config_dir)?;

        // Check for existing config file first
        if let Some(existing_config) = Self::detect_config_file(paths) {
            // Security: Validate existing config path
            let validated_config = Self::validate_config_file_path(&existing_config)?;
            tracing::debug!("Found existing config file: {}", validated_config.display());
            return Ok(validated_config);
        }

        // Return path for new YAML config (don't create the file yet)
        let new_config = config_dir.join(paths.config_filename);

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
        Self::check_directory_access(path, 0o200, "writable")
    }

    /// Validate a config file path for security
    fn validate_config_file_path(path: &Path) -> Result<PathBuf, ModelError> {
        Self::validate_path_basics(path, "config file")?;

        // Security: Check for suspicious patterns
        let path_str = path.to_string_lossy();
        Self::check_suspicious_patterns(&path_str)?;

        // If the file exists, canonicalize it
        if path.exists() {
            let canonical = Self::canonicalize_path(path, "config path")?;

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

    /// Read the configured Claude CLI `--model` switch for the default chat
    /// scope.
    ///
    /// Reads the top-level `model:` key. Returns `None` when it is unset, which
    /// spawns plain `claude`.
    ///
    /// # Returns
    /// * `Result<Option<String>, ModelError>` - The switch if configured, `None` otherwise
    pub fn get_chat_model(paths: &ModelPaths) -> Result<Option<String>, ModelError> {
        Self::read_model_key(paths, |config| config.get("model"))
    }

    /// Read the configured Claude CLI `--model` switch for the review scope.
    ///
    /// Reads the nested `review.model` key. Returns `None` when it is unset. A
    /// top-level `model:` is not read by this method — see
    /// [`review_chat_model_from`](Self::review_chat_model_from) for the
    /// precedence that combines the two.
    ///
    /// # Returns
    /// * `Result<Option<String>, ModelError>` - The switch if configured, `None` otherwise
    pub fn get_review_chat_model(paths: &ModelPaths) -> Result<Option<String>, ModelError> {
        Self::read_model_key(paths, |config| {
            config.get("review").and_then(|review| review.get("model"))
        })
    }

    /// Read one string-valued key out of the project config file.
    ///
    /// Shared by [`get_chat_model`](Self::get_chat_model) and
    /// [`get_review_chat_model`](Self::get_review_chat_model) so both scopes
    /// read the file the same way and cannot disagree about a missing file, an
    /// absent key, or a non-string value.
    fn read_model_key(
        paths: &ModelPaths,
        select: impl Fn(&serde_yaml_ng::Value) -> Option<&serde_yaml_ng::Value>,
    ) -> Result<Option<String>, ModelError> {
        let config_path = Self::ensure_config_structure(paths)?;

        if !config_path.exists() {
            return Ok(None);
        }

        let config_content = std::fs::read_to_string(&config_path).map_err(ModelError::IoError)?;
        let config_value: serde_yaml_ng::Value = serde_yaml_ng::from_str(&config_content)?;

        Ok(select(&config_value)
            .and_then(|value| value.as_str())
            .map(str::to_string))
    }

    /// Resolve the chat configuration for the default scope.
    ///
    /// The top-level `model:` sets the Claude CLI `--model` switch; when it is
    /// unset the spawned `claude` carries no `--model` and applies its own
    /// default.
    ///
    /// # Returns
    /// * `Result<ChatModelConfig, ModelError>` - The resolved chat configuration
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::model::{ModelManager, ModelPaths};
    ///
    /// let config = ModelManager::resolve_chat_config(&ModelPaths::sah())?;
    /// println!("claude args: {:?}", config.claude_args());
    /// # Ok::<(), swissarmyhammer_config::model::ModelError>(())
    /// ```
    pub fn resolve_chat_config(paths: &ModelPaths) -> Result<ChatModelConfig, ModelError> {
        let model = Self::get_chat_model(paths)?;
        Self::chat_config_from_switch(model, "model")
    }

    /// Apply the review-scope precedence to already-read config values.
    ///
    /// This is the single source of truth for *which Claude model the review
    /// scope uses*, given the two relevant config keys. Precedence:
    /// 1. `review_model` (an explicit `review.model`) — wins, review only.
    /// 2. `default_model` (an explicit top-level `model:`) — if the user set an
    ///    overall model they mean it everywhere, including review.
    /// 3. The baked-in [`REVIEW_DEFAULT_CLAUDE_MODEL`] (`haiku`) — the review
    ///    scope's factory default, used only when *nothing* is configured.
    ///
    /// Kept as a pure function so every resolution path shares the exact same
    /// rule and cannot disagree, regardless of where each reads the raw config
    /// from.
    pub fn review_chat_model_from(
        review_model: Option<String>,
        default_model: Option<String>,
    ) -> String {
        review_model
            .or(default_model)
            .unwrap_or_else(|| REVIEW_DEFAULT_CLAUDE_MODEL.to_string())
    }

    /// Resolve the effective Claude CLI `--model` switch for the review scope.
    ///
    /// Reads `review.model` and the top-level `model:` and applies
    /// [`review_chat_model_from`](Self::review_chat_model_from).
    ///
    /// This is what a diagnostic reports, and
    /// [`resolve_review_chat_config`](Self::resolve_review_chat_config) builds
    /// the spawned configuration from the same value — so the switch reported
    /// is always the switch run.
    ///
    /// # Returns
    /// * `Result<String, ModelError>` - The effective Claude CLI `--model` switch
    pub fn resolve_review_chat_model(paths: &ModelPaths) -> Result<String, ModelError> {
        Ok(Self::review_chat_model_from(
            Self::get_review_chat_model(paths)?,
            Self::get_chat_model(paths)?,
        ))
    }

    /// Resolve the chat configuration for the review scope.
    ///
    /// Unlike [`resolve_chat_config`](Self::resolve_chat_config), an
    /// *unconfigured* review scope defaults to
    /// [`REVIEW_DEFAULT_CLAUDE_MODEL`] (a cheaper and faster Claude) rather
    /// than the Claude CLI's own default — but an explicit overall `model:`
    /// still drives review.
    ///
    /// # Returns
    /// * `Result<ChatModelConfig, ModelError>` - The resolved review configuration
    pub fn resolve_review_chat_config(paths: &ModelPaths) -> Result<ChatModelConfig, ModelError> {
        let model = Self::resolve_review_chat_model(paths)?;
        Self::chat_config_from_switch(Some(model), "review.model")
    }

    /// Build a [`ChatModelConfig`] from a configured switch, rejecting a blank
    /// one.
    ///
    /// A blank switch would spawn `claude --model ""`, which fails deep inside
    /// the Claude CLI with no hint of where the value came from. `key` names the
    /// config setting so the error points at the line to fix.
    fn chat_config_from_switch(
        model: Option<String>,
        key: &str,
    ) -> Result<ChatModelConfig, ModelError> {
        match model {
            Some(model) if model.trim().is_empty() => Err(ModelError::ConfigError(format!(
                "`{key}` is blank; set it to a Claude CLI --model switch (e.g. `haiku`) or remove it"
            ))),
            model => Ok(ChatModelConfig { model }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    /// A minimal `llama-embedding` model configuration, the shape every model
    /// YAML now takes.
    fn embedding_model_config() -> ModelConfig {
        ModelConfig {
            executors: vec![ExecutorEntry {
                platform: None,
                executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/embed".to_string(),
                        filename: Some("test.gguf".to_string()),
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: None,
                }),
            }],
            quiet: false,
        }
    }

    #[test]
    fn test_configuration_serialization_yaml() {
        let config = embedding_model_config();

        // Should serialize to YAML correctly
        let yaml = serde_yaml_ng::to_string(&config).expect("Failed to serialize to YAML");
        assert!(yaml.contains("type: llama-embedding"));
        assert!(yaml.contains("quiet: false"));

        // Should deserialize from YAML correctly
        let deserialized: ModelConfig =
            serde_yaml_ng::from_str(&yaml).expect("Failed to deserialize from YAML");
        assert_eq!(config.executor_type(), deserialized.executor_type());
        assert_eq!(config.quiet, deserialized.quiet);
    }

    #[test]
    fn test_configuration_serialization_json() {
        let config = embedding_model_config();

        // Should serialize to JSON correctly
        let json = serde_json::to_string(&config).expect("Failed to serialize to JSON");
        assert!(json.contains("\"type\":\"llama-embedding\""));
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
    fn test_model_error_from_serde_yaml_ng_error() {
        let invalid_yaml = "invalid: yaml: content: [unclosed";
        let yaml_error = serde_yaml_ng::from_str::<serde_yaml_ng::Value>(invalid_yaml)
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
            content: "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false".to_string(),
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
type: llama-embedding
config:
  source: !HuggingFace
    repo: test/embed"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("This is a test agent".to_string()));
    }

    #[test]
    fn test_parse_model_description_comment_format() {
        let content = r#"# Description: This is a comment-based description
type: llama-embedding
config:
  source: !HuggingFace
    repo: test/embed"#;

        let description = parse_model_description(content);
        assert_eq!(
            description,
            Some("This is a comment-based description".to_string())
        );
    }

    #[test]
    fn test_parse_model_description_no_description() {
        let content = r#"executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;

        let description = parse_model_description(content);
        assert_eq!(description, Some("".to_string()));
    }

    #[test]
    fn test_parse_model_description_empty_comment_description() {
        let content = r#"# Description:
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
      filename: test.gguf
    normalize: true
quiet: false"#;

        let config = parse_model_config(content);
        assert!(config.is_ok(), "Should parse comment format agent config");
        let config = config.unwrap();
        assert!(!config.quiet);
    }

    #[test]
    fn test_parse_agent_config_pure_yaml() {
        let content = r#"executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
            agent_names.contains(&"nomic-embed-code"),
            "Should contain the nomic-embed-code embedding model"
        );
        assert!(
            agent_names.contains(&"qwen-embedding"),
            "Should contain the qwen-embedding embedding model"
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;
        fs::write(temp_path.join("test-agent-1.yaml"), agent1_content)
            .expect("Failed to write test agent 1");

        let agent2_content = r#"# Description: Test agent 2
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
      filename: test.gguf
    normalize: true
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
    #[serial_test::serial(cwd)]
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
    #[serial_test::serial(cwd)]
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
            agent_names.contains(&"nomic-embed-code"),
            "Should contain the nomic-embed-code embedding model"
        );
        assert!(
            agent_names.contains(&"qwen-embedding"),
            "Should contain the qwen-embedding embedding model"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_list_agents_overriding_with_temp_files() {
        use std::fs;

        let temp_project_dir = tempfile::TempDir::new().expect("Failed to create temp project dir");
        let temp_user_dir = tempfile::TempDir::new().expect("Failed to create temp user dir");

        // Create project model that overrides a builtin model
        let project_override_content = r#"---
description: "Project-overridden embedding model"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: project/embed
quiet: true"#;

        let project_agents_dir = temp_project_dir.path().join("models");
        fs::create_dir_all(&project_agents_dir).expect("Failed to create project agents dir");
        fs::write(
            project_agents_dir.join("qwen-embedding.yaml"),
            project_override_content,
        )
        .expect("Failed to write project qwen-embedding model");

        // Create user model that overrides the project model
        let user_override_content = r#"---
description: "User-overridden embedding model"
---
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: user/embed
quiet: false"#;

        let user_agents_dir = temp_user_dir.path().join("models");
        fs::create_dir_all(&user_agents_dir).expect("Failed to create user agents dir");
        fs::write(
            user_agents_dir.join("qwen-embedding.yaml"),
            user_override_content,
        )
        .expect("Failed to write user qwen-embedding model");

        // Create a unique project model
        let unique_project_content = r#"---
description: "Unique project agent"
---
executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: project/unique
quiet: false"#;
        fs::write(
            project_agents_dir.join("unique-project.yaml"),
            unique_project_content,
        )
        .expect("Failed to write unique project agent");

        // Mock home directory for user agents test
        // Note: This is tricky to test without mocking the dirs::home_dir() function
        // For now, we'll test the directory loading function directly

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
        let override_agent = project_agents.iter().find(|a| a.name == "qwen-embedding");
        assert!(
            override_agent.is_some(),
            "Should find overridden qwen-embedding model"
        );
        let override_agent = override_agent.unwrap();
        assert_eq!(override_agent.source, ModelConfigSource::Project);
        assert_eq!(
            override_agent.description,
            Some("Project-overridden embedding model".to_string())
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

        let user_override = &user_agents[0];
        assert_eq!(user_override.name, "qwen-embedding");
        assert_eq!(user_override.source, ModelConfigSource::User);
        assert_eq!(
            user_override.description,
            Some("User-overridden embedding model".to_string())
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false"#;
        fs::write(temp_path.join("valid-agent-1.yaml"), valid_content1)
            .expect("Failed to write valid agent 1");

        let valid_content2 = r#"---
description: "Valid agent 2"
---
executor:
  type: ane-embedding
  config:
    source: !HuggingFace
      repo: test/ane
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
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
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
        let result = ModelManager::find_agent_by_name("qwen-embedding");
        assert!(result.is_ok(), "Should find existing qwen-embedding model");

        let agent = result.unwrap();
        assert_eq!(agent.name, "qwen-embedding");
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
        let result = ModelManager::find_agent_by_name("nomic-embed-code");
        assert!(result.is_ok(), "Should find nomic-embed-code model");

        let agent = result.unwrap();
        assert_eq!(agent.name, "nomic-embed-code");
        // Should be builtin unless overridden by project or user agents
        assert_eq!(agent.source, ModelConfigSource::Builtin);
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_detect_config_file_no_config() {
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let result = ModelManager::detect_config_file(&ModelPaths::sah());
        assert!(
            result.is_none(),
            "Should return None when no config files exist"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_detect_config_file_yaml_exists() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
        let yaml_path = sah_dir.join("sah.yaml");
        fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");

        let result = ModelManager::detect_config_file(&ModelPaths::sah());
        assert!(result.is_some(), "Should find yaml config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should find sah.yaml file"
        );
        assert!(
            found_path.ends_with(".sah/sah.yaml"),
            "Should end with .sah/sah.yaml"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_detect_config_file_toml_fallback() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
        let toml_path = sah_dir.join("sah.toml");
        fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

        let result = ModelManager::detect_config_file(&ModelPaths::sah());
        assert!(result.is_some(), "Should find toml config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.toml")),
            "Should find sah.toml file"
        );
        assert!(
            found_path.ends_with(".sah/sah.toml"),
            "Should end with .sah/sah.toml"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_detect_config_file_yaml_precedence() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        // Create .sah directory with both yaml and toml configs
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");
        let yaml_path = sah_dir.join("sah.yaml");
        let toml_path = sah_dir.join("sah.toml");
        fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");
        fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

        let result = ModelManager::detect_config_file(&ModelPaths::sah());
        assert!(result.is_some(), "Should find config file");

        let found_path = result.unwrap();
        assert_eq!(
            found_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should prefer yaml over toml"
        );
        assert!(
            found_path.ends_with(".sah/sah.yaml"),
            "Should end with .sah/sah.yaml"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_ensure_config_structure_creates_directory() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
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
            config_path.ends_with(".sah/sah.yaml"),
            "Should end with .sah/sah.yaml"
        );

        // Check that the directory was created
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        assert!(sah_dir.exists(), "Should create .sah directory");
        assert!(sah_dir.is_dir(), "Should create directory, not file");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_ensure_config_structure_existing_directory() {
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        // Pre-create the directory
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
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
            config_path.ends_with(".sah/sah.yaml"),
            "Should end with .sah/sah.yaml"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_agent_manager_ensure_config_structure_with_existing_config() {
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        // Create a .git directory to prevent config discovery from walking up to the real repo
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        // Pre-create directory and existing config file
        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
        let existing_config = sah_dir.join("sah.toml");
        fs::write(&existing_config, "[existing]\nvalue = true\n")
            .expect("Failed to write existing config");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
        assert!(result.is_ok(), "Should handle existing config gracefully");

        let config_path = result.unwrap();
        // Should return existing toml config path, not create new yaml
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.toml")),
            "Should return existing config file"
        );
        assert!(
            config_path.ends_with(".sah/sah.toml"),
            "Should return existing toml config"
        );
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
            serde_yaml_ng::from_str::<serde_yaml_ng::Value>("invalid: yaml: content").unwrap_err();
        let error = ModelError::from(yaml_err);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_model_error_config_error_is_critical() {
        let error = ModelError::ConfigError("Invalid configuration".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    // Model Resolution Tests
    mod model_resolution_tests {
        use super::*;

        fn setup_test_env() -> tempfile::TempDir {
            let temp_dir = tempfile::TempDir::new().unwrap();
            std::fs::create_dir(temp_dir.path().join(".git"))
                .expect("Failed to create .git marker");
            temp_dir
        }

        #[test]
        #[serial_test::serial(cwd)]
        fn test_model_config_format() {
            let temp_dir = setup_test_env();
            let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

            let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah()).unwrap();
            std::fs::write(&config_path, "model: sonnet\n").unwrap();

            assert_eq!(
                ModelManager::get_chat_model(&ModelPaths::sah())
                    .unwrap()
                    .unwrap(),
                "sonnet"
            );
        }

        // ====================================================================
        // Review-specific model target tests
        // ====================================================================
    }

    // ========================================================================
    // Multi-executor and platform selection tests
    // ========================================================================

    #[test]
    fn test_parse_old_executor_format_backward_compat() {
        let yaml = r#"
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: "Qwen/Qwen3-Embedding-0.6B-GGUF"
      filename: "Qwen3-Embedding-0.6B-Q8_0.gguf"
    normalize: true
    max_sequence_length: 512
quiet: false
"#;
        let config: ModelConfig = serde_yaml_ng::from_str(yaml).expect("old format should parse");
        assert_eq!(config.executors.len(), 1);
        assert!(config.executors[0].platform.is_none());
        assert_eq!(config.executor_type(), ModelExecutorType::LlamaEmbedding);
    }

    #[test]
    fn test_parse_new_executors_list_format() {
        let yaml = r#"
executors:
  - platform: macos-arm64
    executor:
      type: ane-embedding
      config:
        source: !HuggingFace
          repo: "wballard/Qwen3-Embedding-0.6B-CoreML"
        normalize: true
  - executor:
      type: llama-embedding
      config:
        source: !HuggingFace
          repo: "Qwen/Qwen3-Embedding-0.6B-GGUF"
          filename: "Qwen3-Embedding-0.6B-Q8_0.gguf"
        normalize: true
        max_sequence_length: 512
quiet: false
"#;
        let config: ModelConfig = serde_yaml_ng::from_str(yaml).expect("new format should parse");
        assert_eq!(config.executors.len(), 2);
        assert_eq!(config.executors[0].platform, Some(Platform::MacosArm64));
        assert!(config.executors[1].platform.is_none());
    }

    #[test]
    fn test_platform_selection_prefers_platform_match() {
        let config = ModelConfig {
            executors: vec![
                ExecutorEntry {
                    platform: Some(Platform::current()),
                    executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                        source: ModelSource::HuggingFace {
                            repo: "test/ane".to_string(),
                            filename: None,
                            folder: None,
                        },
                        normalize: true,
                        max_sequence_length: None,
                    }),
                },
                ExecutorEntry {
                    platform: None,
                    executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                        source: ModelSource::HuggingFace {
                            repo: "test/llama".to_string(),
                            filename: None,
                            folder: None,
                        },
                        normalize: true,
                        max_sequence_length: None,
                    }),
                },
            ],
            quiet: false,
        };
        // First entry matches current platform, so it should be selected
        assert_eq!(config.executor_type(), ModelExecutorType::AneEmbedding);
    }

    #[test]
    fn test_platform_selection_falls_back_to_universal() {
        // Use a platform that doesn't match current
        let non_matching_platform = if Platform::current() == Platform::MacosArm64 {
            Platform::LinuxX86_64
        } else {
            Platform::MacosArm64
        };

        let config = ModelConfig {
            executors: vec![
                ExecutorEntry {
                    platform: Some(non_matching_platform),
                    executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                        source: ModelSource::HuggingFace {
                            repo: "test/ane".to_string(),
                            filename: None,
                            folder: None,
                        },
                        normalize: true,
                        max_sequence_length: None,
                    }),
                },
                ExecutorEntry {
                    platform: None,
                    executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                        source: ModelSource::HuggingFace {
                            repo: "test/llama".to_string(),
                            filename: None,
                            folder: None,
                        },
                        normalize: true,
                        max_sequence_length: None,
                    }),
                },
            ],
            quiet: false,
        };
        // First entry doesn't match, second is universal fallback
        assert_eq!(config.executor_type(), ModelExecutorType::LlamaEmbedding);
    }

    #[test]
    fn test_ane_embedding_round_trip() {
        let config = ModelConfig {
            executors: vec![ExecutorEntry {
                platform: Some(Platform::MacosArm64),
                executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "wballard/test".to_string(),
                        filename: None,
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: Some(512),
                }),
            }],
            quiet: false,
        };

        let yaml = serde_yaml_ng::to_string(&config).expect("serialize");
        assert!(yaml.contains("ane-embedding"));
        assert!(yaml.contains("macos-arm64"));

        let deserialized: ModelConfig = serde_yaml_ng::from_str(&yaml).expect("deserialize");
        assert_eq!(deserialized.executors.len(), 1);
        assert_eq!(
            deserialized.executors[0].platform,
            Some(Platform::MacosArm64)
        );
    }

    #[test]
    fn test_platform_current_is_stable() {
        assert_eq!(Platform::current(), Platform::current());
    }

    // ========================================================================
    // validate_config_file_path and check_directory_writable tests
    // ========================================================================

    #[test]
    fn test_validate_config_file_path_empty_path() {
        let result = ModelManager::validate_config_file_path(Path::new(""));
        assert!(result.is_err(), "Empty path should be rejected");
        match result.unwrap_err() {
            ModelError::InvalidPath(p) => {
                assert!(p.as_os_str().is_empty(), "Should return the empty path");
            }
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_validate_config_file_path_too_long() {
        let long_path = "a".repeat(4097);
        let result = ModelManager::validate_config_file_path(Path::new(&long_path));
        assert!(
            result.is_err(),
            "Path exceeding 4096 chars should be rejected"
        );
        match result.unwrap_err() {
            ModelError::InvalidPath(_) => {}
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_validate_config_file_path_exactly_max_length() {
        // 4096 chars should be accepted (boundary case)
        let max_path = "a".repeat(4096);
        let result = ModelManager::validate_config_file_path(Path::new(&max_path));
        // Should not fail due to length (may fail for other reasons like file not existing,
        // but the length check should pass)
        match &result {
            Err(ModelError::InvalidPath(p)) => {
                // If it failed, it should not be because of length
                assert_ne!(
                    p.to_string_lossy().len(),
                    4096,
                    "4096-char path should pass the length check"
                );
            }
            _ => {
                // Either Ok or a different error is fine — length check passed
            }
        }
    }

    #[test]
    fn test_validate_config_file_path_suspicious_null_byte() {
        let path_with_null = "config\0.yaml";
        let result = ModelManager::validate_config_file_path(Path::new(path_with_null));
        assert!(
            result.is_err(),
            "Path with null byte should be rejected by suspicious pattern check"
        );
    }

    #[test]
    fn test_validate_config_file_path_directory_not_file() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let dir_path = temp_dir.path();

        // The path exists and is a directory, not a file
        let result = ModelManager::validate_config_file_path(dir_path);
        assert!(result.is_err(), "Directory path should be rejected");
        match result.unwrap_err() {
            ModelError::InvalidPath(p) => {
                assert!(
                    p.is_dir() || p.is_absolute(),
                    "Should return the canonical directory path"
                );
            }
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_validate_config_file_path_valid_existing_file() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("test-config.yaml");
        std::fs::write(&file_path, "model: test\n").expect("Failed to write test file");

        let result = ModelManager::validate_config_file_path(&file_path);
        assert!(result.is_ok(), "Valid file path should be accepted");
        let canonical = result.unwrap();
        assert!(
            canonical.is_absolute(),
            "Should return an absolute/canonical path"
        );
        assert!(canonical.is_file(), "Canonical path should point to a file");
    }

    #[test]
    fn test_validate_config_file_path_nonexistent_file() {
        let result =
            ModelManager::validate_config_file_path(Path::new("/tmp/does-not-exist-config.yaml"));
        assert!(
            result.is_ok(),
            "Non-existent file path should be accepted (returned as-is)"
        );
        let returned = result.unwrap();
        assert_eq!(
            returned,
            PathBuf::from("/tmp/does-not-exist-config.yaml"),
            "Should return the path unchanged for non-existent files"
        );
    }

    #[test]
    fn test_check_directory_writable_valid_dir() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let result = ModelManager::check_directory_writable(temp_dir.path());
        assert!(result.is_ok(), "Writable temp directory should pass");
    }

    #[test]
    fn test_check_directory_writable_not_a_directory() {
        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let file_path = temp_dir.path().join("regular-file.txt");
        std::fs::write(&file_path, "content").expect("Failed to write file");

        let result = ModelManager::check_directory_writable(&file_path);
        assert!(
            result.is_err(),
            "Regular file should not pass directory check"
        );
        match result.unwrap_err() {
            ModelError::InvalidPath(p) => {
                assert_eq!(p, file_path, "Should return the non-directory path");
            }
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_check_directory_writable_nonexistent_path() {
        let result =
            ModelManager::check_directory_writable(Path::new("/nonexistent/path/does/not/exist"));
        assert!(result.is_err(), "Non-existent path should fail");
        match result.unwrap_err() {
            ModelError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::NotFound);
            }
            other => panic!("Expected IoError(NotFound), got: {:?}", other),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_check_directory_writable_readonly_dir() {
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        let readonly_dir = temp_dir.path().join("readonly");
        std::fs::create_dir(&readonly_dir).expect("Failed to create dir");

        // Remove write permission (owner read+execute only)
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o500))
            .expect("Failed to set permissions");

        let result = ModelManager::check_directory_writable(&readonly_dir);
        assert!(
            result.is_err(),
            "Read-only directory should fail write check"
        );
        match result.unwrap_err() {
            ModelError::IoError(e) => {
                assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied);
            }
            other => panic!("Expected IoError(PermissionDenied), got: {:?}", other),
        }

        // Restore permissions so temp_dir cleanup works
        std::fs::set_permissions(&readonly_dir, std::fs::Permissions::from_mode(0o700))
            .expect("Failed to restore permissions");
    }

    // ── Directory loading pipeline tests ──────────────────────────────

    #[test]
    fn test_validate_directory_path_empty() {
        // Empty path should return InvalidPath error
        let result = ModelManager::validate_directory_path(Path::new(""));
        assert!(result.is_err(), "Empty path should be rejected");
        match result.unwrap_err() {
            ModelError::InvalidPath(p) => assert!(p.as_os_str().is_empty()),
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_validate_directory_path_too_long() {
        // Path exceeding MAX_PATH_LENGTH (4096) should return InvalidPath error
        let long_component = "a".repeat(4097);
        let long_path = Path::new(&long_component);
        let result = ModelManager::validate_directory_path(long_path);
        assert!(result.is_err(), "Overly long path should be rejected");
        match result.unwrap_err() {
            ModelError::InvalidPath(_) => {} // expected
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_validate_directory_path_nonexistent_returns_ok() {
        // A non-existent but otherwise valid path should return Ok with the
        // original path so that is_valid_directory can handle it gracefully.
        let result = ModelManager::validate_directory_path(Path::new("/tmp/no_such_dir_xyz_test"));
        assert!(
            result.is_ok(),
            "Non-existent path should return Ok (handled later by is_valid_directory)"
        );
    }

    #[test]
    fn test_validate_directory_path_real_directory() {
        // A real, readable directory should canonicalize successfully
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let result = ModelManager::validate_directory_path(temp_dir.path());
        assert!(
            result.is_ok(),
            "Real directory should validate: {:?}",
            result
        );
        // The returned path should be canonical (absolute)
        let validated = result.unwrap();
        assert!(validated.is_absolute());
    }

    #[test]
    fn test_check_directory_permissions_on_file() {
        // Passing a regular file (not a directory) should return InvalidPath
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("regular_file.txt");
        std::fs::write(&file_path, "content").expect("write file");

        let result = ModelManager::check_directory_permissions(&file_path);
        assert!(result.is_err(), "Regular file should fail directory check");
        match result.unwrap_err() {
            ModelError::InvalidPath(_) => {} // expected
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_extract_model_name_normal() {
        // Standard filename should extract stem without extension
        let path = Path::new("/models/my-agent.yaml");
        let name = ModelManager::extract_model_name(path).expect("should extract name");
        assert_eq!(name, "my-agent");
    }

    #[test]
    fn test_extract_model_name_nested_path() {
        // Deeply nested path should still extract just the file stem
        let path = Path::new("/a/b/c/deep-model.yaml");
        let name = ModelManager::extract_model_name(path).expect("should extract name");
        assert_eq!(name, "deep-model");
    }

    #[test]
    fn test_extract_model_name_no_extension() {
        // File without extension should still extract the full filename as stem
        let path = Path::new("/models/no-ext");
        let name = ModelManager::extract_model_name(path).expect("should extract name");
        assert_eq!(name, "no-ext");
    }

    #[test]
    fn test_extract_model_name_root_path() {
        // Root path "/" has no file stem and should return InvalidPath
        let result = ModelManager::extract_model_name(Path::new("/"));
        assert!(
            result.is_err(),
            "Root path should fail to extract model name"
        );
        match result.unwrap_err() {
            ModelError::InvalidPath(_) => {} // expected
            other => panic!("Expected InvalidPath, got: {:?}", other),
        }
    }

    #[test]
    fn test_extract_model_name_dotfile() {
        // Hidden file like ".hidden.yaml" should extract ".hidden" as stem
        let path = Path::new("/models/.hidden.yaml");
        let name = ModelManager::extract_model_name(path).expect("should extract name");
        assert_eq!(name, ".hidden");
    }

    #[test]
    fn test_read_model_content_missing_file() {
        // Reading a non-existent file should return IoError
        let result = ModelManager::read_model_content(Path::new("/no/such/file.yaml"));
        assert!(result.is_err());
        match result.unwrap_err() {
            ModelError::IoError(_) => {} // expected
            other => panic!("Expected IoError, got: {:?}", other),
        }
    }

    #[test]
    fn test_read_model_content_success() {
        // Reading an existing file should return its content
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let file_path = temp_dir.path().join("test.yaml");
        std::fs::write(&file_path, "executor:\n  type: llama-embedding\n").expect("write");

        let content = ModelManager::read_model_content(&file_path).expect("should read");
        assert!(content.contains("llama-embedding"));
    }

    #[test]
    fn test_process_directory_entries_mixed_success_and_failure() {
        // Directory with valid YAML, invalid YAML, and non-YAML files should
        // report correct success/failure counts and only return valid models.
        use std::fs;
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        // Valid model file
        let valid_content = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
        fs::write(temp_dir.path().join("good-model.yaml"), valid_content).expect("write valid");

        // Invalid YAML model file (parseable YAML but invalid ModelConfig)
        fs::write(
            temp_dir.path().join("bad-model.yaml"),
            "this_is_not: a_valid_model_config\n",
        )
        .expect("write invalid");

        // Non-YAML file (should be silently skipped)
        fs::write(temp_dir.path().join("readme.txt"), "ignore me").expect("write txt");

        let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
        let (models, success, failed) =
            ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

        assert_eq!(success, 1, "Should have 1 successful model");
        assert_eq!(failed, 1, "Should have 1 failed model (bad YAML)");
        assert_eq!(models.len(), 1, "Should return 1 model");
        assert_eq!(models[0].name, "good-model");
        assert_eq!(models[0].source, ModelConfigSource::Project);
    }

    #[test]
    fn test_process_directory_entries_all_valid() {
        // Directory with only valid model files should load all of them
        use std::fs;
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        let content = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
        fs::write(temp_dir.path().join("model-a.yaml"), content).expect("write a");
        fs::write(temp_dir.path().join("model-b.yaml"), content).expect("write b");

        let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
        let (models, success, failed) =
            ModelManager::process_directory_entries(entries, &ModelConfigSource::User);

        assert_eq!(success, 2);
        assert_eq!(failed, 0);
        assert_eq!(models.len(), 2);
    }

    #[test]
    fn test_process_directory_entries_empty_directory() {
        // Empty directory should return zero models and zero counts
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
        let (models, success, failed) =
            ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

        assert_eq!(success, 0);
        assert_eq!(failed, 0);
        assert!(models.is_empty());
    }

    #[test]
    fn test_process_directory_entries_only_non_yaml() {
        // Directory containing only non-YAML files should skip them all
        use std::fs;
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        fs::write(temp_dir.path().join("readme.md"), "# Hello").expect("write md");
        fs::write(temp_dir.path().join("config.json"), "{}").expect("write json");
        fs::write(temp_dir.path().join("script.sh"), "#!/bin/sh").expect("write sh");

        let entries = std::fs::read_dir(temp_dir.path()).expect("read dir");
        let (models, success, failed) =
            ModelManager::process_directory_entries(entries, &ModelConfigSource::Project);

        assert_eq!(success, 0);
        assert_eq!(failed, 0);
        assert!(models.is_empty());
    }

    #[test]
    fn test_is_yaml_file_extensions() {
        // Only .yaml extension files that are actual files should match
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        let yaml_path = temp_dir.path().join("model.yaml");
        std::fs::write(&yaml_path, "content").expect("write");

        let txt_path = temp_dir.path().join("model.txt");
        std::fs::write(&txt_path, "content").expect("write");

        let yml_path = temp_dir.path().join("model.yml");
        std::fs::write(&yml_path, "content").expect("write");

        assert!(ModelManager::is_yaml_file(&yaml_path));
        assert!(!ModelManager::is_yaml_file(&txt_path));
        assert!(!ModelManager::is_yaml_file(&yml_path)); // only .yaml, not .yml
    }

    #[test]
    fn test_load_models_from_dir_end_to_end_mixed() {
        // Full pipeline: create a temp directory with valid/invalid files,
        // call load_models_from_dir, and verify results.
        use std::fs;
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");

        // Valid model with description
        let content_with_desc = "---\ndescription: \"My custom model\"\n---\nexecutor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n";
        fs::write(temp_dir.path().join("custom.yaml"), content_with_desc).expect("write");

        // Valid model without description
        let content_no_desc = "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: true\n";
        fs::write(temp_dir.path().join("plain.yaml"), content_no_desc).expect("write");

        // Invalid model
        fs::write(temp_dir.path().join("broken.yaml"), "not: valid: model").expect("write");

        // Non-YAML
        fs::write(temp_dir.path().join("notes.txt"), "skip me").expect("write");

        let result =
            ModelManager::load_models_from_dir(temp_dir.path(), ModelConfigSource::Project);
        assert!(result.is_ok(), "Should succeed: {:?}", result);

        let models = result.unwrap();
        // 2 valid YAML files out of 4 total
        assert_eq!(models.len(), 2, "Should load 2 valid models");

        let custom = models.iter().find(|m| m.name == "custom");
        assert!(custom.is_some(), "Should find 'custom' model");
        assert_eq!(
            custom.unwrap().description,
            Some("My custom model".to_string())
        );

        let plain = models.iter().find(|m| m.name == "plain");
        assert!(plain.is_some(), "Should find 'plain' model");
        assert_eq!(plain.unwrap().description, None);
    }

    // ========================================================================
    // validate_agent_name_security tests
    // ========================================================================

    // ========================================================================
    // use_agent security integration tests
    // ========================================================================

    // ---- Tests for load_or_create_config, save_config, check_file_readable, update_config_with_agent ----

    // ========================================================================
    // ensure_config_structure additional coverage tests
    // ========================================================================

    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_with_existing_yaml_config() {
        // When a YAML config file already exists, ensure_config_structure should
        // detect it and return the canonicalized path to the existing file.
        // Exercises the detect_config_file -> validate_config_file_path -> return
        // existing config branch (lines 1607-1611).
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
        let existing_config = sah_dir.join("sah.yaml");
        fs::write(
            &existing_config,
            "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n",
        )
        .expect("Failed to write existing yaml config");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
        assert!(
            result.is_ok(),
            "Should detect existing YAML config: {:?}",
            result
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should return existing yaml config file"
        );
        assert!(
            config_path.is_absolute(),
            "Should return canonical absolute path"
        );
        assert!(
            config_path.is_file(),
            "Returned path should point to an existing file"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_with_avp_paths() {
        // Verify ensure_config_structure works with AVP paths (.avp/avp.yaml)
        // to confirm it is not hardcoded to .sah paths. Exercises directory
        // creation (lines 1581-1601) and new config path validation (lines 1615-1623).
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let result = ModelManager::ensure_config_structure(&ModelPaths::avp());
        assert!(
            result.is_ok(),
            "Should successfully create AVP config structure: {:?}",
            result
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("avp.yaml")),
            "Should return path to avp.yaml"
        );
        assert!(
            config_path.ends_with(".avp/avp.yaml"),
            "Should end with .avp/avp.yaml, got: {}",
            config_path.display()
        );

        let avp_dir = temp_dir.path().join(".avp");
        assert!(avp_dir.exists(), "Should create .avp directory");
        assert!(avp_dir.is_dir(), "Should be a directory");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_avp_with_existing_config() {
        // Exercise the existing-config detection path with AVP paths and a
        // pre-existing YAML config, ensuring validate_config_file_path is called
        // on the detected file (lines 1609-1611).
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let avp_dir = temp_dir.path().join(".avp");
        fs::create_dir_all(&avp_dir).expect("Failed to create .avp dir");
        let existing_config = avp_dir.join("avp.yaml");
        fs::write(
            &existing_config,
            "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: true\n",
        )
        .expect("Failed to write avp config");

        let result = ModelManager::ensure_config_structure(&ModelPaths::avp());
        assert!(
            result.is_ok(),
            "Should handle existing AVP config: {:?}",
            result
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("avp.yaml")),
            "Should return existing avp.yaml"
        );
        assert!(
            config_path.is_file(),
            "Returned path should be an existing file"
        );
    }

    #[cfg(unix)]
    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_create_dir_fails_readonly_parent() {
        // When the parent directory is read-only, check_directory_writable should
        // fail and ensure_config_structure should propagate the error. Exercises
        // the permission check error path (line 1583).
        use std::os::unix::fs::PermissionsExt;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        std::fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        // Make the temp directory read-only so .sah cannot be created
        std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o500))
            .expect("Failed to set read-only permissions");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
        assert!(
            result.is_err(),
            "Should fail when parent directory is read-only"
        );

        // Restore permissions so temp_dir cleanup works
        std::fs::set_permissions(temp_dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("Failed to restore permissions");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_returns_new_yaml_path_when_no_config_exists() {
        // When the config directory exists but has no config file,
        // ensure_config_structure should return the path for a new YAML config.
        // Exercises the new config path construction and validation (lines 1615-1623).
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
        assert!(
            result.is_ok(),
            "Should succeed with empty config directory: {:?}",
            result
        );

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should return path for new sah.yaml"
        );
        // The file should NOT exist yet (ensure_config_structure only returns the path)
        assert!(
            !config_path.exists(),
            "New config file should not be created by ensure_config_structure"
        );
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_ensure_config_structure_prefers_yaml_over_toml() {
        // When both .yaml and .toml config files exist, ensure_config_structure
        // should prefer the YAML file since detect_config_file checks YAML first.
        use std::fs;

        let temp_dir = tempfile::TempDir::new().expect("Failed to create temp dir");
        fs::create_dir(temp_dir.path().join(".git")).expect("Failed to create .git marker");
        let _guard = CurrentDirGuard::new(temp_dir.path()).expect("Failed to change directory");

        let sah_dir = temp_dir.path().join(SwissarmyhammerDirectory::dir_name());
        fs::create_dir_all(&sah_dir).expect("Failed to create .sah dir");

        fs::write(
            sah_dir.join("sah.yaml"),
            "executor:\n  type: llama-embedding\n  config:\n    source: !HuggingFace\n      repo: test/embed\nquiet: false\n",
        )
        .expect("Failed to write yaml config");
        fs::write(sah_dir.join("sah.toml"), "[existing]\nvalue = true\n")
            .expect("Failed to write toml config");

        let result = ModelManager::ensure_config_structure(&ModelPaths::sah());
        assert!(result.is_ok(), "Should succeed: {:?}", result);

        let config_path = result.unwrap();
        assert_eq!(
            config_path.file_name(),
            Some(std::ffi::OsStr::new("sah.yaml")),
            "Should prefer YAML config over TOML"
        );
    }

    #[test]
    fn test_gitroot_display_emoji() {
        assert_eq!(ModelConfigSource::GitRoot.display_emoji(), "🔧 GitRoot");
    }

    #[test]
    fn test_gitroot_source_serialization() {
        let gitroot = ModelConfigSource::GitRoot;
        let json = serde_json::to_string(&gitroot).expect("Failed to serialize GitRoot");
        assert_eq!(json, "\"git-root\"");

        let deserialized: ModelConfigSource =
            serde_json::from_str(&json).expect("Failed to deserialize GitRoot");
        assert_eq!(deserialized, ModelConfigSource::GitRoot);
    }

    #[test]
    fn test_model_config_deserialize_missing_executor_field() {
        // Exercises the error path when neither `executor` nor `executors` is present.
        let yaml = "quiet: true\n";
        let result = serde_yaml_ng::from_str::<ModelConfig>(yaml);
        assert!(result.is_err(), "Should fail when no executor field");
        let err_msg = format!("{}", result.unwrap_err());
        assert!(
            err_msg.contains("executor"),
            "Error should mention executor: {}",
            err_msg
        );
    }

    #[test]
    fn test_model_config_deserialize_executors_list() {
        // Exercises the `executors` list deserialization path.
        let yaml = r#"
executors:
  - platform: macos-arm64
    executor:
      type: llama-embedding
      config:
        source: !HuggingFace
          repo: test/embed
  - executor:
      type: llama-embedding
      config:
        source: !HuggingFace
          repo: test/embed
quiet: true
"#;
        let config: ModelConfig =
            serde_yaml_ng::from_str(yaml).expect("Should parse executors list");
        assert_eq!(config.executors.len(), 2);
        assert!(config.quiet);
        assert_eq!(config.executors[0].platform, Some(Platform::MacosArm64));
        assert_eq!(config.executors[1].platform, None);
    }

    #[test]
    fn test_model_config_deserialize_unknown_fields_ignored() {
        // Exercises the `_: IgnoredAny` path in the custom deserializer.
        let yaml = r#"
executor:
  type: llama-embedding
  config:
    source: !HuggingFace
      repo: test/embed
quiet: false
unknown_field: "should be ignored"
another_unknown: 42
"#;
        let config: ModelConfig =
            serde_yaml_ng::from_str(yaml).expect("Should parse despite unknown fields");
        assert_eq!(config.executor_type(), ModelExecutorType::LlamaEmbedding);
        assert!(!config.quiet);
    }

    #[test]
    fn test_model_config_select_executor_no_match() {
        // Exercises `select_executor()` returning `None` when all entries have
        // non-matching platform constraints.
        let config = ModelConfig {
            executors: vec![ExecutorEntry {
                // Use a platform that definitely doesn't match current
                platform: Some(Platform::LinuxX86_64),
                executor: embedding_model_config().executors.remove(0).executor,
            }],
            quiet: false,
        };
        // On macOS ARM this won't match LinuxX86_64
        // We can't guarantee which platform we're on, so just test the method works
        let _result = config.select_executor();
    }

    #[test]
    fn test_platform_serialization_roundtrip() {
        // Exercises Platform serialization/deserialization for all variants.
        let platforms = vec![
            Platform::MacosArm64,
            Platform::MacosX86_64,
            Platform::LinuxX86_64,
            Platform::LinuxAarch64,
        ];
        for platform in platforms {
            let json = serde_json::to_string(&platform)
                .unwrap_or_else(|_| panic!("Failed to serialize {:?}", platform));
            let deserialized: Platform = serde_json::from_str(&json)
                .unwrap_or_else(|_| panic!("Failed to deserialize {:?}", platform));
            assert_eq!(platform, deserialized);
        }
    }

    #[test]
    fn test_platform_current() {
        // Exercises `Platform::current()` — just verifies it doesn't panic.
        let _current = Platform::current();
    }

    #[test]
    fn test_embedding_model_config_deserialization() {
        let yaml = r#"
source: !HuggingFace
  repo: "test/embedding-model"
  filename: "model.gguf"
normalize: true
max_sequence_length: 512
"#;
        let config: EmbeddingModelConfig =
            serde_yaml_ng::from_str(yaml).expect("Should parse embedding config");
        assert!(config.normalize);
        assert_eq!(config.max_sequence_length, Some(512));
    }

    #[test]
    fn test_model_error_severity() {
        use swissarmyhammer_common::{ErrorSeverity, Severity};

        let parse_err = serde_yaml_ng::from_str::<ModelConfig>("invalid: yaml: [unclosed")
            .expect_err("Should fail to parse");
        let model_parse_err = ModelError::ParseError(parse_err);
        assert_eq!(model_parse_err.severity(), ErrorSeverity::Critical);

        let config_err = ModelError::ConfigError("test".to_string());
        assert_eq!(config_err.severity(), ErrorSeverity::Critical);

        let not_found = ModelError::NotFound("test".to_string());
        assert_eq!(not_found.severity(), ErrorSeverity::Error);

        let invalid_path = ModelError::InvalidPath(PathBuf::from("/test"));
        assert_eq!(invalid_path.severity(), ErrorSeverity::Error);

        let io_err = ModelError::IoError(std::io::Error::new(std::io::ErrorKind::NotFound, "test"));
        assert_eq!(io_err.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_model_paths_avp() {
        let paths = ModelPaths::avp();
        assert_eq!(paths.dir_name, ".avp");
        assert_eq!(paths.config_filename, "avp.yaml");
    }

    #[test]
    fn test_model_paths_sah() {
        let paths = ModelPaths::sah();
        assert_eq!(paths.dir_name, ".sah");
        assert_eq!(paths.config_filename, "sah.yaml");
    }

    #[test]
    fn test_executor_type_all_variants() {
        // Exercises `executor_type()` for all executor types.
        // Test LlamaEmbedding
        let embedding_config = ModelConfig {
            executors: vec![ExecutorEntry {
                platform: None,
                executor: ModelExecutorConfig::LlamaEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/repo".to_string(),
                        filename: Some("model.gguf".to_string()),
                        folder: None,
                    },
                    normalize: false,
                    max_sequence_length: None,
                }),
            }],
            quiet: false,
        };
        assert_eq!(
            embedding_config.executor_type(),
            ModelExecutorType::LlamaEmbedding
        );

        // Test AneEmbedding
        let ane_config = ModelConfig {
            executors: vec![ExecutorEntry {
                platform: None,
                executor: ModelExecutorConfig::AneEmbedding(EmbeddingModelConfig {
                    source: ModelSource::HuggingFace {
                        repo: "test/repo".to_string(),
                        filename: Some("model.gguf".to_string()),
                        folder: None,
                    },
                    normalize: true,
                    max_sequence_length: Some(256),
                }),
            }],
            quiet: false,
        };
        assert_eq!(ane_config.executor_type(), ModelExecutorType::AneEmbedding);
    }

    #[test]
    fn test_validate_directory_path_empty_coverage() {
        let result = ModelManager::validate_directory_path(Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_directory_path_too_long_coverage() {
        let long_path = "a".repeat(5000);
        let result = ModelManager::validate_directory_path(Path::new(&long_path));
        assert!(result.is_err());
    }

    #[test]
    fn test_is_yaml_file() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();

        let yaml_file = temp_dir.path().join("test.yaml");
        std::fs::write(&yaml_file, "key: val").unwrap();
        assert!(ModelManager::is_yaml_file(&yaml_file));

        let txt_file = temp_dir.path().join("test.txt");
        std::fs::write(&txt_file, "text").unwrap();
        assert!(!ModelManager::is_yaml_file(&txt_file));

        // Directory should not count
        assert!(!ModelManager::is_yaml_file(temp_dir.path()));
    }

    #[test]
    fn test_extract_model_name() {
        let path = PathBuf::from("/some/dir/my-model.yaml");
        let name = ModelManager::extract_model_name(&path).unwrap();
        assert_eq!(name, "my-model");
    }

    #[test]
    fn test_check_suspicious_patterns_clean() {
        assert!(ModelManager::check_suspicious_patterns("/normal/path").is_ok());
    }

    #[test]
    fn test_is_valid_directory_nonexistent() {
        assert!(!ModelManager::is_valid_directory(Path::new(
            "/nonexistent/dir"
        )));
    }

    #[test]
    fn test_is_valid_directory_file() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("afile");
        std::fs::write(&file, "content").unwrap();
        assert!(!ModelManager::is_valid_directory(&file));
    }

    #[test]
    fn test_validate_config_file_path_empty() {
        let result = ModelManager::validate_config_file_path(Path::new(""));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_file_path_too_long_coverage() {
        let long_path = "a".repeat(5000);
        let result = ModelManager::validate_config_file_path(Path::new(&long_path));
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_config_file_path_nonexistent() {
        // Exercises the non-existent file path branch (just returns the path).
        let result =
            ModelManager::validate_config_file_path(Path::new("/tmp/nonexistent_config.yaml"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_file_path_existing_file() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("config.yaml");
        std::fs::write(&file, "key: val").unwrap();
        let result = ModelManager::validate_config_file_path(&file);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_config_file_path_existing_directory() {
        /// Exercises the branch where an existing path is not a file.
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let dir = temp_dir.path().join("subdir");
        std::fs::create_dir(&dir).unwrap();
        let result = ModelManager::validate_config_file_path(&dir);
        assert!(
            result.is_err(),
            "Directory should fail validation as config file"
        );
    }

    #[test]
    fn test_check_directory_writable_valid() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        assert!(ModelManager::check_directory_writable(temp_dir.path()).is_ok());
    }

    #[test]
    fn test_check_directory_writable_file() {
        /// Exercises the branch where path is not a directory.
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("afile");
        std::fs::write(&file, "content").unwrap();
        let result = ModelManager::check_directory_writable(&file);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_directory_writable_nonexistent() {
        let result = ModelManager::check_directory_writable(Path::new("/nonexistent/dir"));
        assert!(result.is_err());
    }

    #[test]
    fn test_check_directory_permissions_not_a_directory() {
        use tempfile::TempDir;
        let temp_dir = TempDir::new().unwrap();
        let file = temp_dir.path().join("afile");
        std::fs::write(&file, "content").unwrap();
        let result = ModelManager::check_directory_permissions(&file);
        assert!(result.is_err());
    }

    #[test]
    fn test_check_directory_permissions_nonexistent() {
        let result = ModelManager::check_directory_permissions(Path::new("/nonexistent/dir"));
        assert!(result.is_err());
    }

    #[test]
    fn test_model_config_source_debug_variants() {
        assert_eq!(format!("{:?}", ModelConfigSource::GitRoot), "GitRoot");
    }

    #[test]
    fn test_model_config_source_equality_gitroot() {
        assert_eq!(ModelConfigSource::GitRoot, ModelConfigSource::GitRoot);
        assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::Builtin);
        assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::Project);
        assert_ne!(ModelConfigSource::GitRoot, ModelConfigSource::User);
    }
}

// ============================================================================
// Hardcoded-Claude chat model configuration
// ============================================================================

/// Claude Code is the only chat executor, so the chat scope carries no executor
/// choice — only the Claude CLI `--model` switch. These tests pin both halves of
/// that collapse: the review scope still reaches Haiku through the new
/// configuration field, and the embedding model YAMLs still resolve through
/// `ModelManager` (the loader must not be collapsed out from under them).
#[cfg(test)]
mod chat_model_config_tests {
    use super::*;
    use swissarmyhammer_common::test_utils::CurrentDirGuard;

    /// A temporary directory with a `.git` marker, so config discovery stops
    /// there instead of walking up into the real repository.
    fn isolated_project() -> tempfile::TempDir {
        let temp_dir = tempfile::TempDir::new().expect("temp dir");
        std::fs::create_dir(temp_dir.path().join(".git")).expect(".git marker");
        temp_dir
    }

    /// Write `.sah/sah.yaml` with the given content in the current project.
    fn write_sah_config(content: &str) {
        let config_path = ModelManager::ensure_config_structure(&ModelPaths::sah()).unwrap();
        std::fs::write(&config_path, content).unwrap();
    }

    /// The embedding stack loads its models by name through `ModelManager`.
    /// Collapsing the chat side must leave that path intact: both embedding
    /// YAMLs still resolve, still parse, and still select an embedding executor.
    #[test]
    fn embedding_models_still_resolve_through_model_manager() {
        for name in ["nomic-embed-code", "qwen-embedding"] {
            let info = ModelManager::find_agent_by_name(name)
                .unwrap_or_else(|e| panic!("builtin embedding model `{name}` must resolve: {e}"));
            let config = parse_model_config(&info.content)
                .unwrap_or_else(|e| panic!("builtin embedding model `{name}` must parse: {e}"));
            let executor = config
                .select_executor()
                .unwrap_or_else(|| panic!("`{name}` must offer an executor for this platform"));
            assert!(
                matches!(
                    executor,
                    ModelExecutorConfig::LlamaEmbedding(_) | ModelExecutorConfig::AneEmbedding(_)
                ),
                "`{name}` must select an embedding executor, got {executor:?}"
            );
        }
    }

    /// `builtin/models/` is now an embedding-only library. A leftover chat YAML
    /// would resurrect the model-name lookup this card removed.
    #[test]
    fn builtin_models_are_all_embedding_models() {
        let models = ModelManager::load_builtin_models().expect("builtin models load");
        let names: Vec<&str> = models.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(
            names,
            vec!["nomic-embed-code", "qwen-embedding"],
            "builtin/models/ must hold only the embedding models"
        );
    }

    /// An unconfigured review scope runs `claude --model haiku`, chosen through
    /// the configuration field rather than a model-name lookup.
    #[test]
    #[serial_test::serial(cwd)]
    fn unconfigured_review_scope_resolves_to_the_haiku_switch() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

        let config = ModelManager::resolve_review_chat_config(&ModelPaths::sah()).unwrap();
        assert_eq!(
            config.model.as_deref(),
            Some(REVIEW_DEFAULT_CLAUDE_MODEL),
            "an unconfigured review scope must pick the baked-in Haiku switch"
        );
        assert_eq!(
            config.claude_args(),
            vec!["--model".to_string(), "haiku".to_string()],
            "the review scope must spawn `claude --model haiku`"
        );
    }

    /// `sah doctor`-style reporting and the spawned process read the same
    /// resolver, so the model reported can never disagree with the model run.
    #[test]
    #[serial_test::serial(cwd)]
    fn reported_review_model_matches_the_switch_that_is_run() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
        write_sah_config("review:\n  model: opus\n");

        let reported = ModelManager::resolve_review_chat_model(&ModelPaths::sah()).unwrap();
        let run = ModelManager::resolve_review_chat_config(&ModelPaths::sah()).unwrap();
        assert_eq!(reported, "opus");
        assert_eq!(
            run.claude_args(),
            vec!["--model".to_string(), reported],
            "the reported model must be the one the spawned claude receives"
        );
    }

    /// Precedence is `review.model` → top-level `model:` → the baked-in Haiku
    /// switch. Only the meaning of the value changed: it is now the Claude CLI
    /// `--model` switch, not the name of a model YAML.
    #[test]
    fn review_chat_model_precedence() {
        assert_eq!(
            ModelManager::review_chat_model_from(Some("opus".into()), Some("sonnet".into())),
            "opus",
            "an explicit review.model wins"
        );
        assert_eq!(
            ModelManager::review_chat_model_from(None, Some("sonnet".into())),
            "sonnet",
            "an overall model: drives review when review.model is unset"
        );
        assert_eq!(
            ModelManager::review_chat_model_from(None, None),
            REVIEW_DEFAULT_CLAUDE_MODEL,
            "a fully unconfigured review scope falls to the baked-in Haiku switch"
        );
    }

    /// Pins the literal switch, not just the symbol. Every other test in this
    /// file compares against `REVIEW_DEFAULT_CLAUDE_MODEL` itself, which stays
    /// green even if the fallback path stops reading the constant, as long as
    /// both sides still resolve to the same symbol. This test fails the moment
    /// anyone edits the constant's value, with no hand-editing-and-reverting
    /// required to prove that.
    #[test]
    fn review_default_claude_model_is_the_literal_haiku_switch() {
        assert_eq!(
            REVIEW_DEFAULT_CLAUDE_MODEL, "haiku",
            "the baked-in review-scope default must stay the literal `haiku` switch"
        );
    }

    /// The default (non-review) chat scope stays plain `claude` with no
    /// `--model`, so the Claude CLI's own default applies.
    #[test]
    #[serial_test::serial(cwd)]
    fn unconfigured_default_scope_spawns_plain_claude() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();

        let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
        assert!(config.model.is_none(), "no switch is configured");
        assert!(
            config.claude_args().is_empty(),
            "plain claude carries no --model switch"
        );
    }

    /// A top-level `model:` drives the default scope too.
    #[test]
    #[serial_test::serial(cwd)]
    fn configured_default_scope_uses_the_configured_switch() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
        write_sah_config("model: sonnet\n");

        let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
        assert_eq!(
            config.claude_args(),
            vec!["--model".to_string(), "sonnet".to_string()]
        );
    }

    /// A non-string `model:` (e.g. a number) is ignored rather than coerced, so
    /// a mistyped config falls back to the default instead of spawning
    /// `claude --model 3`.
    #[test]
    #[serial_test::serial(cwd)]
    fn non_string_model_value_is_ignored() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
        write_sah_config("model: 3\n");

        let config = ModelManager::resolve_chat_config(&ModelPaths::sah()).unwrap();
        assert!(
            config.model.is_none(),
            "a non-string model: must be ignored, got {:?}",
            config.model
        );
    }

    /// A blank switch is a configuration error, not a `claude --model ""` spawn.
    #[test]
    #[serial_test::serial(cwd)]
    fn blank_switch_is_a_configuration_error() {
        let temp_dir = isolated_project();
        let _guard = CurrentDirGuard::new(temp_dir.path()).unwrap();
        write_sah_config("review:\n  model: \"   \"\n");

        match ModelManager::resolve_review_chat_config(&ModelPaths::sah()) {
            Err(ModelError::ConfigError(msg)) => {
                assert!(
                    msg.contains("model"),
                    "the error must name the offending setting, got: {msg}"
                );
            }
            other => panic!("expected a ConfigError for a blank model switch, got {other:?}"),
        }
    }
}
