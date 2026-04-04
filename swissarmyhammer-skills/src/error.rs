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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_found() {
        let err = SkillError::NotFound {
            name: "my-skill".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("skill not found"));
        assert!(msg.contains("my-skill"));
    }

    #[test]
    fn test_display_invalid_skill() {
        let err = SkillError::InvalidSkill {
            message: "bad frontmatter".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("invalid skill"));
        assert!(msg.contains("bad frontmatter"));
    }

    #[test]
    fn test_display_parse() {
        let err = SkillError::Parse {
            message: "unexpected token".to_string(),
        };
        let msg = format!("{}", err);
        assert!(msg.contains("parse error"));
        assert!(msg.contains("unexpected token"));
    }

    #[test]
    fn test_display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = SkillError::Io(io_err);
        let msg = format!("{}", err);
        assert!(msg.contains("I/O error"));
        assert!(msg.contains("file missing"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let err: SkillError = io_err.into();
        match &err {
            SkillError::Io(e) => assert_eq!(e.kind(), std::io::ErrorKind::PermissionDenied),
            other => panic!("expected Io variant, got {:?}", other),
        }
    }

    #[test]
    fn test_error_trait_impl() {
        let err = SkillError::NotFound {
            name: "x".to_string(),
        };
        // Verify it implements std::error::Error (source returns None for our variants)
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_debug_format() {
        let err = SkillError::NotFound {
            name: "test".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotFound"));
    }
}
