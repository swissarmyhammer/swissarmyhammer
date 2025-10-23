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
//! use swissarmyhammer_config::agent::AgentManager;
//!
//! // List all available agents
//! let agents = AgentManager::list_agents()?;
//! for agent in agents {
//!     println!("{}: {:?} - {}",
//!         agent.name,
//!         agent.source,
//!         agent.description.unwrap_or_default()
//!     );
//! }
//!
//! // Find specific agent
//! let claude_agent = AgentManager::find_agent_by_name("claude-code")?;
//! println!("Found: {}", claude_agent.name);
//!
//! // Apply agent to project
//! AgentManager::use_agent("claude-code")?;
//! # Ok::<(), swissarmyhammer_config::agent::AgentError>(())
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
//! use swissarmyhammer_config::agent::{parse_agent_config, parse_agent_description};
//!
//! let agent_content = std::fs::read_to_string("./agents/my-agent.yaml")?;
//!
//! // Extract description
//! let description = parse_agent_description(&agent_content);
//! println!("Description: {:?}", description);
//!
//! // Parse configuration
//! let config = parse_agent_config(&agent_content)?;
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
//! use swissarmyhammer_config::agent::{AgentManager, AgentError};
//!
//! match AgentManager::find_agent_by_name("nonexistent") {
//!     Ok(agent) => println!("Found: {}", agent.name),
//!     Err(AgentError::NotFound(name)) => {
//!         eprintln!("Agent '{}' not found", name);
//!         // Show available agents as suggestion
//!         let agents = AgentManager::list_agents()?;
//!         eprintln!("Available agents:");
//!         for agent in agents {
//!             eprintln!("  - {}", agent.name);
//!         }
//!     },
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! # Ok::<(), AgentError>(())
//! ```

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use swissarmyhammer_common::{ErrorSeverity, Severity};
use thiserror::Error;

/// Agent executor type enumeration
///
/// Defines the available agent executor types with system default being Claude Code
/// for maximum compatibility.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "kebab-case")]
pub enum AgentExecutorType {
    /// Shell out to Claude Code CLI (system default)
    #[default]
    ClaudeCode,
    /// Use local LlamaAgent with in-process execution
    LlamaAgent,
}

/// Complete agent configuration with executor-specific settings
///
/// Combines executor configuration with global agent settings like quiet mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    /// Agent executor configuration with associated data
    pub executor: AgentExecutorConfig,
    /// Global quiet mode
    pub quiet: bool,
}

/// Tagged union of agent executor configurations
///
/// Uses serde's tagged representation to ensure type safety and proper
/// serialization of executor-specific configuration data.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "config")]
pub enum AgentExecutorConfig {
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
    pub model: ModelConfig,
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

fn default_repetition_enabled() -> bool {
    true
}

fn default_repetition_penalty() -> f64 {
    1.1
}

fn default_repetition_threshold() -> usize {
    50
}

fn default_repetition_window() -> usize {
    64
}

/// Model configuration for LlamaAgent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
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

fn default_batch_size() -> u32 {
    512
}

fn default_use_hf_params() -> bool {
    true
}

/// Model source specification
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for AgentConfig {
    fn default() -> Self {
        // System default is always Claude Code
        Self {
            executor: AgentExecutorConfig::ClaudeCode(ClaudeCodeConfig::default()),
            quiet: false,
        }
    }
}

impl Default for ModelConfig {
    fn default() -> Self {
        Self {
            source: ModelSource::HuggingFace {
                repo: "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF".to_string(),
                filename: Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string()),
                folder: None,
            },
            batch_size: default_batch_size(),
            use_hf_params: default_use_hf_params(),
            debug: false,
        }
    }
}

