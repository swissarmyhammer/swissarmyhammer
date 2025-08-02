use thiserror::Error;

/// Comprehensive error types for sah.toml configuration
#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ConfigError {
    /// IO error occurred while reading the configuration file
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// TOML parsing error occurred while parsing the configuration
    #[error("TOML parsing error: {message}")]
    TomlParse {
        /// Error message from TOML parser
        message: String,
        /// Line number where error occurred
        line: Option<usize>,
        /// Column number where error occurred
        column: Option<usize>,
    },

    /// Configuration file exceeds maximum allowed size
    #[error("File is too large: {size} bytes (max: {max_size} bytes)")]
    FileTooLarge {
        /// Actual file size in bytes
        size: u64,
        /// Maximum allowed size in bytes
        max_size: u64,
    },

    /// Configuration file was not found at the specified path
    #[error("Configuration file not found: {path}")]
    FileNotFound {
        /// Path where the file was expected
        path: String,
    },

    /// File contains invalid UTF-8 characters
    #[error("Invalid UTF-8 in configuration file")]
    InvalidUtf8 {
        /// Byte position where invalid UTF-8 was encountered
        position: Option<usize>,
    },

    /// Error occurred during environment variable substitution
    #[error("Environment variable substitution error in '{variable}': {message}")]
    EnvVarSubstitution {
        /// The variable name that caused the error
        variable: String,
        /// Detailed error message
        message: String,
    },

    /// Configuration has too many levels of nesting
    #[error("Configuration nesting too deep: {depth} levels (max: {max_depth})")]
    NestingTooDeep {
        /// Actual nesting depth
        depth: usize,
        /// Maximum allowed depth
        max_depth: usize,
    },

    /// String value exceeds maximum allowed length
    #[error("String value too long: {length} characters (max: {max_length}) in field '{field}'")]
    StringTooLong {
        /// Actual string length
        length: usize,
        /// Maximum allowed length
        max_length: usize,
        /// Field name containing the long string
        field: String,
    },

    /// Array exceeds maximum allowed number of elements
    #[error("Array too large: {length} elements (max: {max_elements}) in field '{field}'")]
    ArrayTooLarge {
        /// Actual number of elements
        length: usize,
        /// Maximum allowed elements
        max_elements: usize,
        /// Field name containing the large array
        field: String,
    },

    /// Variable name is invalid according to naming rules
    #[error("Invalid variable name '{name}': {reason}")]
    InvalidVariableName {
        /// The invalid variable name
        name: String,
        /// Reason why the name is invalid
        reason: String,
    },

    /// Variable name is reserved and cannot be used
    #[error("Variable name '{name}' is reserved and cannot be used")]
    ReservedVariableName {
        /// The reserved variable name
        name: String,
    },

    /// Value type coercion failed
    #[error("Cannot coerce value of type {from_type} to {to_type} for field '{field}'")]
    TypeCoercionFailed {
        /// Source type name
        from_type: String,
        /// Target type name
        to_type: String,
        /// Field name where coercion failed
        field: String,
    },

    /// Dot notation access failed
    #[error("Cannot access nested field '{path}': {reason}")]
    NestedAccessFailed {
        /// The dot notation path that failed
        path: String,
        /// Reason for the failure
        reason: String,
    },

    /// Custom validation rule failed
    #[error("Validation rule '{rule}' failed: {message}")]
    ValidationFailed {
        /// Name of the failed validation rule
        rule: String,
        /// Error message from the validation rule
        message: String,
    },
}

impl ConfigError {
    /// Create a TOML parsing error from a toml::de::Error
    pub fn from_toml_error(error: toml::de::Error) -> Self {
        let message = error.to_string();

        // Try to extract line and column information from the error message
        // TOML parser errors often contain "at line X, column Y" information
        let (line, column) = Self::extract_position_from_message(&message);

        Self::TomlParse {
            message,
            line,
            column,
        }
    }

    /// Create an invalid UTF-8 error with position information
    pub fn invalid_utf8_at_position(position: Option<usize>) -> Self {
        Self::InvalidUtf8 { position }
    }

    /// Create an environment variable substitution error
    pub fn env_var_substitution<S: Into<String>>(variable: S, message: S) -> Self {
        Self::EnvVarSubstitution {
            variable: variable.into(),
            message: message.into(),
        }
    }

    /// Create a validation failed error
    pub fn validation_failed<S: Into<String>>(rule: S, message: S) -> Self {
        Self::ValidationFailed {
            rule: rule.into(),
            message: message.into(),
        }
    }

    /// Extract line and column information from TOML error message
    fn extract_position_from_message(message: &str) -> (Option<usize>, Option<usize>) {
        // Common patterns in TOML error messages:
        // "at line 5, column 10"
        // "at line 5"
        // "expected ... at line 5, column 10"

        let line_regex = regex::Regex::new(r"at line (\d+)").expect("Failed to compile line regex");
        let column_regex =
            regex::Regex::new(r"column (\d+)").expect("Failed to compile column regex");

        let line = line_regex
            .captures(message)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse().ok());

        let column = column_regex
            .captures(message)
            .and_then(|caps| caps.get(1))
            .and_then(|m| m.as_str().parse().ok());

        (line, column)
    }
}

impl From<toml::de::Error> for ConfigError {
    fn from(error: toml::de::Error) -> Self {
        Self::from_toml_error(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_error_display() {
        let error = ConfigError::FileTooLarge {
            size: 2_000_000,
            max_size: 1_048_576,
        };
        let error_str = error.to_string();
        assert!(error_str.contains("File is too large"));
        assert!(error_str.contains("2000000"));
        assert!(error_str.contains("1048576"));
    }

    #[test]
    fn test_env_var_substitution_error() {
        let error = ConfigError::env_var_substitution("MY_VAR", "Variable not found");
        assert!(matches!(error, ConfigError::EnvVarSubstitution { .. }));
        assert!(error.to_string().contains("MY_VAR"));
        assert!(error.to_string().contains("Variable not found"));
    }

    #[test]
    fn test_validation_failed_error() {
        let error = ConfigError::validation_failed("MaxLength", "String too long");
        assert!(matches!(error, ConfigError::ValidationFailed { .. }));
        assert!(error.to_string().contains("MaxLength"));
        assert!(error.to_string().contains("String too long"));
    }

    #[test]
    fn test_position_extraction() {
        let (line, column) =
            ConfigError::extract_position_from_message("error at line 5, column 10");
        assert_eq!(line, Some(5));
        assert_eq!(column, Some(10));

        let (line, column) = ConfigError::extract_position_from_message("error at line 5");
        assert_eq!(line, Some(5));
        assert_eq!(column, None);

        let (line, column) = ConfigError::extract_position_from_message("no position info");
        assert_eq!(line, None);
        assert_eq!(column, None);
    }
}
