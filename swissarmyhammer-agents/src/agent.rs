//! Core agent types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A validated agent name (lowercase, alphanumeric with hyphens)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AgentName(String);

impl AgentName {
    /// Create a new AgentName, validating the format
    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        if name.is_empty() {
            return Err("agent name cannot be empty".to_string());
        }
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!(
                "agent name '{}' must be lowercase alphanumeric with hyphens only",
                name
            ));
        }
        Ok(Self(name))
    }

    /// Get the name as a string slice
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where an agent was loaded from
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentSource {
    /// Embedded in the binary
    Builtin,
    /// From project-level .agents/ or .swissarmyhammer/agents/
    Local,
    /// From user-level ~/.agents/ or ~/.swissarmyhammer/agents/
    User,
}

impl std::fmt::Display for AgentSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentSource::Builtin => write!(f, "builtin"),
            AgentSource::Local => write!(f, "local"),
            AgentSource::User => write!(f, "user"),
        }
    }
}

/// Resources bundled with an agent (additional files in the agent directory)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AgentResources {
    /// Map of filename -> content for additional files
    pub files: HashMap<String, String>,
}

/// A parsed agent definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Agent {
    /// Validated agent name
    pub name: AgentName,
    /// Human-readable description
    pub description: String,
    /// Model preference (e.g., "default", "sonnet", "opus", "haiku")
    pub model: Option<String>,
    /// Allowed MCP tools for this agent
    pub tools: Vec<String>,
    /// Disallowed MCP tools for this agent
    pub disallowed_tools: Vec<String>,
    /// Isolation level (e.g., "none", "worktree")
    pub isolation: Option<String>,
    /// Maximum number of turns for execution
    pub max_turns: Option<u32>,
    /// Whether this agent runs in the background
    pub background: bool,
    /// Arbitrary metadata key-value pairs
    pub metadata: HashMap<String, String>,
    /// The full AGENT.md body (system prompt instructions)
    pub instructions: String,
    /// Source path on disk (None for builtin)
    pub source_path: Option<PathBuf>,
    /// Where this agent was loaded from
    pub source: AgentSource,
    /// Additional resource files
    pub resources: AgentResources,
}
