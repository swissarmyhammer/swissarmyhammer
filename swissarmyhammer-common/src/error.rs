//! Error types for SwissArmyHammer Common
//!
//! This module provides structured error handling for common operations
//! throughout the SwissArmyHammer ecosystem. This includes core infrastructure
//! errors that are shared across all SwissArmyHammer crates.

use std::fmt;
use std::io;
use std::path::PathBuf;
use thiserror::Error as ThisError;

/// Severity levels for error classification
///
/// These levels help categorize errors by their impact and urgency, enabling
/// appropriate handling, logging, and user notification strategies.
///
/// # Severity Levels
///
/// - **Warning**: Potential issue but operation can proceed. The system continues
///   normally but alerts users to non-critical concerns.
/// - **Error**: Operation failed but the system can continue. The specific operation
///   cannot complete, but the system remains stable.
/// - **Critical**: System cannot continue, requires immediate attention. Indicates
///   severe problems that prevent continued operation.
///
/// # Examples
///
/// ```rust
/// use swissarmyhammer_common::ErrorSeverity;
///
/// // Warning: empty file is unusual but not fatal
/// let empty_file = ErrorSeverity::Warning;
///
/// // Error: file not found prevents this operation but system continues
/// let not_found = ErrorSeverity::Error;
///
/// // Critical: database corruption requires immediate action
/// let corruption = ErrorSeverity::Critical;
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorSeverity {
    /// Potential issue but operation can proceed
    ///
    /// Use for non-critical issues that should be noted but don't prevent
    /// successful operation.
    ///
    /// # Examples
    /// - Empty files that are expected to contain data
    /// - Deprecation notices
    /// - Non-critical configuration issues
    Warning,

    /// Operation failed but system can continue
    ///
    /// Use when a specific operation cannot complete but the system remains
    /// stable and can handle other operations.
    ///
    /// # Examples
    /// - File not found
    /// - Invalid format
    /// - Permission denied for non-critical resource
    Error,

    /// System cannot continue, requires immediate attention
    ///
    /// Use when the system encounters a problem that prevents continued
    /// operation or risks data integrity.
    ///
    /// # Examples
    /// - Database corruption
    /// - Workflow execution failures
    /// - Critical resource unavailable
    Critical,
}

/// Trait for error types that have severity levels
///
/// All SwissArmyHammer error types should implement this trait to provide
/// consistent severity reporting across the codebase. This enables:
///
/// - Consistent error classification across all SwissArmyHammer crates
/// - Appropriate logging levels based on severity
/// - Error filtering and handling strategies
/// - User-facing error presentation
///
/// # Severity Guidelines
///
/// When implementing this trait, follow these guidelines:
///
/// - **Warning**: Use for issues that don't prevent operation completion
///   - Empty files, deprecation notices, non-critical configuration issues
/// - **Error**: Use when a specific operation fails but the system continues
///   - File not found, invalid format, permission denied
/// - **Critical**: Use for system-level failures requiring immediate attention
///   - Database corruption, workflow failures, critical resource unavailable
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_common::{ErrorSeverity, Severity};
///
/// #[derive(Debug)]
/// enum MyError {
///     DatabaseCorrupted,
///     FileNotFound,
///     EmptyFile,
/// }
///
/// impl Severity for MyError {
///     fn severity(&self) -> ErrorSeverity {
///         match self {
///             MyError::DatabaseCorrupted => ErrorSeverity::Critical,
///             MyError::FileNotFound => ErrorSeverity::Error,
///             MyError::EmptyFile => ErrorSeverity::Warning,
///         }
///     }
/// }
///
/// let error = MyError::DatabaseCorrupted;
/// assert_eq!(error.severity(), ErrorSeverity::Critical);
/// ```
pub trait Severity {
    /// Get the severity level of this error
    ///
    /// This method should return the appropriate severity level based on
    /// the error variant. The severity should reflect the impact and urgency
    /// of the error condition.
    fn severity(&self) -> ErrorSeverity;
}

/// Result type alias for SwissArmyHammer operations
pub type Result<T> = std::result::Result<T, SwissArmyHammerError>;

