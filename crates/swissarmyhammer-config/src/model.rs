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
//! println!("Executor: {:?}", config.executor_type()?);
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
use swissarmyhammer_common::frontmatter::split_frontmatter_body;
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
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// An explicitly configured `review.model` — the review scope's own Claude CLI
/// `--model` switch, or `None` when the key is unset.
///
/// A newtype rather than a bare `Option<String>` so it cannot be swapped with
/// [`DefaultModel`], which has the same shape and the opposite precedence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct ReviewModel(pub Option<String>);

/// Wraps a raw `review.model` value read from configuration.
impl From<Option<String>> for ReviewModel {
    fn from(model: Option<String>) -> Self {
        Self(model)
    }
}

/// An explicitly configured top-level `model:` — the overall Claude CLI
/// `--model` switch, or `None` when the key is unset.
///
/// The counterpart to [`ReviewModel`]; see that type for why both are newtypes.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct DefaultModel(pub Option<String>);

/// Wraps a raw top-level `model:` value read from configuration.
impl From<Option<String>> for DefaultModel {
    fn from(model: Option<String>) -> Self {
        Self(model)
    }
}

/// Runtime platform for executor selection
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    /// macOS ARM64 (Apple Silicon) platform.
    MacosArm64,
    /// macOS x86-64 (Intel) platform.
    ///
    /// The wire name is pinned because `kebab-case` renaming derives it from
    /// the variant spelling, and user model YAML already carries the value.
    #[serde(rename = "macos-x86-64")]
    MacosX8664,
    /// Linux x86-64 platform.
    ///
    /// The wire name is pinned for the same reason as [`Platform::MacosX8664`].
    #[serde(rename = "linux-x86-64")]
    LinuxX8664,
    /// Linux ARM64 (aarch64) platform.
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
            Platform::MacosX8664
        }
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        {
            Platform::LinuxX8664
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
            Platform::LinuxX8664
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ModelConfig {
    /// Ordered list of executor entries; first compatible match wins.
    pub executors: Vec<ExecutorEntry>,
    /// Global quiet mode
    pub quiet: bool,
}

/// Accepts both the singular `executor:` field and the plural `executors:`
/// list, normalizing either into the [`ModelConfig::executors`] list, and
/// ignores unknown keys.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum ModelExecutorConfig {
    /// Run the embedding model locally through llama.cpp.
    #[serde(rename = "llama-embedding")]
    LlamaEmbedding(EmbeddingModelConfig),
    /// Run the embedding model on the Apple Neural Engine.
    #[serde(rename = "ane-embedding")]
    AneEmbedding(EmbeddingModelConfig),
}

