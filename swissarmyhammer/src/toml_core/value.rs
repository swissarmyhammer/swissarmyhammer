use crate::toml_core::error::ConfigError;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    /// DateTime value (converted to string for template compatibility)
    DateTime(String),
}

impl ConfigValue {
    /// Convert ConfigValue to liquid::model::Value for template rendering
    pub fn to_liquid_value(&self) -> liquid::model::Value {
        match self {
            ConfigValue::String(s) => liquid::model::Value::scalar(s.clone()),
            ConfigValue::Integer(i) => liquid::model::Value::scalar(*i),
            ConfigValue::Float(f) => liquid::model::Value::scalar(*f),
            ConfigValue::Boolean(b) => liquid::model::Value::scalar(*b),
            ConfigValue::DateTime(dt) => liquid::model::Value::scalar(dt.clone()),
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

    /// Convert ConfigValue to serde_json::Value for JSON serialization
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            ConfigValue::String(s) => serde_json::Value::String(s.clone()),
            ConfigValue::Integer(i) => serde_json::Value::Number(serde_json::Number::from(*i)),
            ConfigValue::Float(f) => serde_json::Number::from_f64(*f)
                .map(serde_json::Value::Number)
                .unwrap_or(serde_json::Value::Null),
            ConfigValue::Boolean(b) => serde_json::Value::Bool(*b),
            ConfigValue::DateTime(dt) => serde_json::Value::String(dt.clone()),
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
            ConfigValue::DateTime(_) => "datetime",
        }
    }

    /// Try to coerce this value to a string
    pub fn coerce_to_string(&self) -> Result<String, ConfigError> {
        match self {
            ConfigValue::String(s) => Ok(s.clone()),
            ConfigValue::Integer(i) => Ok(i.to_string()),
            ConfigValue::Float(f) => Ok(f.to_string()),
            ConfigValue::Boolean(b) => Ok(b.to_string()),
            ConfigValue::DateTime(dt) => Ok(dt.clone()),
            _ => Err(ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "string".to_string(),
                field: "unknown".to_string(),
            }),
        }
    }

    /// Try to coerce this value to an integer
    pub fn coerce_to_integer(&self) -> Result<i64, ConfigError> {
        match self {
            ConfigValue::Integer(i) => Ok(*i),
            ConfigValue::Float(f) => Ok(*f as i64),
            ConfigValue::String(s) => s.parse().map_err(|_| ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "integer".to_string(),
                field: "unknown".to_string(),
            }),
            ConfigValue::Boolean(b) => Ok(if *b { 1 } else { 0 }),
            _ => Err(ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "integer".to_string(),
                field: "unknown".to_string(),
            }),
        }
    }

    /// Try to coerce this value to a float
    pub fn coerce_to_float(&self) -> Result<f64, ConfigError> {
        match self {
            ConfigValue::Float(f) => Ok(*f),
            ConfigValue::Integer(i) => Ok(*i as f64),
            ConfigValue::String(s) => s.parse().map_err(|_| ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "float".to_string(),
                field: "unknown".to_string(),
            }),
            _ => Err(ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "float".to_string(),
                field: "unknown".to_string(),
            }),
        }
    }

    /// Try to coerce this value to a boolean
    pub fn coerce_to_boolean(&self) -> Result<bool, ConfigError> {
        match self {
            ConfigValue::Boolean(b) => Ok(*b),
            ConfigValue::Integer(i) => Ok(*i != 0),
            ConfigValue::Float(f) => Ok(*f != 0.0),
            ConfigValue::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" | "on" => Ok(true),
                "false" | "no" | "0" | "off" => Ok(false),
                _ => Err(ConfigError::TypeCoercionFailed {
                    from_type: self.type_name().to_string(),
                    to_type: "boolean".to_string(),
                    field: "unknown".to_string(),
                }),
            },
            _ => Err(ConfigError::TypeCoercionFailed {
                from_type: self.type_name().to_string(),
                to_type: "boolean".to_string(),
                field: "unknown".to_string(),
            }),
        }
    }

    /// Try to get this value as an array
    pub fn as_array(&self) -> Option<&Vec<ConfigValue>> {
        match self {
            ConfigValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Try to get this value as a table
    pub fn as_table(&self) -> Option<&HashMap<String, ConfigValue>> {
        match self {
            ConfigValue::Table(table) => Some(table),
            _ => None,
        }
    }

    /// Check if this value is empty (empty string, empty array, empty table)
    pub fn is_empty(&self) -> bool {
        match self {
            ConfigValue::String(s) => s.is_empty(),
            ConfigValue::Array(arr) => arr.is_empty(),
            ConfigValue::Table(table) => table.is_empty(),
            _ => false,
        }
    }

    /// Get the size/length of this value (for arrays, tables, strings)
    pub fn size(&self) -> usize {
        match self {
            ConfigValue::String(s) => s.len(),
            ConfigValue::Array(arr) => arr.len(),
            ConfigValue::Table(table) => table.len(),
            _ => 0,
        }
    }

    /// Process environment variable substitution in this value
    pub fn substitute_env_vars(&mut self) -> Result<(), ConfigError> {
        match self {
            ConfigValue::String(s) => {
                *s = substitute_env_vars_in_string(s)?;
                Ok(())
            }
            ConfigValue::Array(arr) => {
                for item in arr.iter_mut() {
                    item.substitute_env_vars()?;
                }
                Ok(())
            }
            ConfigValue::Table(table) => {
                for value in table.values_mut() {
                    value.substitute_env_vars()?;
                }
                Ok(())
            }
            ConfigValue::DateTime(dt) => {
                *dt = substitute_env_vars_in_string(dt)?;
                Ok(())
            }
            // Other types don't need environment variable substitution
            _ => Ok(()),
        }
    }
}

impl From<toml::Value> for ConfigValue {
    fn from(value: toml::Value) -> Self {
        match value {
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
                let config_table: HashMap<String, ConfigValue> = table
                    .into_iter()
                    .map(|(key, value)| (key, ConfigValue::from(value)))
                    .collect();
                ConfigValue::Table(config_table)
            }
            toml::Value::Datetime(dt) => ConfigValue::DateTime(dt.to_string()),
        }
    }
}

