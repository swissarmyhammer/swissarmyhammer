//! Core skill types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// A validated skill name (lowercase, alphanumeric with hyphens)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SkillName(String);

impl SkillName {
    /// Create a new SkillName, validating the format
    pub fn new(name: impl Into<String>) -> Result<Self, String> {
        let name = name.into();
        if name.is_empty() {
            return Err("skill name cannot be empty".to_string());
        }
        // Agent Skills spec: lowercase alphanumeric with hyphens
        if !name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        {
            return Err(format!(
                "skill name '{}' must be lowercase alphanumeric with hyphens only",
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

impl std::fmt::Display for SkillName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Where a skill was loaded from
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SkillSource {
    /// Embedded in the binary
    Builtin,
    /// From project-level .skills/ or .swissarmyhammer/skills/
    Local,
    /// From user-level ~/.skills/ or ~/.swissarmyhammer/skills/
    User,
}

impl std::fmt::Display for SkillSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SkillSource::Builtin => write!(f, "builtin"),
            SkillSource::Local => write!(f, "local"),
            SkillSource::User => write!(f, "user"),
        }
    }
}

/// Resources bundled with a skill (additional files in the skill directory)
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillResources {
    /// Map of filename -> content for additional files
    pub files: HashMap<String, String>,
}

/// A parsed skill definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Skill {
    /// Validated skill name
    pub name: SkillName,
    /// Human-readable description
    pub description: String,
    /// Optional license identifier
    pub license: Option<String>,
    /// Optional compatibility string
    pub compatibility: Option<String>,
    /// Arbitrary metadata key-value pairs
    pub metadata: HashMap<String, String>,
    /// Allowed MCP tools for this skill
    pub allowed_tools: Vec<String>,
    /// The full SKILL.md body (instructions)
    pub instructions: String,
    /// Source path on disk (None for builtin)
    pub source_path: Option<PathBuf>,
    /// Where this skill was loaded from
    pub source: SkillSource,
    /// Additional resource files
    pub resources: SkillResources,
}