/// Configuration for embedding model execution
///
/// Used with the `llama-embedding` executor type for semantic embedding models.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelSource {
    /// HuggingFace model source with repository and optional filename.
    HuggingFace {
        /// Repository identifier (e.g., 'owner/repo' on HuggingFace).
        repo: String,
        /// Optional filename within the repository.
        filename: Option<String>,
        /// Optional folder path within the repository.
        #[serde(skip_serializing_if = "Option::is_none")]
        folder: Option<String>,
    },
    /// Local filesystem model source.
    Local {
        /// Path to the model file on the local filesystem.
        filename: PathBuf,
        /// Optional folder path prefix for the model.
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
    /// Returns `Err(ModelError::ConfigError)` if no executor matches — see
    /// `select_executor()` for the `Option`-returning alternative.
    pub fn executor(&self) -> Result<&ModelExecutorConfig, ModelError> {
        self.select_executor().ok_or_else(|| {
            ModelError::ConfigError("no compatible executor for current platform".to_string())
        })
    }

    /// Get the executor type from the configuration
    pub fn executor_type(&self) -> Result<ModelExecutorType, ModelError> {
        match self.executor()? {
            ModelExecutorConfig::LlamaEmbedding(_) => Ok(ModelExecutorType::LlamaEmbedding),
            ModelExecutorConfig::AneEmbedding(_) => Ok(ModelExecutorType::AneEmbedding),
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
    #[error("model '{0}' not found")]
    NotFound(String),
    /// Invalid file path for model configuration
    #[error("invalid model path: {0}")]
    InvalidPath(PathBuf),
    /// IO error during file operations
    #[error("io error: {0}")]
    IoError(#[from] std::io::Error),
    /// Configuration parsing error
    #[error("parse error: {0}")]
    ParseError(#[from] serde_yaml_ng::Error),
    /// Configuration validation error
    #[error("configuration error: {0}")]
    ConfigError(String),
}

/// A failure to parse or validate model configuration is `Critical` — nothing
/// downstream can trust the configuration. A failed lookup or file operation is
/// `Error`: the caller can fall back to another source and continue.
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
///
/// Delegates the split to [`split_frontmatter_body`], so only a line that is
/// exactly three hyphens delimits: a `---` run embedded in a value stays part
/// of the frontmatter instead of ending it early.
fn extract_yaml_frontmatter_field(content: &str, field: &str) -> Option<String> {
    let (front_matter, _body) = split_frontmatter_body(content)?;

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

    // Check for YAML front matter. Delegates the split to
    // `split_frontmatter_body`, so only a line that is exactly three hyphens
    // delimits: a `---` run embedded in a frontmatter value stays part of the
    // frontmatter instead of ending it -- and cutting the config body -- early.
    if let Some((_front_matter, body)) = split_frontmatter_body(content) {
        return serde_yaml_ng::from_str::<ModelConfig>(body.trim());
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
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
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

    /// Owner read permission bit, passed as `required_mode` to
    /// [`Self::check_directory_access`] when checking readability.
    const READABLE_MODE_BIT: u32 = 0o400;

    /// Owner write permission bit, passed as `required_mode` to
    /// [`Self::check_directory_access`] when checking writability.
    const WRITABLE_MODE_BIT: u32 = 0o200;

    /// Check that `path` is a directory with `required_mode` permission bits
    /// set for the owner (e.g. `Self::READABLE_MODE_BIT` for read,
    /// `Self::WRITABLE_MODE_BIT` for write).
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
        Self::check_directory_access(path, Self::READABLE_MODE_BIT, "readable")
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
        Self::load_models_from(|| Ok(dirs::home_dir()), ".models", ModelConfigSource::User)
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
        Self::load_models_from(
            || {
                std::env::current_dir()
                    .map(Some)
                    .map_err(ModelError::IoError)
            },
            "models",
            ModelConfigSource::Project,
        )
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

        Self::load_models_from(
            || Ok(find_git_repository_root()),
            "models",
            ModelConfigSource::GitRoot,
        )
    }

    /// Load models from a root directory obtained via `root_provider`, joined
    /// with `segment`, tagged with `source`.
    ///
    /// `root_provider` returns `Ok(None)` when the root simply doesn't exist
    /// (e.g. no home directory, not in a git repository) — that case yields
    /// an empty vector rather than an error. `Err` propagates as-is.
    fn load_models_from<F>(
        root_provider: F,
        segment: &str,
        source: ModelConfigSource,
    ) -> Result<Vec<ModelInfo>, ModelError>
    where
        F: FnOnce() -> Result<Option<PathBuf>, ModelError>,
    {
        match root_provider()? {
            Some(root) => Self::load_models_from_dir(&root.join(segment), source),
            None => Ok(Vec::new()),
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
        Self::check_directory_access(path, Self::WRITABLE_MODE_BIT, "writable")
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
        review_model: ReviewModel,
        default_model: DefaultModel,
    ) -> String {
        review_model
            .0
            .or(default_model.0)
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
            ReviewModel(Self::get_review_chat_model(paths)?),
            DefaultModel(Self::get_chat_model(paths)?),
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
mod tests;

// ============================================================================
// Hardcoded-Claude chat model configuration
// ============================================================================

/// Claude Code is the only chat executor, so the chat scope carries no executor
/// choice — only the Claude CLI `--model` switch. These tests pin both halves of
/// that collapse: the review scope still reaches Haiku through the new
/// configuration field, and the embedding model YAMLs still resolve through
/// `ModelManager` (the loader must not be collapsed out from under them).
#[cfg(test)]
mod chat_model_config_tests;
