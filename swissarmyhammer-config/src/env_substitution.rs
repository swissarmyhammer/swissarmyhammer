//! Environment Variable Substitution for Template Context
//!
//! This module provides environment variable substitution compatible with the existing
//! sah_config template_integration.rs system while enhancing the new TemplateContext.
//!
//! Supports patterns:
//! - `${VAR_NAME}` - Replace with environment variable value, empty string if not set
//! - `${VAR_NAME:-default}` - Replace with environment variable value, or default if not set
//!
//! The implementation uses thread-local regex compilation for performance and maintains
//! exact behavioral compatibility with the legacy system.

use crate::{ConfigError, ConfigResult};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use tracing::{debug, trace, warn};

/// Environment variable substitution processor
///
/// This processor handles environment variable substitution in template contexts
/// using the same patterns and behavior as the legacy template_integration.rs system.
pub struct EnvVarProcessor {
    /// Compiled regex for matching environment variable patterns
    var_regex: Regex,
    /// Whether to return errors for missing variables (true) or empty strings (false)
    strict_mode: bool,
}

impl EnvVarProcessor {
    /// Regex pattern for environment variables
    /// Matches: ${VAR_NAME} and ${VAR_NAME:-default_value}
    /// Variable names must be valid (alphanumeric and underscore only, no spaces)
    const ENV_VAR_PATTERN: &'static str = r"\$\{([A-Za-z_][A-Za-z0-9_]*)(?::-([^}]*))?\}";

    /// Create new processor with compiled regex
    ///
    /// # Arguments
    /// * `strict_mode` - If true, missing variables without defaults return errors.
    ///   If false, missing variables without defaults return empty strings (legacy behavior).
    ///
    /// # Example
    /// ```rust
    /// use swissarmyhammer_config::env_substitution::EnvVarProcessor;
    ///
    /// // Legacy compatible mode (empty strings for missing vars)
    /// let processor = EnvVarProcessor::new(false).unwrap();
    ///
    /// // Strict mode (errors for missing vars)
    /// let processor = EnvVarProcessor::new(true).unwrap();
    /// ```
    pub fn new(strict_mode: bool) -> ConfigResult<Self> {
        let var_regex = Regex::new(Self::ENV_VAR_PATTERN).map_err(|e| {
            ConfigError::validation_error(format!(
                "Failed to compile environment variable regex: {}",
                e
            ))
        })?;

        Ok(Self {
            var_regex,
            strict_mode,
        })
    }

    /// Create processor in legacy compatibility mode (empty strings for missing vars)
    pub fn legacy() -> ConfigResult<Self> {
        Self::new(false)
    }

    /// Create processor in strict mode (errors for missing vars)
    pub fn strict() -> ConfigResult<Self> {
        Self::new(true)
    }

    /// Process environment variable substitution in a JSON value
    ///
    /// This method recursively processes all string values in the JSON structure,
    /// performing environment variable substitution according to the processor's mode.
    ///
    /// # Arguments
    /// * `value` - Mutable reference to the JSON value to process
    ///
    /// # Example
    /// ```rust
    /// use swissarmyhammer_config::env_substitution::EnvVarProcessor;
    /// use serde_json::json;
    /// use std::env;
    ///
    /// env::set_var("TEST_VAR", "test_value");
    ///
    /// let processor = EnvVarProcessor::legacy().unwrap();
    /// let mut value = json!({
    ///     "message": "Hello ${TEST_VAR}!",
    ///     "array": ["${TEST_VAR}", "static"],
    ///     "number": 42
    /// });
    ///
    /// processor.substitute_value(&mut value).unwrap();
    ///
    /// assert_eq!(value["message"], "Hello test_value!");
    /// assert_eq!(value["array"][0], "test_value");
    /// assert_eq!(value["number"], 42); // unchanged
    ///
    /// env::remove_var("TEST_VAR");
    /// ```
    pub fn substitute_value(&self, value: &mut Value) -> ConfigResult<()> {
        match value {
            Value::String(s) => {
                *s = self.substitute_string(s)?;
            }
            Value::Array(arr) => {
                for item in arr.iter_mut() {
                    self.substitute_value(item)?;
                }
            }
            Value::Object(obj) => {
                for (_, val) in obj.iter_mut() {
                    self.substitute_value(val)?;
                }
            }
            // Numbers, booleans, null don't need substitution
            _ => {}
        }
        Ok(())
    }