impl Default for McpServerConfig {
    fn default() -> Self {
        Self {
            port: 0,              // Random available port
            timeout_seconds: 900, // 15 minutes
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

impl AgentConfig {
    /// Get the executor type from the configuration
    pub fn executor_type(&self) -> AgentExecutorType {
        match &self.executor {
            AgentExecutorConfig::ClaudeCode(_) => AgentExecutorType::ClaudeCode,
            AgentExecutorConfig::LlamaAgent(_) => AgentExecutorType::LlamaAgent,
        }
    }

    /// Create configuration for Claude Code execution
    pub fn claude_code() -> Self {
        Self {
            executor: AgentExecutorConfig::ClaudeCode(ClaudeCodeConfig::default()),
            quiet: false,
        }
    }

    /// Create configuration for LlamaAgent execution
    pub fn llama_agent(config: LlamaAgentConfig) -> Self {
        Self {
            executor: AgentExecutorConfig::LlamaAgent(config),
            quiet: false,
        }
    }
}

impl LlamaAgentConfig {
    /// Configuration optimized for testing with small, fast models
    ///
    /// Uses the 1.5B Qwen3-Coder model with Q4_K_M quantization which provides:
    /// - Fast test execution (small model size)
    /// - Good tool calling capabilities
    /// - Reasonable output quality for testing
    /// - Balanced settings to avoid repetition issues
    pub fn for_testing() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: crate::DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
                    filename: Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                    folder: None,
                },
                batch_size: 256, // Good balance for test throughput
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(), // Use default settings

            repetition_detection: RepetitionDetectionConfig {
                enabled: true,
                repetition_penalty: 1.05,  // Lower penalty for small models
                repetition_threshold: 150, // Higher threshold to be more permissive
                repetition_window: 128,    // Larger window for better context
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
/// Defines where an agent configuration originates from, used for
/// precedence resolution in the agent discovery hierarchy.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentSource {
    /// Built-in agents compiled into the binary
    Builtin,
    /// Project-specific agents from agents/ directory
    Project,
    /// User-defined agents from .swissarmyhammer/agents/
    User,
}

impl AgentSource {
    /// Get emoji-based display string for the agent source
    ///
    /// - 📦 Built-in: System-provided built-in agents
    /// - 📁 Project: Project-specific agents from agents/ directory
    /// - 👤 User: User-defined agents from .swissarmyhammer/agents/
    ///
    /// # Examples
    ///
    /// ```
    /// use swissarmyhammer_config::agent::AgentSource;
    ///
    /// assert_eq!(AgentSource::Builtin.display_emoji(), "📦 Built-in");
    /// assert_eq!(AgentSource::Project.display_emoji(), "📁 Project");
    /// assert_eq!(AgentSource::User.display_emoji(), "👤 User");
    /// ```
    pub fn display_emoji(&self) -> &'static str {
        match self {
            AgentSource::Builtin => "📦 Built-in",
            AgentSource::Project => "📁 Project",
            AgentSource::User => "👤 User",
        }
    }
}

/// Agent-specific error types
///
/// Comprehensive error handling for agent discovery, parsing, and management operations.
#[derive(Error, Debug)]
pub enum AgentError {
    /// Agent not found in any source
    #[error("Agent '{0}' not found")]
    NotFound(String),
    /// Invalid file path for agent configuration
    #[error("Invalid agent path: {0}")]
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

impl Severity for AgentError {
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

/// Agent information structure
///
/// Holds complete metadata for an agent configuration including its source,
/// content, and optional description for discovery and management operations.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentInfo {
    /// Agent name (typically filename without extension)
    pub name: String,
    /// Complete agent configuration content
    pub content: String,
    /// Source location of the agent
    pub source: AgentSource,
    /// Optional description extracted from configuration
    pub description: Option<String>,
}

/// Parse agent description from configuration content
///
/// Extracts description from YAML front matter or comment-based format.
/// Looks for `description:` field in YAML front matter first, then falls
/// back to `# Description:` comment lines.
pub fn parse_agent_description(content: &str) -> Option<String> {
    let content = content.trim();

    // Check for YAML front matter
    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_pos) = stripped.find("---") {
            let front_matter = &stripped[..end_pos];

            // Try to parse as YAML and extract description
            if let Ok(yaml_value) = serde_yaml::from_str::<serde_yaml::Value>(front_matter) {
                if let Some(description) = yaml_value.get("description") {
                    if let Some(desc_str) = description.as_str() {
                        return Some(desc_str.trim().to_string());
                    }
                }
            }
        }
    }

    // Fall back to comment-based description
    for line in content.lines() {
        let line = line.trim();
        if let Some(desc) = line.strip_prefix("# Description:") {
            let desc = desc.trim();
            if !desc.is_empty() {
                return Some(desc.to_string());
            }
        }
    }

    None
}

/// Extracts the agent configuration portion from content that may have YAML frontmatter
///
/// Handles two formats:
/// 1. Frontmatter format: `---\ndescription: "..."\n---\nactual_config`
/// 2. Pure config format: just the AgentConfig YAML
pub fn parse_agent_config(content: &str) -> Result<AgentConfig, serde_yaml::Error> {
    let content = content.trim();

    // Check for YAML front matter
    if let Some(stripped) = content.strip_prefix("---") {
        if let Some(end_pos) = stripped.find("---") {
            // Extract the content after the second ---
            let config_content = &stripped[end_pos + 3..].trim();
            return serde_yaml::from_str::<AgentConfig>(config_content);
        }
    }

    // Fall back to parsing entire content as AgentConfig
    serde_yaml::from_str::<AgentConfig>(content)
}

/// Agent Manager for discovery and loading of agents from various sources
///
/// Provides functionality to load agents from built-in sources, user directories,
/// and project directories with proper precedence handling.
pub struct AgentManager;

impl AgentManager {
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
    /// * `Result<Vec<AgentInfo>, AgentError>` - Combined list of all available agents
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let all_agents = AgentManager::list_agents()?;
    /// for agent in all_agents {
    ///     println!("Agent: {} ({})", agent.name,
    ///              match agent.source {
    ///                  swissarmyhammer_config::agent::AgentSource::Builtin => "built-in",
    ///                  swissarmyhammer_config::agent::AgentSource::Project => "project",
    ///                  swissarmyhammer_config::agent::AgentSource::User => "user",
    ///              });
    /// }
    /// # Ok::<(), swissarmyhammer_config::agent::AgentError>(())
    /// ```
    pub fn list_agents() -> Result<Vec<AgentInfo>, AgentError> {
        tracing::debug!("Starting agent discovery with precedence hierarchy");

        // Start with built-in agents as the base (lowest precedence)
        let mut agents = Self::load_builtin_agents()?;
        tracing::debug!("Loaded {} built-in agents", agents.len());

        let initial_builtin_count = agents.len();
        let mut project_overrides = 0;
        let mut project_new = 0;
        let mut user_overrides = 0;
        let mut user_new = 0;

        // Override with project agents (medium precedence)
        match Self::load_project_agents() {
            Ok(project_agents) => {
                tracing::debug!("Loaded {} project agents", project_agents.len());

                for project_agent in project_agents {
                    // Find existing agent by name and replace, or append if new
                    if let Some(existing_pos) =
                        agents.iter().position(|a| a.name == project_agent.name)
                    {
                        let previous_source = &agents[existing_pos].source;
                        tracing::debug!(
                            "Project agent '{}' overriding {:?} agent at position {}",
                            project_agent.name,
                            previous_source,
                            existing_pos
                        );
                        agents[existing_pos] = project_agent;
                        project_overrides += 1;
                    } else {
                        tracing::debug!(
                            "Adding new project agent '{}' at position {}",
                            project_agent.name,
                            agents.len()
                        );
                        agents.push(project_agent);
                        project_new += 1;
                    }
                }
            }
            Err(e) => {
                // Log warning with details but continue - project agents are optional
                tracing::warn!(
                    "Failed to load project agents: {}. Continuing with built-in agents only",
                    e
                );
            }
        }

        // Override with user agents (highest precedence)
        match Self::load_user_agents() {
            Ok(user_agents) => {
                tracing::debug!("Loaded {} user agents", user_agents.len());

                for user_agent in user_agents {
                    // Find existing agent by name and replace, or append if new
                    if let Some(existing_pos) =
                        agents.iter().position(|a| a.name == user_agent.name)
                    {
                        let previous_source = &agents[existing_pos].source;
                        tracing::debug!(
                            "User agent '{}' overriding {:?} agent at position {}",
                            user_agent.name,
                            previous_source,
                            existing_pos
                        );
                        agents[existing_pos] = user_agent;
                        user_overrides += 1;
                    } else {
                        tracing::debug!(
                            "Adding new user agent '{}' at position {}",
                            user_agent.name,
                            agents.len()
                        );
                        agents.push(user_agent);
                        user_new += 1;
                    }
                }
            }
            Err(e) => {
                // Log warning with details but continue - user agents are optional
                tracing::warn!(
                    "Failed to load user agents: {}. Continuing with existing agents",
                    e
                );
            }
        }

        tracing::info!(
            "Agent discovery complete: {} total agents ({} built-in, {} project overrides, {} new project, {} user overrides, {} new user)",
            agents.len(),
            initial_builtin_count,
            project_overrides,
            project_new,
            user_overrides,
            user_new
        );

        // Log final agent list for debugging
        for (idx, agent) in agents.iter().enumerate() {
            tracing::trace!(
                "Agent[{}]: '{}' ({:?}) - {}",
                idx,
                agent.name,
                agent.source,
                agent.description.as_deref().unwrap_or("no description")
            );
        }

        Ok(agents)
    }

