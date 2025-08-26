use crate::error::{ConfigurationError, ConfigurationResult};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use std::env;
use tracing::debug;

/// Environment variable substitution utility
pub struct EnvVarSubstitution {
    env_var_regex: Regex,
}

impl EnvVarSubstitution {
    /// Create a new environment variable substitution instance
    pub fn new() -> ConfigurationResult<Self> {
        let env_var_regex = Regex::new(r"\$\{([^}:]+)(?::-([^}]*))?\}")
            .map_err(|e| ConfigurationError::env_var(format!("Failed to compile regex: {}", e)))?;

        Ok(Self { env_var_regex })
    }

    /// Substitute environment variables in a JSON value
    ///
    /// Supports patterns:
    /// - `${VAR_NAME}` - Replace with environment variable value, empty string if not set
    /// - `${VAR_NAME:-default}` - Replace with environment variable value, or default if not set
    pub fn substitute_in_value(&self, value: Value) -> ConfigurationResult<Value> {
        match value {
            Value::String(s) => {
                let substituted = self.substitute_in_string(&s)?;
                Ok(Value::String(substituted))
            }
            Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(self.substitute_in_value(item)?);
                }
                Ok(Value::Array(new_arr))
            }
            Value::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (key, val) in obj {
                    new_obj.insert(key, self.substitute_in_value(val)?);
                }
                Ok(Value::Object(new_obj))
            }
            // Other types (Number, Bool, Null) don't need substitution
            other => Ok(other),
        }
    }

    /// Substitute environment variables in a string
    pub fn substitute_in_string(&self, s: &str) -> ConfigurationResult<String> {
        let result = self.env_var_regex.replace_all(s, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match env::var(var_name) {
                Ok(value) => {
                    debug!("Substituting environment variable {}: {}", var_name, value);
                    value
                }
                Err(_) => {
                    // Check if we have a default value (pattern was ${VAR:-default})
                    if let Some(default_match) = caps.get(2) {
                        let default_value = default_match.as_str();
                        debug!(
                            "Using default value for environment variable {}: {}",
                            var_name, default_value
                        );
                        default_value.to_string()
                    } else {
                        debug!(
                            "Environment variable {} not set, using empty string",
                            var_name
                        );
                        String::new() // No default, return empty string
                    }
                }
            }
        });

        Ok(result.to_string())
    }

    /// Substitute environment variables in a hashmap of values
    pub fn substitute_in_map(
        &self,
        map: HashMap<String, Value>,
    ) -> ConfigurationResult<HashMap<String, Value>> {
        let mut result = HashMap::new();

        for (key, value) in map {
            result.insert(key, self.substitute_in_value(value)?);
        }

        Ok(result)
    }

    /// Get environment variables with SAH_ and SWISSARMYHAMMER_ prefixes
    pub fn get_sah_env_vars(&self) -> HashMap<String, String> {
        let mut env_vars = HashMap::new();

        for (key, value) in env::vars() {
            if let Some(suffix) = key.strip_prefix("SAH_") {
                // Convert SAH_FOO_BAR to foo.bar
                let config_key = suffix.to_lowercase().replace('_', ".");
                debug!("Found SAH environment variable: {} -> {}", key, config_key);
                env_vars.insert(config_key, value);
            } else if let Some(suffix) = key.strip_prefix("SWISSARMYHAMMER_") {
                // Convert SWISSARMYHAMMER_FOO_BAR to foo.bar
                let config_key = suffix.to_lowercase().replace('_', ".");
                debug!(
                    "Found SWISSARMYHAMMER environment variable: {} -> {}",
                    key, config_key
                );
                env_vars.insert(config_key, value);
            }
        }

        env_vars
    }
}

