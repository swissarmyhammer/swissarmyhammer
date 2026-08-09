//! Environment variable loading utilities
//!
//! This module provides common patterns for loading environment variables
//! with type conversion and fallback defaults.

use std::env;
use std::str::FromStr;

/// Load an environment variable with a string default
pub fn load_env_string(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}

/// The spellings of a set environment variable that mean "true".
///
/// Compared against the trimmed, ASCII-lowercased value, so `TRUE`, `Yes`, and
/// ` on ` all count.
const TRUTHY_ENV_VALUES: [&str; 4] = ["1", "true", "yes", "on"];

/// Load an environment variable as a boolean flag, falling back to `default`
/// when the variable is not set.
///
/// This is the one truthy-value parser for the workspace. A set variable is
/// true when it spells one of [`TRUTHY_ENV_VALUES`] and false otherwise, so
/// `0`, `false`, the empty string, and any unrecognised word all read as
/// false. An unset variable takes `default`, which lets a caller express both
/// an opt-in toggle (`default = false`) and an opt-out one (`default = true`).
///
/// ```
/// use swissarmyhammer_common::env_loader::load_env_bool;
/// use swissarmyhammer_common::test_utils::EnvVarGuard;
///
/// let _guard = EnvVarGuard::set("SAH_DOC_EXAMPLE_FLAG", "TRUE");
/// assert!(load_env_bool("SAH_DOC_EXAMPLE_FLAG", false));
/// ```
pub fn load_env_bool(key: &str, default: bool) -> bool {
    match env::var(key) {
        Ok(value) => TRUTHY_ENV_VALUES.contains(&value.trim().to_ascii_lowercase().as_str()),
        Err(_) => default,
    }
}

