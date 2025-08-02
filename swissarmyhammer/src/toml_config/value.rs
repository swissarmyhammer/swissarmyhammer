use crate::toml_config::error::{ConfigError, ValidationLimits};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::env;
use std::sync::OnceLock;

/// Represents a configuration value from sah.toml with full TOML type support
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// String value from configuration
    String(String),
    /// Integer value from configuration
    Integer(i64),
    /// Floating point value from configuration
    Float(f64),
    /// Boolean value from configuration
    Boolean(bool),
    /// Array of configuration values
    Array(Vec<ConfigValue>),
    /// Table (map) of configuration values
    Table(HashMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Convert ConfigValue to liquid::model::Value for template rendering
    pub fn to_liquid_value(&self) -> liquid::model::Value {
        match self {
            ConfigValue::String(s) => liquid::model::Value::scalar(s.clone()),
            ConfigValue::Integer(i) => liquid::model::Value::scalar(*i),
            ConfigValue::Float(f) => liquid::model::Value::scalar(*f),
            ConfigValue::Boolean(b) => liquid::model::Value::scalar(*b),
            ConfigValue::Array(arr) => {
                let liquid_array: Vec<liquid::model::Value> =
                    arr.iter().map(|v| v.to_liquid_value()).collect();
                liquid::model::Value::Array(liquid_array)
            }
            ConfigValue::Table(table) => {
                let mut liquid_object = liquid::model::Object::new();
                for (key, value) in table {
                    liquid_object.insert(key.clone().into(), value.to_liquid_value());
                }
                liquid::model::Value::Object(liquid_object)
            }
        }
    }

    /// Convert to JSON value for template processing
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            ConfigValue::String(s) => serde_json::Value::String(s.clone()),
            ConfigValue::Integer(i) => serde_json::Value::Number((*i).into()),
            ConfigValue::Float(f) => serde_json::Value::Number(
                serde_json::Number::from_f64(*f).unwrap_or_else(|| 0.into()),
            ),
            ConfigValue::Boolean(b) => serde_json::Value::Bool(*b),
            ConfigValue::Array(arr) => {
                let json_array: Vec<serde_json::Value> =
                    arr.iter().map(|v| v.to_json_value()).collect();
                serde_json::Value::Array(json_array)
            }
            ConfigValue::Table(table) => {
                let mut json_object = serde_json::Map::new();
                for (key, value) in table {
                    json_object.insert(key.clone(), value.to_json_value());
                }
                serde_json::Value::Object(json_object)
            }
        }
    }

    /// Get the type name as a string for error messages
    pub fn type_name(&self) -> &'static str {
        match self {
            ConfigValue::String(_) => "string",
            ConfigValue::Integer(_) => "integer",
            ConfigValue::Float(_) => "float",
            ConfigValue::Boolean(_) => "boolean",
            ConfigValue::Array(_) => "array",
            ConfigValue::Table(_) => "table",
        }
    }

    /// Attempt to coerce this value to a string
    pub fn coerce_to_string(&self) -> Result<String, ConfigError> {
        match self {
            ConfigValue::String(s) => Ok(s.clone()),
            ConfigValue::Integer(i) => Ok(i.to_string()),
            ConfigValue::Float(f) => Ok(f.to_string()),
            ConfigValue::Boolean(b) => Ok(b.to_string()),
            _ => Err(ConfigError::type_coercion(
                self.type_name().to_string(),
                "string".to_string(),
            )),
        }
    }

    /// Attempt to coerce this value to an integer
    pub fn coerce_to_integer(&self) -> Result<i64, ConfigError> {
        match self {
            ConfigValue::Integer(i) => Ok(*i),
            ConfigValue::Float(f) => Ok(*f as i64),
            ConfigValue::String(s) => s.parse().map_err(|_| {
                ConfigError::type_coercion(self.type_name().to_string(), "integer".to_string())
            }),
            ConfigValue::Boolean(b) => Ok(if *b { 1 } else { 0 }),
            _ => Err(ConfigError::type_coercion(
                self.type_name().to_string(),
                "integer".to_string(),
            )),
        }
    }

    /// Attempt to coerce this value to a float
    pub fn coerce_to_float(&self) -> Result<f64, ConfigError> {
        match self {
            ConfigValue::Float(f) => Ok(*f),
            ConfigValue::Integer(i) => Ok(*i as f64),
            ConfigValue::String(s) => s.parse().map_err(|_| {
                ConfigError::type_coercion(self.type_name().to_string(), "float".to_string())
            }),
            _ => Err(ConfigError::type_coercion(
                self.type_name().to_string(),
                "float".to_string(),
            )),
        }
    }

    /// Attempt to coerce this value to a boolean
    pub fn coerce_to_boolean(&self) -> Result<bool, ConfigError> {
        match self {
            ConfigValue::Boolean(b) => Ok(*b),
            ConfigValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "on" | "1" => Ok(true),
                "false" | "no" | "off" | "0" => Ok(false),
                _ => Err(ConfigError::type_coercion(
                    self.type_name().to_string(),
                    "boolean".to_string(),
                )),
            },
            ConfigValue::Integer(i) => Ok(*i != 0),
            _ => Err(ConfigError::type_coercion(
                self.type_name().to_string(),
                "boolean".to_string(),
            )),
        }
    }

    /// Validate this value against size and nesting limits
    pub fn validate(&self, current_depth: usize) -> Result<(), ConfigError> {
        if current_depth > ValidationLimits::MAX_NESTING_DEPTH {
            return Err(ConfigError::nesting_too_deep(
                current_depth,
                ValidationLimits::MAX_NESTING_DEPTH,
            ));
        }

        match self {
            ConfigValue::String(s) => {
                if s.len() > ValidationLimits::MAX_STRING_SIZE {
                    return Err(ConfigError::string_too_large(
                        s.len(),
                        ValidationLimits::MAX_STRING_SIZE,
                    ));
                }
            }
            ConfigValue::Array(arr) => {
                if arr.len() > ValidationLimits::MAX_ARRAY_SIZE {
                    return Err(ConfigError::array_too_large(
                        arr.len(),
                        ValidationLimits::MAX_ARRAY_SIZE,
                    ));
                }
                for value in arr {
                    value.validate(current_depth + 1)?;
                }
            }
            ConfigValue::Table(table) => {
                for value in table.values() {
                    value.validate(current_depth + 1)?;
                }
            }
            _ => {} // Other types don't need validation
        }

        Ok(())
    }

    /// Process environment variable substitution in string values
    pub fn substitute_env_vars(&mut self) -> Result<(), ConfigError> {
        match self {
            ConfigValue::String(s) => {
                *s = substitute_env_vars_in_string(s)?;
            }
            ConfigValue::Array(arr) => {
                for value in arr {
                    value.substitute_env_vars()?;
                }
            }
            ConfigValue::Table(table) => {
                for value in table.values_mut() {
                    value.substitute_env_vars()?;
                }
            }
            _ => {} // Other types don't contain strings to substitute
        }
        Ok(())
    }
}

