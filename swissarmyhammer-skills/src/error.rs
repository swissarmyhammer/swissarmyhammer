//! Error types for the skills crate

use std::fmt;

/// Errors that can occur during skill operations
#[derive(Debug)]
pub enum SkillError {
    /// Skill not found by name
    NotFound { name: String },
    /// Invalid skill definition
    InvalidSkill { message: String },
    /// Parse error
    Parse { message: String },
    /// I/O error
    Io(std::io::Error),
}

impl fmt::Display for SkillError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SkillError::NotFound { name } => write!(f, "skill not found: '{}'", name),
            SkillError::InvalidSkill { message } => write!(f, "invalid skill: {}", message),
            SkillError::Parse { message } => write!(f, "parse error: {}", message),
            SkillError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for SkillError {}

impl From<std::io::Error> for SkillError {
    fn from(e: std::io::Error) -> Self {
        SkillError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, SkillError>;