    /// Load all built-in agents compiled into the binary
    ///
    /// Uses the build-time generated `get_builtin_agents()` function to access
    /// agents embedded from the `builtin/agents/` directory.
    ///
    /// # Returns
    /// * `Result<Vec<AgentInfo>, AgentError>` - Vector of built-in agent information
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let builtin_agents = AgentManager::load_builtin_agents()?;
    /// for agent in builtin_agents {
    ///     println!("Built-in agent: {} ({})", agent.name,
    ///              agent.description.unwrap_or_default());
    /// }
    /// # Ok::<(), swissarmyhammer_config::AgentError>(())
    /// ```
    pub fn load_builtin_agents() -> Result<Vec<AgentInfo>, AgentError> {
        let builtin_agents = crate::get_builtin_agents();
        let mut agents = Vec::with_capacity(builtin_agents.len());

        for (name, content) in builtin_agents {
            let description = parse_agent_description(content);
            agents.push(AgentInfo {
                name: name.to_string(),
                content: content.to_string(),
                source: AgentSource::Builtin,
                description,
            });
        }

        Ok(agents)
    }

    /// Load agents from a specific directory
    ///
    /// Scans the given directory for `.yaml` agent configuration files and loads them
    /// with the specified source type. Missing directories are handled gracefully by
    /// returning an empty vector. Individual agent validation failures are logged but
    /// don't prevent loading other agents.
    ///
    /// # Arguments
    /// * `dir_path` - Path to the directory to scan for agent files
    /// * `source` - The source type to assign to loaded agents
    ///
    /// # Returns
    /// * `Result<Vec<AgentInfo>, AgentError>` - Vector of agent information from the directory
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::{AgentManager, AgentSource};
    /// use std::path::Path;
    ///
    /// let agents = AgentManager::load_agents_from_dir(
    ///     Path::new("./agents"),
    ///     AgentSource::Project
    /// )?;
    /// # Ok::<(), swissarmyhammer_config::AgentError>(())
    /// ```
    pub fn load_agents_from_dir(
        dir_path: &Path,
        source: AgentSource,
    ) -> Result<Vec<AgentInfo>, AgentError> {
        // Handle missing directory gracefully
        if !dir_path.exists() || !dir_path.is_dir() {
            tracing::debug!(
                "Agent directory does not exist or is not a directory: {}",
                dir_path.display()
            );
            return Ok(Vec::new());
        }

        tracing::debug!(
            "Loading agents from directory: {} (source: {:?})",
            dir_path.display(),
            source
        );

        let mut agents = Vec::new();
        let mut successful_count = 0;
        let mut failed_count = 0;

        let entries = std::fs::read_dir(dir_path).map_err(|e| {
            tracing::error!(
                "Failed to read agent directory {}: {}",
                dir_path.display(),
                e
            );
            AgentError::IoError(e)
        })?;

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

            // Only process .yaml files
            if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("yaml") {
                // Use filename stem as agent name
                let agent_name = match path.file_stem().and_then(|name| name.to_str()) {
                    Some(name) => name,
                    None => {
                        tracing::warn!(
                            "Failed to extract agent name from path: {}",
                            path.display()
                        );
                        failed_count += 1;
                        continue;
                    }
                };

                // Read and validate agent content
                let content = match std::fs::read_to_string(&path) {
                    Ok(content) => content,
                    Err(e) => {
                        tracing::warn!("Failed to read agent file {}: {}", path.display(), e);
                        failed_count += 1;
                        continue;
                    }
                };

                // Validate agent configuration by attempting to parse it
                match parse_agent_config(&content) {
                    Ok(_) => {
                        let description = parse_agent_description(&content);
                        tracing::trace!(
                            "Successfully loaded agent '{}' from {} (description: {:?})",
                            agent_name,
                            path.display(),
                            description
                        );

                        agents.push(AgentInfo {
                            name: agent_name.to_string(),
                            content,
                            source: source.clone(),
                            description,
                        });
                        successful_count += 1;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Agent configuration validation failed for {}: {}. Skipping this agent.",
                            path.display(),
                            e
                        );
                        failed_count += 1;
                    }
                }
            }
        }