    /// Process environment variable substitution in a string
    ///
    /// This is the core substitution method that handles the regex matching
    /// and replacement logic.
    ///
    /// # Arguments
    /// * `s` - String to process
    ///
    /// # Returns
    /// * `String` - String with environment variables substituted
    ///
    /// # Example
    /// ```rust
    /// use swissarmyhammer_config::env_substitution::EnvVarProcessor;
    /// use std::env;
    ///
    /// env::set_var("HOST", "localhost");
    /// env::set_var("PORT", "8080");
    ///
    /// let processor = EnvVarProcessor::legacy().unwrap();
    /// let result = processor.substitute_string("${HOST}:${PORT}").unwrap();
    /// assert_eq!(result, "localhost:8080");
    ///
    /// let with_default = processor.substitute_string("${MISSING:-default}").unwrap();
    /// assert_eq!(with_default, "default");
    ///
    /// env::remove_var("HOST");
    /// env::remove_var("PORT");
    /// ```
    pub fn substitute_string(&self, s: &str) -> ConfigResult<String> {
        // In strict mode, check for missing variables first
        if self.strict_mode {
            for caps in self.var_regex.captures_iter(s) {
                let var_name = &caps[1];
                if std::env::var(var_name).is_err() && caps.get(2).is_none() {
                    // Missing variable with no default
                    return Err(ConfigError::environment_error(format!(
                        "Environment variable '{}' not found and no default provided",
                        var_name
                    )));
                }
            }
        }

        // Now do the actual substitution
        let result = self
            .var_regex
            .replace_all(s, |caps: &regex::Captures| {
                let var_name = &caps[1];
                match std::env::var(var_name) {
                    Ok(value) => {
                        trace!(
                            "Environment variable substitution: {} = {}",
                            var_name,
                            value
                        );
                        value
                    }
                    Err(_) => {
                        // Check for default value pattern ${VAR:-default}
                        if let Some(default_match) = caps.get(2) {
                            trace!("Using default value for {}: {}", var_name, default_match.as_str());
                            default_match.as_str().to_string()
                        } else {
                            // In legacy mode, missing vars become empty strings
                            trace!(
                                "Environment variable '{}' not found, using empty string (legacy mode)",
                                var_name
                            );
                            String::new() // No default, return empty string (legacy behavior)
                        }
                    }
                }
            })
            .to_string();

        Ok(result)
    }

    /// Check if string contains substitution patterns
    ///
    /// This is useful for optimization - if no patterns are found,
    /// substitution can be skipped entirely.
    ///
    /// # Arguments
    /// * `s` - String to check
    ///
    /// # Returns
    /// * `bool` - True if string contains environment variable patterns
    ///
    /// # Example
    /// ```rust
    /// use swissarmyhammer_config::env_substitution::EnvVarProcessor;
    ///
    /// let processor = EnvVarProcessor::legacy().unwrap();
    /// assert!(processor.contains_patterns("Hello ${WORLD}!"));
    /// assert!(processor.contains_patterns("${VAR:-default}"));
    /// assert!(!processor.contains_patterns("Hello World!"));
    /// ```
    pub fn contains_patterns(&self, s: &str) -> bool {
        self.var_regex.is_match(s)
    }

    /// Process environment variable substitution in all values of a HashMap
    ///
    /// This is a convenience method for processing template context variables.
    ///
    /// # Arguments
    /// * `vars` - Mutable reference to HashMap of template variables
    ///
    /// # Example
    /// ```rust
    /// use swissarmyhammer_config::env_substitution::EnvVarProcessor;
    /// use std::collections::HashMap;
    /// use serde_json::json;
    /// use std::env;
    ///
    /// env::set_var("APP_NAME", "MyApp");
    ///
    /// let processor = EnvVarProcessor::legacy().unwrap();
    /// let mut vars = HashMap::new();
    /// vars.insert("title".to_string(), json!("Welcome to ${APP_NAME}"));
    /// vars.insert("config".to_string(), json!({"app": "${APP_NAME}", "debug": true}));
    ///
    /// processor.substitute_vars(&mut vars).unwrap();
    ///
    /// assert_eq!(vars["title"], json!("Welcome to MyApp"));
    /// assert_eq!(vars["config"]["app"], json!("MyApp"));
    /// assert_eq!(vars["config"]["debug"], json!(true)); // unchanged
    ///
    /// env::remove_var("APP_NAME");
    /// ```
    pub fn substitute_vars(&self, vars: &mut HashMap<String, Value>) -> ConfigResult<()> {
        debug!(
            "Processing environment variable substitution in {} variables",
            vars.len()
        );

        for (key, value) in vars.iter_mut() {
            trace!("Processing variable: {}", key);
            self.substitute_value(value)?;
        }

        Ok(())
    }
}

