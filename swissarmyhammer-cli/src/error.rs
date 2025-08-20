//! Error handling for the SwissArmyHammer CLI
//!
//! This module provides a robust error handling approach that preserves
//! error context while still providing appropriate exit codes for CLI applications.

use std::error::Error;
use std::fmt;

use crate::exit_codes::{EXIT_ERROR, EXIT_SUCCESS, EXIT_WARNING};

/// CLI-specific result type that preserves error information
pub type CliResult<T> = Result<T, CliError>;

/// CLI error type that includes both error information and suggested exit code
#[derive(Debug)]
pub struct CliError {
    pub message: String,
    pub exit_code: i32,
    pub source: Option<Box<dyn Error + Send + Sync>>,
}

impl CliError {
    /// Create a new CLI error with a message and exit code
    pub fn new(message: impl Into<String>, exit_code: i32) -> Self {
        Self {
            message: message.into(),
            exit_code,
            source: None,
        }
    }

    /// Create a CLI error from a SwissArmyHammer error
    #[allow(dead_code)]
    pub fn from_swissarmyhammer_error(error: swissarmyhammer::SwissArmyHammerError) -> Self {
        let error_msg = error.to_string();
        Self {
            message: error_msg,
            exit_code: EXIT_ERROR,
            source: Some(Box::new(error)),
        }
    }