/// Environment variable substitution pattern regex
static ENV_VAR_REGEX: OnceLock<Regex> = OnceLock::new();

/// Get the compiled regex for environment variable substitution
fn get_env_var_regex() -> &'static Regex {
    ENV_VAR_REGEX.get_or_init(|| {
        Regex::new(r"\$\{([A-Z_][A-Z0-9_]*)(:-([^}]*))?\}")
            .expect("Invalid environment variable regex")
    })
}

/// Substitute environment variables in a string
/// Supports patterns like ${VAR} and ${VAR:-default}
fn substitute_env_vars_in_string(input: &str) -> Result<String, ConfigError> {
    let regex = get_env_var_regex();
    let mut result = input.to_string();
    let mut errors = Vec::new();

    for captures in regex.captures_iter(input) {
        let full_match = captures.get(0).unwrap().as_str();
        let var_name = captures.get(1).unwrap().as_str();
        let default_value = captures.get(3).map(|m| m.as_str());

        match env::var(var_name) {
            Ok(value) => {
                result = result.replace(full_match, &value);
            }
            Err(_) => {
                if let Some(default) = default_value {
                    result = result.replace(full_match, default);
                } else {
                    errors.push(ConfigError::env_var_substitution(
                        var_name.to_string(),
                        "Environment variable not found and no default provided".to_string(),
                    ));
                }
            }
        }
    }

    if !errors.is_empty() {
        return Err(errors.into_iter().next().unwrap());
    }

    Ok(result)
}

/// Convert from TOML value to ConfigValue
impl From<toml::Value> for ConfigValue {
    fn from(toml_value: toml::Value) -> Self {
        match toml_value {
            toml::Value::String(s) => ConfigValue::String(s),
            toml::Value::Integer(i) => ConfigValue::Integer(i),
            toml::Value::Float(f) => ConfigValue::Float(f),
            toml::Value::Boolean(b) => ConfigValue::Boolean(b),
            toml::Value::Array(arr) => {
                let config_array: Vec<ConfigValue> =
                    arr.into_iter().map(ConfigValue::from).collect();
                ConfigValue::Array(config_array)
            }
            toml::Value::Table(table) => {
                let mut config_table = HashMap::new();
                for (key, value) in table {
                    config_table.insert(key, ConfigValue::from(value));
                }
                ConfigValue::Table(config_table)
            }
            toml::Value::Datetime(dt) => ConfigValue::String(dt.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_value_to_liquid_value() {
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(
            string_val.to_liquid_value(),
            liquid::model::Value::scalar("test")
        );

        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.to_liquid_value(), liquid::model::Value::scalar(42));

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(
            bool_val.to_liquid_value(),
            liquid::model::Value::scalar(true)
        );

        let array_val = ConfigValue::Array(vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::Integer(2),
        ]);
        let liquid_array = array_val.to_liquid_value();
        assert!(matches!(liquid_array, liquid::model::Value::Array(_)));
    }

    #[test]
    fn test_config_value_type_coercion() {
        // Test string coercion
        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.coerce_to_string().unwrap(), "42");

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.coerce_to_string().unwrap(), "true");

