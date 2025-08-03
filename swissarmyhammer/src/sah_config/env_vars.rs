//! Environment variable processing for sah.toml configuration files
//!
//! This module provides functionality to substitute environment variables in configuration
//! values using the syntax `${VAR:-default}` for optional variables with defaults and
//! `${VAR}` for required variables.

use regex::Regex;
use std::collections::HashMap;
use std::env;
use thiserror::Error;

/// Errors that can occur during environment variable processing
#[derive(Error, Debug)]
pub enum EnvVarError {
    /// Required environment variable was not found
    #[error("Required environment variable '{name}' not found")]
    RequiredVariableNotFound {
        /// The name of the missing environment variable
        name: String,
    },

    /// Environment variable name contains invalid characters
    #[error("Invalid environment variable name '{name}': {reason}")]
    InvalidVariableName {
        /// The invalid variable name
        name: String,
        /// Reason why the name is invalid
        reason: String,
    },

    /// Environment variable value cannot be converted to the expected type
    #[error("Cannot convert environment variable '{name}' value '{value}' to {expected_type}")]
    TypeConversionError {
        /// Environment variable name
        name: String,
        /// The value that failed to convert
        value: String,
        /// The expected type for conversion
        expected_type: String,
    },

    /// Error in regular expression processing
    #[error("Regex processing error: {0}")]
    RegexError(String),
}

/// Environment variable processor that handles substitution in configuration values
pub struct EnvVarProcessor {
    /// Compiled regex for matching environment variable patterns
    var_pattern: Regex,
}

impl EnvVarProcessor {
    /// Create a new environment variable processor
    pub fn new() -> Result<Self, EnvVarError> {
        // Pattern matches ${VAR} or ${VAR:-default}
        // Group 1: variable name
        // Group 2: default value (optional)
        let var_pattern = Regex::new(r"\$\{([A-Za-z_][A-Za-z0-9_]*)(:-([^}]*))?\}")
            .map_err(|e| EnvVarError::RegexError(e.to_string()))?;

        Ok(Self { var_pattern })
    }

    /// Process environment variable substitution in a string value
    ///
    /// # Arguments
    /// * `input` - Input string potentially containing environment variable references
    ///
    /// # Returns
    /// * `Ok(String)` - String with environment variables substituted
    /// * `Err(EnvVarError)` - Error if required variable not found or invalid format
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer::sah_config::env_vars::EnvVarProcessor;
    ///
    /// let processor = EnvVarProcessor::new()?;
    ///
    /// // With default value
    /// std::env::remove_var("MISSING_VAR");
    /// let result = processor.substitute_variables("Database: ${MISSING_VAR:-localhost}")?;
    /// assert_eq!(result, "Database: localhost");
    ///
    /// // With existing environment variable
    /// std::env::set_var("DB_HOST", "production.example.com");
    /// let result = processor.substitute_variables("Database: ${DB_HOST:-localhost}")?;
    /// assert_eq!(result, "Database: production.example.com");
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn substitute_variables(&self, input: &str) -> Result<String, EnvVarError> {
        let mut result = input.to_string();
        let mut processed_vars: HashMap<String, String> = HashMap::new();

        // Use find_iter to get all matches and their positions
        let matches: Vec<_> = self.var_pattern.find_iter(input).collect();

        // Process matches in reverse order to avoid offset issues
        for regex_match in matches.iter().rev() {
            let full_match = regex_match.as_str();
            let captures = self
                .var_pattern
                .captures(full_match)
                .ok_or_else(|| EnvVarError::RegexError("Failed to capture groups".to_string()))?;

            let var_name = captures
                .get(1)
                .ok_or_else(|| EnvVarError::RegexError("Missing variable name".to_string()))?
                .as_str();

            let default_value = captures.get(3).map(|m| m.as_str());

            // Validate variable name
            self.validate_variable_name(var_name)?;

            // Get the substitution value, using cache if already processed
            let substitution = if let Some(cached) = processed_vars.get(var_name) {
                cached.clone()
            } else {
                let value = self.get_variable_value(var_name, default_value)?;
                processed_vars.insert(var_name.to_string(), value.clone());
                value
            };

            // Replace the match in the result string
            let start = regex_match.start();
            let end = regex_match.end();
            result.replace_range(start..end, &substitution);
        }

        Ok(result)
    }

    /// Convert a string value to boolean
    ///
    /// Recognizes common boolean representations:
    /// - true, True, TRUE, yes, Yes, YES, on, On, ON, 1
    /// - false, False, FALSE, no, No, NO, off, Off, OFF, 0, empty string
    pub fn convert_to_boolean(&self, value: &str, var_name: &str) -> Result<bool, EnvVarError> {
        match value.trim().to_lowercase().as_str() {
            "true" | "yes" | "on" | "1" => Ok(true),
            "false" | "no" | "off" | "0" | "" => Ok(false),
            _ => Err(EnvVarError::TypeConversionError {
                name: var_name.to_string(),
                value: value.to_string(),
                expected_type: "boolean".to_string(),
            }),
        }
    }

