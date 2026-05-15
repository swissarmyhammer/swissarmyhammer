//! Error types for the agents crate

use std::fmt;

/// Errors that can occur during agent operations
#[derive(Debug)]
pub enum AgentError {
    /// Agent not found by name
    NotFound { name: String },
    /// Invalid agent definition
    InvalidAgent { message: String },
    /// Parse error
    Parse { message: String },
    /// I/O error
    Io(std::io::Error),
}

impl fmt::Display for AgentError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AgentError::NotFound { name } => write!(f, "agent not found: '{}'", name),
            AgentError::InvalidAgent { message } => write!(f, "invalid agent: {}", message),
            AgentError::Parse { message } => write!(f, "parse error: {}", message),
            AgentError::Io(e) => write!(f, "I/O error: {}", e),
        }
    }
}

impl std::error::Error for AgentError {}

impl From<std::io::Error> for AgentError {
    fn from(e: std::io::Error) -> Self {
        AgentError::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_display_not_found() {
        let err = AgentError::NotFound {
            name: "my-agent".to_string(),
        };
        assert_eq!(format!("{}", err), "agent not found: 'my-agent'");
    }

    #[test]
    fn test_display_invalid_agent() {
        let err = AgentError::InvalidAgent {
            message: "bad config".to_string(),
        };
        assert_eq!(format!("{}", err), "invalid agent: bad config");
    }

    #[test]
    fn test_display_parse() {
        let err = AgentError::Parse {
            message: "unexpected token".to_string(),
        };
        assert_eq!(format!("{}", err), "parse error: unexpected token");
    }

    #[test]
    fn test_display_io() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "file missing");
        let err = AgentError::Io(io_err);
        let display = format!("{}", err);
        assert!(display.starts_with("I/O error:"));
        assert!(display.contains("file missing"));
    }

    #[test]
    fn test_from_io_error() {
        let io_err = std::io::Error::new(std::io::ErrorKind::PermissionDenied, "access denied");
        let agent_err: AgentError = io_err.into();
        assert!(matches!(agent_err, AgentError::Io(_)));
        assert!(format!("{}", agent_err).contains("access denied"));
    }

    #[test]
    fn test_error_trait_is_implemented() {
        let err = AgentError::NotFound {
            name: "x".to_string(),
        };
        // Verify the Error trait is implemented by using it as a trait object
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_debug_format() {
        let err = AgentError::Parse {
            message: "test".to_string(),
        };
        let debug = format!("{:?}", err);
        assert!(debug.contains("Parse"));
        assert!(debug.contains("test"));
    }
}
