//! Unified error handling for the SwissArmyHammer library
//!
//! This module provides a comprehensive error type hierarchy that replaces
//! ad-hoc error handling throughout the codebase with typed, structured errors.

use std::fmt;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

/// The main error type for the SwissArmyHammer library
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SwissArmyHammerError {
    /// IO operation failed
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Template parsing or rendering failed
    #[error("Template error: {0}")]
    Template(String),

    /// Prompt not found
    #[error("Prompt not found: {0}")]
    PromptNotFound(String),

    /// Invalid configuration
    #[error("Configuration error: {0}")]
    Config(String),

    /// Storage backend error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Workflow not found
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    /// Workflow run not found
    #[error("Workflow run not found: {0}")]
    WorkflowRunNotFound(String),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_yaml::Error),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// Issue not found
    #[error("Issue not found: {0}")]
    IssueNotFound(String),

    /// Issue already exists
    #[error("Issue already exists: {0}")]
    IssueAlreadyExists(u32),

    /// Git operation failed
    #[error("Git operation '{operation}' failed: {details}")]
    GitOperationFailed {
        /// The git operation that failed
        operation: String,
        /// Details about the failure
        details: String,
    },

    /// Git command failed with exit code
    #[error("Git command '{command}' failed with exit code {exit_code}: {stderr}")]
    GitCommandFailed {
        /// The git command that failed
        command: String,
        /// The exit code returned by the command
        exit_code: i32,
        /// Standard error output from the command
        stderr: String,
    },

    /// Git repository not found or not initialized
    #[error("Git repository not found or not initialized in path: {path}")]
    GitRepositoryNotFound {
        /// The path where git repository was expected
        path: String,
    },

    /// Git branch operation failed
    #[error("Git branch operation '{operation}' failed on branch '{branch}': {details}")]
    GitBranchOperationFailed {
        /// The branch operation that failed
        operation: String,
        /// The branch involved in the operation
        branch: String,
        /// Details about the failure
        details: String,
    },

    /// Memo not found
    #[error("Memo not found: {0}")]
    MemoNotFound(String),

    /// Invalid memo ID format
    #[error("Invalid memo ID: {0}")]
    InvalidMemoId(String),

    /// Memo already exists
    #[error("Memo already exists: {0}")]
    MemoAlreadyExists(String),

    /// Memo validation error
    #[error("Memo validation failed: {0}")]
    MemoValidationFailed(String),

    /// Semantic search error
    #[error("Semantic search error: {0}")]
    Semantic(#[from] crate::search::SemanticError),

    /// Workflow executor error
    #[error("Workflow executor error: {0}")]
    ExecutorError(#[from] crate::workflow::ExecutorError),

    /// Other errors
    #[error("{0}")]
    Other(String),

    /// Generic error with context
    #[error("{message}")]
    Context {
        /// The error message providing context
        message: String,
        #[source]
        /// The underlying error that caused this error
        source: Box<dyn std::error::Error + Send + Sync>,
    },
}

/// Workflow-specific errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum WorkflowError {
    /// Workflow not found
    #[error("Workflow '{name}' not found")]
    NotFound {
        /// The name of the workflow that was not found
        name: String,
    },

    /// Invalid workflow definition
    #[error("Invalid workflow '{name}': {reason}")]
    Invalid {
        /// The name of the invalid workflow
        name: String,
        /// The reason why the workflow is invalid
        reason: String,
    },

    /// Circular dependency detected
    #[error("Circular dependency detected: {cycle}")]
    CircularDependency {
        /// The string representation of the dependency cycle
        cycle: String,
    },

    /// State not found in workflow
    #[error("State '{state}' not found in workflow '{workflow}'")]
    StateNotFound {
        /// The state that was not found
        state: String,
        /// The workflow that should contain the state
        workflow: String,
    },

    /// Invalid transition
    #[error("Invalid transition from '{from}' to '{to}' in workflow '{workflow}'")]
    InvalidTransition {
        /// The source state of the invalid transition
        from: String,
        /// The target state of the invalid transition
        to: String,
        /// The workflow containing the invalid transition
        workflow: String,
    },

    /// Workflow execution error
    #[error("Workflow execution failed: {reason}")]
    ExecutionFailed {
        /// The reason why the workflow execution failed
        reason: String,
    },

    /// Timeout during workflow execution
    #[error("Workflow execution timed out after {duration:?}")]
    Timeout {
        /// The duration after which the workflow timed out
        duration: std::time::Duration,
    },
}