        // Test integer coercion
        let string_val = ConfigValue::String("123".to_string());
        assert_eq!(string_val.coerce_to_integer().unwrap(), 123);

        let float_val = ConfigValue::Float(3.15);
        assert_eq!(float_val.coerce_to_integer().unwrap(), 3);

        // Test boolean coercion
        let string_true = ConfigValue::String("true".to_string());
        assert!(string_true.coerce_to_boolean().unwrap());

        let string_false = ConfigValue::String("false".to_string());
        assert!(!string_false.coerce_to_boolean().unwrap());

        let int_zero = ConfigValue::Integer(0);
        assert!(!int_zero.coerce_to_boolean().unwrap());

        let int_nonzero = ConfigValue::Integer(42);
        assert!(int_nonzero.coerce_to_boolean().unwrap());
    }

    #[test]
    fn test_config_value_validation() {
        // Test string size validation
        let large_string = "x".repeat(ValidationLimits::MAX_STRING_SIZE + 1);
        let string_val = ConfigValue::String(large_string);
        assert!(string_val.validate(0).is_err());

        // Test array size validation
        let large_array = vec![ConfigValue::Integer(1); ValidationLimits::MAX_ARRAY_SIZE + 1];
        let array_val = ConfigValue::Array(large_array);
        assert!(array_val.validate(0).is_err());

        // Test nesting depth validation
        let deep_table = create_deep_nested_table(ValidationLimits::MAX_NESTING_DEPTH + 1);
        assert!(deep_table.validate(0).is_err());
    }

    fn create_deep_nested_table(depth: usize) -> ConfigValue {
        if depth == 0 {
            ConfigValue::String("value".to_string())
        } else {
            let mut table = HashMap::new();
            table.insert("nested".to_string(), create_deep_nested_table(depth - 1));
            ConfigValue::Table(table)
        }
    }

    #[test]
    fn test_env_var_substitution() {
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another_value");

        // Test simple substitution
        let result = substitute_env_vars_in_string("${TEST_VAR}").unwrap();
        assert_eq!(result, "test_value");

        // Test substitution with default (var exists)
        let result = substitute_env_vars_in_string("${TEST_VAR:-default}").unwrap();
        assert_eq!(result, "test_value");

        // Test substitution with default (var doesn't exist)
        let result = substitute_env_vars_in_string("${NONEXISTENT_VAR:-default_value}").unwrap();
        assert_eq!(result, "default_value");

        // Test multiple substitutions
        let result = substitute_env_vars_in_string("${TEST_VAR} and ${ANOTHER_VAR}").unwrap();
        assert_eq!(result, "test_value and another_value");

        // Test substitution failure (no default)
        let result = substitute_env_vars_in_string("${NONEXISTENT_VAR}");
        assert!(result.is_err());

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");
    }

    #[test]
    fn test_config_value_substitute_env_vars() {
        env::set_var("TEST_CONFIG_VAR", "config_test_value");

        let mut string_val = ConfigValue::String("Value: ${TEST_CONFIG_VAR}".to_string());
        string_val.substitute_env_vars().unwrap();
        assert_eq!(
            string_val,
            ConfigValue::String("Value: config_test_value".to_string())
        );

        let mut array_val = ConfigValue::Array(vec![
            ConfigValue::String("${TEST_CONFIG_VAR}".to_string()),
            ConfigValue::Integer(42),
        ]);
        array_val.substitute_env_vars().unwrap();
        if let ConfigValue::Array(arr) = array_val {
            assert_eq!(arr[0], ConfigValue::String("config_test_value".to_string()));
            assert_eq!(arr[1], ConfigValue::Integer(42));
        } else {
            panic!("Expected array");
        }

        env::remove_var("TEST_CONFIG_VAR");
    }

    #[test]
    fn test_from_toml_value() {
        let toml_str = toml::Value::String("test".to_string());
        let config_val = ConfigValue::from(toml_str);
        assert_eq!(config_val, ConfigValue::String("test".to_string()));

        let toml_int = toml::Value::Integer(42);
        let config_val = ConfigValue::from(toml_int);
        assert_eq!(config_val, ConfigValue::Integer(42));
    }

    #[test]
    fn test_to_json_value() {
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(
            string_val.to_json_value(),
            serde_json::Value::String("test".to_string())
        );

        let int_val = ConfigValue::Integer(42);
        assert_eq!(
            int_val.to_json_value(),
            serde_json::Value::Number(42.into())
        );

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.to_json_value(), serde_json::Value::Bool(true));
    }
}
