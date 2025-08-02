use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Represents a configuration value from sah.toml
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
}

/// Main configuration structure containing all sah.toml variables
#[derive(Debug, Clone)]
pub struct Configuration {
    /// The parsed configuration values
    values: HashMap<String, ConfigValue>,
    /// Path to the configuration file (if loaded from file)
    file_path: Option<PathBuf>,
}

impl Configuration {
    /// Create a new empty configuration
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
            file_path: None,
        }
    }

    /// Create a configuration with values and file path
    pub fn with_values(values: HashMap<String, ConfigValue>, file_path: Option<PathBuf>) -> Self {
        Self { values, file_path }
    }

    /// Get a configuration value by key
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Get all configuration values
    pub fn values(&self) -> &HashMap<String, ConfigValue> {
        &self.values
    }

    /// Get the file path if this configuration was loaded from a file
    pub fn file_path(&self) -> Option<&PathBuf> {
        self.file_path.as_ref()
    }

    /// Insert a new configuration value
    pub fn insert(&mut self, key: String, value: ConfigValue) {
        self.values.insert(key, value);
    }

    /// Check if the configuration is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of configuration values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Convert all configuration values to liquid objects for template rendering
    pub fn to_liquid_object(&self) -> liquid::model::Object {
        let mut object = liquid::model::Object::new();
        for (key, value) in &self.values {
            object.insert(key.clone().into(), value.to_liquid_value());
        }
        object
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        // Test array conversion
        let array_val = ConfigValue::Array(vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::Integer(2),
        ]);
        let liquid_array = array_val.to_liquid_value();
        assert!(matches!(liquid_array, liquid::model::Value::Array(_)));
    }

    #[test]
    fn test_configuration_operations() {
        let mut config = Configuration::new();
        assert!(config.is_empty());
        assert_eq!(config.len(), 0);

        config.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        assert!(!config.is_empty());
        assert_eq!(config.len(), 1);

        let value = config.get("key1");
        assert!(value.is_some());
        assert_eq!(value.unwrap(), &ConfigValue::String("value1".to_string()));
    }

    #[test]
    fn test_config_value_type_names() {
        assert_eq!(
            ConfigValue::String("test".to_string()).type_name(),
            "string"
        );
        assert_eq!(ConfigValue::Integer(42).type_name(), "integer");
        assert_eq!(ConfigValue::Float(3.15).type_name(), "float"); // Using 3.15 to avoid clippy PI warning
        assert_eq!(ConfigValue::Boolean(true).type_name(), "boolean");
        assert_eq!(ConfigValue::Array(vec![]).type_name(), "array");
        assert_eq!(ConfigValue::Table(HashMap::new()).type_name(), "table");
    }
}