thread_local! {
    /// Legacy-compatible processor (empty strings for missing vars)
    pub static LEGACY_PROCESSOR: EnvVarProcessor = EnvVarProcessor::legacy()
        .expect("Failed to initialize legacy environment variable processor");

    /// Strict processor (errors for missing vars)
    pub static STRICT_PROCESSOR: EnvVarProcessor = EnvVarProcessor::strict()
        .expect("Failed to initialize strict environment variable processor");
}

/// Substitute environment variables using the legacy-compatible processor
///
/// This function provides the same interface as the existing template_integration.rs
/// substitute_env_vars_in_string function, ensuring drop-in compatibility.
///
/// # Arguments  
/// * `s` - String to process
///
/// # Returns
/// * `String` - String with environment variables substituted
///
/// # Example
/// ```rust
/// use swissarmyhammer_config::env_substitution::substitute_env_vars_legacy;
/// use std::env;
///
/// env::set_var("USER", "alice");
///
/// let result = substitute_env_vars_legacy("Hello ${USER}!");
/// assert_eq!(result, "Hello alice!");
///
/// // Missing vars return empty strings (legacy behavior)
/// let missing = substitute_env_vars_legacy("Hello ${MISSING_USER}!");
/// assert_eq!(missing, "Hello !");
///
/// env::remove_var("USER");
/// ```
pub fn substitute_env_vars_legacy(s: &str) -> String {
    LEGACY_PROCESSOR.with(|processor| {
        processor.substitute_string(s).unwrap_or_else(|_| {
            // Should never happen in legacy mode, but provide fallback
            warn!("Unexpected error in legacy environment variable substitution");
            s.to_string()
        })
    })
}

/// Substitute environment variables using the strict processor
///
/// This function returns errors for missing environment variables without defaults,
/// providing stricter validation than the legacy behavior.
///
/// # Arguments  
/// * `s` - String to process
///
/// # Returns
/// * `ConfigResult<String>` - String with environment variables substituted, or error
///
/// # Example
/// ```rust
/// use swissarmyhammer_config::env_substitution::substitute_env_vars_strict;
/// use std::env;
///
/// env::set_var("USER", "alice");
///
/// let result = substitute_env_vars_strict("Hello ${USER}!").unwrap();
/// assert_eq!(result, "Hello alice!");
///
/// // Missing vars return errors in strict mode
/// let missing = substitute_env_vars_strict("Hello ${MISSING_USER}!");
/// assert!(missing.is_err());
///
/// env::remove_var("USER");
/// ```
pub fn substitute_env_vars_strict(s: &str) -> ConfigResult<String> {
    STRICT_PROCESSOR.with(|processor| processor.substitute_string(s))
}