/// Common error types for SwissArmyHammer operations
///
/// This enum contains core infrastructure errors that are shared across
/// the SwissArmyHammer ecosystem. Domain-specific errors should be defined
/// in their respective crates and converted to these common types as needed.
#[derive(Debug, ThisError)]
#[non_exhaustive]
pub enum SwissArmyHammerError {
    /// IO operation failed
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Serialization/deserialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_yaml::Error),

    /// Workflow not found
    #[error("Workflow not found: {0}")]
    WorkflowNotFound(String),

    /// Workflow run not found
    #[error("Workflow run not found: {0}")]
    WorkflowRunNotFound(String),

    /// Storage backend error
    #[error("Storage error: {0}")]
    Storage(String),

    /// JSON serialization/deserialization error
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

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

    /// Invalid path encountered
    #[error("Invalid path: {path}")]
    InvalidPath {
        /// The invalid path that caused the error
        path: PathBuf,
    },

    /// General I/O error with context
    #[error("I/O error: {message}")]
    IoContext {
        /// Descriptive message about the I/O error
        message: String,
    },

    /// Semantic search related error
    #[error("Semantic search error: {message}")]
    Semantic {
        /// Error message from semantic search operations
        message: String,
    },

    /// Generic error with context
    #[error("{message}")]
    Context {
        /// The error message providing context
        message: String,
        #[source]
        /// The underlying error that caused this error
        source: Box<dyn std::error::Error + Send + Sync>,
    },

    /// Rule violation detected during checking
    ///
    /// This variant is used when a rule violation is found during checking.
    /// The violation has already been logged by the rule checker at the appropriate
    /// level, so CLI/command layer should not log it again to avoid duplicate output.
    #[error("Rule violation: {0}")]
    RuleViolation(String),

    /// Other error with custom message
    #[error("{message}")]
    Other {
        /// Custom error message
        message: String,
    },
}

impl SwissArmyHammerError {
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

    /// Create a new invalid path error
    pub fn invalid_path(path: PathBuf) -> Self {
        Self::InvalidPath { path }
    }

    /// Create a new I/O error with context
    pub fn io_context(message: String) -> Self {
        Self::IoContext { message }
    }

    /// Create a new semantic search error
    pub fn semantic(message: String) -> Self {
        Self::Semantic { message }
    }

    /// Create a new other error
    pub fn other(message: String) -> Self {
        Self::Other { message }
    }

    /// Create a new rule violation error
    pub fn rule_violation(message: String) -> Self {
        Self::RuleViolation(message)
    }

    /// Check if this error is a rule violation
    pub fn is_rule_violation(&self) -> bool {
        matches!(self, SwissArmyHammerError::RuleViolation(_))
    }
}