/// Substitute environment variables in a string value
///
/// Supports patterns:
/// - `${VAR_NAME}` - Replace with environment variable value, return error if not set
/// - `${VAR_NAME:-default}` - Replace with environment variable value, or default if not set
fn substitute_env_vars_in_string(s: &str) -> Result<String, ConfigError> {
    use regex::Regex;

    thread_local! {
        static ENV_VAR_REGEX: Regex = Regex::new(r"\$\{([^}:]+)(?::-([^}]*))?\}")
            .expect("Failed to compile environment variable regex");
    }

    let mut result = s.to_string();
    let mut error = None;

    ENV_VAR_REGEX.with(|re| {
        result = re
            .replace_all(s, |caps: &regex::Captures| {
                let var_name = &caps[1];
                match std::env::var(var_name) {
                    Ok(value) => value,
                    Err(_) => {
                        // Check if we have a default value (pattern was ${VAR:-default})
                        if let Some(default_match) = caps.get(2) {
                            default_match.as_str().to_string()
                        } else {
                            // No default and variable not found - this is an error
                            error = Some(ConfigError::env_var_substitution(
                                var_name,
                                "Environment variable not found and no default provided",
                            ));
                            format!("${{{}}}", var_name) // Return original pattern
                        }
                    }
                }
            })
            .to_string();
    });

    match error {
        Some(err) => Err(err),
        None => Ok(result),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_value_to_liquid_value() {
        // Test string conversion
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(
            string_val.to_liquid_value(),
            liquid::model::Value::scalar("test")
        );

        // Test integer conversion
        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.to_liquid_value(), liquid::model::Value::scalar(42));

        // Test boolean conversion
        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(
            bool_val.to_liquid_value(),
            liquid::model::Value::scalar(true)
        );

        // Test datetime conversion
        let dt_val = ConfigValue::DateTime("2023-01-01T00:00:00Z".to_string());
        assert_eq!(
            dt_val.to_liquid_value(),
            liquid::model::Value::scalar("2023-01-01T00:00:00Z")
        );
    }

    #[test]
    fn test_config_value_to_json_value() {
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(string_val.to_json_value(), serde_json::json!("test"));

        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.to_json_value(), serde_json::json!(42));

        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(bool_val.to_json_value(), serde_json::json!(true));
    }

    #[test]
    fn test_type_coercion() {
        // String to integer
        let string_val = ConfigValue::String("42".to_string());
        assert_eq!(string_val.coerce_to_integer().unwrap(), 42);

        // Integer to string
        let int_val = ConfigValue::Integer(42);
        assert_eq!(int_val.coerce_to_string().unwrap(), "42");

        // Boolean coercion
        let true_str = ConfigValue::String("true".to_string());
        assert!(true_str.coerce_to_boolean().unwrap());

        let false_str = ConfigValue::String("false".to_string());
        assert!(!false_str.coerce_to_boolean().unwrap());

        // Invalid coercion should fail
        let array_val = ConfigValue::Array(vec![]);
        assert!(array_val.coerce_to_string().is_err());
    }

    #[test]
    fn test_value_properties() {
        let empty_string = ConfigValue::String("".to_string());
        assert!(empty_string.is_empty());

        let non_empty_string = ConfigValue::String("hello".to_string());
        assert!(!non_empty_string.is_empty());
        assert_eq!(non_empty_string.size(), 5);

        let empty_array = ConfigValue::Array(vec![]);
        assert!(empty_array.is_empty());

        let non_empty_array = ConfigValue::Array(vec![ConfigValue::Integer(1)]);
        assert!(!non_empty_array.is_empty());
        assert_eq!(non_empty_array.size(), 1);
    }

    #[test]
    fn test_env_var_substitution() {
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another_value");

        // Test simple substitution
        let result = substitute_env_vars_in_string("Hello ${TEST_VAR}!");
        assert_eq!(result.unwrap(), "Hello test_value!");

        // Test with default value (should use env var)
        let result = substitute_env_vars_in_string("Hello ${TEST_VAR:-default}!");
        assert_eq!(result.unwrap(), "Hello test_value!");

        // Test with missing var and default
        let result = substitute_env_vars_in_string("Hello ${MISSING_VAR:-default_value}!");
        assert_eq!(result.unwrap(), "Hello default_value!");

        // Test with missing var and no default (should error)
        let result = substitute_env_vars_in_string("Hello ${MISSING_VAR}!");
        assert!(result.is_err());

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");
    }

    #[test]
    fn test_config_value_env_var_substitution() {
        env::set_var("PROJECT_NAME", "MyProject");

        let mut config_val = ConfigValue::String("Project: ${PROJECT_NAME}".to_string());
        config_val.substitute_env_vars().unwrap();

        assert_eq!(
            config_val,
            ConfigValue::String("Project: MyProject".to_string())
        );

        env::remove_var("PROJECT_NAME");
    }

    #[test]
    fn test_from_toml_value() {
        // Test string conversion
        let toml_str = toml::Value::String("test".to_string());
        let config_val = ConfigValue::from(toml_str);
        assert_eq!(config_val, ConfigValue::String("test".to_string()));

        // Test array conversion
        let toml_array = toml::Value::Array(vec![
            toml::Value::String("item1".to_string()),
            toml::Value::Integer(42),
        ]);
        let config_val = ConfigValue::from(toml_array);
        assert_eq!(
            config_val,
            ConfigValue::Array(vec![
                ConfigValue::String("item1".to_string()),
                ConfigValue::Integer(42)
            ])
        );
    }

    #[test]
    fn test_type_names() {
        assert_eq!(
            ConfigValue::String("test".to_string()).type_name(),
            "string"
        );
        assert_eq!(ConfigValue::Integer(42).type_name(), "integer");
        assert_eq!(ConfigValue::Float(3.15).type_name(), "float");
        assert_eq!(ConfigValue::Boolean(true).type_name(), "boolean");
        assert_eq!(ConfigValue::Array(vec![]).type_name(), "array");
        assert_eq!(ConfigValue::Table(HashMap::new()).type_name(), "table");
        assert_eq!(
            ConfigValue::DateTime("2023-01-01T00:00:00Z".to_string()).type_name(),
            "datetime"
        );
    }
}
