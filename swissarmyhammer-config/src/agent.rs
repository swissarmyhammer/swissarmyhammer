//! Agent configuration types and infrastructure
//!
//! This module defines the type system for agent configuration in SwissArmyHammer,
//! supporting hierarchical configuration with proper fallback chains.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
    /// Configuration for unit testing with a small model - optimized for speed
    pub fn for_testing() -> Self {
        Self {
            model: ModelConfig {
                source: ModelSource::HuggingFace {
                    repo: crate::DEFAULT_TEST_LLM_MODEL_REPO.to_string(),
                    filename: Some(crate::DEFAULT_TEST_LLM_MODEL_FILENAME.to_string()),
                    folder: None,
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
                    folder: None,
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
            content: "type: claude-code\nconfig: {}".to_string(),
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
        let content = r#"type: claude-code
config: {}"#;

        let description = parse_agent_description(content);
        assert_eq!(description, None);
    }

    #[test]
    fn test_parse_agent_description_empty_yaml_description() {
        let content = r#"---
description: ""
other_field: value
---
type: claude-code"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("".to_string()));
    }

    #[test]
    fn test_parse_agent_description_empty_comment_description() {
        let content = r#"# Description:
type: claude-code"#;

        let description = parse_agent_description(content);
        assert_eq!(description, None); // Empty descriptions are treated as None
    }

    #[test]
    fn test_parse_agent_description_yaml_precedence() {
        let content = r#"---
description: "YAML description"
---
# Description: Comment description
type: claude-code"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("YAML description".to_string()));
    }

    #[test]
    fn test_parse_agent_description_malformed_yaml() {
        let content = r#"---
invalid: yaml: content: [unclosed
---
# Description: Fallback comment description
type: claude-code"#;

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
type: claude-code"#;

        let description = parse_agent_description(content);
        assert_eq!(description, Some("First line".to_string()));
    }
}
