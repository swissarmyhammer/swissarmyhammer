use std::io;
use thiserror::Error;

/// Comprehensive error type for sah.toml configuration operations
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// I/O error when reading configuration files
    #[error("Failed to read configuration file: {0}")]
    Io(#[from] io::Error),

    /// TOML parsing error with detailed context
    #[error("TOML parse error at line {line}, column {column}: {message}")]
    TomlParse {
        line: usize,
        column: usize,
        message: String,
    },

    /// Generic TOML parsing error when line/column info is not available
    #[error("TOML parse error: {0}")]
    TomlParseGeneric(#[from] toml::de::Error),

    /// File size validation error
    #[error("Configuration file is too large: {size} bytes (maximum: {max_size} bytes)")]
    FileTooLarge { size: u64, max_size: u64 },

    /// Nesting depth validation error
    #[error("Configuration nesting depth too deep: {depth} levels (maximum: {max_depth} levels)")]
    NestingTooDeep { depth: usize, max_depth: usize },

    /// UTF-8 encoding validation error
    #[error("Configuration file is not valid UTF-8: {0}")]
    InvalidUtf8(#[from] std::string::FromUtf8Error),

    /// Environment variable substitution error
    #[error("Environment variable substitution failed for variable '{variable}': {reason}")]
    EnvVarSubstitution { variable: String, reason: String },

    /// Invalid variable name error
    #[error("Invalid variable name '{name}': {reason}")]
    InvalidVariableName { name: String, reason: String },

    /// Reserved variable name error
    #[error("Variable name '{name}' is reserved and cannot be used")]
    ReservedVariableName { name: String },

    /// String value too large error
    #[error("String value too large: {size} bytes (maximum: {max_size} bytes)")]
    StringTooLarge { size: usize, max_size: usize },

    /// Array too large error
    #[error("Array too large: {size} elements (maximum: {max_size} elements)")]
    ArrayTooLarge { size: usize, max_size: usize },

    /// Type coercion error
    #[error("Cannot coerce value of type {from_type} to {to_type}")]
    TypeCoercion { from_type: String, to_type: String },

    /// Circular reference error
    #[error("Circular reference detected in configuration")]
    CircularReference,

    /// Validation error with context
    #[error("Validation error: {message}")]
    Validation { message: String },
}

impl ConfigError {
    /// Create a TOML parse error with line and column information
    pub fn toml_parse(line: usize, column: usize, message: String) -> Self {
        Self::TomlParse {
            line,
            column,
            message,
        }
    }

    /// Create a file too large error
    pub fn file_too_large(size: u64, max_size: u64) -> Self {
        Self::FileTooLarge { size, max_size }
    }

    /// Create a nesting too deep error
    pub fn nesting_too_deep(depth: usize, max_depth: usize) -> Self {
        Self::NestingTooDeep { depth, max_depth }
    }

    /// Create an environment variable substitution error
    pub fn env_var_substitution(variable: String, reason: String) -> Self {
        Self::EnvVarSubstitution { variable, reason }
    }

    /// Create an invalid variable name error
    pub fn invalid_variable_name(name: String, reason: String) -> Self {
        Self::InvalidVariableName { name, reason }
    }

    /// Create a reserved variable name error
    pub fn reserved_variable_name(name: String) -> Self {
        Self::ReservedVariableName { name }
    }

    /// Create a string too large error
    pub fn string_too_large(size: usize, max_size: usize) -> Self {
        Self::StringTooLarge { size, max_size }
    }

    /// Create an array too large error
    pub fn array_too_large(size: usize, max_size: usize) -> Self {
        Self::ArrayTooLarge { size, max_size }
    }

    /// Create a type coercion error
    pub fn type_coercion(from_type: String, to_type: String) -> Self {
        Self::TypeCoercion { from_type, to_type }
    }

    /// Create a circular reference error
    pub fn circular_reference() -> Self {
        Self::CircularReference
    }

    /// Create a validation error with custom message
    pub fn validation(message: String) -> Self {
        Self::Validation { message }
    }

    /// Check if this error is related to file size limits
    pub fn is_size_limit_error(&self) -> bool {
        matches!(
            self,
            Self::FileTooLarge { .. } | Self::StringTooLarge { .. } | Self::ArrayTooLarge { .. }
        )
    }

    /// Check if this error is related to validation
    pub fn is_validation_error(&self) -> bool {
        matches!(
            self,
            Self::InvalidVariableName { .. }
                | Self::ReservedVariableName { .. }
                | Self::NestingTooDeep { .. }
                | Self::Validation { .. }
        )
    }

    /// Check if this error is related to parsing
    pub fn is_parse_error(&self) -> bool {
        matches!(
            self,
            Self::TomlParse { .. } | Self::TomlParseGeneric(_) | Self::InvalidUtf8(_)
        )
    }
}

/// Configuration validation limits and constants
pub struct ValidationLimits;

impl ValidationLimits {
    /// Maximum file size in bytes (1MB)
    pub const MAX_FILE_SIZE: u64 = 1_048_576;

    /// Maximum nesting depth (10 levels)
    pub const MAX_NESTING_DEPTH: usize = 10;

    /// Maximum string value size in bytes (10KB)
    pub const MAX_STRING_SIZE: usize = 10_240;

    /// Maximum array size (1000 elements)
    pub const MAX_ARRAY_SIZE: usize = 1_000;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_creation() {
        let error = ConfigError::toml_parse(10, 5, "Invalid syntax".to_string());
        assert!(matches!(
            error,
            ConfigError::TomlParse {
                line: 10,
                column: 5,
                ..
            }
        ));

        let error = ConfigError::file_too_large(2_000_000, ValidationLimits::MAX_FILE_SIZE);
        assert!(matches!(
            error,
            ConfigError::FileTooLarge {
                size: 2_000_000,
                ..
            }
        ));

        let error = ConfigError::nesting_too_deep(15, ValidationLimits::MAX_NESTING_DEPTH);
        assert!(matches!(
            error,
            ConfigError::NestingTooDeep { depth: 15, .. }
        ));
    }

    #[test]
    fn test_error_classification() {
        let parse_error = ConfigError::toml_parse(1, 1, "test".to_string());
        assert!(parse_error.is_parse_error());
        assert!(!parse_error.is_validation_error());
        assert!(!parse_error.is_size_limit_error());

        let validation_error =
            ConfigError::invalid_variable_name("test".to_string(), "reason".to_string());
        assert!(!validation_error.is_parse_error());
        assert!(validation_error.is_validation_error());
        assert!(!validation_error.is_size_limit_error());

        let size_error = ConfigError::file_too_large(100, 50);
        assert!(!size_error.is_parse_error());
        assert!(!size_error.is_validation_error());
        assert!(size_error.is_size_limit_error());
    }

    #[test]
    fn test_validation_limits() {
        assert_eq!(ValidationLimits::MAX_FILE_SIZE, 1_048_576);
        assert_eq!(ValidationLimits::MAX_NESTING_DEPTH, 10);
        assert_eq!(ValidationLimits::MAX_STRING_SIZE, 10_240);
        assert_eq!(ValidationLimits::MAX_ARRAY_SIZE, 1_000);
    }
}
