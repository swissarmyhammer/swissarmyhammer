//! Unified error handling for the SwissArmyHammer library
//!
//! This module provides a comprehensive error type hierarchy that replaces
//! ad-hoc error handling throughout the codebase with typed, structured errors.

use std::error::Error;
use std::fmt;
use std::io;
use std::path::PathBuf;
use thiserror::Error as ThisError;

/// The main error type for the SwissArmyHammer library
#[derive(Debug, ThisError)]
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

    /// File not found
    #[error("File not found: {path}\nSuggestion: {suggestion}")]
    FileNotFound {
        /// The file path that was not found
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Path is not a file (e.g., directory)
    #[error("Path is not a file: {path}\nSuggestion: {suggestion}")]
    NotAFile {
        /// The path that is not a file
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Permission denied when accessing file
    #[error("Permission denied accessing file: {path}\nError: {error}\nSuggestion: {suggestion}")]
    PermissionDenied {
        /// The file path that could not be accessed
        path: String,
        /// The underlying error message
        error: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Invalid file path format
    #[error("Invalid file path: {path}\nSuggestion: {suggestion}")]
    InvalidFilePath {
        /// The invalid file path
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// SwissArmyHammer must be run from within a Git repository
    #[error("SwissArmyHammer must be run from within a Git repository")]
    NotInGitRepository,
    
    /// Failed to create .swissarmyhammer directory
    #[error("Failed to create .swissarmyhammer directory: {0}")]
    DirectoryCreation(String),
    
    /// Git repository found but .swissarmyhammer directory is not accessible
    #[error("Git repository found but .swissarmyhammer directory is not accessible: {0}")]
    DirectoryAccess(String),

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

    /// Plan command specific error
    #[error("Plan command error: {0}")]
    PlanCommand(#[from] PlanCommandError),
}

/// Workflow-specific errors
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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
#[derive(Debug, ThisError)]
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

/// Plan command specific errors
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum PlanCommandError {
    /// Plan file not found
    #[error("Plan file not found: {path}")]
    FileNotFound {
        /// The file path that was not found
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Permission denied accessing plan file
    #[error("Permission denied accessing plan file: {path}")]
    PermissionDenied {
        /// The file path that could not be accessed
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Invalid plan file format
    #[error("Invalid plan file format: {path}\nReason: {reason}")]
    InvalidFileFormat {
        /// The file path with invalid format
        path: String,
        /// The reason why the file format is invalid
        reason: String,
    },

    /// Workflow execution failed for plan
    #[error("Workflow execution failed for plan: {plan_filename}")]
    WorkflowExecutionFailed {
        /// The plan filename that failed workflow execution
        plan_filename: String,
        #[source]
        /// The underlying workflow error
        source: WorkflowError,
    },

    /// Issue creation failed during planning
    #[error("Issue creation failed during planning")]
    IssueCreationFailed {
        /// The plan filename during which issue creation failed
        plan_filename: String,
        #[source]
        /// The underlying error that caused issue creation to fail
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Plan file is empty or contains no valid content
    #[error("Plan file is empty or contains no valid content: {path}")]
    EmptyPlanFile {
        /// The path of the empty plan file
        path: String,
    },

    /// Plan file too large to process
    #[error("Plan file too large to process: {path} ({size} bytes)")]
    FileTooLarge {
        /// The path of the oversized file
        path: String,
        /// The size of the file in bytes
        size: u64,
    },

    /// Issues directory is not writable
    #[error("Issues directory is not writable")]
    IssuesDirectoryNotWritable {
        /// The path of the issues directory
        path: String,
        #[source]
        /// The underlying IO error
        source: std::io::Error,
    },

    /// Specification file has insufficient content
    #[error("Specification file has insufficient content: {path} ({length} characters)")]
    InsufficientContent {
        /// The path of the specification file
        path: String,
        /// The length of the content in characters
        length: usize,
    },

    /// Specification file has no headers
    #[error("Specification file has no headers: {path}\nSuggestion: {suggestion}")]
    NoHeaders {
        /// The path of the specification file
        path: String,
        /// Suggestion for fixing the issue
        suggestion: String,
    },

    /// Specification file may not be suitable for planning
    #[error("Specification file may not be suitable for planning: {path}\nReason: {reason}")]
    UnsuitableForPlanning {
        /// The path of the specification file
        path: String,
        /// The reason why the file may not be suitable
        reason: String,
    },
}

/// Error severity levels for user-facing error messages
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ErrorSeverity {
    /// Warning level - non-critical issues
    Warning,
    /// Error level - significant problems that prevent operation
    Error,
    /// Critical level - severe failures requiring immediate attention
    Critical,
}

impl PlanCommandError {
    /// Provide user-friendly guidance for resolving the error
    pub fn user_guidance(&self) -> String {
        match self {
            PlanCommandError::FileNotFound { path, .. } => {
                format!(
                    "The plan file '{}' was not found.\n\
                    \n\
                    Suggestions:\n\
                    • Check the file path for typos\n\
                    • Ensure the file exists: ls -la '{}'\n\
                    • Try using an absolute path: swissarmyhammer plan /full/path/to/{}\n\
                    • Create the file if it doesn't exist",
                    path,
                    path,
                    path.split('/').next_back().unwrap_or(path)
                )
            }
            PlanCommandError::PermissionDenied { path, .. } => {
                format!(
                    "Permission denied when trying to read '{path}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check file permissions: ls -la '{path}'\n\
                    • Ensure you have read access: chmod +r '{path}'\n\
                    • Try running with appropriate permissions"
                )
            }
            PlanCommandError::InvalidFileFormat { path, reason } => {
                // Provide different suggestions based on the specific issue
                let suggestions = if reason.contains("directory") {
                    "• Provide a file path, not a directory path\n\
                    • Example: instead of './plans/', use './plans/feature.md'\n\
                    • Use 'ls' to see available files in the directory"
                } else if reason.contains("null bytes") || reason.contains("binary") {
                    "• Ensure the file is a text file, not a binary file\n\
                    • Check the file extension is .md, .txt, or similar\n\
                    • Verify the file wasn't corrupted during transfer"
                } else {
                    "• Ensure the file is a valid markdown file\n\
                    • Check for proper UTF-8 encoding\n\
                    • Verify the file isn't corrupted"
                };

                format!(
                    "The plan file '{path}' has an invalid format.\n\
                    Reason: {reason}\n\
                    \n\
                    Suggestions:\n\
                    {suggestions}"
                )
            }
            PlanCommandError::WorkflowExecutionFailed { plan_filename, .. } => {
                format!(
                    "Failed to execute planning workflow for '{plan_filename}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check that the plan file contains valid content\n\
                    • Ensure the issues directory is writable\n\
                    • Try running with --debug for more details\n\
                    • Check system resources and permissions"
                )
            }
            PlanCommandError::EmptyPlanFile { path } => {
                format!(
                    "The plan file '{path}' is empty or contains no valid content.\n\
                    \n\
                    Suggestions:\n\
                    • Add content to the plan file\n\
                    • Ensure the file isn't just whitespace\n\
                    • Check that the file saved properly"
                )
            }
            PlanCommandError::FileTooLarge { path, size } => {
                format!(
                    "The plan file '{path}' is too large ({size} bytes).\n\
                    \n\
                    Suggestions:\n\
                    • Break large plans into smaller, focused files\n\
                    • Remove unnecessary content from the plan\n\
                    • Consider splitting into multiple planning sessions"
                )
            }
            PlanCommandError::IssuesDirectoryNotWritable { path, .. } => {
                format!(
                    "Cannot write to issues directory: '{path}'.\n\
                    \n\
                    Suggestions:\n\
                    • Check directory permissions: ls -la '{path}'\n\
                    • Ensure you have write access: chmod +w '{path}'\n\
                    • Create the directory if it doesn't exist: mkdir -p '{path}'"
                )
            }
            PlanCommandError::IssueCreationFailed { plan_filename, .. } => {
                format!(
                    "Failed to create issue files for plan '{plan_filename}'.\n\
                    \n\
                    Suggestions:\n\
                    • Ensure the issues directory exists and is writable\n\
                    • Check available disk space\n\
                    • Verify no conflicting files exist\n\
                    • Try running with --debug for more details"
                )
            }
            PlanCommandError::InsufficientContent { path, length } => {
                format!(
                    "The specification file '{path}' is too short ({length} characters) to be a meaningful specification.\n\
                    \n\
                    Suggestions:\n\
                    • Add more detail to your specification\n\
                    • Include sections like overview, requirements, or goals\n\
                    • Provide context and background information\n\
                    • Consider what information would help someone implement this plan"
                )
            }
            PlanCommandError::NoHeaders { path, suggestion } => {
                format!(
                    "The specification file '{path}' lacks markdown structure.\n\
                    \n\
                    Suggestions:\n\
                    • {suggestion}\n\
                    • Use # for main sections, ## for subsections\n\
                    • Example structure: # Overview, ## Requirements, ## Implementation\n\
                    • Well-structured documents are easier to process and understand"
                )
            }
            PlanCommandError::UnsuitableForPlanning { path, reason } => {
                format!(
                    "The specification file '{path}' may not work well with the planning workflow.\n\
                    Reason: {reason}\n\
                    \n\
                    Suggestions:\n\
                    • Consider adding an overview or goal section\n\
                    • Include requirements or feature descriptions\n\
                    • Add background context for better planning\n\
                    • Review the specification format guidelines"
                )
            }
        }
    }

    /// Get the error severity level
    pub fn severity(&self) -> ErrorSeverity {
        match self {
            PlanCommandError::FileNotFound { .. } => ErrorSeverity::Error,
            PlanCommandError::PermissionDenied { .. } => ErrorSeverity::Error,
            PlanCommandError::InvalidFileFormat { .. } => ErrorSeverity::Error,
            PlanCommandError::WorkflowExecutionFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::IssueCreationFailed { .. } => ErrorSeverity::Critical,
            PlanCommandError::EmptyPlanFile { .. } => ErrorSeverity::Warning,
            PlanCommandError::FileTooLarge { .. } => ErrorSeverity::Error,
            PlanCommandError::IssuesDirectoryNotWritable { .. } => ErrorSeverity::Error,
            PlanCommandError::InsufficientContent { .. } => ErrorSeverity::Warning,
            PlanCommandError::NoHeaders { .. } => ErrorSeverity::Warning,
            PlanCommandError::UnsuitableForPlanning { .. } => ErrorSeverity::Warning,
        }
    }

    /// Display error with appropriate formatting for CLI
    pub fn display_to_user(&self, use_color: bool) -> String {
        let error_prefix = if use_color {
            match self.severity() {
                ErrorSeverity::Warning => "\x1b[33mWarning:\x1b[0m", // Yellow "Warning:"
                ErrorSeverity::Error => "\x1b[31mError:\x1b[0m",     // Red "Error:"
                ErrorSeverity::Critical => "\x1b[91mCritical:\x1b[0m", // Bright red "Critical:"
            }
        } else {
            match self.severity() {
                ErrorSeverity::Warning => "Warning:",
                ErrorSeverity::Error => "Error:",
                ErrorSeverity::Critical => "Critical:",
            }
        };

        let guidance = self.user_guidance();

        format!("{error_prefix} {self}\n\n{guidance}")
    }

    /// Log error with appropriate level
    pub fn log_error(&self) {
        match self.severity() {
            ErrorSeverity::Warning => tracing::warn!("{}", self),
            ErrorSeverity::Error => tracing::error!("{}", self),
            ErrorSeverity::Critical => tracing::error!("CRITICAL: {}", self),
        }

        // Log source chain for debugging
        let mut source = self.source();
        while let Some(err) = source {
            tracing::debug!("Caused by: {}", err);
            source = err.source();
        }
    }
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

    /// Create a file not found error with suggestion
    pub fn file_not_found(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::FileNotFound {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a not a file error (for directories) with suggestion
    pub fn not_a_file(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::NotAFile {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a permission denied error with suggestion
    pub fn permission_denied(path: &str, error: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::PermissionDenied {
            path: path.to_string(),
            error: error.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create an invalid file path error with suggestion
    pub fn invalid_file_path(path: &str, suggestion: &str) -> Self {
        SwissArmyHammerError::InvalidFilePath {
            path: path.to_string(),
            suggestion: suggestion.to_string(),
        }
    }

    /// Create a directory creation error
    pub fn directory_creation(error: std::io::Error) -> Self {
        SwissArmyHammerError::DirectoryCreation(error.to_string())
    }

    /// Create a directory access error
    pub fn directory_access(details: &str) -> Self {
        SwissArmyHammerError::DirectoryAccess(details.to_string())
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

    #[test]
    fn test_plan_command_error_file_not_found() {
        let error = PlanCommandError::FileNotFound {
            path: "test.md".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "No such file or directory"),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Plan file not found: test.md"));

        // Test severity
        assert_eq!(error.severity(), ErrorSeverity::Error);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("test.md"));
        assert!(guidance.contains("Suggestions:"));
        assert!(guidance.contains("Check the file path for typos"));
    }

    #[test]
    fn test_plan_command_error_permission_denied() {
        let error = PlanCommandError::PermissionDenied {
            path: "/restricted/test.md".to_string(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "Permission denied"),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Permission denied accessing plan file: /restricted/test.md"));

        // Test severity
        assert_eq!(error.severity(), ErrorSeverity::Error);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("Permission denied"));
        assert!(guidance.contains("chmod +r"));
    }

    #[test]
    fn test_plan_command_error_empty_file() {
        let error = PlanCommandError::EmptyPlanFile {
            path: "empty.md".to_string(),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Plan file is empty or contains no valid content: empty.md"));

        // Test severity - this should be a warning, not an error
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("Add content to the plan file"));
        assert!(guidance.contains("Ensure the file isn't just whitespace"));
    }

    #[test]
    fn test_plan_command_error_too_large() {
        let error = PlanCommandError::FileTooLarge {
            path: "huge.md".to_string(),
            size: 50_000_000,
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Plan file too large to process: huge.md (50000000 bytes)"));

        // Test severity
        assert_eq!(error.severity(), ErrorSeverity::Error);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("Break large plans into smaller"));
        assert!(guidance.contains("multiple planning sessions"));
    }

    #[test]
    fn test_plan_command_error_workflow_failed() {
        let workflow_error = WorkflowError::ExecutionFailed {
            reason: "State not found".to_string(),
        };
        let error = PlanCommandError::WorkflowExecutionFailed {
            plan_filename: "test.md".to_string(),
            source: workflow_error,
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Workflow execution failed for plan: test.md"));

        // Test severity - workflow failures are critical
        assert_eq!(error.severity(), ErrorSeverity::Critical);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("Failed to execute planning workflow"));
        assert!(guidance.contains("--debug for more details"));
    }

    #[test]
    fn test_plan_command_error_display_with_color() {
        let error = PlanCommandError::FileNotFound {
            path: "test.md".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "No such file or directory"),
        };

        // Test with color
        let display_color = error.display_to_user(true);
        assert!(display_color.contains("\x1b[31mError:\x1b[0m")); // Red "Error:"
        assert!(display_color.contains("Plan file not found"));
        assert!(display_color.contains("Suggestions:"));

        // Test without color
        let display_no_color = error.display_to_user(false);
        assert!(display_no_color.contains("Error:"));
        assert!(!display_no_color.contains("\x1b[")); // No escape sequences
        assert!(display_no_color.contains("Plan file not found"));
    }

    #[test]
    fn test_plan_command_error_display_warning_color() {
        let error = PlanCommandError::EmptyPlanFile {
            path: "empty.md".to_string(),
        };

        // Test warning with color
        let display_color = error.display_to_user(true);
        assert!(display_color.contains("\x1b[33mWarning:\x1b[0m")); // Yellow "Warning:"

        // Test warning without color
        let display_no_color = error.display_to_user(false);
        assert!(display_no_color.contains("Warning:"));
        assert!(!display_no_color.contains("\x1b["));
    }

    #[test]
    fn test_plan_command_error_display_critical_color() {
        let workflow_error = WorkflowError::ExecutionFailed {
            reason: "Critical failure".to_string(),
        };
        let error = PlanCommandError::WorkflowExecutionFailed {
            plan_filename: "test.md".to_string(),
            source: workflow_error,
        };

        // Test critical with color
        let display_color = error.display_to_user(true);
        assert!(display_color.contains("\x1b[91mCritical:\x1b[0m")); // Bright red "Critical:"

        // Test critical without color
        let display_no_color = error.display_to_user(false);
        assert!(display_no_color.contains("Critical:"));
        assert!(!display_no_color.contains("\x1b["));
    }

    #[test]
    fn test_plan_command_error_invalid_format() {
        let error = PlanCommandError::InvalidFileFormat {
            path: "binary.md".to_string(),
            reason: "Contains null bytes".to_string(),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Invalid plan file format: binary.md"));
        assert!(msg.contains("Contains null bytes"));

        // Test user guidance - should show binary file suggestions for null bytes
        let guidance = error.user_guidance();
        assert!(guidance.contains("text file, not a binary file"));
        assert!(guidance.contains("file extension"));
        assert!(guidance.contains("corrupted during transfer"));
    }

    #[test]
    fn test_plan_command_error_issues_directory() {
        let error = PlanCommandError::IssuesDirectoryNotWritable {
            path: "./issues".to_string(),
            source: io::Error::new(io::ErrorKind::PermissionDenied, "Access denied"),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Issues directory is not writable"));

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("mkdir -p"));
        assert!(guidance.contains("chmod +w"));
        assert!(guidance.contains("./issues"));
    }

    #[test]
    fn test_error_severity_levels() {
        // Test Warning severity
        let warning_error = PlanCommandError::EmptyPlanFile {
            path: "test.md".to_string(),
        };
        assert_eq!(warning_error.severity(), ErrorSeverity::Warning);

        // Test Error severity
        let error = PlanCommandError::FileNotFound {
            path: "test.md".to_string(),
            source: io::Error::new(io::ErrorKind::NotFound, "Not found"),
        };
        assert_eq!(error.severity(), ErrorSeverity::Error);

        // Test Critical severity
        let critical_error = PlanCommandError::WorkflowExecutionFailed {
            plan_filename: "test.md".to_string(),
            source: WorkflowError::ExecutionFailed {
                reason: "Critical failure".to_string(),
            },
        };
        assert_eq!(critical_error.severity(), ErrorSeverity::Critical);
    }

    #[test]
    fn test_error_severity_equality() {
        assert_eq!(ErrorSeverity::Warning, ErrorSeverity::Warning);
        assert_eq!(ErrorSeverity::Error, ErrorSeverity::Error);
        assert_eq!(ErrorSeverity::Critical, ErrorSeverity::Critical);
        assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Error);
        assert_ne!(ErrorSeverity::Error, ErrorSeverity::Critical);
    }

    #[test]
    fn test_plan_command_error_insufficient_content() {
        let error = PlanCommandError::InsufficientContent {
            path: "short.md".to_string(),
            length: 25,
        };

        // Test error message
        let msg = error.to_string();
        assert!(
            msg.contains("Specification file has insufficient content: short.md (25 characters)")
        );

        // Test severity - should be warning to allow processing with feedback
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("too short (25 characters)"));
        assert!(guidance.contains("Add more detail to your specification"));
        assert!(guidance.contains("Include sections like overview"));
    }

    #[test]
    fn test_plan_command_error_no_headers() {
        let error = PlanCommandError::NoHeaders {
            path: "no-headers.md".to_string(),
            suggestion: "Add markdown headers (# ## ###) to structure your specification"
                .to_string(),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Specification file has no headers: no-headers.md"));
        assert!(msg.contains("Add markdown headers"));

        // Test severity - should be warning
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("lacks markdown structure"));
        assert!(guidance.contains("Use # for main sections"));
        assert!(guidance.contains("## for subsections"));
    }

    #[test]
    fn test_plan_command_error_unsuitable_for_planning() {
        let error = PlanCommandError::UnsuitableForPlanning {
            path: "unclear.md".to_string(),
            reason: "Missing overview and requirements sections".to_string(),
        };

        // Test error message
        let msg = error.to_string();
        assert!(msg.contains("Specification file may not be suitable for planning: unclear.md"));
        assert!(msg.contains("Missing overview and requirements sections"));

        // Test severity - should be warning
        assert_eq!(error.severity(), ErrorSeverity::Warning);

        // Test user guidance
        let guidance = error.user_guidance();
        assert!(guidance.contains("may not work well with the planning workflow"));
        assert!(guidance.contains("Consider adding an overview or goal section"));
        assert!(guidance.contains("Include requirements or feature descriptions"));
    }

    #[test]
    fn test_plan_command_error_new_types_display_with_color() {
        let error = PlanCommandError::InsufficientContent {
            path: "test.md".to_string(),
            length: 50,
        };

        // Test with color - should show warning color
        let display_color = error.display_to_user(true);
        assert!(display_color.contains("\x1b[33mWarning:\x1b[0m")); // Yellow "Warning:"
        assert!(display_color.contains("insufficient content"));

        // Test without color
        let display_no_color = error.display_to_user(false);
        assert!(display_no_color.contains("Warning:"));
        assert!(!display_no_color.contains("\x1b[")); // No escape sequences
    }

    #[test]
    fn test_plan_command_error_severity_consistency() {
        // Test that all new specification validation errors are warnings
        let insufficient_content = PlanCommandError::InsufficientContent {
            path: "test.md".to_string(),
            length: 10,
        };
        assert_eq!(insufficient_content.severity(), ErrorSeverity::Warning);

        let no_headers = PlanCommandError::NoHeaders {
            path: "test.md".to_string(),
            suggestion: "Add headers".to_string(),
        };
        assert_eq!(no_headers.severity(), ErrorSeverity::Warning);

        let unsuitable = PlanCommandError::UnsuitableForPlanning {
            path: "test.md".to_string(),
            reason: "Test reason".to_string(),
        };
        assert_eq!(unsuitable.severity(), ErrorSeverity::Warning);
    }
}