/// Load an environment variable with type conversion and default
pub fn load_env_parsed<T>(key: &str, default: T) -> T
where
    T: FromStr,
{
    env::var(key)
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

/// Load an environment variable as an `Option<T>`
pub fn load_env_optional<T>(key: &str) -> Option<T>
where
    T: FromStr,
{
    env::var(key).ok().and_then(|v| v.parse().ok())
}

/// Load an environment variable with validation
pub fn load_env_validated<T, F>(key: &str, default: T, validator: F) -> T
where
    T: FromStr + Clone,
    F: Fn(&T) -> bool,
{
    let value = load_env_parsed(key, default.clone());
    if validator(&value) {
        value
    } else {
        default
    }
}

/// Builder for loading multiple environment variables with consistent prefix
#[derive(Debug)]
pub struct EnvLoader {
    prefix: String,
}

impl EnvLoader {
    /// Create a new environment loader with the given prefix
    pub fn new(prefix: &str) -> Self {
        Self {
            prefix: prefix.to_string(),
        }
    }

    /// Load a string value with default
    pub fn load_string(&self, suffix: &str, default: &str) -> String {
        let key = format!("{}_{}", self.prefix, suffix);
        load_env_string(&key, default)
    }

    /// Load a parsed value with default
    pub fn load_parsed<T>(&self, suffix: &str, default: T) -> T
    where
        T: FromStr,
    {
        let key = format!("{}_{}", self.prefix, suffix);
        load_env_parsed(&key, default)
    }

    /// Load an optional value
    pub fn load_optional<T>(&self, suffix: &str) -> Option<T>
    where
        T: FromStr,
    {
        let key = format!("{}_{}", self.prefix, suffix);
        load_env_optional(&key)
    }

    /// Load a validated value
    pub fn load_validated<T, F>(&self, suffix: &str, default: T, validator: F) -> T
    where
        T: FromStr + Clone,
        F: Fn(&T) -> bool,
    {
        let key = format!("{}_{}", self.prefix, suffix);
        load_env_validated(&key, default, validator)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::EnvVarGuard;

    #[test]
    fn test_load_env_string() {
        let key = "TEST_STRING_VAR";
        let default = "default_value";

        // Test with missing environment variable
        env::remove_var(key);
        assert_eq!(load_env_string(key, default), default);

        // Test with present environment variable
        env::set_var(key, "test_value");
        assert_eq!(load_env_string(key, default), "test_value");

        // Clean up
        env::remove_var(key);
    }

    #[test]
    fn test_load_env_parsed() {
        let key = "TEST_PARSED_VAR";
        let default = 42u32;

        // Test with missing environment variable
        env::remove_var(key);
        assert_eq!(load_env_parsed(key, default), default);

        // Test with valid value
        env::set_var(key, "123");
        assert_eq!(load_env_parsed::<u32>(key, default), 123);

        // Test with invalid value (should return default)
        env::set_var(key, "invalid");
        assert_eq!(load_env_parsed(key, default), default);

        // Clean up
        env::remove_var(key);
    }

    #[test]
    fn test_load_env_optional() {
        let key = "TEST_OPTIONAL_VAR";

        // Test with missing environment variable
        env::remove_var(key);
        assert_eq!(load_env_optional::<u32>(key), None);

        // Test with valid value
        env::set_var(key, "456");
        assert_eq!(load_env_optional::<u32>(key), Some(456));

        // Test with invalid value
        env::set_var(key, "invalid");
        assert_eq!(load_env_optional::<u32>(key), None);

        // Clean up
        env::remove_var(key);
    }

    #[test]
    fn test_load_env_validated() {
        let key = "TEST_VALIDATED_VAR";
        let default = 10u32;
        let validator = |v: &u32| *v > 0 && *v < 100;

        // Test with missing environment variable
        env::remove_var(key);
        assert_eq!(load_env_validated(key, default, validator), default);

        // Test with valid value
        env::set_var(key, "50");
        assert_eq!(load_env_validated(key, default, validator), 50);

        // Test with invalid value (out of range)
        env::set_var(key, "150");
        assert_eq!(load_env_validated(key, default, validator), default);

        // Clean up
        env::remove_var(key);
    }

    #[test]
    fn test_env_loader() {
        let loader = EnvLoader::new("SWISSARMYHAMMER_TEST");

        // Test string loading
        let key = "SWISSARMYHAMMER_TEST_STRING";
        env::remove_var(key);
        assert_eq!(loader.load_string("STRING", "default"), "default");

        env::set_var(key, "value");
        assert_eq!(loader.load_string("STRING", "default"), "value");

        // Test parsed loading
        let num_key = "SWISSARMYHAMMER_TEST_NUMBER";
        env::remove_var(num_key);
        assert_eq!(loader.load_parsed::<u32>("NUMBER", 42), 42);

        env::set_var(num_key, "123");
        assert_eq!(loader.load_parsed::<u32>("NUMBER", 42), 123);

        // Test optional loading
        env::remove_var(num_key);
        assert_eq!(loader.load_optional::<u32>("NUMBER"), None);

        env::set_var(num_key, "456");
        assert_eq!(loader.load_optional::<u32>("NUMBER"), Some(456));

        // Clean up
        env::remove_var(key);
        env::remove_var(num_key);
    }

    #[test]
    fn test_env_loader_validated() {
        let loader = EnvLoader::new("SWISSARMYHAMMER_TEST");
        let validator = |v: &u32| *v >= 1 && *v <= 10;

        let key = "SWISSARMYHAMMER_TEST_VALIDATED";
        env::remove_var(key);

        // Test with missing var (should use default)
        assert_eq!(loader.load_validated("VALIDATED", 5u32, validator), 5);

        // Test with valid value
        env::set_var(key, "7");
        assert_eq!(loader.load_validated("VALIDATED", 5u32, validator), 7);

        // Test with invalid value (should use default)
        env::set_var(key, "15");
        assert_eq!(loader.load_validated("VALIDATED", 5u32, validator), 5);

        // Clean up
        env::remove_var(key);
    }

    /// The variable the boolean-flag tests below scope with an [`EnvVarGuard`].
    const BOOL_KEY: &str = "SWISSARMYHAMMER_TEST_BOOL";

    #[test]
    #[serial_test::serial(env_loader_bool)]
    fn an_unset_variable_takes_the_default() {
        let _guard = EnvVarGuard::unset(BOOL_KEY);

        assert!(!load_env_bool(BOOL_KEY, false));
        assert!(load_env_bool(BOOL_KEY, true));
    }

    #[test]
    #[serial_test::serial(env_loader_bool)]
    fn every_truthy_spelling_reads_as_true() {
        for value in TRUTHY_ENV_VALUES {
            let _guard = EnvVarGuard::set(BOOL_KEY, value);

            assert!(load_env_bool(BOOL_KEY, false), "{value} must read as true");
        }
    }

    #[test]
    #[serial_test::serial(env_loader_bool)]
    fn a_truthy_spelling_is_matched_without_regard_to_case_or_padding() {
        for value in ["TRUE", "Yes", " On "] {
            let _guard = EnvVarGuard::set(BOOL_KEY, value);

            assert!(load_env_bool(BOOL_KEY, false), "{value} must read as true");
        }
    }

    #[test]
    #[serial_test::serial(env_loader_bool)]
    fn a_set_variable_that_is_not_truthy_reads_as_false() {
        for value in ["0", "false", "off", "", "banana"] {
            let _guard = EnvVarGuard::set(BOOL_KEY, value);

            assert!(
                !load_env_bool(BOOL_KEY, true),
                "{value} must read as false even against a true default"
            );
        }
    }
}
