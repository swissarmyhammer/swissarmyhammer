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
    /// From project-level .skills/ or .sah/skills/
    Local,
    /// From user-level ~/.skills/ or ~/.sah/skills/
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_skill_name_display() {
        let name = SkillName::new("my-skill").unwrap();
        assert_eq!(format!("{}", name), "my-skill");
        assert_eq!(name.to_string(), "my-skill");
    }

    #[test]
    fn test_skill_name_valid() {
        assert!(SkillName::new("plan").is_ok());
        assert!(SkillName::new("my-skill").is_ok());
        assert!(SkillName::new("skill123").is_ok());
        assert!(SkillName::new("a-b-c-1-2-3").is_ok());
    }

    #[test]
    fn test_skill_name_empty() {
        let err = SkillName::new("").unwrap_err();
        assert!(err.contains("cannot be empty"));
    }

    #[test]
    fn test_skill_name_invalid_chars() {
        let err = SkillName::new("My-Skill").unwrap_err();
        assert!(err.contains("lowercase alphanumeric"));

        let err = SkillName::new("skill_name").unwrap_err();
        assert!(err.contains("lowercase alphanumeric"));

        let err = SkillName::new("skill name").unwrap_err();
        assert!(err.contains("lowercase alphanumeric"));
    }

    #[test]
    fn test_skill_name_as_str() {
        let name = SkillName::new("test").unwrap();
        assert_eq!(name.as_str(), "test");
    }

    #[test]
    fn test_skill_source_display() {
        assert_eq!(format!("{}", SkillSource::Builtin), "builtin");
        assert_eq!(format!("{}", SkillSource::Local), "local");
        assert_eq!(format!("{}", SkillSource::User), "user");
    }

    #[test]
    fn test_skill_resources_default() {
        let resources = SkillResources::default();
        assert!(resources.files.is_empty());
    }
}
