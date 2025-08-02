use crate::toml_config::error::ConfigError;
use crate::toml_config::value::ConfigValue;
use std::collections::HashMap;
use std::path::PathBuf;

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

    /// Get a configuration value by key (supports dot notation)
    ///
    /// # Arguments
    /// * `key` - The key to look up, supports dot notation like "database.host"
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer::config::{Configuration, ConfigValue};
    /// use std::collections::HashMap;
    ///
    /// let mut config = Configuration::new();
    /// let mut db_table = HashMap::new();
    /// db_table.insert("host".to_string(), ConfigValue::String("localhost".to_string()));
    /// config.insert("database".to_string(), ConfigValue::Table(db_table));
    ///
    /// // Access with dot notation
    /// let host = config.get("database.host");
    /// assert!(host.is_some());
    /// ```
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        if key.contains('.') {
            self.get_nested(key)
        } else {
            self.values.get(key)
        }
    }

    /// Get a nested configuration value using dot notation
    fn get_nested(&self, key: &str) -> Option<&ConfigValue> {
        let parts: Vec<&str> = key.split('.').collect();
        let mut current_value = self.values.get(parts[0])?;

        for part in &parts[1..] {
            match current_value {
                ConfigValue::Table(table) => {
                    current_value = table.get(*part)?;
                }
                _ => return None,
            }
        }

        Some(current_value)
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

    /// Set a configuration value using dot notation
    ///
    /// # Arguments
    /// * `key` - The key to set, supports dot notation like "database.host"
    /// * `value` - The value to set
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer::config::{Configuration, ConfigValue};
    ///
    /// let mut config = Configuration::new();
    /// config.set("database.host".to_string(), ConfigValue::String("localhost".to_string()));
    ///
    /// let host = config.get("database.host");
    /// assert!(host.is_some());
    /// ```
    pub fn set(&mut self, key: String, value: ConfigValue) {
        if key.contains('.') {
            self.set_nested(key, value);
        } else {
            self.values.insert(key, value);
        }
    }

    /// Set a nested configuration value using dot notation
    fn set_nested(&mut self, key: String, value: ConfigValue) {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.is_empty() {
            return;
        }

        let first_key = parts[0].to_string();

        if parts.len() == 1 {
            self.values.insert(first_key, value);
            return;
        }

        // Ensure the first level exists as a table
        if !self.values.contains_key(&first_key) {
            self.values
                .insert(first_key.clone(), ConfigValue::Table(HashMap::new()));
        }

        // Navigate and create nested structure
        let mut current_table = match self.values.get_mut(&first_key) {
            Some(ConfigValue::Table(table)) => table,
            _ => {
                // Replace non-table value with table
                self.values
                    .insert(first_key.clone(), ConfigValue::Table(HashMap::new()));
                match self.values.get_mut(&first_key).unwrap() {
                    ConfigValue::Table(table) => table,
                    _ => unreachable!(),
                }
            }
        };

        // Navigate through intermediate levels
        for part in &parts[1..parts.len() - 1] {
            let part_key = part.to_string();
            
            // Ensure the key exists as a table or create it
            match current_table.get(&part_key) {
                Some(ConfigValue::Table(_)) => {
                    // Already a table, continue
                }
                _ => {
                    // Replace non-table value or create new table
                    current_table.insert(part_key.clone(), ConfigValue::Table(HashMap::new()));
                }
            }

            // Now safely get the mutable reference
            current_table = match current_table.get_mut(&part_key).unwrap() {
                ConfigValue::Table(table) => table,
                _ => unreachable!(),
            };
        }

        // Set the final value
        let final_key = parts[parts.len() - 1].to_string();
        current_table.insert(final_key, value);
    }

    /// Check if the configuration contains a key (supports dot notation)
    pub fn contains_key(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    /// Remove a configuration value by key (supports dot notation)
    pub fn remove(&mut self, key: &str) -> Option<ConfigValue> {
        if key.contains('.') {
            self.remove_nested(key)
        } else {
            self.values.remove(key)
        }
    }

    /// Remove a nested configuration value using dot notation
    fn remove_nested(&mut self, key: &str) -> Option<ConfigValue> {
        let parts: Vec<&str> = key.split('.').collect();
        if parts.len() < 2 {
            return self.values.remove(key);
        }

        let mut current_table = match self.values.get_mut(parts[0]) {
            Some(ConfigValue::Table(table)) => table,
            _ => return None,
        };

        // Navigate to parent of target
        for part in &parts[1..parts.len() - 1] {
            current_table = match current_table.get_mut(*part) {
                Some(ConfigValue::Table(table)) => table,
                _ => return None,
            };
        }

        // Remove the final key
        let final_key = parts[parts.len() - 1];
        current_table.remove(final_key)
    }

    /// Check if the configuration is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of top-level configuration values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Get all keys in the configuration (including nested keys with dot notation)
    pub fn keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        self.collect_keys("", &self.values, &mut keys);
        keys
    }

    /// Recursively collect all keys including nested ones
    fn collect_keys(
        &self,
        prefix: &str,
        values: &HashMap<String, ConfigValue>,
        keys: &mut Vec<String>,
    ) {
        for (key, value) in values {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{}.{}", prefix, key)
            };

            keys.push(full_key.clone());

            if let ConfigValue::Table(table) = value {
                self.collect_keys(&full_key, table, keys);
            }
        }
    }

    /// Convert all configuration values to liquid objects for template rendering
    pub fn to_liquid_object(&self) -> liquid::model::Object {
        let mut object = liquid::model::Object::new();
        for (key, value) in &self.values {
            object.insert(key.clone().into(), value.to_liquid_value());
        }
        object
    }

    /// Merge another configuration into this one
    ///
    /// # Arguments
    /// * `other` - The configuration to merge into this one
    /// * `overwrite` - Whether to overwrite existing values
    pub fn merge(&mut self, other: Configuration, overwrite: bool) {
        for (key, value) in other.values {
            if overwrite || !self.values.contains_key(&key) {
                self.values.insert(key, value);
            }
        }
    }

    /// Validate all configuration values
    pub fn validate(&self) -> Result<(), ConfigError> {
        for (key, value) in &self.values {
            // Validate variable name
            validate_variable_name(key)?;

            // Validate value
            value.validate(0)?;
        }
        Ok(())
    }

    /// Process environment variable substitution for all string values
    pub fn substitute_env_vars(&mut self) -> Result<(), ConfigError> {
        for value in self.values.values_mut() {
            value.substitute_env_vars()?;
        }
        Ok(())
    }
}

