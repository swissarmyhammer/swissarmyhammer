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
            parameter,
            message,
            explanation,
            examples,
            suggestions,
            ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' validation failed: {}",
                parameter, message
            );

            if let Some(explanation) = explanation {
                output.push_str(&format!("\n   {}", explanation));
            }

            if !examples.is_empty() {
                output.push_str(&format!("\n   Examples: {}", examples.join(", ")));
            }

            for suggestion in suggestions {
                output.push_str(&format!("\n💡 {}", suggestion));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::PatternMismatchEnhanced {
            parameter,
            value,
            pattern_description,
            examples,
            ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' format is invalid: '{}'",
                parameter, value
            );
            output.push_str(&format!("\n   {}", pattern_description));

            if !examples.is_empty() && examples.len() <= 3 {
                output.push_str(&format!("\n   Examples: {}", examples.join(", ")));
            } else if !examples.is_empty() {
                output.push_str(&format!("\n   Examples: {}", examples[..2].join(", ")));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::InvalidChoiceEnhanced {
            parameter,
            value,
            choices,
            did_you_mean,
            ..
        } => {
            let mut output = format!(
                "❌ Parameter '{}' has invalid value: '{}'",
                parameter, value
            );

            if let Some(suggestion) = did_you_mean {
                output.push_str(&format!("\n💡 Did you mean '{}'?", suggestion));
            } else if choices.len() <= 5 {
                output.push_str(&format!("\n💡 Valid options: {}", choices.join(", ")));
            } else {
                output.push_str(&format!("\n💡 {} options available", choices.len()));
            }

            output.push_str("\n\n📖 For parameter details, run: sah <command> --help");
            output.push_str("\n🔄 To fix this interactively, run: sah <command> --interactive");

            output
        }

        ParameterError::MaxAttemptsExceeded {
            parameter,
            attempts,
        } => {
            format!("❌ Maximum retry attempts exceeded for parameter '{}' ({} attempts)\n\n📖 Use --help to see parameter requirements\n🔄 Check your input format and try again",
                   parameter, attempts)
        }

        _ => {
            format!("❌ Workflow parameter error: {}\n\n📖 For parameter details, run: sah <command> --help\n🔄 To fix this interactively, run: sah <command> --interactive",
                   error)
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
