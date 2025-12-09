//! Base64 validation module
//!
//! This module provides shared base64 format validation logic to eliminate
//! duplication between `base64_processor` and `content_security_validator`.
//!
//! ## Design
//!
//! The module exports a single validation function that returns a Result type,
//! allowing callers to adapt errors to their own error types. This approach
//! maintains a single source of truth for base64 validation while preserving
//! the existing APIs of both calling modules.
//!
//! ## Usage
//!
//! **In `base64_processor`:** The validation errors are mapped to specific
//! `Base64ProcessorError` variants to maintain backward compatibility with
//! existing error messages.
//!
//! **In `content_security_validator`:** The validation errors are converted
//! to `ContentSecurityError::Base64SecurityViolation` with descriptive
//! error messages for security auditing.

use thiserror::Error;

#[derive(Debug, Error, Clone)]
pub enum Base64ValidationError {
    #[error("Empty base64 data")]
    EmptyData,
    #[error("Contains invalid characters")]
    InvalidCharacters,
    #[error("Invalid base64 padding")]
    InvalidPadding,
}

/// Validate base64 format
///
/// Checks that the data:
/// - Is not empty
/// - Contains only valid base64 characters (alphanumeric, +, /, =, and whitespace)
/// - Has valid padding (length is multiple of 4 after trimming)
///
/// # Examples
///
/// ```
/// use lib::base64_validation::validate_base64_format;
///
/// assert!(validate_base64_format("SGVsbG8gV29ybGQ=").is_ok());
/// assert!(validate_base64_format("").is_err());
/// assert!(validate_base64_format("Invalid!@#$").is_err());
/// ```
pub fn validate_base64_format(data: &str) -> Result<(), Base64ValidationError> {
    // Check for empty data
    if data.is_empty() {
        return Err(Base64ValidationError::EmptyData);
    }

    // Check for invalid characters
    if !data
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=' || c.is_whitespace())
    {
        return Err(Base64ValidationError::InvalidCharacters);
    }

    // Check basic base64 padding rules
    let trimmed = data.trim();
    if !trimmed.len().is_multiple_of(4) {
        return Err(Base64ValidationError::InvalidPadding);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_base64() {
        assert!(validate_base64_format("SGVsbG8gV29ybGQ=").is_ok());
        assert!(validate_base64_format("YQ==").is_ok());
        assert!(validate_base64_format("YWI=").is_ok());
        assert!(validate_base64_format("YWJj").is_ok());
    }

    #[test]
    fn test_empty_data() {
        assert!(matches!(
            validate_base64_format(""),
            Err(Base64ValidationError::EmptyData)
        ));
    }

    #[test]
    fn test_invalid_characters() {
        assert!(matches!(
            validate_base64_format("Invalid!@#$"),
            Err(Base64ValidationError::InvalidCharacters)
        ));
        assert!(matches!(
            validate_base64_format("abc*def"),
            Err(Base64ValidationError::InvalidCharacters)
        ));
        assert!(matches!(
            validate_base64_format("test<>data"),
            Err(Base64ValidationError::InvalidCharacters)
        ));
    }

    #[test]
    fn test_invalid_padding() {
        // Strings with lengths not divisible by 4 after trimming
        assert!(matches!(
            validate_base64_format("SGVsbG8"), // 7 chars
            Err(Base64ValidationError::InvalidPadding)
        ));
        assert!(matches!(
            validate_base64_format("YQ"), // 2 chars
            Err(Base64ValidationError::InvalidPadding)
        ));
        assert!(matches!(
            validate_base64_format("ABCDE"), // 5 chars
            Err(Base64ValidationError::InvalidPadding)
        ));
    }

    #[test]
    fn test_whitespace_handling() {
        // Base64 with whitespace should be valid (will be trimmed)
        assert!(validate_base64_format(" SGVsbG8gV29ybGQ= ").is_ok());
        assert!(validate_base64_format("\nSGVsbG8gV29ybGQ=\n").is_ok());
        assert!(validate_base64_format("\tYWJj\t").is_ok());
    }

    #[test]
    fn test_valid_chars_but_invalid_padding() {
        // All valid base64 chars but wrong length
        assert!(matches!(
            validate_base64_format("abc"),
            Err(Base64ValidationError::InvalidPadding)
        ));
        assert!(matches!(
            validate_base64_format("abcde"),
            Err(Base64ValidationError::InvalidPadding)
        ));
    }
}