    /// Get the full error chain as a formatted string
    pub fn full_chain(&self) -> String {
        let mut result = self.message.clone();

        let mut current_source = self.source();
        while let Some(err) = current_source {
            result.push_str(&format!("\n  Caused by: {err}"));
            current_source = err.source();
        }

        result
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl Error for CliError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        self.source
            .as_ref()
            .map(|e| e.as_ref() as &(dyn Error + 'static))
    }
}

/// Convert a CliResult to an exit code, printing the full error chain if needed
pub fn handle_cli_result<T>(result: CliResult<T>) -> i32 {
    match result {
        Ok(_) => EXIT_SUCCESS,
        Err(e) => {
            tracing::error!("Error: {}", e.full_chain());
            e.exit_code
        }
    }
}

/// Centralized error message formatting functions for Git repository requirements
/// Format a generic Git repository requirement error message
fn format_git_repository_requirement_error() -> String {
    format!(
        "❌ Git repository required\n\n\
        SwissArmyHammer operations require a Git repository context.\n\
        \n\
        Solutions:\n\
        • Run this command from within a Git repository\n\
        • Initialize a Git repository: git init\n\
        • Clone an existing repository: git clone <url>\n\
        \n\
        Current directory: {}",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unable to determine>".to_string())
    )
}

/// Format directory creation error message
fn format_directory_creation_error(details: &str) -> String {
    format!(
        "❌ Failed to create .swissarmyhammer directory\n\n\
        Error: {details}\n\
        \n\
        SwissArmyHammer requires a .swissarmyhammer directory to store:\n\
        • Memos in .swissarmyhammer/memos/\n\
        • Todo lists in .swissarmyhammer/todo/\n\
        • Search index in .swissarmyhammer/semantic.db\n\
        • Workflow runs in .swissarmyhammer/runs/\n\
        \n\
        Solutions:\n\
        • Check directory permissions in current location\n\
        • Ensure you have write access to create directories\n\
        • Try running from a different directory with write permissions"
    )
}

/// Format directory access error message
fn format_directory_access_error(details: &str) -> String {
    format!(
        "❌ Git repository found but .swissarmyhammer directory is not accessible\n\n\
        Error: {details}\n\
        \n\
        The .swissarmyhammer directory exists but cannot be accessed.\n\
        \n\
        Solutions:\n\
        • Check directory permissions: ls -la .swissarmyhammer/\n\
        • Ensure read/write access: chmod 755 .swissarmyhammer/\n\
        • Verify the directory is not corrupted or locked\n\
        • Try running with appropriate permissions"
    )
}

/// Format Git repository not found error message
fn format_git_repository_not_found_error(path: &str) -> String {
    format!(
        "❌ Git repository not found\n\n\
        No Git repository found at: {path}\n\
        \n\
        SwissArmyHammer requires a Git repository context for:\n\
        • Issue tracking and branch management\n\
        • Workflow execution and state tracking\n\
        • File organization at repository root\n\
        \n\
        Solutions:\n\
        • Navigate to an existing Git repository\n\
        • Initialize a new Git repository: git init\n\
        • Clone an existing repository: git clone <url>"
    )
}

/// Format component-specific Git repository requirement error
pub fn format_component_specific_git_error(component: &str, explanation: &str) -> String {
    format!(
        "❌ {component} require a Git repository\n\n\
        {explanation}\n\
        \n\
        Solutions:\n\
        • Run this command from within a Git repository\n\
        • Initialize a Git repository: git init\n\
        • Clone an existing repository: git clone <url>\n\
        \n\
        Current directory: {}",
        std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| "<unable to determine>".to_string())
    )
}

/// Convert parameter errors to CLI errors with enhanced context
impl From<swissarmyhammer::common::parameters::ParameterError> for CliError {
    fn from(error: swissarmyhammer::common::parameters::ParameterError) -> Self {
        use swissarmyhammer::common::parameters::{ErrorMessageEnhancer, ParameterError};

        let enhancer = ErrorMessageEnhancer::new();
        let enhanced_error = enhancer.enhance_parameter_error(&error);

        let exit_code = match &enhanced_error {
            ParameterError::MaxAttemptsExceeded { .. } => EXIT_ERROR,
            ParameterError::ValidationFailedWithContext { recoverable, .. }
            | ParameterError::PatternMismatchEnhanced { recoverable, .. }
            | ParameterError::InvalidChoiceEnhanced { recoverable, .. } => {
                if *recoverable {
                    EXIT_WARNING
                } else {
                    EXIT_ERROR
                }
            }
            _ => EXIT_ERROR,
        };

        Self {
            message: format_enhanced_parameter_error(&enhanced_error),
            exit_code,
            source: Some(Box::new(error)),
        }
    }
}

/// Format enhanced parameter errors for CLI display
fn format_enhanced_parameter_error(
    error: &swissarmyhammer::common::parameters::ParameterError,
) -> String {
    use swissarmyhammer::common::parameters::ParameterError;

    match error {
        ParameterError::ValidationFailedWithContext {
            parameter, details, ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' validation failed: {}",
                parameter, details.message
            );

            if let Some(explanation) = &details.explanation {
                output.push_str(&format!("\n   {explanation}"));
            }

            if !details.examples.is_empty() {
                output.push_str(&format!("\n   Examples: {}", details.examples.join(", ")));
            }

            for suggestion in &details.suggestions {
                output.push_str(&format!("\n💡 {suggestion}"));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::PatternMismatchEnhanced {
            parameter, details, ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' format is invalid: '{}'",
                parameter, details.value
            );
            output.push_str(&format!("\n   {}", details.pattern_description));

            if !details.examples.is_empty() && details.examples.len() <= 3 {
                output.push_str(&format!("\n   Examples: {}", details.examples.join(", ")));
            } else if !details.examples.is_empty() {
                output.push_str(&format!(
                    "\n   Examples: {}",
                    details.examples[..2].join(", ")
                ));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::InvalidChoiceEnhanced {
            parameter, details, ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' has invalid value: '{}'",
                parameter, details.value
            );

            if let Some(suggestion) = &details.did_you_mean {
                output.push_str(&format!("\n💡 Did you mean '{suggestion}'?"));
            } else if details.choices.len() <= 5 {
                output.push_str(&format!(
                    "\n💡 Valid options: {}",
                    details.choices.join(", ")
                ));
            } else {
                output.push_str(&format!("\n💡 {} options available", details.choices.len()));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::MaxAttemptsExceeded {
            parameter,
            attempts,
        } => {
            format!("❌ Maximum retry attempts exceeded for parameter '{parameter}' ({attempts} attempts)\n\n📖 Use --help to see parameter requirements\n🔄 Check your input format and try again")
        }

        _ => {
            format!("❌ Workflow parameter error: {error}\n\n📖 For parameter details, run: sah <command> --help\n🔄 To fix this interactively, run: sah <command> --interactive")
        }
    }
}

/// Convert SwissArmyHammer errors to CLI errors with specific handling for Git repository requirements
impl From<swissarmyhammer::SwissArmyHammerError> for CliError {
    fn from(err: swissarmyhammer::SwissArmyHammerError) -> Self {
        match err {
            swissarmyhammer::SwissArmyHammerError::NotInGitRepository => CliError {
                message: format_git_repository_requirement_error(),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
            swissarmyhammer::SwissArmyHammerError::DirectoryCreation(ref details) => CliError {
                message: format_directory_creation_error(details),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
            swissarmyhammer::SwissArmyHammerError::DirectoryAccess(ref details) => CliError {
                message: format_directory_access_error(details),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
            swissarmyhammer::SwissArmyHammerError::GitRepositoryNotFound { ref path } => CliError {
                message: format_git_repository_not_found_error(path),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
            _ => CliError {
                message: err.to_string(),
                exit_code: EXIT_ERROR,
                source: Some(Box::new(err)),
            },
        }
    }
}

/// Convert MCP errors to CLI errors with appropriate exit codes
impl From<rmcp::Error> for CliError {
    fn from(error: rmcp::Error) -> Self {
        let error_msg = error.to_string();
        // Regular MCP error handling - use EXIT_WARNING for standard MCP errors
        Self {
            message: format!("MCP error: {error_msg}"),
            exit_code: EXIT_WARNING,
            source: Some(Box::new(error)),
        }
    }
}
