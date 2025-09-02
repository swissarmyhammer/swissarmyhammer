//! Agent configuration types and infrastructure
//!
//! This module defines the type system for agent configuration in SwissArmyHammer,
//! supporting hierarchical configuration with proper fallback chains.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

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
    },
    Local {
        filename: PathBuf,
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
            port: 0, // Random available port
            timeout_seconds: 30,
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
    /// Configuration for unit testing with a small model - optimized for speed
    pub fn for_testing() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: crate::DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
                    filename: Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                },
                batch_size: 64, // Much smaller batch size for faster testing
                use_hf_params: true,
                debug: false, // Disable debug to reduce output overhead
            },
            mcp_server: McpServerConfig {
                port: 0,
                timeout_seconds: 5, // Shorter timeout for tests
            },

            repetition_detection: RepetitionDetectionConfig {
                enabled: true,             // Keep enabled to match test expectations
                repetition_penalty: 1.05,  // Lower penalty for small models
                repetition_threshold: 100, // Higher threshold to be more permissive
                repetition_window: 32,     // Smaller window for testing
            },
        }
    }

    /// Configuration optimized for small models like Qwen3-1.7B
    pub fn for_small_model() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: "unsloth/Qwen3-Coder-1.5B-Instruct-GGUF".to_string(),
                    filename: Some("Qwen3-Coder-1.5B-Instruct-Q4_K_M.gguf".to_string()),
                },
                batch_size: 256,
                use_hf_params: true,
                debug: false,
            },
            mcp_server: McpServerConfig::default(),

            repetition_detection: RepetitionDetectionConfig {
                enabled: true,
                repetition_penalty: 1.05,  // Lower penalty for small models
                repetition_threshold: 150, // Higher threshold to be more permissive
                repetition_window: 128,    // Larger window for better context
            },
        }
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
            ModelSource::HuggingFace { repo, filename } => {
                assert_eq!(repo, "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF");
                assert_eq!(
                    filename,
                    Some("Qwen3-Coder-30B-A3B-Instruct-UD-Q6_K_XL.gguf".to_string())
                );
            }
            ModelSource::Local { .. } => panic!("Default should be HuggingFace"),
        }
        assert_eq!(config.mcp_server.port, 0);
        assert_eq!(config.mcp_server.timeout_seconds, 30);
    }

    #[test]
    fn test_llama_agent_config_for_testing() {
        let config = LlamaAgentConfig::for_testing();
        match config.model.source {
            ModelSource::HuggingFace { repo, filename } => {
                assert_eq!(repo, crate::DEFAULT_TEST_LLM_MODEL_REPO);
                assert_eq!(
                    filename,
                    Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string())
                );
            }
            ModelSource::Local { .. } => panic!("Testing config should be HuggingFace"),
        }
        assert_eq!(config.mcp_server.port, 0);
        assert_eq!(config.mcp_server.timeout_seconds, 5);
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
                assert_eq!(agent_config.mcp_server.timeout_seconds, 5);
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
        };

        let json = serde_json::to_string(&huggingface_source)
            .expect("Failed to serialize HuggingFace source");
        let deserialized: ModelSource =
            serde_json::from_str(&json).expect("Failed to deserialize HuggingFace source");

        match deserialized {
            ModelSource::HuggingFace { repo, filename } => {
                assert_eq!(repo, "test/repo");
                assert_eq!(filename, Some("model.gguf".to_string()));
            }
            ModelSource::Local { .. } => panic!("Should be HuggingFace source"),
        }

        let local_source = ModelSource::Local {
            filename: PathBuf::from("/path/to/model.gguf"),
        };

        let json = serde_json::to_string(&local_source).expect("Failed to serialize Local source");
        let deserialized: ModelSource =
            serde_json::from_str(&json).expect("Failed to deserialize Local source");

        match deserialized {
            ModelSource::Local { filename } => {
                assert_eq!(filename, PathBuf::from("/path/to/model.gguf"));
            }
            ModelSource::HuggingFace { .. } => panic!("Should be Local source"),
        }
    }
}
