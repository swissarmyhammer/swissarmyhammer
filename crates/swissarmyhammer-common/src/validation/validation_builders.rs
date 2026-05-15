//! Validation builders and error construction utilities
//!
//! This module provides common patterns for building validation errors
//! and constructing consistent error messages.

use crate::SwissArmyHammerError;
use std::path::Path;

/// Builder for validation errors with consistent formatting
#[derive(Debug, Clone)]
pub struct ValidationErrorBuilder {
    context: Option<String>,
    field: Option<String>,
    value: Option<String>,
    reason: Option<String>,
    suggestions: Vec<String>,
}

impl ValidationErrorBuilder {
    /// Create a new validation error builder
    pub fn new() -> Self {
        Self {
            context: None,
            field: None,
            value: None,
            reason: None,
            suggestions: Vec::new(),
        }
    }

    /// Set the context where the validation failed
    pub fn context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    /// Set the field name that failed validation
    pub fn field(mut self, field: impl Into<String>) -> Self {
        self.field = Some(field.into());
        self
    }

    /// Set the value that failed validation
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set the reason for the validation failure
    pub fn reason(mut self, reason: impl Into<String>) -> Self {
        self.reason = Some(reason.into());
        self
    }

    /// Add a suggestion for fixing the validation error
    pub fn suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestions.push(suggestion.into());
        self
    }

    /// Add multiple suggestions
    pub fn suggestions(mut self, suggestions: Vec<impl Into<String>>) -> Self {
        self.suggestions
            .extend(suggestions.into_iter().map(|s| s.into()));
        self
    }

    /// Build the validation error
    pub fn build(self) -> SwissArmyHammerError {
        let mut message = String::new();

        if let Some(context) = self.context {
            message.push_str(&format!("Validation failed in {context}"));
        } else {
            message.push_str("Validation failed");
        }

        if let Some(field) = self.field {
            message.push_str(&format!(" for field '{field}'"));
        }

        if let Some(value) = self.value {
            message.push_str(&format!(" with value '{value}'"));
        }

        if let Some(reason) = self.reason {
            message.push_str(&format!(": {reason}"));
        } else {
            message.push_str(": validation constraint not met");
        }

        if !self.suggestions.is_empty() {
            message.push_str(". Suggestions: ");
            message.push_str(&self.suggestions.join(", "));
        }

        SwissArmyHammerError::Other { message }
    }
}

impl Default for ValidationErrorBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Quick validation error constructors for common patterns
pub mod quick {
    use super::*;

    /// Create a required field validation error
    pub fn required_field(field: &str) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .field(field)
            .reason("field is required but was not provided")
            .build()
    }

    /// Create an invalid value validation error
    pub fn invalid_value(field: &str, value: &str, reason: &str) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .field(field)
            .value(value)
            .reason(reason)
            .build()
    }

    /// Create a range validation error
    pub fn out_of_range<T: std::fmt::Display>(
        field: &str,
        value: T,
        min: T,
        max: T,
    ) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .field(field)
            .value(value.to_string())
            .reason(format!("value must be between {min} and {max} (inclusive)"))
            .build()
    }

    /// Create a format validation error
    pub fn invalid_format(field: &str, value: &str, expected_format: &str) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .field(field)
            .value(value)
            .reason(format!(
                "value does not match expected format: {expected_format}"
            ))
            .build()
    }

    /// Create a length validation error
    pub fn invalid_length(
        field: &str,
        actual_length: usize,
        min_length: Option<usize>,
        max_length: Option<usize>,
    ) -> SwissArmyHammerError {
        let reason = match (min_length, max_length) {
            (Some(min), Some(max)) => format!("length must be between {min} and {max}"),
            (Some(min), None) => format!("length must be at least {min}"),
            (None, Some(max)) => format!("length must be at most {max}"),
            (None, None) => "invalid length".to_string(),
        };

        ValidationErrorBuilder::new()
            .field(field)
            .value(actual_length.to_string())
            .reason(format!("{reason} (actual: {actual_length})"))
            .build()
    }

    /// Create a file validation error
    pub fn file_error<P: AsRef<Path>>(path: P, reason: &str) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .context("file validation")
            .field("path")
            .value(path.as_ref().display().to_string())
            .reason(reason)
            .build()
    }

    /// Create a duplicate validation error
    pub fn duplicate_value(field: &str, value: &str) -> SwissArmyHammerError {
        ValidationErrorBuilder::new()
            .field(field)
            .value(value)
            .reason("duplicate value found, must be unique")
            .build()
    }

    /// Create a dependency validation error
    pub fn missing_dependency(
        field: &str,
        dependency: &str,
        suggestion: Option<&str>,
    ) -> SwissArmyHammerError {
        let mut builder = ValidationErrorBuilder::new()
            .field(field)
            .reason(format!("requires '{dependency}' to be set"));

        if let Some(suggestion) = suggestion {
            builder = builder.suggestion(suggestion);
        }

        builder.build()
    }
}

/// Validation result helper type
pub type ValidationResult<T> = Result<T, SwissArmyHammerError>;

/// Chain multiple validation functions together
pub struct ValidationChain<T> {
    value: T,
    errors: Vec<SwissArmyHammerError>,
}

impl<T> ValidationChain<T> {
    /// Create a new validation chain with a value
    pub fn new(value: T) -> Self {
        Self {
            value,
            errors: Vec::new(),
        }
    }