/// Action-specific errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ActionError {
    /// Action not found
    #[error("Action '{name}' not found")]
    NotFound {
        /// The name of the action that was not found
        name: String,
    },

    /// Invalid action configuration
    #[error("Invalid action configuration: {reason}")]
    InvalidConfig {
        /// The reason why the configuration is invalid
        reason: String,
    },

    /// Action execution failed
    #[error("Action '{name}' failed: {reason}")]
    ExecutionFailed {
        /// The name of the action that failed
        name: String,
        /// The reason why the action failed
        reason: String,
    },

    /// Variable not found in context
    #[error("Variable '{variable}' not found in context")]
    VariableNotFound {
        /// The name of the variable that was not found
        variable: String,
    },

    /// Invalid variable name
    #[error("Invalid variable name '{name}': {reason}")]
    InvalidVariableName {
        /// The invalid variable name
        name: String,
        /// The reason why the variable name is invalid
        reason: String,
    },

    /// Rate limit exceeded
    #[error("Rate limit exceeded: {message}. Retry after {retry_after:?}")]
    RateLimit {
        /// The rate limit error message
        message: String,
        /// The duration to wait before retrying
        retry_after: std::time::Duration,
    },

    /// External command failed
    #[error("External command failed: {command}")]
    CommandFailed {
        /// The command that failed
        command: String,
    },
}

/// Parsing errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ParseError {
    /// Invalid syntax
    #[error("Invalid syntax at line {line}, column {column}: {message}")]
    Syntax {
        /// The line number where the syntax error occurred
        line: usize,
        /// The column number where the syntax error occurred
        column: usize,
        /// The error message describing the syntax error
        message: String,
    },

    /// Missing required field
    #[error("Missing required field '{field}'")]
    MissingField {
        /// The name of the missing field
        field: String,
    },

    /// Invalid field value
    #[error("Invalid value for field '{field}': {reason}")]
    InvalidField {
        /// The name of the field with invalid value
        field: String,
        /// The reason why the field value is invalid
        reason: String,
    },

    /// Unsupported format
    #[error("Unsupported format: {format}")]
    UnsupportedFormat {
        /// The format that is not supported
        format: String,
    },
}

/// Validation errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ValidationError {
    /// Schema validation failed
    #[error("Schema validation failed: {reason}")]
    Schema {
        /// The reason why schema validation failed
        reason: String,
    },

    /// Content validation failed
    #[error("Content validation failed in {file}: {reason}")]
    Content {
        /// The file that failed content validation
        file: PathBuf,
        /// The reason why content validation failed
        reason: String,
    },

    /// Structure validation failed
    #[error("Structure validation failed: {reason}")]
    Structure {
        /// The reason why structure validation failed
        reason: String,
    },

    /// Security validation failed
    #[error("Security validation failed: {reason}")]
    Security {
        /// The reason why security validation failed
        reason: String,
    },
}

/// Storage-related errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StorageError {
    /// Storage not found
    #[error("Storage '{name}' not found")]
    NotFound {
        /// The name of the storage that was not found
        name: String,
    },

    /// Storage already exists
    #[error("Storage '{name}' already exists")]
    AlreadyExists {
        /// The name of the storage that already exists
        name: String,
    },

    /// Storage operation failed
    #[error("Storage operation failed: {reason}")]
    OperationFailed {
        /// The reason why the storage operation failed
        reason: String,
    },

    /// Invalid storage path
    #[error("Invalid storage path: {path}")]
    InvalidPath {
        /// The invalid storage path
        path: PathBuf,
    },
}

/// MCP (Model Context Protocol) errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum McpError {
    /// Connection failed
    #[error("MCP connection failed: {reason}")]
    ConnectionFailed {
        /// The reason why the connection failed
        reason: String,
    },

    /// Protocol error
    #[error("MCP protocol error: {reason}")]
    Protocol {
        /// The reason for the protocol error
        reason: String,
    },

    /// Tool execution failed
    #[error("MCP tool '{tool}' failed: {reason}")]
    ToolFailed {
        /// The name of the tool that failed
        tool: String,
        /// The reason why the tool failed
        reason: String,
    },

    /// Resource not found
    #[error("MCP resource '{resource}' not found")]
    ResourceNotFound {
        /// The name of the resource that was not found
        resource: String,
    },
}

/// Configuration errors
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// Missing configuration
    #[error("Missing configuration: {name}")]
    Missing {
        /// The name of the missing configuration
        name: String,
    },

    /// Invalid configuration
    #[error("Invalid configuration '{name}': {reason}")]
    Invalid {
        /// The name of the invalid configuration
        name: String,
        /// The reason why the configuration is invalid
        reason: String,
    },

    /// Environment variable error
    #[error("Environment variable '{var}' error: {reason}")]
    EnvVar {
        /// The name of the environment variable
        var: String,
        /// The reason for the environment variable error
        reason: String,
    },
}

/// Result type alias for SwissArmyHammer operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Extension trait for adding context to errors
pub trait ErrorContext<T> {
    /// Add context to an error
    fn context<S: Into<String>>(self, msg: S) -> Result<T>;

    /// Add context with a closure that's only called on error
    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>;
}