impl Default for EnvVarSubstitution {
    fn default() -> Self {
        Self::new().expect("Failed to create default EnvVarSubstitution")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_substitute_in_string() {
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another_value");

        let substitution = EnvVarSubstitution::new().unwrap();

        // Test simple substitution
        let result = substitution
            .substitute_in_string("Hello ${TEST_VAR}!")
            .unwrap();
        assert_eq!(result, "Hello test_value!");

        // Test with default value (should use env var)
        let result = substitution
            .substitute_in_string("Hello ${TEST_VAR:-default}!")
            .unwrap();
        assert_eq!(result, "Hello test_value!");

        // Test with missing var and default
        let result = substitution
            .substitute_in_string("Hello ${MISSING_VAR:-default_value}!")
            .unwrap();
        assert_eq!(result, "Hello default_value!");

        // Test with missing var and no default
        let result = substitution
            .substitute_in_string("Hello ${MISSING_VAR}!")
            .unwrap();
        assert_eq!(result, "Hello !");

        // Test multiple substitutions
        let result = substitution
            .substitute_in_string("${TEST_VAR} and ${ANOTHER_VAR}")
            .unwrap();
        assert_eq!(result, "test_value and another_value");

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");
    }

    #[test]
    fn test_substitute_in_value() {
        env::set_var("PROJECT_NAME", "MyProject");
        env::set_var("VERSION", "1.0.0");

        let substitution = EnvVarSubstitution::new().unwrap();

        // Test string substitution
        let value = json!("${PROJECT_NAME} v${VERSION}");
        let result = substitution.substitute_in_value(value).unwrap();
        assert_eq!(result, json!("MyProject v1.0.0"));

        // Test array substitution
        let value = json!(["${PROJECT_NAME}-server1", "${PROJECT_NAME}-server2"]);
        let result = substitution.substitute_in_value(value).unwrap();
        assert_eq!(result, json!(["MyProject-server1", "MyProject-server2"]));

        // Test object substitution
        let value = json!({
            "title": "${PROJECT_NAME} v${VERSION}",
            "debug": true,
            "servers": ["${PROJECT_NAME}-server"]
        });
        let result = substitution.substitute_in_value(value).unwrap();
        assert_eq!(
            result,
            json!({
                "title": "MyProject v1.0.0",
                "debug": true,
                "servers": ["MyProject-server"]
            })
        );

        env::remove_var("PROJECT_NAME");
        env::remove_var("VERSION");
    }

    #[test]
    fn test_get_sah_env_vars() {
        // Use unique variable names to avoid test interference
        env::set_var("SAH_TEST_PROJECT", "SahProject");
        env::set_var("SAH_TEST_MODE", "true");
        env::set_var("SWISSARMYHAMMER_TEST_TIMEOUT", "30");
        env::set_var("SWISSARMYHAMMER_TEST_KEY", "secret");
        env::set_var("OTHER_VAR", "ignored"); // Should be ignored

        let substitution = EnvVarSubstitution::new().unwrap();
        let env_vars = substitution.get_sah_env_vars();

        assert_eq!(
            env_vars.get("test.project"),
            Some(&"SahProject".to_string())
        );
        assert_eq!(env_vars.get("test.mode"), Some(&"true".to_string()));
        assert_eq!(env_vars.get("test.timeout"), Some(&"30".to_string()));
        assert_eq!(env_vars.get("test.key"), Some(&"secret".to_string()));
        assert!(!env_vars.contains_key("other.var"));

        env::remove_var("SAH_TEST_PROJECT");
        env::remove_var("SAH_TEST_MODE");
        env::remove_var("SWISSARMYHAMMER_TEST_TIMEOUT");
        env::remove_var("SWISSARMYHAMMER_TEST_KEY");
        env::remove_var("OTHER_VAR");
    }

    #[test]
    fn test_substitute_in_map() {
        env::set_var("TEST_VALUE", "substituted");

        let substitution = EnvVarSubstitution::new().unwrap();
        let mut map = HashMap::new();
        map.insert("key1".to_string(), json!("${TEST_VALUE}"));
        map.insert("key2".to_string(), json!(42)); // Should remain unchanged

        let result = substitution.substitute_in_map(map).unwrap();

        assert_eq!(result.get("key1").unwrap(), &json!("substituted"));
        assert_eq!(result.get("key2").unwrap(), &json!(42));

        env::remove_var("TEST_VALUE");
    }
}