    /// Convert a string value to integer
    pub fn convert_to_integer(&self, value: &str, var_name: &str) -> Result<i64, EnvVarError> {
        value
            .trim()
            .parse::<i64>()
            .map_err(|_| EnvVarError::TypeConversionError {
                name: var_name.to_string(),
                value: value.to_string(),
                expected_type: "integer".to_string(),
            })
    }

    /// Convert a string value to float
    pub fn convert_to_float(&self, value: &str, var_name: &str) -> Result<f64, EnvVarError> {
        value
            .trim()
            .parse::<f64>()
            .map_err(|_| EnvVarError::TypeConversionError {
                name: var_name.to_string(),
                value: value.to_string(),
                expected_type: "float".to_string(),
            })
    }

    /// Get environment variable value with optional default
    fn get_variable_value(
        &self,
        var_name: &str,
        default_value: Option<&str>,
    ) -> Result<String, EnvVarError> {
        match env::var(var_name) {
            Ok(value) => Ok(value),
            Err(env::VarError::NotPresent) => {
                if let Some(default) = default_value {
                    Ok(default.to_string())
                } else {
                    Err(EnvVarError::RequiredVariableNotFound {
                        name: var_name.to_string(),
                    })
                }
            }
            Err(env::VarError::NotUnicode(_)) => Err(EnvVarError::TypeConversionError {
                name: var_name.to_string(),
                value: "<invalid unicode>".to_string(),
                expected_type: "UTF-8 string".to_string(),
            }),
        }
    }

    /// Validate that an environment variable name is valid
    ///
    /// Valid names:
    /// - Start with letter or underscore
    /// - Contain only letters, digits, and underscores
    /// - Not empty
    fn validate_variable_name(&self, name: &str) -> Result<(), EnvVarError> {
        if name.is_empty() {
            return Err(EnvVarError::InvalidVariableName {
                name: name.to_string(),
                reason: "Variable name cannot be empty".to_string(),
            });
        }

        let first_char = name.chars().next().unwrap();
        if !first_char.is_ascii_alphabetic() && first_char != '_' {
            return Err(EnvVarError::InvalidVariableName {
                name: name.to_string(),
                reason: "Variable name must start with letter or underscore".to_string(),
            });
        }

        for (i, ch) in name.chars().enumerate() {
            if !ch.is_ascii_alphanumeric() && ch != '_' {
                return Err(EnvVarError::InvalidVariableName {
                    name: name.to_string(),
                    reason: format!("Invalid character '{ch}' at position {i}"),
                });
            }
        }

        Ok(())
    }
}

impl Default for EnvVarProcessor {
    fn default() -> Self {
        Self::new().expect("Failed to create EnvVarProcessor")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_new_processor() {
        let processor = EnvVarProcessor::new();
        assert!(processor.is_ok());
    }

    #[test]
    #[serial]
    fn test_substitute_with_default() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        // Ensure variable is not set
        env::remove_var("TEST_VAR_NOT_SET");

        let result = processor.substitute_variables("Database: ${TEST_VAR_NOT_SET:-localhost}")?;
        assert_eq!(result, "Database: localhost");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_substitute_with_env_var() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::set_var("TEST_DB_HOST", "production.example.com");

        let result = processor.substitute_variables("Database: ${TEST_DB_HOST:-localhost}")?;
        assert_eq!(result, "Database: production.example.com");

        // Clean up
        env::remove_var("TEST_DB_HOST");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_substitute_required_variable_found() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::set_var("TEST_REQUIRED_VAR", "required_value");

        let result = processor.substitute_variables("Key: ${TEST_REQUIRED_VAR}")?;
        assert_eq!(result, "Key: required_value");

        // Clean up
        env::remove_var("TEST_REQUIRED_VAR");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_substitute_required_variable_missing() {
        let processor = EnvVarProcessor::new().unwrap();

        env::remove_var("TEST_MISSING_REQUIRED");

        let result = processor.substitute_variables("Key: ${TEST_MISSING_REQUIRED}");
        assert!(matches!(
            result,
            Err(EnvVarError::RequiredVariableNotFound { .. })
        ));
    }

    #[test]
    #[serial]
    fn test_multiple_substitutions() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::set_var("DB_HOST", "db.example.com");
        env::set_var("DB_PORT", "5432");
        env::remove_var("DB_NAME");

        let result = processor
            .substitute_variables("Connection: ${DB_HOST}:${DB_PORT}/${DB_NAME:-myapp}")?;
        assert_eq!(result, "Connection: db.example.com:5432/myapp");

        // Clean up
        env::remove_var("DB_HOST");
        env::remove_var("DB_PORT");

        Ok(())
    }