        tracing::debug!(
            "Finished loading agents from {}: {} successful, {} failed",
            dir_path.display(),
            successful_count,
            failed_count
        );

        Ok(agents)
    }

    /// Load user-defined agents from ~/.swissarmyhammer/agents/
    ///
    /// Scans the user's home directory `.swissarmyhammer/agents/` for agent configuration
    /// files. Missing directory is handled gracefully by returning an empty vector.
    ///
    /// # Returns
    /// * `Result<Vec<AgentInfo>, AgentError>` - Vector of user-defined agent information
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let user_agents = AgentManager::load_user_agents()?;
    /// for agent in user_agents {
    ///     println!("User agent: {}", agent.name);
    /// }
    /// # Ok::<(), swissarmyhammer_config::AgentError>(())
    /// ```
    pub fn load_user_agents() -> Result<Vec<AgentInfo>, AgentError> {
        if let Some(home_dir) = dirs::home_dir() {
            let user_agents_dir = home_dir.join(".swissarmyhammer").join("agents");
            Self::load_agents_from_dir(&user_agents_dir, AgentSource::User)
        } else {
            // No home directory available (rare case)
            Ok(Vec::new())
        }
    }

    /// Load project-specific agents from ./agents/
    ///
    /// Scans the current working directory's `agents/` subdirectory for agent configuration
    /// files. Missing directory is handled gracefully by returning an empty vector.
    ///
    /// # Returns
    /// * `Result<Vec<AgentInfo>, AgentError>` - Vector of project-specific agent information
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let project_agents = AgentManager::load_project_agents()?;
    /// for agent in project_agents {
    ///     println!("Project agent: {}", agent.name);
    /// }
    /// # Ok::<(), swissarmyhammer_config::AgentError>(())
    /// ```
    pub fn load_project_agents() -> Result<Vec<AgentInfo>, AgentError> {
        let project_agents_dir = std::env::current_dir()
            .map_err(AgentError::IoError)?
            .join("agents");
        Self::load_agents_from_dir(&project_agents_dir, AgentSource::Project)
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
    /// * `Result<AgentInfo, AgentError>` - The found agent information or NotFound error
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let agent = AgentManager::find_agent_by_name("claude-code")?;
    /// println!("Found agent: {} from {:?}", agent.name, agent.source);
    /// # Ok::<(), swissarmyhammer_config::agent::AgentError>(())
    /// ```
    pub fn find_agent_by_name(agent_name: &str) -> Result<AgentInfo, AgentError> {
        let agents = Self::list_agents()?;

        agents
            .into_iter()
            .find(|agent| agent.name == agent_name)
            .ok_or_else(|| AgentError::NotFound(agent_name.to_string()))
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
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// match AgentManager::detect_config_file() {
    ///     Some(config_path) => println!("Found config: {}", config_path.display()),
    ///     None => println!("No existing config found"),
    /// }
    /// ```
    pub fn detect_config_file() -> Option<PathBuf> {
        let current_dir = std::env::current_dir().ok()?;
        let sah_dir = current_dir.join(".swissarmyhammer");

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
    /// # Returns
    /// * `Result<PathBuf, AgentError>` - Path to config file (existing or new) or error
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// let config_path = AgentManager::ensure_config_structure()?;
    /// println!("Config file path: {}", config_path.display());
    /// # Ok::<(), swissarmyhammer_config::agent::AgentError>(())
    /// ```
    pub fn ensure_config_structure() -> Result<PathBuf, AgentError> {
        let current_dir = std::env::current_dir().map_err(AgentError::IoError)?;
        let sah_dir = current_dir.join(".swissarmyhammer");

        // Create .swissarmyhammer directory if it doesn't exist
        if !sah_dir.exists() {
            std::fs::create_dir_all(&sah_dir).map_err(AgentError::IoError)?;
            tracing::debug!("Created .swissarmyhammer directory: {}", sah_dir.display());
        }

        // Check for existing config file first
        if let Some(existing_config) = Self::detect_config_file() {
            tracing::debug!("Found existing config file: {}", existing_config.display());
            return Ok(existing_config);
        }

        // Return path for new YAML config (don't create the file yet)
        let new_config = sah_dir.join("sah.yaml");
        tracing::debug!("Will use new config file: {}", new_config.display());
        Ok(new_config)
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
    /// * `Result<(), AgentError>` - Success or error details
    ///
    /// # Examples
    /// ```no_run
    /// use swissarmyhammer_config::agent::AgentManager;
    ///
    /// // Apply built-in claude-code agent to project
    /// AgentManager::use_agent("claude-code")?;
    ///
    /// // Apply a custom user agent
    /// AgentManager::use_agent("my-custom-agent")?;
    /// # Ok::<(), swissarmyhammer_config::agent::AgentError>(())
    /// ```
    pub fn use_agent(agent_name: &str) -> Result<(), AgentError> {
        tracing::info!("Applying agent '{}' to project configuration", agent_name);

        // Step 1: Find the agent by name
        let agent_info = Self::find_agent_by_name(agent_name)?;
        tracing::debug!(
            "Found agent '{}' from source: {:?}",
            agent_info.name,
            agent_info.source
        );

        // Step 2: Parse agent configuration to validate it
        let agent_config = parse_agent_config(&agent_info.content)?;
        tracing::debug!(
            "Successfully parsed agent configuration for '{}'",
            agent_name
        );

        // Step 3: Ensure config structure exists and get config file path
        let config_path = Self::ensure_config_structure()?;
        tracing::debug!("Using config file: {}", config_path.display());

        // Step 4: Load existing config or create new one
        let mut project_config = if config_path.exists() {
            tracing::debug!(
                "Loading existing configuration from {}",
                config_path.display()
            );
            let config_content =
                std::fs::read_to_string(&config_path).map_err(AgentError::IoError)?;
            serde_yaml::from_str::<serde_yaml::Value>(&config_content)?
        } else {
            tracing::debug!("Creating new configuration file");
            serde_yaml::Value::Mapping(serde_yaml::Mapping::new())
        };

        // Step 5: Convert agent config to YAML value and update project config
        let agent_yaml = serde_yaml::to_value(&agent_config)?;

        match &mut project_config {
            serde_yaml::Value::Mapping(ref mut map) => {
                map.insert(serde_yaml::Value::String("agent".to_string()), agent_yaml);
                tracing::debug!("Updated agent section in project configuration");
            }
            _ => {
                return Err(AgentError::ConfigError(
                    "Project configuration must be a YAML mapping".to_string(),
                ));
            }
        }

        // Step 6: Write updated config back to file
        let updated_content = serde_yaml::to_string(&project_config)?;
        std::fs::write(&config_path, &updated_content).map_err(AgentError::IoError)?;

        tracing::info!(
            "Successfully applied agent '{}' to project config: {}",
            agent_name,
            config_path.display()
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_system_default_is_claude() {
        let config = AgentConfig::default();
        assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
        assert!(!config.quiet);
    }

    #[test]
    fn test_agent_executor_type_default() {
        let executor_type = AgentExecutorType::default();
        assert_eq!(executor_type, AgentExecutorType::ClaudeCode);
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
                assert_eq!(repo, "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF");
                assert_eq!(
                    filename,
                    Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string())
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
        assert_eq!(config.mcp_server.timeout_seconds, 900); // Default timeout
                                                            // Removed test_mode field - now always uses real models
    }

    #[test]
    fn test_agent_config_claude_code_factory() {
        let config = AgentConfig::claude_code();
        assert_eq!(config.executor_type(), AgentExecutorType::ClaudeCode);
        assert!(!config.quiet);

        match config.executor {
            AgentExecutorConfig::ClaudeCode(claude_config) => {
                assert!(claude_config.claude_path.is_none());
                assert!(claude_config.args.is_empty());
            }
            AgentExecutorConfig::LlamaAgent(_) => panic!("Should be Claude Code config"),
        }
    }

    #[test]
    fn test_agent_config_llama_agent_factory() {
        let llama_config = LlamaAgentConfig::for_testing();
        let config = AgentConfig::llama_agent(llama_config.clone());

        assert_eq!(config.executor_type(), AgentExecutorType::LlamaAgent);
        assert!(!config.quiet);

        match config.executor {
            AgentExecutorConfig::LlamaAgent(agent_config) => {
                assert_eq!(agent_config.mcp_server.timeout_seconds, 900);
            }
            AgentExecutorConfig::ClaudeCode(_) => panic!("Should be LlamaAgent config"),
        }
    }

    #[test]
    fn test_configuration_serialization_yaml() {
        let config = AgentConfig::llama_agent(LlamaAgentConfig::for_testing());

        // Should serialize to YAML correctly
        let yaml = serde_yaml::to_string(&config).expect("Failed to serialize to YAML");
        assert!(yaml.contains("type: llama-agent"));
        assert!(yaml.contains("quiet: false"));

        // Should deserialize from YAML correctly
        let deserialized: AgentConfig =
            serde_yaml::from_str(&yaml).expect("Failed to deserialize from YAML");
        assert_eq!(config.executor_type(), deserialized.executor_type());
        assert_eq!(config.quiet, deserialized.quiet);
    }

    #[test]
    fn test_configuration_serialization_json() {
        let config = AgentConfig::claude_code();

        // Should serialize to JSON correctly
        let json = serde_json::to_string(&config).expect("Failed to serialize to JSON");
        assert!(json.contains("\"type\":\"claude-code\""));
        assert!(json.contains("\"quiet\":false"));

        // Should deserialize from JSON correctly
        let deserialized: AgentConfig =
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
    fn test_agent_source_variants() {
        // Test all AgentSource variants exist and have correct Debug output
        assert_eq!(format!("{:?}", AgentSource::Builtin), "Builtin");
        assert_eq!(format!("{:?}", AgentSource::Project), "Project");
        assert_eq!(format!("{:?}", AgentSource::User), "User");
    }

    #[test]
    fn test_agent_source_equality() {
        assert_eq!(AgentSource::Builtin, AgentSource::Builtin);
        assert_eq!(AgentSource::Project, AgentSource::Project);
        assert_eq!(AgentSource::User, AgentSource::User);

        assert_ne!(AgentSource::Builtin, AgentSource::Project);
        assert_ne!(AgentSource::Builtin, AgentSource::User);
        assert_ne!(AgentSource::Project, AgentSource::User);
    }

    #[test]
    fn test_agent_source_display_emoji() {
        assert_eq!(AgentSource::Builtin.display_emoji(), "📦 Built-in");
        assert_eq!(AgentSource::Project.display_emoji(), "📁 Project");
        assert_eq!(AgentSource::User.display_emoji(), "👤 User");
    }

    #[test]
    fn test_agent_source_serialization() {
        // Test serde serialization with kebab-case
        let builtin = AgentSource::Builtin;
        let json = serde_json::to_string(&builtin).expect("Failed to serialize Builtin");
        assert_eq!(json, "\"builtin\"");

        let project = AgentSource::Project;
        let json = serde_json::to_string(&project).expect("Failed to serialize Project");
        assert_eq!(json, "\"project\"");

        let user = AgentSource::User;
        let json = serde_json::to_string(&user).expect("Failed to serialize User");
        assert_eq!(json, "\"user\"");
    }

    #[test]
    fn test_agent_source_deserialization() {
        let builtin: AgentSource =
            serde_json::from_str("\"builtin\"").expect("Failed to deserialize builtin");
        assert_eq!(builtin, AgentSource::Builtin);

        let project: AgentSource =
            serde_json::from_str("\"project\"").expect("Failed to deserialize project");
        assert_eq!(project, AgentSource::Project);

        let user: AgentSource =
            serde_json::from_str("\"user\"").expect("Failed to deserialize user");
        assert_eq!(user, AgentSource::User);
    }

    #[test]
    fn test_agent_error_display() {
        let not_found = AgentError::NotFound("test-agent".to_string());
        assert_eq!(format!("{}", not_found), "Agent 'test-agent' not found");

        let invalid_path = AgentError::InvalidPath(PathBuf::from("/invalid/path"));
        assert!(format!("{}", invalid_path).contains("Invalid agent path"));
        assert!(format!("{}", invalid_path).contains("/invalid/path"));
    }

    #[test]
    fn test_agent_error_from_io_error() {
        let io_error = std::io::Error::new(std::io::ErrorKind::NotFound, "File not found");
        let agent_error: AgentError = io_error.into();

        match agent_error {
            AgentError::IoError(_) => {} // Expected
            _ => panic!("Should convert to IoError variant"),
        }
    }

    #[test]
    fn test_agent_error_from_serde_yaml_error() {
        let invalid_yaml = "invalid: yaml: content: [unclosed";
        let yaml_error = serde_yaml::from_str::<serde_yaml::Value>(invalid_yaml)
            .expect_err("Should fail to parse invalid YAML");
        let agent_error: AgentError = yaml_error.into();

        match agent_error {
            AgentError::ParseError(_) => {} // Expected
            _ => panic!("Should convert to ParseError variant"),
        }
    }

    #[test]
    fn test_agent_info_creation() {
        let agent_info = AgentInfo {
            name: "test-agent".to_string(),
            content: "agent: config".to_string(),
            source: AgentSource::Builtin,
            description: Some("Test agent description".to_string()),
        };

        assert_eq!(agent_info.name, "test-agent");
        assert_eq!(agent_info.content, "agent: config");
        assert_eq!(agent_info.source, AgentSource::Builtin);
        assert_eq!(
            agent_info.description,
            Some("Test agent description".to_string())
        );
    }

    #[test]
    fn test_agent_info_equality() {
        let agent1 = AgentInfo {
            name: "test".to_string(),
            content: "config".to_string(),
            source: AgentSource::Builtin,
            description: None,
        };

        let agent2 = AgentInfo {
            name: "test".to_string(),
            content: "config".to_string(),
            source: AgentSource::Builtin,
            description: None,
        };

        let agent3 = AgentInfo {
            name: "different".to_string(),
            content: "config".to_string(),
            source: AgentSource::Builtin,
            description: None,
        };

        assert_eq!(agent1, agent2);
        assert_ne!(agent1, agent3);
    }

    #[test]
    fn test_agent_info_serialization() {
        let agent_info = AgentInfo {
            name: "test-agent".to_string(),
            content: "executor:\n  type: claude-code\n  config: {}\nquiet: false".to_string(),
            source: AgentSource::User,
            description: Some("A test agent".to_string()),
        };

        let json = serde_json::to_string(&agent_info).expect("Failed to serialize AgentInfo");
        let deserialized: AgentInfo =
            serde_json::from_str(&json).expect("Failed to deserialize AgentInfo");

        assert_eq!(agent_info, deserialized);
    }

    #[test]
    fn test_parse_agent_description_yaml_frontmatter() {
        let content = r#"---
description: "This is a test agent"
other_field: value
---
type: claude-code
config: {}"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("This is a test agent".to_string()));
    }

    #[test]
    fn test_parse_agent_description_comment_format() {
        let content = r#"# Description: This is a comment-based description
type: claude-code
config: {}"#;

        let description = parse_agent_description(content);
        assert_eq!(
            description,
            Some("This is a comment-based description".to_string())
        );
    }

    #[test]
    fn test_parse_agent_description_no_description() {
        let content = r#"executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
        assert_eq!(description, None);
    }

    #[test]
    fn test_parse_agent_description_empty_yaml_description() {
        let content = r#"---
description: ""
other_field: value
---
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("".to_string()));
    }

    #[test]
    fn test_parse_agent_description_empty_comment_description() {
        let content = r#"# Description:
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
        assert_eq!(description, None); // Empty descriptions are treated as None
    }

    #[test]
    fn test_parse_agent_description_yaml_precedence() {
        let content = r#"---
description: "YAML description"
---
# Description: Comment description
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("YAML description".to_string()));
    }

    #[test]
    fn test_parse_agent_description_malformed_yaml() {
        let content = r#"---
invalid: yaml: content: [unclosed
---
# Description: Fallback comment description
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
        assert_eq!(
            description,
            Some("Fallback comment description".to_string())
        );
    }

    #[test]
    fn test_parse_agent_description_whitespace_handling() {
        let content = r#"---
description: "  Padded description  "
---"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("Padded description".to_string()));

        let comment_content = r#"# Description:   Padded comment   "#;
        let description = parse_agent_description(comment_content);
        assert_eq!(description, Some("Padded comment".to_string()));
    }

    #[test]
    fn test_parse_agent_description_multiline_comment() {
        let content = r#"# Description: First line
# This is additional content
executor:
  type: claude-code
  config: {}
quiet: false"#;

        let description = parse_agent_description(content);
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

        let config = parse_agent_config(content);
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

        let config = parse_agent_config(content);
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

        let config = parse_agent_config(content);
        assert!(config.is_ok(), "Should parse pure YAML agent config");
        let config = config.unwrap();
        assert!(config.quiet);
    }

    #[test]
    fn test_agent_manager_load_builtin_agents() {
        let agents = AgentManager::load_builtin_agents().expect("Failed to load builtin agents");

        // Should contain at least the known builtin agents
        assert!(!agents.is_empty(), "Builtin agents should not be empty");

        // All agents should have Builtin source
        for agent in &agents {
            assert_eq!(agent.source, AgentSource::Builtin);
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
        let result = AgentManager::load_agents_from_dir(non_existent_dir, AgentSource::User);

        assert!(result.is_ok(), "Should handle missing directory gracefully");
        let agents = result.unwrap();
        assert!(
            agents.is_empty(),
            "Should return empty vector for missing directory"
        );
    }

    #[test]
    fn test_agent_manager_load_agents_from_dir_with_temp_files() {
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

        let result = AgentManager::load_agents_from_dir(temp_path, AgentSource::Project);
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
            assert_eq!(agent.source, AgentSource::Project);
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
        let result = AgentManager::load_user_agents();

        // Should not fail even if no user agents exist
        assert!(
            result.is_ok(),
            "Should handle user agent loading gracefully"
        );

        let agents = result.unwrap();
        // All agents should have User source
        for agent in &agents {
            assert_eq!(agent.source, AgentSource::User);
        }
    }

    #[test]
    fn test_agent_manager_load_project_agents() {
        let result = AgentManager::load_project_agents();

        // Should not fail even if no project agents exist
        assert!(
            result.is_ok(),
            "Should handle project agent loading gracefully"
        );

        let agents = result.unwrap();
        // All agents should have Project source
        for agent in &agents {
            assert_eq!(agent.source, AgentSource::Project);
        }
    }

    #[test]
    fn test_agent_manager_list_agents_precedence() {
        // This test verifies the complete agent discovery hierarchy with precedence
        let result = AgentManager::list_agents();

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
                AgentSource::Builtin | AgentSource::Project | AgentSource::User => {
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

        let project_agents_dir = temp_project_dir.path().join("agents");
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

        let user_agents_dir = temp_user_dir.path().join("agents");
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
            AgentManager::load_agents_from_dir(&project_agents_dir, AgentSource::Project);
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
        assert_eq!(claude_agent.source, AgentSource::Project);
        assert_eq!(
            claude_agent.description,
            Some("Project-overridden Claude Code agent".to_string())
        );

        let unique_agent = project_agents.iter().find(|a| a.name == "unique-project");
        assert!(unique_agent.is_some(), "Should find unique project agent");
        let unique_agent = unique_agent.unwrap();
        assert_eq!(unique_agent.source, AgentSource::Project);
        assert_eq!(
            unique_agent.description,
            Some("Unique project agent".to_string())
        );

        // Test user agents
        let user_agents = AgentManager::load_agents_from_dir(&user_agents_dir, AgentSource::User);
        assert!(user_agents.is_ok(), "Should load user agents successfully");

        let user_agents = user_agents.unwrap();
        assert_eq!(user_agents.len(), 1, "Should load 1 user agent");

        let user_claude = &user_agents[0];
        assert_eq!(user_claude.name, "claude-code");
        assert_eq!(user_claude.source, AgentSource::User);
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
        let result = AgentManager::load_agents_from_dir(temp_path, AgentSource::Project);

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
            assert_eq!(agent.source, AgentSource::Project);
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

        let result = AgentManager::load_agents_from_dir(&empty_dir, AgentSource::Project);
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

        let result = AgentManager::load_agents_from_dir(temp_path, AgentSource::User);
        assert!(
            result.is_ok(),
            "Should load agents while ignoring non-YAML files"
        );

        let agents = result.unwrap();
        assert_eq!(agents.len(), 1, "Should load only the YAML file");
        assert_eq!(agents[0].name, "real-agent");
        assert_eq!(agents[0].source, AgentSource::User);
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_existing() {
        let result = AgentManager::find_agent_by_name("claude-code");
        assert!(result.is_ok(), "Should find existing claude-code agent");

        let agent = result.unwrap();
        assert_eq!(agent.name, "claude-code");
        assert_eq!(agent.source, AgentSource::Builtin);
        assert!(!agent.content.is_empty());
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_not_found() {
        let result = AgentManager::find_agent_by_name("non-existent-agent");
        assert!(
            result.is_err(),
            "Should return error for non-existent agent"
        );

        match result {
            Err(AgentError::NotFound(name)) => {
                assert_eq!(name, "non-existent-agent");
            }
            _ => panic!("Should return NotFound error"),
        }
    }

    #[test]
    fn test_agent_manager_find_agent_by_name_precedence() {
        // This test will pass the existing agent names from builtin agents
        // Test with known builtin agent
        let result = AgentManager::find_agent_by_name("qwen-coder");
        assert!(result.is_ok(), "Should find qwen-coder agent");

        let agent = result.unwrap();
        assert_eq!(agent.name, "qwen-coder");
        // Should be builtin unless overridden by project or user agents
        assert_eq!(agent.source, AgentSource::Builtin);
    }

    #[test]
    fn test_agent_manager_detect_config_file_no_config() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        let result = AgentManager::detect_config_file();
        assert!(
            result.is_none(),
            "Should return None when no config files exist"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_detect_config_file_yaml_exists() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Create .swissarmyhammer directory and yaml config
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer dir");
        let yaml_path = sah_dir.join("sah.yaml");
        fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");

        let result = AgentManager::detect_config_file();
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

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_detect_config_file_toml_fallback() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Create .swissarmyhammer directory and toml config only
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer dir");
        let toml_path = sah_dir.join("sah.toml");
        fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

        let result = AgentManager::detect_config_file();
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

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
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
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to create .swissarmyhammer dir");
        let yaml_path = sah_dir.join("sah.yaml");
        let toml_path = sah_dir.join("sah.toml");
        fs::write(&yaml_path, "agent: {}\n").expect("Failed to write yaml config");
        fs::write(&toml_path, "[agent]\n").expect("Failed to write toml config");

        let result = AgentManager::detect_config_file();
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

        let result = AgentManager::ensure_config_structure();
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
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
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
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");

        let result = AgentManager::ensure_config_structure();
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
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to pre-create directory");
        let existing_config = sah_dir.join("sah.toml");
        fs::write(&existing_config, "[existing]\nvalue = true\n")
            .expect("Failed to write existing config");

        let result = AgentManager::ensure_config_structure();
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
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Use a known builtin agent
        let result = AgentManager::use_agent("claude-code");
        assert!(result.is_ok(), "Should successfully use claude-code agent");

        // Check that config file was created
        let config_path = temp_dir.path().join(".swissarmyhammer").join("sah.yaml");
        assert!(config_path.exists(), "Should create config file");

        // Read and verify config content
        let config_content = fs::read_to_string(&config_path).expect("Failed to read config");
        assert!(
            config_content.contains("agent:"),
            "Should contain agent section"
        );
        assert!(
            config_content.contains("executor:"),
            "Should contain executor config"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_use_agent_updates_existing_config() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Create existing config with other sections
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to create sah dir");
        let config_path = sah_dir.join("sah.yaml");
        let existing_config = r#"# Existing config
other_section:
  value: "preserved"
  number: 42
"#;
        fs::write(&config_path, existing_config).expect("Failed to write existing config");

        // Use agent
        let result = AgentManager::use_agent("claude-code");
        if let Err(e) = &result {
            panic!("Failed to use claude-code agent: {:?}", e);
        }
        assert!(result.is_ok(), "Should successfully update existing config");

        // Read and verify updated config
        println!("About to read config file: {}", config_path.display());
        println!("Config file exists: {}", config_path.exists());
        let updated_config =
            fs::read_to_string(&config_path).expect("Failed to read updated config");
        println!("Updated config content:\n{}", updated_config);
        println!("Config length: {}", updated_config.len());
        assert!(
            updated_config.contains("other_section:"),
            "Should preserve existing sections"
        );
        assert!(
            updated_config.contains("value: preserved"),
            "Should preserve existing values"
        );
        assert!(
            updated_config.contains("agent:"),
            "Should add agent section"
        );
        assert!(
            updated_config.contains("executor:"),
            "Should contain executor config"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_use_agent_replaces_existing_agent() {
        use std::env;
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        // Create config with existing agent section
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).expect("Failed to create sah dir");
        let config_path = sah_dir.join("sah.yaml");
        let existing_config = r#"# Config with existing agent
other_section:
  value: "preserved"
agent:
  executor:
    type: llama-agent
    config: {}
  quiet: true
"#;
        fs::write(&config_path, existing_config).expect("Failed to write existing config");

        // Use different agent
        let result = AgentManager::use_agent("claude-code");
        assert!(result.is_ok(), "Should successfully replace existing agent");

        // Read and verify updated config
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
            updated_config.contains("agent:"),
            "Should have agent section"
        );
        assert!(
            updated_config.contains("claude-code"),
            "Should contain new agent config"
        );
        assert!(
            !updated_config.contains("llama-agent"),
            "Should replace old agent config"
        );

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_manager_use_agent_not_found() {
        use std::env;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().expect("Failed to create temp dir");
        let original_dir = env::current_dir().expect("Failed to get current dir");
        env::set_current_dir(&temp_dir).expect("Failed to change to temp dir");

        let result = AgentManager::use_agent("non-existent-agent");
        assert!(result.is_err(), "Should fail for non-existent agent");

        match result {
            Err(AgentError::NotFound(name)) => {
                assert_eq!(name, "non-existent-agent");
            }
            _ => panic!("Should return NotFound error"),
        }

        env::set_current_dir(&original_dir).expect("Failed to restore original dir");
    }

    #[test]
    fn test_agent_error_not_found_is_error() {
        let error = AgentError::NotFound("test-agent".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_agent_error_invalid_path_is_error() {
        let error = AgentError::InvalidPath(PathBuf::from("/invalid/path"));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_agent_error_io_error_is_error() {
        let error = AgentError::from(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        assert_eq!(error.severity(), ErrorSeverity::Error);
    }

    #[test]
    fn test_agent_error_parse_error_is_critical() {
        let yaml_err =
            serde_yaml::from_str::<serde_yaml::Value>("invalid: yaml: content").unwrap_err();
        let error = AgentError::from(yaml_err);
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_agent_error_config_error_is_critical() {
        let error = AgentError::ConfigError("Invalid configuration".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Critical);
    }
}