impl Default for Configuration {
    fn default() -> Self {
        Self::new()
    }
}

/// Validate a variable name according to Liquid identifier rules
fn validate_variable_name(name: &str) -> Result<(), ConfigError> {
    // Check if empty
    if name.is_empty() {
        return Err(ConfigError::invalid_variable_name(
            name.to_string(),
            "Variable name cannot be empty".to_string(),
        ));
    }

    // Check if starts with letter or underscore
    let first_char = name.chars().next().unwrap();
    if !first_char.is_ascii_alphabetic() && first_char != '_' {
        return Err(ConfigError::invalid_variable_name(
            name.to_string(),
            "Variable name must start with a letter or underscore".to_string(),
        ));
    }

    // Check if contains only valid characters
    for c in name.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' && c != '.' {
            return Err(ConfigError::invalid_variable_name(
                name.to_string(),
                format!("Variable name contains invalid character: '{}'", c),
            ));
        }
    }

    // Check for reserved names
    const RESERVED_NAMES: &[&str] = &[
        "for",
        "if",
        "unless",
        "case",
        "when",
        "else",
        "endif",
        "endfor",
        "endunless",
        "endcase",
        "break",
        "continue",
        "assign",
        "capture",
        "include",
        "layout",
        "raw",
        "endraw",
        "comment",
        "endcomment",
    ];

    if RESERVED_NAMES.contains(&name) {
        return Err(ConfigError::reserved_variable_name(name.to_string()));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn test_configuration_basic_operations() {
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

        assert!(config.contains_key("key1"));
        assert!(!config.contains_key("nonexistent"));
    }

    #[test]
    fn test_configuration_dot_notation() {
        let mut config = Configuration::new();

        // Test setting with dot notation
        config.set(
            "database.host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        config.set("database.port".to_string(), ConfigValue::Integer(5432));

        // Test getting with dot notation
        let host = config.get("database.host");
        assert!(host.is_some());
        assert_eq!(host.unwrap(), &ConfigValue::String("localhost".to_string()));

        let port = config.get("database.port");
        assert!(port.is_some());
        assert_eq!(port.unwrap(), &ConfigValue::Integer(5432));

        // Test contains_key with dot notation
        assert!(config.contains_key("database.host"));
        assert!(config.contains_key("database.port"));
        assert!(!config.contains_key("database.password"));
    }

    #[test]
    fn test_configuration_nested_structures() {
        let mut config = Configuration::new();

        // Create deeply nested structure
        config.set(
            "app.database.primary.host".to_string(),
            ConfigValue::String("db1.example.com".to_string()),
        );
        config.set(
            "app.database.primary.port".to_string(),
            ConfigValue::Integer(5432),
        );
        config.set(
            "app.database.replica.host".to_string(),
            ConfigValue::String("db2.example.com".to_string()),
        );

        // Test access to deeply nested values
        assert_eq!(
            config.get("app.database.primary.host"),
            Some(&ConfigValue::String("db1.example.com".to_string()))
        );
        assert_eq!(
            config.get("app.database.primary.port"),
            Some(&ConfigValue::Integer(5432))
        );
        assert_eq!(
            config.get("app.database.replica.host"),
            Some(&ConfigValue::String("db2.example.com".to_string()))
        );

        // Test intermediate access
        let database_config = config.get("app.database");
        assert!(database_config.is_some());
        assert!(matches!(database_config, Some(ConfigValue::Table(_))));
    }

    #[test]
    fn test_configuration_remove() {
        let mut config = Configuration::new();

        config.set(
            "section.key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config.set(
            "section.key2".to_string(),
            ConfigValue::String("value2".to_string()),
        );

        // Remove nested key
        let removed = config.remove("section.key1");
        assert!(removed.is_some());
        assert_eq!(removed.unwrap(), ConfigValue::String("value1".to_string()));

        // Verify removal
        assert!(!config.contains_key("section.key1"));
        assert!(config.contains_key("section.key2"));
    }

    #[test]
    fn test_configuration_keys() {
        let mut config = Configuration::new();

        config.set(
            "simple".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config.set("nested.key".to_string(), ConfigValue::Integer(42));
        config.set("deep.nested.key".to_string(), ConfigValue::Boolean(true));

        let keys = config.keys();
        assert!(keys.contains(&"simple".to_string()));
        assert!(keys.contains(&"nested".to_string()));
        assert!(keys.contains(&"nested.key".to_string()));
        assert!(keys.contains(&"deep".to_string()));
        assert!(keys.contains(&"deep.nested".to_string()));
        assert!(keys.contains(&"deep.nested.key".to_string()));
    }

    #[test]
    fn test_configuration_merge() {
        let mut config1 = Configuration::new();
        config1.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config1.insert(
            "shared".to_string(),
            ConfigValue::String("original".to_string()),
        );

        let mut config2 = Configuration::new();
        config2.insert(
            "key2".to_string(),
            ConfigValue::String("value2".to_string()),
        );
        config2.insert(
            "shared".to_string(),
            ConfigValue::String("updated".to_string()),
        );

        // Test merge without overwrite
        let mut merged = config1.clone();
        merged.merge(config2.clone(), false);
        assert_eq!(
            merged.get("shared"),
            Some(&ConfigValue::String("original".to_string()))
        );
        assert_eq!(
            merged.get("key2"),
            Some(&ConfigValue::String("value2".to_string()))
        );

        // Test merge with overwrite
        let mut merged = config1.clone();
        merged.merge(config2, true);
        assert_eq!(
            merged.get("shared"),
            Some(&ConfigValue::String("updated".to_string()))
        );
        assert_eq!(
            merged.get("key2"),
            Some(&ConfigValue::String("value2".to_string()))
        );
    }

    #[test]
    fn test_validate_variable_name() {
        // Valid names
        assert!(validate_variable_name("valid_name").is_ok());
        assert!(validate_variable_name("_underscore").is_ok());
        assert!(validate_variable_name("with123numbers").is_ok());
        assert!(validate_variable_name("nested.key").is_ok());

        // Invalid names
        assert!(validate_variable_name("").is_err());
        assert!(validate_variable_name("123invalid").is_err());
        assert!(validate_variable_name("with-dash").is_err());
        assert!(validate_variable_name("with space").is_err());

        // Reserved names
        assert!(validate_variable_name("for").is_err());
        assert!(validate_variable_name("if").is_err());
        assert!(validate_variable_name("include").is_err());
    }

    #[test]
    fn test_configuration_validation() {
        let mut config = Configuration::new();

        // Valid configuration
        config.insert(
            "valid_key".to_string(),
            ConfigValue::String("value".to_string()),
        );
        assert!(config.validate().is_ok());

        // Invalid variable name
        config.insert(
            "123invalid".to_string(),
            ConfigValue::String("value".to_string()),
        );
        assert!(config.validate().is_err());
    }
}