impl<T, E> ErrorContext<T> for std::result::Result<T, E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    fn context<S: Into<String>>(self, msg: S) -> Result<T> {
        self.map_err(|e| SwissArmyHammerError::Context {
            message: msg.into(),
            source: Box::new(e),
        })
    }

    fn with_context<F, S>(self, f: F) -> Result<T>
    where
        F: FnOnce() -> S,
        S: Into<String>,
    {
        self.map_err(|e| SwissArmyHammerError::Context {
            message: f().into(),
            source: Box::new(e),
        })
    }
}

/// Error chain formatter for detailed error reporting
pub struct ErrorChain<'a>(&'a dyn std::error::Error);

impl fmt::Display for ErrorChain<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "Error: {}", self.0)?;

        let mut current = self.0.source();
        let mut level = 1;

        while let Some(err) = current {
            writeln!(f, "{:indent$}Caused by: {}", "", err, indent = level * 2)?;
            current = err.source();
            level += 1;
        }

        Ok(())
    }
}

/// Helper functions for creating standardized error messages
impl SwissArmyHammerError {
    /// Create a git operation error with consistent formatting
    pub fn git_operation_failed(operation: &str, details: &str) -> Self {
        SwissArmyHammerError::GitOperationFailed {
            operation: operation.to_string(),
            details: details.to_string(),
        }
    }

    /// Create a git command error with consistent formatting
    pub fn git_command_failed(command: &str, exit_code: i32, stderr: &str) -> Self {
        SwissArmyHammerError::GitCommandFailed {
            command: command.to_string(),
            exit_code,
            stderr: stderr.to_string(),
        }
    }

    /// Create a git repository not found error
    pub fn git_repository_not_found(path: &str) -> Self {
        SwissArmyHammerError::GitRepositoryNotFound {
            path: path.to_string(),
        }
    }

    /// Create a git branch operation error
    pub fn git_branch_operation_failed(operation: &str, branch: &str, details: &str) -> Self {
        SwissArmyHammerError::GitBranchOperationFailed {
            operation: operation.to_string(),
            branch: branch.to_string(),
            details: details.to_string(),
        }
    }

    /// Create a file operation error with consistent formatting
    pub fn file_operation_failed(operation: &str, path: &str, details: &str) -> Self {
        SwissArmyHammerError::Other(format!(
            "File operation '{operation}' failed on '{path}': {details}"
        ))
    }

    /// Create a validation error with consistent formatting
    pub fn validation_failed(field: &str, value: &str, reason: &str) -> Self {
        SwissArmyHammerError::Other(format!(
            "Validation failed for {field}: '{value}' - {reason}"
        ))
    }

    /// Create a parsing error with consistent formatting
    pub fn parsing_failed(what: &str, input: &str, reason: &str) -> Self {
        SwissArmyHammerError::Other(format!("Failed to parse {what}: '{input}' - {reason}"))
    }

    /// Create a directory operation error with consistent formatting
    pub fn directory_operation_failed(operation: &str, path: &str, details: &str) -> Self {
        SwissArmyHammerError::Other(format!(
            "Directory operation '{operation}' failed on '{path}': {details}"
        ))
    }

    /// Create a memo not found error
    pub fn memo_not_found(memo_id: &str) -> Self {
        SwissArmyHammerError::MemoNotFound(memo_id.to_string())
    }

    /// Create an invalid memo ID error
    pub fn invalid_memo_id(memo_id: &str) -> Self {
        SwissArmyHammerError::InvalidMemoId(memo_id.to_string())
    }

    /// Create a memo already exists error
    pub fn memo_already_exists(memo_id: &str) -> Self {
        SwissArmyHammerError::MemoAlreadyExists(memo_id.to_string())
    }

    /// Create a memo validation error
    pub fn memo_validation_failed(reason: &str) -> Self {
        SwissArmyHammerError::MemoValidationFailed(reason.to_string())
    }
}

/// Extension trait for error types to format the full error chain
pub trait ErrorChainExt {
    /// Format the full error chain
    fn error_chain(&self) -> ErrorChain<'_>;
}

impl<E: std::error::Error> ErrorChainExt for E {
    fn error_chain(&self) -> ErrorChain<'_> {
        ErrorChain(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_context() {
        let err: Result<()> = Err(io::Error::new(io::ErrorKind::NotFound, "file not found").into());
        let err_with_context = err.context("Failed to open config file");

        assert!(err_with_context.is_err());
        let msg = err_with_context.unwrap_err().to_string();
        assert!(msg.contains("Failed to open config file"));
    }

    #[test]
    fn test_error_chain_display() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let err = SwissArmyHammerError::Context {
            message: "Failed to load workflow".to_string(),
            source: Box::new(io_err),
        };

        let chain = err.error_chain().to_string();
        assert!(chain.contains("Failed to load workflow"));
        assert!(chain.contains("file not found"));
    }
}