    /// Add a validation function to the chain
    pub fn validate<F>(mut self, validator: F) -> Self
    where
        F: FnOnce(&T) -> ValidationResult<()>,
    {
        if let Err(error) = validator(&self.value) {
            self.errors.push(error);
        }
        self
    }

    /// Finish the validation chain
    pub fn finish(self) -> ValidationResult<T> {
        if self.errors.is_empty() {
            Ok(self.value)
        } else {
            // Combine all errors into a single error message
            let combined_message = self
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("; ");

            Err(SwissArmyHammerError::Other {
                message: format!("Multiple validation errors: {combined_message}"),
            })
        }
    }

    /// Get the current value (for inspection during chaining)
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Check if there are any validation errors so far
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validation_error_builder() {
        let error = ValidationErrorBuilder::new()
            .context("user input")
            .field("email")
            .value("invalid-email")
            .reason("must be a valid email address")
            .suggestion("use format: user@example.com")
            .build();

        let message = error.to_string();
        assert!(message.contains("Validation failed in user input"));
        assert!(message.contains("field 'email'"));
        assert!(message.contains("value 'invalid-email'"));
        assert!(message.contains("must be a valid email address"));
        assert!(message.contains("user@example.com"));
    }

    #[test]
    fn test_quick_validators() {
        let error = quick::required_field("name");
        assert!(error.to_string().contains("field 'name'"));
        assert!(error.to_string().contains("required"));

        let error = quick::invalid_value("age", "abc", "must be a number");
        assert!(error.to_string().contains("field 'age'"));
        assert!(error.to_string().contains("value 'abc'"));
        assert!(error.to_string().contains("must be a number"));

        let error = quick::out_of_range("score", 150, 0, 100);
        assert!(error.to_string().contains("field 'score'"));
        assert!(error.to_string().contains("value '150'"));
        assert!(error.to_string().contains("between 0 and 100"));
    }

    #[test]
    fn test_invalid_length_validator() {
        let error = quick::invalid_length("password", 3, Some(8), Some(20));
        assert!(error.to_string().contains("field 'password'"));
        assert!(error.to_string().contains("between 8 and 20"));
        assert!(error.to_string().contains("actual: 3"));

        let error = quick::invalid_length("comment", 500, None, Some(100));
        assert!(error.to_string().contains("at most 100"));

        let error = quick::invalid_length("title", 2, Some(5), None);
        assert!(error.to_string().contains("at least 5"));
    }

    #[test]
    fn test_file_error_validator() {
        let path = Path::new("/nonexistent/file.txt");
        let error = quick::file_error(path, "file does not exist");

        assert!(error.to_string().contains("file validation"));
        assert!(error.to_string().contains("/nonexistent/file.txt"));
        assert!(error.to_string().contains("does not exist"));
    }

    #[test]
    fn test_validation_chain_success() {
        let result = ValidationChain::new("test@example.com")
            .validate(|email| {
                if email.contains('@') {
                    Ok(())
                } else {
                    Err(quick::invalid_format("email", email, "user@domain.com"))
                }
            })
            .validate(|email| {
                if email.len() >= 5 {
                    Ok(())
                } else {
                    Err(quick::invalid_length("email", email.len(), Some(5), None))
                }
            })
            .finish();

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "test@example.com");
    }

    #[test]
    fn test_validation_chain_failure() {
        let result = ValidationChain::new("invalid")
            .validate(|email| {
                if email.contains('@') {
                    Ok(())
                } else {
                    Err(quick::invalid_format("email", email, "user@domain.com"))
                }
            })
            .validate(|email| {
                if email.len() >= 10 {
                    Ok(())
                } else {
                    Err(quick::invalid_length("email", email.len(), Some(10), None))
                }
            })
            .finish();

        assert!(result.is_err());
        let error = result.err().unwrap();
        let message = error.to_string();
        assert!(message.contains("Multiple validation errors"));
        assert!(message.contains("user@domain.com"));
        assert!(message.contains("at least 10"));
    }

    #[test]
    fn test_validation_chain_inspection() {
        let chain = ValidationChain::new(42).validate(|n| {
            if *n > 0 {
                Ok(())
            } else {
                Err(quick::invalid_value(
                    "number",
                    &n.to_string(),
                    "must be positive",
                ))
            }
        });

        assert_eq!(*chain.value(), 42);
        assert!(!chain.has_errors());

        let chain = chain.validate(|n| {
            if *n < 100 {
                Ok(())
            } else {
                Err(quick::out_of_range("number", *n, 0, 99))
            }
        });
        assert!(!chain.has_errors());

        let result = chain.finish();
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 42);
    }

    #[test]
    fn test_missing_dependency_validator() {
        let error = quick::missing_dependency(
            "database_url",
            "database_driver",
            Some("install the database driver first"),
        );

        let message = error.to_string();
        assert!(message.contains("field 'database_url'"));
        assert!(message.contains("requires 'database_driver'"));
        assert!(message.contains("install the database driver first"));
    }

    #[test]
    fn test_duplicate_value_validator() {
        let error = quick::duplicate_value("username", "admin");

        let message = error.to_string();
        assert!(message.contains("field 'username'"));
        assert!(message.contains("value 'admin'"));
        assert!(message.contains("duplicate value"));
        assert!(message.contains("must be unique"));
    }
}
