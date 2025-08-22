//! Default configuration values for SwissArmyHammer

use figment::{providers::Serialized, Figment};
use serde_json::{json, Value};
use std::collections::HashMap;
use tracing::debug;

/// Default configuration values provider
///
/// This struct provides the baseline configuration values that serve as the foundation
/// for the configuration precedence system. These values have the lowest priority and
/// can be overridden by any other configuration source.
pub struct ConfigDefaults;

impl ConfigDefaults {
    /// Get all default configuration values
    ///
    /// Returns a HashMap containing sensible default values for common SwissArmyHammer
    /// configuration options. These defaults are intended to provide a working baseline
    /// configuration that users can customize as needed.
    pub fn values() -> HashMap<String, Value> {
        debug!("Loading default configuration values");
        
        let mut defaults = HashMap::new();

        // Environment and runtime defaults
        defaults.insert("environment".to_string(), json!("development"));
        defaults.insert("debug".to_string(), json!(false));
        defaults.insert("verbose".to_string(), json!(false));
        
        // Project metadata defaults
        defaults.insert("project_name".to_string(), json!("swissarmyhammer-project"));
        defaults.insert("version".to_string(), json!("1.0.0"));
        
        // File handling defaults
        defaults.insert("config_dir".to_string(), json!(".swissarmyhammer"));
        defaults.insert("output_dir".to_string(), json!("output"));
        defaults.insert("temp_dir".to_string(), json!("tmp"));
        
        // Template processing defaults
        defaults.insert("template_extension".to_string(), json!(".liquid"));
        defaults.insert("preserve_whitespace".to_string(), json!(false));
        defaults.insert("strict_variables".to_string(), json!(true));
        
        // Performance and behavior defaults
        defaults.insert("max_file_size".to_string(), json!(10485760)); // 10MB
        defaults.insert("timeout_seconds".to_string(), json!(300)); // 5 minutes
        defaults.insert("retry_attempts".to_string(), json!(3));
        
        // Logging and output defaults
        defaults.insert("log_level".to_string(), json!("info"));
        defaults.insert("log_format".to_string(), json!("human"));
        defaults.insert("color_output".to_string(), json!(true));
        
        debug!("Loaded {} default configuration values", defaults.len());
        defaults
    }

    /// Apply default values to a figment instance
    ///
    /// Creates a new figment instance with the default configuration values applied
    /// using figment's Serialized provider. The defaults are applied at the lowest priority,
    /// meaning the existing figment values will override them.
    ///
    /// # Arguments
    /// * `figment` - The figment instance to apply defaults to
    ///
    /// # Returns
    /// A new figment instance with the default values merged in at the lowest priority
    pub fn apply_to(figment: Figment) -> Figment {
        debug!("Applying default configuration values to figment");
        
        let defaults = Self::values();
        // Apply defaults first, then merge the existing figment (higher priority)
        Figment::new()
            .merge(Serialized::defaults(defaults))
            .merge(figment)
    }

    /// Create a new figment instance with only default values
    ///
    /// This is useful for testing or when you need a baseline figment instance
    /// with just the default configuration values.
    pub fn figment() -> Figment {
        debug!("Creating figment with default values only");
        Self::apply_to(Figment::new())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::RawConfig;

    #[test]
    fn test_default_values_not_empty() {
        let defaults = ConfigDefaults::values();
        assert!(!defaults.is_empty());
        assert!(!defaults.is_empty());
    }

    #[test]
    fn test_default_values_contain_expected_keys() {
        let defaults = ConfigDefaults::values();
        
        // Check for some expected default keys
        assert!(defaults.contains_key("environment"));
        assert!(defaults.contains_key("debug"));
        assert!(defaults.contains_key("project_name"));
        assert!(defaults.contains_key("log_level"));
    }

    #[test]
    fn test_default_values_types() {
        let defaults = ConfigDefaults::values();
        
        // Verify expected types for key defaults
        assert!(defaults["environment"].is_string());
        assert!(defaults["debug"].is_boolean());
        assert!(defaults["max_file_size"].is_number());
        assert!(defaults["timeout_seconds"].is_number());
    }

    #[test]
    fn test_apply_to_empty_figment() {
        let figment = Figment::new();
        let with_defaults = ConfigDefaults::apply_to(figment);
        
        let config: RawConfig = with_defaults.extract().expect("Should extract successfully");
        assert!(!config.is_empty());
    }

    #[test]
    fn test_apply_to_existing_figment() {
        // Create a figment with some existing values that should override defaults
        let existing_values = std::collections::HashMap::from([
            ("custom_key".to_string(), json!("custom_value")),
            ("environment".to_string(), json!("production")), // Should override default
        ]);
        
        let existing_figment = Figment::new().merge(Serialized::defaults(existing_values));
        let final_figment = ConfigDefaults::apply_to(existing_figment);
        
        let config: HashMap<String, Value> = final_figment.extract().expect("Should extract successfully");
        
        // Custom value should be preserved
        assert_eq!(config["custom_key"], json!("custom_value"));
        
        // Existing value should override default (higher priority)
        assert_eq!(config["environment"], json!("production"));
        
        // Default values should be present for keys that don't conflict
        assert!(config.contains_key("debug"));
        assert!(config.contains_key("project_name"));
    }

    #[test]
    fn test_figment_creation() {
        let figment = ConfigDefaults::figment();
        let config: RawConfig = figment.extract().expect("Should extract successfully");
        assert!(!config.is_empty());
        
        // Should contain all default values
        let defaults_count = ConfigDefaults::values().len();
        assert_eq!(config.values.len(), defaults_count);
    }

    #[test]
    fn test_precedence_order() {
        // Test that values added after defaults take precedence
        let base_figment = ConfigDefaults::figment();
        
        let override_values = std::collections::HashMap::from([
            ("environment".to_string(), json!("testing")),
            ("debug".to_string(), json!(true)),
        ]);
        
        let final_figment = base_figment.merge(Serialized::defaults(override_values));
        let config: HashMap<String, Value> = final_figment.extract().expect("Should extract successfully");
        
        // Override values should win
        assert_eq!(config["environment"], json!("testing"));
        assert_eq!(config["debug"], json!(true));
        
        // Other defaults should remain
        assert_eq!(config["project_name"], json!("swissarmyhammer-project"));
    }
}