    #[test]
    fn test_no_substitution_needed() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        let result = processor.substitute_variables("No variables here")?;
        assert_eq!(result, "No variables here");

        Ok(())
    }

    #[test]
    fn test_convert_to_boolean() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        // Test true values
        assert!(processor.convert_to_boolean("true", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("True", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("TRUE", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("yes", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("YES", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("on", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("ON", "TEST_VAR")?);
        assert!(processor.convert_to_boolean("1", "TEST_VAR")?);

        // Test false values
        assert!(!processor.convert_to_boolean("false", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("False", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("FALSE", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("no", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("NO", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("off", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("OFF", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("0", "TEST_VAR")?);
        assert!(!processor.convert_to_boolean("", "TEST_VAR")?);

        // Test invalid value
        let result = processor.convert_to_boolean("maybe", "TEST_VAR");
        assert!(matches!(
            result,
            Err(EnvVarError::TypeConversionError { .. })
        ));

        Ok(())
    }

    #[test]
    fn test_convert_to_integer() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        assert_eq!(processor.convert_to_integer("42", "TEST_VAR")?, 42);
        assert_eq!(processor.convert_to_integer("-123", "TEST_VAR")?, -123);
        assert_eq!(processor.convert_to_integer("  456  ", "TEST_VAR")?, 456);

        // Test invalid value
        let result = processor.convert_to_integer("not_a_number", "TEST_VAR");
        assert!(matches!(
            result,
            Err(EnvVarError::TypeConversionError { .. })
        ));

        Ok(())
    }

    #[test]
    fn test_convert_to_float() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        assert_eq!(processor.convert_to_float("42.5", "TEST_VAR")?, 42.5);
        assert_eq!(
            processor.convert_to_float("-123.456", "TEST_VAR")?,
            -123.456
        );
        assert_eq!(
            processor.convert_to_float("  456.789  ", "TEST_VAR")?,
            456.789
        );

        // Test invalid value
        let result = processor.convert_to_float("not_a_number", "TEST_VAR");
        assert!(matches!(
            result,
            Err(EnvVarError::TypeConversionError { .. })
        ));

        Ok(())
    }

    #[test]
    fn test_validate_variable_name() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        // Valid names
        processor.validate_variable_name("VAR")?;
        processor.validate_variable_name("_VAR")?;
        processor.validate_variable_name("VAR_123")?;
        processor.validate_variable_name("a1b2c3")?;

        // Invalid names
        assert!(matches!(
            processor.validate_variable_name(""),
            Err(EnvVarError::InvalidVariableName { .. })
        ));
        assert!(matches!(
            processor.validate_variable_name("123VAR"),
            Err(EnvVarError::InvalidVariableName { .. })
        ));
        assert!(matches!(
            processor.validate_variable_name("VAR-NAME"),
            Err(EnvVarError::InvalidVariableName { .. })
        ));
        assert!(matches!(
            processor.validate_variable_name("VAR.NAME"),
            Err(EnvVarError::InvalidVariableName { .. })
        ));

        Ok(())
    }

    #[test]
    #[serial]
    fn test_empty_default_value() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::remove_var("TEST_EMPTY_DEFAULT");

        let result = processor.substitute_variables("Value: ${TEST_EMPTY_DEFAULT:-}")?;
        assert_eq!(result, "Value: ");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_special_characters_in_default() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::remove_var("TEST_SPECIAL_DEFAULT");

        let result = processor
            .substitute_variables("Config: ${TEST_SPECIAL_DEFAULT:-http://localhost:8080/api}")?;
        assert_eq!(result, "Config: http://localhost:8080/api");

        Ok(())
    }

    #[test]
    #[serial]
    fn test_same_variable_multiple_times() -> Result<(), Box<dyn std::error::Error>> {
        let processor = EnvVarProcessor::new()?;

        env::set_var("TEST_REPEATED_VAR", "repeated_value");

        let result = processor
            .substitute_variables("First: ${TEST_REPEATED_VAR}, Second: ${TEST_REPEATED_VAR}")?;
        assert_eq!(result, "First: repeated_value, Second: repeated_value");

        // Clean up
        env::remove_var("TEST_REPEATED_VAR");

        Ok(())
    }

    #[test]
    fn test_default_trait() {
        let processor = EnvVarProcessor::default();
        // Just ensure it can be created without error
        assert!(processor.var_pattern.is_match("${TEST}"));
    }
}