/// Check if a string contains environment variable patterns
///
/// This is a utility function for optimization - processing can be skipped
/// if no substitution patterns are detected.
///
/// # Arguments
/// * `s` - String to check
///
/// # Returns
/// * `bool` - True if string contains ${VAR} or ${VAR:-default} patterns
///
/// # Example
/// ```rust
/// use swissarmyhammer_config::env_substitution::contains_env_patterns;
///
/// assert!(contains_env_patterns("Config: ${DATABASE_URL}"));
/// assert!(contains_env_patterns("Port: ${PORT:-3000}"));
/// assert!(!contains_env_patterns("Static configuration"));
/// ```
pub fn contains_env_patterns(s: &str) -> bool {
    LEGACY_PROCESSOR.with(|processor| processor.contains_patterns(s))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;

    #[test]
    fn test_env_var_processor_new() {
        let legacy_processor = EnvVarProcessor::new(false).unwrap();
        assert!(!legacy_processor.strict_mode);

        let strict_processor = EnvVarProcessor::new(true).unwrap();
        assert!(strict_processor.strict_mode);
    }

    #[test]
    fn test_env_var_processor_convenience_constructors() {
        let legacy = EnvVarProcessor::legacy().unwrap();
        assert!(!legacy.strict_mode);

        let strict = EnvVarProcessor::strict().unwrap();
        assert!(strict.strict_mode);
    }

    #[test]
    fn test_substitute_string_basic() {
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another_value");

        let processor = EnvVarProcessor::legacy().unwrap();

        // Test simple substitution
        let result = processor.substitute_string("Hello ${TEST_VAR}!").unwrap();
        assert_eq!(result, "Hello test_value!");

        // Test with default value (should use env var)
        let result = processor
            .substitute_string("Hello ${TEST_VAR:-default}!")
            .unwrap();
        assert_eq!(result, "Hello test_value!");

        // Test multiple substitutions
        let result = processor
            .substitute_string("${TEST_VAR} and ${ANOTHER_VAR}")
            .unwrap();
        assert_eq!(result, "test_value and another_value");

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");
    }

    #[test]
    fn test_substitute_string_missing_vars_legacy_mode() {
        let processor = EnvVarProcessor::legacy().unwrap();

        // Test with missing var and default
        let result = processor
            .substitute_string("Hello ${MISSING_VAR:-default_value}!")
            .unwrap();
        assert_eq!(result, "Hello default_value!");

        // Test with missing var and no default (should return empty string in legacy mode)
        let result = processor
            .substitute_string("Hello ${MISSING_VAR}!")
            .unwrap();
        assert_eq!(result, "Hello !");
    }

    #[test]
    fn test_substitute_string_missing_vars_strict_mode() {
        let processor = EnvVarProcessor::strict().unwrap();

        // Test with missing var and default (should work)
        let result = processor
            .substitute_string("Hello ${MISSING_VAR:-default_value}!")
            .unwrap();
        assert_eq!(result, "Hello default_value!");

        // Test with missing var and no default (should return error in strict mode)
        let result = processor.substitute_string("Hello ${MISSING_VAR}!");
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert!(error_msg.contains("MISSING_VAR"));
    }

    #[test]
    fn test_substitute_value_string() {
        env::set_var("VALUE_TEST", "substituted");

        let processor = EnvVarProcessor::legacy().unwrap();
        let mut value = json!("Original ${VALUE_TEST} text");

        processor.substitute_value(&mut value).unwrap();
        assert_eq!(value, json!("Original substituted text"));

        env::remove_var("VALUE_TEST");
    }

    #[test]
    fn test_substitute_value_array() {
        env::set_var("ARRAY_TEST", "item");

        let processor = EnvVarProcessor::legacy().unwrap();
        let mut value = json!(["${ARRAY_TEST}_1", "${ARRAY_TEST}_2", "static"]);

        processor.substitute_value(&mut value).unwrap();
        assert_eq!(value, json!(["item_1", "item_2", "static"]));

        env::remove_var("ARRAY_TEST");
    }

    #[test]
    fn test_substitute_value_object() {
        env::set_var("OBJECT_TEST", "nested");

        let processor = EnvVarProcessor::legacy().unwrap();
        let mut value = json!({
            "key1": "${OBJECT_TEST}_value",
            "key2": 42,
            "nested": {
                "inner": "${OBJECT_TEST}_inner",
                "static": "unchanged"
            }
        });

        processor.substitute_value(&mut value).unwrap();

        let expected = json!({
            "key1": "nested_value",
            "key2": 42,
            "nested": {
                "inner": "nested_inner",
                "static": "unchanged"
            }
        });
        assert_eq!(value, expected);

        env::remove_var("OBJECT_TEST");
    }

    #[test]
    fn test_substitute_value_preserves_non_strings() {
        let processor = EnvVarProcessor::legacy().unwrap();
        let mut value = json!({
            "number": 123,
            "boolean": true,
            "null": null
        });

        processor.substitute_value(&mut value).unwrap();

        // Values should be unchanged
        assert_eq!(
            value,
            json!({
                "number": 123,
                "boolean": true,
                "null": null
            })
        );
    }

    #[test]
    fn test_contains_patterns() {
        let processor = EnvVarProcessor::legacy().unwrap();

        assert!(processor.contains_patterns("${VAR}"));
        assert!(processor.contains_patterns("${VAR:-default}"));
        assert!(processor.contains_patterns("prefix ${VAR} suffix"));
        assert!(processor.contains_patterns("${VAR1} and ${VAR2}"));

        assert!(!processor.contains_patterns("no variables here"));
        assert!(!processor.contains_patterns("$VAR")); // Missing braces
        assert!(!processor.contains_patterns("{VAR}")); // Missing dollar sign
    }

    #[test]
    fn test_substitute_vars() {
        env::set_var("VARS_TEST", "substituted");

        let processor = EnvVarProcessor::legacy().unwrap();
        let mut vars = std::collections::HashMap::new();
        vars.insert("key1".to_string(), json!("${VARS_TEST}_value"));
        vars.insert("key2".to_string(), json!(42));
        vars.insert("key3".to_string(), json!({"nested": "${VARS_TEST}_nested"}));

        processor.substitute_vars(&mut vars).unwrap();

        assert_eq!(vars["key1"], json!("substituted_value"));
        assert_eq!(vars["key2"], json!(42)); // unchanged
        assert_eq!(vars["key3"]["nested"], json!("substituted_nested"));

        env::remove_var("VARS_TEST");
    }

    #[test]
    fn test_thread_local_functions() {
        env::set_var("THREAD_LOCAL_TEST", "thread_value");

        // Test legacy function
        let result = substitute_env_vars_legacy("Hello ${THREAD_LOCAL_TEST}!");
        assert_eq!(result, "Hello thread_value!");

        // Test missing var in legacy mode (should return empty string)
        let missing = substitute_env_vars_legacy("Hello ${MISSING}!");
        assert_eq!(missing, "Hello !");

        // Test strict function
        let result = substitute_env_vars_strict("Hello ${THREAD_LOCAL_TEST}!").unwrap();
        assert_eq!(result, "Hello thread_value!");

        // Test missing var in strict mode (should return error)
        let missing = substitute_env_vars_strict("Hello ${MISSING}!");
        assert!(missing.is_err());

        // Test contains patterns
        assert!(contains_env_patterns("${THREAD_LOCAL_TEST}"));
        assert!(!contains_env_patterns("no patterns"));

        env::remove_var("THREAD_LOCAL_TEST");
    }

    #[test]
    fn test_exact_compatibility_with_legacy() {
        env::set_var("COMPAT_VAR", "compat_value");

        // Test patterns that should match legacy behavior exactly
        let test_cases = vec![
            ("${COMPAT_VAR}", "compat_value"),
            ("${COMPAT_VAR:-default}", "compat_value"),
            ("${MISSING_VAR:-default}", "default"),
            ("${MISSING_VAR}", ""), // Empty string in legacy mode
            ("prefix_${COMPAT_VAR}_suffix", "prefix_compat_value_suffix"),
            ("${COMPAT_VAR}${COMPAT_VAR}", "compat_valuecompat_value"),
            ("no vars", "no vars"),
        ];

        for (input, expected) in test_cases {
            let result = substitute_env_vars_legacy(input);
            assert_eq!(
                result, expected,
                "Failed for input '{}': expected '{}', got '{}'",
                input, expected, result
            );
        }

        env::remove_var("COMPAT_VAR");
    }

    #[test]
    fn test_special_characters_in_defaults() {
        let processor = EnvVarProcessor::legacy().unwrap();

        let test_cases = vec![
            ("${MISSING:-}", ""),
            ("${MISSING:-default with spaces}", "default with spaces"),
            ("${MISSING:-default:with:colons}", "default:with:colons"),
            ("${MISSING:-default-with-dashes}", "default-with-dashes"),
            (
                "${MISSING:-default_with_underscores}",
                "default_with_underscores",
            ),
            ("${MISSING:-default/with/slashes}", "default/with/slashes"),
        ];

        for (input, expected) in test_cases {
            let result = processor.substitute_string(input).unwrap();
            assert_eq!(
                result, expected,
                "Failed for input '{}': expected '{}', got '{}'",
                input, expected, result
            );
        }
    }
}
