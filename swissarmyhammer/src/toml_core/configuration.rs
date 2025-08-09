use crate::toml_core::{error::ConfigError, value::ConfigValue};
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

    /// Get a configuration value by key (supports dot notation for nested access)
    ///
    /// # Examples
    /// ```
    /// use swissarmyhammer::toml_core::{Configuration, ConfigValue};
    /// use std::collections::HashMap;
    ///
    /// let mut config = Configuration::new();
    /// let mut nested = HashMap::new();
    /// nested.insert("port".to_string(), ConfigValue::Integer(5432));
    /// config.insert("database".to_string(), ConfigValue::Table(nested));
    ///
    /// // Direct access
    /// let db_table = config.get("database").unwrap();
    ///
    /// // Dot notation access
    /// let port = config.get_nested("database.port").unwrap().unwrap();
    /// assert_eq!(port, &ConfigValue::Integer(5432));
    /// ```
    pub fn get(&self, key: &str) -> Option<&ConfigValue> {
        self.values.get(key)
    }

    /// Get a nested configuration value using dot notation
    ///
    /// This method supports accessing nested table values using dot notation,
    /// e.g., "database.host" will access the "host" key in the "database" table.
    ///
    /// # Arguments
    /// * `path` - Dot-separated path to the desired value
    ///
    /// # Returns
    /// * `Ok(Some(&ConfigValue))` - If the value exists at the path
    /// * `Ok(None)` - If any part of the path doesn't exist
    /// * `Err(ConfigError)` - If the path traverses through a non-table value
    pub fn get_nested(&self, path: &str) -> Result<Option<&ConfigValue>, ConfigError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            return Ok(None);
        }

        // Start with the root value
        let mut current_value = match self.values.get(parts[0]) {
            Some(value) => value,
            None => return Ok(None),
        };

        // Traverse the path
        for (index, part) in parts.iter().skip(1).enumerate() {
            match current_value {
                ConfigValue::Table(table) => {
                    current_value = match table.get(*part) {
                        Some(value) => value,
                        None => return Ok(None),
                    };
                }
                _ => {
                    // Trying to access a nested value on a non-table
                    let traversed_path = parts[..=index].join(".");
                    return Err(ConfigError::NestedAccessFailed {
                        path: path.to_string(),
                        reason: format!(
                            "Cannot access '{}' because '{}' is not a table (it's a {})",
                            part,
                            traversed_path,
                            current_value.type_name()
                        ),
                    });
                }
            }
        }

        Ok(Some(current_value))
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

    /// Insert a nested configuration value using dot notation
    ///
    /// This method creates intermediate tables as needed to set a value at the given path.
    ///
    /// # Arguments
    /// * `path` - Dot-separated path where to insert the value
    /// * `value` - The value to insert
    ///
    /// # Returns
    /// * `Ok(())` - If the value was successfully inserted
    /// * `Err(ConfigError)` - If the path conflicts with existing non-table values
    pub fn insert_nested(&mut self, path: &str, value: ConfigValue) -> Result<(), ConfigError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            return Err(ConfigError::NestedAccessFailed {
                path: path.to_string(),
                reason: "Empty path not allowed".to_string(),
            });
        }

        if parts.len() == 1 {
            // Simple insert
            self.values.insert(parts[0].to_string(), value);
            return Ok(());
        }

        // Navigate/create nested structure
        let root_key = parts[0];
        let mut current_table = match self.values.get_mut(root_key) {
            Some(ConfigValue::Table(table)) => table,
            Some(other) => {
                return Err(ConfigError::NestedAccessFailed {
                    path: path.to_string(),
                    reason: format!(
                        "Cannot create nested path because '{}' exists and is not a table (it's a {})",
                        root_key,
                        other.type_name()
                    ),
                });
            }
            None => {
                // Create the root table
                self.values
                    .insert(root_key.to_string(), ConfigValue::Table(HashMap::new()));
                match self.values.get_mut(root_key).unwrap() {
                    ConfigValue::Table(table) => table,
                    _ => unreachable!(),
                }
            }
        };

        // Navigate through intermediate parts, creating tables as needed
        for part in parts.iter().skip(1).take(parts.len() - 2) {
            // Check if the part exists and what type it is
            let needs_creation = match current_table.get(*part) {
                Some(ConfigValue::Table(_)) => false,
                Some(other) => {
                    let current_path =
                        parts[..parts.iter().position(|&p| p == *part).unwrap() + 1].join(".");
                    return Err(ConfigError::NestedAccessFailed {
                        path: path.to_string(),
                        reason: format!(
                            "Cannot create nested path because '{}' exists and is not a table (it's a {})",
                            current_path,
                            other.type_name()
                        ),
                    });
                }
                None => true,
            };

            // Create table if needed
            if needs_creation {
                current_table.insert(part.to_string(), ConfigValue::Table(HashMap::new()));
            }

            // Now get mutable reference to the table
            current_table = match current_table.get_mut(*part).unwrap() {
                ConfigValue::Table(table) => table,
                _ => unreachable!(),
            };
        }

        // Insert the final value
        let final_key = parts[parts.len() - 1];
        current_table.insert(final_key.to_string(), value);

        Ok(())
    }

    /// Remove a configuration value by key
    pub fn remove(&mut self, key: &str) -> Option<ConfigValue> {
        self.values.remove(key)
    }

    /// Remove a nested configuration value using dot notation
    pub fn remove_nested(&mut self, path: &str) -> Result<Option<ConfigValue>, ConfigError> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.is_empty() || (parts.len() == 1 && parts[0].is_empty()) {
            return Ok(None);
        }

        if parts.len() == 1 {
            return Ok(self.values.remove(parts[0]));
        }

        // Navigate to the parent table
        let mut current_table = match self.values.get_mut(parts[0]) {
            Some(ConfigValue::Table(table)) => table,
            Some(_) => return Ok(None), // Parent is not a table
            None => return Ok(None),    // Parent doesn't exist
        };

        for part in parts.iter().skip(1).take(parts.len() - 2) {
            current_table = match current_table.get_mut(*part) {
                Some(ConfigValue::Table(table)) => table,
                _ => return Ok(None), // Intermediate path doesn't exist or isn't a table
            };
        }

        // Remove the final key
        let final_key = parts[parts.len() - 1];
        Ok(current_table.remove(final_key))
    }

    /// Check if the configuration contains a key (supports dot notation)
    pub fn contains_key(&self, key: &str) -> bool {
        if key.contains('.') {
            self.get_nested(key).unwrap_or(None).is_some()
        } else {
            self.values.contains_key(key)
        }
    }

    /// Check if the configuration is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// Get the number of top-level configuration values
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Get the total number of configuration values (including nested ones)
    pub fn total_values(&self) -> usize {
        Self::count_values_recursive(&self.values)
    }

    /// Recursively count all configuration values
    fn count_values_recursive(values: &HashMap<String, ConfigValue>) -> usize {
        let mut count = values.len();
        for value in values.values() {
            if let ConfigValue::Table(nested_table) = value {
                count += Self::count_values_recursive(nested_table);
            } else if let ConfigValue::Array(arr) = value {
                for item in arr {
                    if let ConfigValue::Table(table) = item {
                        count += Self::count_values_recursive(table);
                    }
                }
            }
        }
        count
    }

    /// Convert all configuration values to liquid objects for template rendering
    pub fn to_liquid_object(&self) -> liquid::model::Object {
        let mut object = liquid::model::Object::new();
        for (key, value) in &self.values {
            object.insert(key.clone().into(), value.to_liquid_value());
        }
        object
    }

    /// Convert all configuration values to JSON for serialization
    pub fn to_json_object(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut object = serde_json::Map::new();
        for (key, value) in &self.values {
            object.insert(key.clone(), value.to_json_value());
        }
        object
    }

    /// Get all keys in the configuration (including nested keys with dot notation)
    pub fn all_keys(&self) -> Vec<String> {
        let mut keys = Vec::new();
        Self::collect_keys_recursive(&self.values, String::new(), &mut keys);
        keys
    }

    /// Recursively collect all keys including nested ones
    fn collect_keys_recursive(
        values: &HashMap<String, ConfigValue>,
        prefix: String,
        keys: &mut Vec<String>,
    ) {
        for (key, value) in values {
            let full_key = if prefix.is_empty() {
                key.clone()
            } else {
                format!("{prefix}.{key}")
            };

            keys.push(full_key.clone());

            if let ConfigValue::Table(nested_table) = value {
                Self::collect_keys_recursive(nested_table, full_key, keys);
            }
        }
    }

    /// Merge another configuration into this one
    ///
    /// Values from the other configuration will override values in this configuration
    /// with the same key. Nested tables are merged recursively.
    pub fn merge(&mut self, other: &Configuration) {
        for (key, value) in &other.values {
            self.merge_value(key, value);
        }
    }

    /// Merge a single value, handling table merging recursively
    fn merge_value(&mut self, key: &str, value: &ConfigValue) {
        match (self.values.get_mut(key), value) {
            (Some(ConfigValue::Table(existing_table)), ConfigValue::Table(new_table)) => {
                // Merge tables recursively
                for (nested_key, nested_value) in new_table {
                    match (existing_table.get_mut(nested_key), nested_value) {
                        (
                            Some(ConfigValue::Table(existing_nested)),
                            ConfigValue::Table(new_nested),
                        ) => {
                            // Continue recursive merge
                            let mut temp_config = Configuration::with_values(
                                [(
                                    nested_key.clone(),
                                    ConfigValue::Table(existing_nested.clone()),
                                )]
                                .iter()
                                .cloned()
                                .collect(),
                                None,
                            );
                            let other_config = Configuration::with_values(
                                [(nested_key.clone(), ConfigValue::Table(new_nested.clone()))]
                                    .iter()
                                    .cloned()
                                    .collect(),
                                None,
                            );
                            temp_config.merge(&other_config);
                            if let Some(ConfigValue::Table(merged)) =
                                temp_config.values.get(nested_key)
                            {
                                *existing_nested = merged.clone();
                            }
                        }
                        _ => {
                            // Replace with new value
                            existing_table.insert(nested_key.clone(), nested_value.clone());
                        }
                    }
                }
            }
            _ => {
                // Replace with new value
                self.values.insert(key.to_string(), value.clone());
            }
        }
    }

    /// Process environment variable substitution in all values
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_basic_operations() {
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
        assert!(!config.contains_key("key2"));
    }

    #[test]
    fn test_nested_access() {
        let mut config = Configuration::new();

        // Create nested structure: database.host = "localhost"
        let mut db_table = HashMap::new();
        db_table.insert(
            "host".to_string(),
            ConfigValue::String("localhost".to_string()),
        );
        db_table.insert("port".to_string(), ConfigValue::Integer(5432));

        config.insert("database".to_string(), ConfigValue::Table(db_table));

        // Test dot notation access
        let host = config.get_nested("database.host").unwrap();
        assert_eq!(host, Some(&ConfigValue::String("localhost".to_string())));

        let port = config.get_nested("database.port").unwrap();
        assert_eq!(port, Some(&ConfigValue::Integer(5432)));

        // Test non-existent path
        let missing = config.get_nested("database.missing").unwrap();
        assert_eq!(missing, None);

        let missing_root = config.get_nested("missing.key").unwrap();
        assert_eq!(missing_root, None);
    }

    #[test]
    fn test_nested_insert() {
        let mut config = Configuration::new();

        // Insert nested value
        config
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();
        config
            .insert_nested("database.port", ConfigValue::Integer(5432))
            .unwrap();

        // Verify structure was created
        let host = config.get_nested("database.host").unwrap();
        assert_eq!(host, Some(&ConfigValue::String("localhost".to_string())));

        let port = config.get_nested("database.port").unwrap();
        assert_eq!(port, Some(&ConfigValue::Integer(5432)));

        // Insert deeper nested value
        config
            .insert_nested(
                "app.logging.level",
                ConfigValue::String("debug".to_string()),
            )
            .unwrap();

        let log_level = config.get_nested("app.logging.level").unwrap();
        assert_eq!(log_level, Some(&ConfigValue::String("debug".to_string())));
    }

    #[test]
    fn test_nested_remove() {
        let mut config = Configuration::new();

        config
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();
        config
            .insert_nested("database.port", ConfigValue::Integer(5432))
            .unwrap();

        // Remove nested value
        let removed = config.remove_nested("database.host").unwrap();
        assert_eq!(removed, Some(ConfigValue::String("localhost".to_string())));

        // Verify it's gone
        let missing = config.get_nested("database.host").unwrap();
        assert_eq!(missing, None);

        // Port should still be there
        let port = config.get_nested("database.port").unwrap();
        assert_eq!(port, Some(&ConfigValue::Integer(5432)));
    }

    #[test]
    fn test_contains_key_with_dot_notation() {
        let mut config = Configuration::new();

        config
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();

        assert!(config.contains_key("database"));
        assert!(config.contains_key("database.host"));
        assert!(!config.contains_key("database.port"));
        assert!(!config.contains_key("missing"));
    }

    #[test]
    fn test_all_keys() {
        let mut config = Configuration::new();

        config.insert(
            "simple".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();
        config
            .insert_nested("database.port", ConfigValue::Integer(5432))
            .unwrap();
        config
            .insert_nested("app.name", ConfigValue::String("myapp".to_string()))
            .unwrap();

        let mut keys = config.all_keys();
        keys.sort();

        let expected = vec![
            "app".to_string(),
            "app.name".to_string(),
            "database".to_string(),
            "database.host".to_string(),
            "database.port".to_string(),
            "simple".to_string(),
        ];

        assert_eq!(keys, expected);
    }

    #[test]
    fn test_total_values() {
        let mut config = Configuration::new();

        config.insert(
            "simple".to_string(),
            ConfigValue::String("value".to_string()),
        );
        config
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();
        config
            .insert_nested("database.port", ConfigValue::Integer(5432))
            .unwrap();

        // simple + database + database.host + database.port = 4 total values
        assert_eq!(config.total_values(), 4);
        assert_eq!(config.len(), 2); // Only top-level keys
    }

    #[test]
    fn test_merge_configurations() {
        let mut config1 = Configuration::new();
        config1.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        config1
            .insert_nested(
                "database.host",
                ConfigValue::String("localhost".to_string()),
            )
            .unwrap();

        let mut config2 = Configuration::new();
        config2.insert(
            "key2".to_string(),
            ConfigValue::String("value2".to_string()),
        );
        config2
            .insert_nested("database.port", ConfigValue::Integer(5432))
            .unwrap();

        // key1 exists in both - config2 should override
        config2.insert(
            "key1".to_string(),
            ConfigValue::String("new_value1".to_string()),
        );

        config1.merge(&config2);

        // Check merged results
        assert_eq!(
            config1.get("key1").unwrap(),
            &ConfigValue::String("new_value1".to_string())
        ); // Overridden
        assert_eq!(
            config1.get("key2").unwrap(),
            &ConfigValue::String("value2".to_string())
        ); // Added

        // Check nested merge
        let host = config1.get_nested("database.host").unwrap();
        assert_eq!(host, Some(&ConfigValue::String("localhost".to_string()))); // Preserved

        let port = config1.get_nested("database.port").unwrap();
        assert_eq!(port, Some(&ConfigValue::Integer(5432))); // Added
    }

    #[test]
    fn test_nested_access_error() {
        let mut config = Configuration::new();
        config.insert(
            "not_table".to_string(),
            ConfigValue::String("value".to_string()),
        );

        // Try to access nested value on non-table
        let result = config.get_nested("not_table.nested");
        assert!(result.is_err());

        if let Err(ConfigError::NestedAccessFailed { path, reason }) = result {
            assert_eq!(path, "not_table.nested");
            assert!(reason.contains("not a table"));
        } else {
            panic!("Expected NestedAccessFailed error");
        }
    }

    #[test]
    fn test_nested_insert_conflict() {
        let mut config = Configuration::new();
        config.insert(
            "existing".to_string(),
            ConfigValue::String("value".to_string()),
        );

        // Try to create nested path where existing key is not a table
        let result =
            config.insert_nested("existing.nested", ConfigValue::String("new".to_string()));
        assert!(result.is_err());

        if let Err(ConfigError::NestedAccessFailed { path, reason }) = result {
            assert_eq!(path, "existing.nested");
            assert!(reason.contains("not a table"));
        } else {
            panic!("Expected NestedAccessFailed error");
        }
    }
}