/// Implementation of Severity trait for SwissArmyHammerError
///
/// This implementation categorizes all SwissArmyHammerError variants by their
/// severity level to enable appropriate error handling, logging, and user notification.
///
/// # Severity Assignment Guidelines
///
/// - **Critical**: System-level failures that prevent continued operation
///   - Repository not found (NotInGitRepository)
///   - Directory creation/access failures (DirectoryCreation, DirectoryAccess)
///   - Workflow system failures (WorkflowNotFound, WorkflowRunNotFound)
///   - Storage backend failures (Storage)
///   - Permission denied for critical resources (PermissionDenied)
///
/// - **Error**: Operation-specific failures that are recoverable
///   - I/O errors (Io, IoContext)
///   - Serialization failures (Serialization, Json)
///   - File operations (FileNotFound, NotAFile, InvalidFilePath, InvalidPath)
///   - Semantic search errors (Semantic)
///   - General errors with context (Context, Other)
///
/// - **Warning**: Non-critical issues that don't prevent operation
///   - Rule violations (RuleViolation)
impl Severity for SwissArmyHammerError {
    fn severity(&self) -> ErrorSeverity {
        match self {
            // Critical: System cannot continue, requires immediate attention
            SwissArmyHammerError::NotInGitRepository => ErrorSeverity::Critical,
            SwissArmyHammerError::DirectoryCreation(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::DirectoryAccess(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::WorkflowNotFound(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::WorkflowRunNotFound(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::Storage(_) => ErrorSeverity::Critical,
            SwissArmyHammerError::PermissionDenied { .. } => ErrorSeverity::Critical,

            // Error: Operation failed but system can continue
            SwissArmyHammerError::Io(_) => ErrorSeverity::Error,
            SwissArmyHammerError::Serialization(_) => ErrorSeverity::Error,
            SwissArmyHammerError::Json(_) => ErrorSeverity::Error,
            SwissArmyHammerError::FileNotFound { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::NotAFile { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::InvalidFilePath { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::InvalidPath { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::IoContext { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Semantic { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Context { .. } => ErrorSeverity::Error,
            SwissArmyHammerError::Other { .. } => ErrorSeverity::Error,

            // Warning: Non-critical issues
            SwissArmyHammerError::RuleViolation(_) => ErrorSeverity::Warning,
        }
    }
}

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
    fn test_rule_violation_error() {
        let error = SwissArmyHammerError::rule_violation("test violation".to_string());
        match error {
            SwissArmyHammerError::RuleViolation(ref msg) => {
                assert_eq!(msg, "test violation");
            }
            _ => panic!("Expected RuleViolation variant"),
        }
        assert!(error.to_string().contains("test violation"));
    }

    #[test]
    fn test_is_rule_violation() {
        let violation = SwissArmyHammerError::rule_violation("test".to_string());
        assert!(violation.is_rule_violation());

        let other = SwissArmyHammerError::other("test".to_string());
        assert!(!other.is_rule_violation());
    }

    #[test]
    fn test_error_severity_equality() {
        // Test that ErrorSeverity variants can be compared for equality
        assert_eq!(ErrorSeverity::Warning, ErrorSeverity::Warning);
        assert_eq!(ErrorSeverity::Error, ErrorSeverity::Error);
        assert_eq!(ErrorSeverity::Critical, ErrorSeverity::Critical);

        assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Error);
        assert_ne!(ErrorSeverity::Error, ErrorSeverity::Critical);
        assert_ne!(ErrorSeverity::Warning, ErrorSeverity::Critical);
    }

    #[test]
    fn test_severity_trait_implementation() {
        // Test enum that implements Severity trait
        #[derive(Debug)]
        enum TestError {
            CriticalFailure,
            NotFound,
            Deprecated,
        }

        impl Severity for TestError {
            fn severity(&self) -> ErrorSeverity {
                match self {
                    TestError::CriticalFailure => ErrorSeverity::Critical,
                    TestError::NotFound => ErrorSeverity::Error,
                    TestError::Deprecated => ErrorSeverity::Warning,
                }
            }
        }

        let critical = TestError::CriticalFailure;
        let error = TestError::NotFound;
        let warning = TestError::Deprecated;

        assert_eq!(critical.severity(), ErrorSeverity::Critical);
        assert_eq!(error.severity(), ErrorSeverity::Error);
        assert_eq!(warning.severity(), ErrorSeverity::Warning);
    }

    #[test]
    fn test_swissarmyhammer_error_critical_severity() {
        let errors = vec![
            SwissArmyHammerError::NotInGitRepository,
            SwissArmyHammerError::DirectoryCreation("test".to_string()),
            SwissArmyHammerError::DirectoryAccess("test".to_string()),
            SwissArmyHammerError::WorkflowNotFound("test".to_string()),
            SwissArmyHammerError::WorkflowRunNotFound("test".to_string()),
            SwissArmyHammerError::Storage("test".to_string()),
            SwissArmyHammerError::PermissionDenied {
                path: "test".to_string(),
                error: "denied".to_string(),
                suggestion: "check permissions".to_string(),
            },
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Critical,
                "Expected Critical severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_swissarmyhammer_error_error_severity() {
        use std::io;

        let errors: Vec<SwissArmyHammerError> = vec![
            SwissArmyHammerError::Io(io::Error::new(io::ErrorKind::NotFound, "test")),
            SwissArmyHammerError::FileNotFound {
                path: "test".to_string(),
                suggestion: "check path".to_string(),
            },
            SwissArmyHammerError::NotAFile {
                path: "test".to_string(),
                suggestion: "check path".to_string(),
            },
            SwissArmyHammerError::InvalidFilePath {
                path: "test".to_string(),
                suggestion: "fix path".to_string(),
            },
            SwissArmyHammerError::InvalidPath {
                path: PathBuf::from("test"),
            },
            SwissArmyHammerError::IoContext {
                message: "test".to_string(),
            },
            SwissArmyHammerError::Semantic {
                message: "test".to_string(),
            },
            SwissArmyHammerError::Other {
                message: "test".to_string(),
            },
        ];

        for error in errors {
            assert_eq!(
                error.severity(),
                ErrorSeverity::Error,
                "Expected Error severity for: {}",
                error
            );
        }
    }

    #[test]
    fn test_swissarmyhammer_error_warning_severity() {
        let error = SwissArmyHammerError::RuleViolation("test".to_string());
        assert_eq!(
            error.severity(),
            ErrorSeverity::Warning,
            "Expected Warning severity for: {}",
            error
        );
    }
}
