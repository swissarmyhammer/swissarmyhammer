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
