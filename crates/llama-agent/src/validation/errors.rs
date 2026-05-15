//! Validation error types

use llama_common::error::{ErrorCategory, LlamaError};

/// Validation errors that can occur during validation
#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub enum ValidationError {
    /// Security violation detected
    #[error("Security violation: {0}\n🔒 Review your input for potentially dangerous content and ensure it follows security guidelines")]
    SecurityViolation(String),

    /// Parameter is outside acceptable bounds
    #[error("Parameter out of bounds: {0}\n📏 Check parameter limits in the documentation and adjust your values accordingly")]
    ParameterBounds(String),

    /// Invalid state detected
    #[error("Invalid state: {0}\n⚠️ Ensure prerequisites are met and the operation is valid in the current context")]
    InvalidState(String),

    /// Content validation failed
    #[error("Content validation failed: {0}\n📝 Verify your content format, encoding, and structure meet the requirements")]
    ContentValidation(String),

    /// Schema validation failed
    #[error("Schema validation failed: {0}\n📋 Check that your data structure matches the expected schema format")]
    SchemaValidation(String),

    /// Multiple validation errors occurred
    #[error("Multiple validation errors:\n{}", .0.iter().enumerate().map(|(i, e)| format!("  {}. {}", i + 1, e.to_string().lines().next().unwrap_or(""))).collect::<Vec<_>>().join("\n"))]
    Multiple(Vec<ValidationError>),
}

impl ValidationError {
    /// Create a security violation error
    pub fn security_violation(msg: impl Into<String>) -> Self {
        Self::SecurityViolation(msg.into())
    }

    /// Create a parameter bounds error
    pub fn parameter_bounds(msg: impl Into<String>) -> Self {
        Self::ParameterBounds(msg.into())
    }

    /// Create an invalid state error
    pub fn invalid_state(msg: impl Into<String>) -> Self {
        Self::InvalidState(msg.into())
    }

    /// Create a content validation error
    pub fn content_validation(msg: impl Into<String>) -> Self {
        Self::ContentValidation(msg.into())
    }

    /// Create a schema validation error
    pub fn schema_validation(msg: impl Into<String>) -> Self {
        Self::SchemaValidation(msg.into())
    }

    /// Combine multiple validation errors
    pub fn multiple(errors: Vec<ValidationError>) -> Self {
        if errors.len() == 1 {
            errors.into_iter().next().unwrap()
        } else {
            Self::Multiple(errors)
        }
    }
}

impl LlamaError for ValidationError {
    fn category(&self) -> ErrorCategory {
        match self {
            ValidationError::SecurityViolation(_) => ErrorCategory::User,
            ValidationError::ParameterBounds(_) => ErrorCategory::User,
            ValidationError::InvalidState(_) => ErrorCategory::User,
            ValidationError::ContentValidation(_) => ErrorCategory::User,
            ValidationError::SchemaValidation(_) => ErrorCategory::User,
            ValidationError::Multiple(errors) => {
                // Use the most severe category from the multiple errors
                errors
                    .iter()
                    .map(|e| e.category())
                    .fold(ErrorCategory::User, |acc, cat| match (acc, cat) {
                        (ErrorCategory::Internal, _) | (_, ErrorCategory::Internal) => {
                            ErrorCategory::Internal
                        }
                        (ErrorCategory::System, _) | (_, ErrorCategory::System) => {
                            ErrorCategory::System
                        }
                        (ErrorCategory::External, _) | (_, ErrorCategory::External) => {
                            ErrorCategory::External
                        }
                        _ => ErrorCategory::User,
                    })
            }
        }
    }

    fn error_code(&self) -> &'static str {
        match self {
            ValidationError::SecurityViolation(_) => "VALIDATION_SECURITY",
            ValidationError::ParameterBounds(_) => "VALIDATION_BOUNDS",
            ValidationError::InvalidState(_) => "VALIDATION_STATE",
            ValidationError::ContentValidation(_) => "VALIDATION_CONTENT",
            ValidationError::SchemaValidation(_) => "VALIDATION_SCHEMA",
            ValidationError::Multiple(_) => "VALIDATION_MULTIPLE",
        }
    }

    fn user_friendly_message(&self) -> String {
        // The display implementation already includes user-friendly formatting with emojis
        format!("{}", self)
    }
}

/// Result type for validation operations
pub type ValidationResult<T = ()> = Result<T, ValidationError>;
