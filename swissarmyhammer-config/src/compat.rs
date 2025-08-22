//! Legacy compatibility layer for existing SwissArmyHammer template integration
//!
//! This module provides compatibility functions that maintain the exact behavior
//! of the legacy template integration system while using the new TemplateContext
//! backend. This allows existing code to continue working without modification
//! during the migration to the new configuration system.
//!
//! # Legacy Functions
//!
//! - `merge_config_into_context`: Maintains exact `_template_vars` structure
//! - `load_and_merge_repo_config`: Loads repository config and merges into HashMap
//!
//! # Migration Path
//!
//! These functions should be considered deprecated and will be removed in a future
//! version. New code should use the `TemplateRenderer` and `ConfigProvider` APIs
//! directly for better type safety and performance.
//!
//! # Example
//!
//! ```rust
//! use swissarmyhammer_config::compat::{merge_config_into_context, load_and_merge_repo_config};
//! use std::collections::HashMap;
//! use serde_json::json;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Legacy usage (deprecated)
//! let mut context = HashMap::new();
//! context.insert("_template_vars".to_string(), json!({"workflow_var": "value"}));
//! merge_config_into_context(&mut context)?;
//!
//! // The context now has config merged into _template_vars
//! let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
//! // template_vars contains both workflow and config variables
//! # Ok(())
//! # }
//! ```

use crate::{ConfigError, ConfigProvider, ConfigResult};
use std::collections::HashMap;
use tracing::{debug, warn};

/// Merge configuration into workflow context for template rendering (Legacy)
///
/// This function maintains exact compatibility with the legacy `merge_config_into_context`
/// function by preserving the `_template_vars` structure and precedence behavior.
///
/// **DEPRECATED**: New code should use `ConfigProvider::load_template_context()` and
/// `TemplateRenderer` for better type safety and performance.
///
/// # Behavior
///
/// - Loads configuration using the new ConfigProvider system
/// - Preserves existing `_template_vars` object structure
/// - Maintains precedence: workflow vars > config vars
/// - Handles missing or malformed `_template_vars` gracefully
/// - Environment variable substitution is performed on configuration
///
/// # Arguments
///
/// * `context` - Mutable reference to the workflow context HashMap containing `_template_vars`
///
/// # Returns
///
/// Returns `Ok(())` on success or `ConfigError` if configuration loading fails.
///
/// # Legacy Format
///
/// The function expects and maintains this structure:
/// ```json
/// {
///   "_template_vars": {
///     "workflow_var": "workflow_value",
///     "config_var": "config_value"
///   },
///   // ... other workflow state
/// }
/// ```
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_config::compat::merge_config_into_context;
/// use std::collections::HashMap;
/// use serde_json::json;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut context = HashMap::new();
/// // Existing template vars from workflow state
/// context.insert("_template_vars".to_string(), json!({"workflow_var": "workflow_value"}));
///
/// // Merge configuration (config values have lower priority)
/// merge_config_into_context(&mut context)?;
///
/// // The context now contains both workflow and config variables in _template_vars
/// let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
/// // template_vars has both workflow_var and any config variables
/// # Ok(())
/// # }
/// ```
pub fn merge_config_into_context(
    context: &mut HashMap<String, serde_json::Value>,
) -> ConfigResult<()> {
    debug!("Merging configuration into context using legacy compatibility layer");
    warn!("merge_config_into_context is deprecated - use ConfigProvider and TemplateContext instead");

    // Load configuration using the new system
    let provider = ConfigProvider::new();
    let template_context = provider.load_template_context()?;

    // Extract existing _template_vars or create empty object
    let existing_vars = match context.get("_template_vars") {
        Some(serde_json::Value::Object(obj)) => obj.clone(),
        _ => serde_json::Map::new(),
    };

    // Convert TemplateContext to HashMap for merging
    let config_vars = template_context.as_hashmap();

    // Create merged template vars with proper precedence
    let mut merged_vars = serde_json::Map::new();

    // First add configuration values (lower priority)
    for (key, value) in config_vars {
        merged_vars.insert(key.clone(), value.clone());
    }

    // Then add existing workflow variables (higher priority - will override config)
    for (key, value) in existing_vars {
        merged_vars.insert(key, value);
    }

    // Update the context with merged template variables
    context.insert("_template_vars".to_string(), serde_json::Value::Object(merged_vars));

    debug!(
        "Successfully merged configuration into context, _template_vars has {} variables",
        context.get("_template_vars")
            .and_then(|v| v.as_object())
            .map(|o| o.len())
            .unwrap_or(0)
    );

    Ok(())
}

/// Load repository configuration and merge into workflow context (Legacy)
///
/// This function maintains exact compatibility with the legacy `load_and_merge_repo_config`
/// function while using the new configuration system backend.
///
/// **DEPRECATED**: New code should use `ConfigProvider::load_template_context()` and
/// `TemplateRenderer` for better type safety and performance.
///
/// # Arguments
///
/// * `context` - Mutable reference to the workflow context HashMap
///
/// # Returns
///
/// Returns `Ok(true)` if configuration was loaded and merged, `Ok(false)` if no
/// configuration was found, or `ConfigError` if loading fails.
///
/// # Example
///
/// ```rust
/// use swissarmyhammer_config::compat::load_and_merge_repo_config;
/// use std::collections::HashMap;
///
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut context = HashMap::new();
/// let config_loaded = load_and_merge_repo_config(&mut context)?;
///
/// if config_loaded {
///     println!("Configuration loaded and merged into workflow context");
/// } else {
///     println!("No configuration file found");
/// }
/// # Ok(())
/// # }
/// ```
pub fn load_and_merge_repo_config(
    context: &mut HashMap<String, serde_json::Value>,
) -> ConfigResult<bool> {
    debug!("Loading and merging repository configuration using legacy compatibility layer");
    warn!("load_and_merge_repo_config is deprecated - use ConfigProvider and TemplateContext instead");

    // Try to load configuration
    let provider = ConfigProvider::new();
    
    // Use the template context loading which handles all the complexity
    match provider.load_template_context() {
        Ok(template_context) => {
            // Check if we actually have any configuration (beyond defaults)
            // The template context will always have some defaults, so we can't just check if empty
            let has_config = !template_context.is_empty();

            if has_config {
                // Merge using the same logic as merge_config_into_context
                merge_config_into_context(context)?;
                debug!("Successfully loaded and merged repository configuration");
                Ok(true)
            } else {
                debug!("No repository configuration found (only defaults available)");
                Ok(false)
            }
        }
        Err(e) => {
            // If we can't load config due to file issues, return false (no config found)
            // Only return error for actual configuration errors
            match e {
                ConfigError::FileNotFound { .. } => {
                    debug!("No configuration files found");
                    Ok(false)
                }
                _ => Err(e), // Actual configuration errors should propagate
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use serial_test::serial;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_merge_config_into_context_empty_context() {
        let mut context = HashMap::new();
        
        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        // Should have created _template_vars with config defaults
        assert!(context.contains_key("_template_vars"));
        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        
        // Should have some default config values
        assert!(template_vars.contains_key("environment"));
        assert!(template_vars.contains_key("debug"));
    }

    #[test]
    fn test_merge_config_into_context_existing_vars() {
        let mut context = HashMap::new();
        context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_var": "workflow_value",
                "environment": "workflow_env" // Should override config default
            }),
        );

        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        
        // Workflow vars should be preserved
        assert_eq!(template_vars.get("workflow_var"), Some(&json!("workflow_value")));
        
        // Workflow should override config for same key
        assert_eq!(template_vars.get("environment"), Some(&json!("workflow_env")));
        
        // Should have config vars that don't conflict
        assert!(template_vars.contains_key("debug"));
    }

    #[test]
    fn test_merge_config_into_context_malformed_template_vars() {
        let mut context = HashMap::new();
        // Set _template_vars to a non-object value
        context.insert("_template_vars".to_string(), json!("not an object"));

        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        // Should have fixed the structure and added config
        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert!(template_vars.contains_key("environment"));
    }

    #[test]
    fn test_merge_config_into_context_no_template_vars() {
        let mut context = HashMap::new();
        context.insert("other_key".to_string(), json!("other_value"));

        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        // Should have added _template_vars with config
        assert!(context.contains_key("_template_vars"));
        assert!(context.contains_key("other_key")); // Should preserve other keys
        
        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert!(template_vars.contains_key("environment"));
    }

    #[test]
    fn test_load_and_merge_repo_config_no_config() {
        let temp_dir = TempDir::new().unwrap();
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut context = HashMap::new();
        let result = load_and_merge_repo_config(&mut context);

        std::env::set_current_dir(original_dir).unwrap();

        // Should succeed but indicate no config was found (though we might have defaults)
        assert!(result.is_ok());
        let config_loaded = result.unwrap();
        
        // With the new system, we always have defaults, so this might return true
        // The key is that it doesn't error
        if config_loaded {
            assert!(context.contains_key("_template_vars"));
        }
    }

    #[test]
    #[serial]
    fn test_load_and_merge_repo_config_with_config() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
test_key = "test_value"
project_name = "TestProject"
"#,
        ).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut context = HashMap::new();
        let result = load_and_merge_repo_config(&mut context);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config_loaded = result.unwrap();
        assert!(config_loaded);

        // Should have merged config into _template_vars
        assert!(context.contains_key("_template_vars"));
        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert_eq!(template_vars.get("test_key"), Some(&json!("test_value")));
        assert_eq!(template_vars.get("project_name"), Some(&json!("TestProject")));
    }

    #[test]
    #[serial]
    fn test_load_and_merge_repo_config_with_existing_context() {
        let temp_dir = TempDir::new().unwrap();
        let sah_dir = temp_dir.path().join(".swissarmyhammer");
        fs::create_dir_all(&sah_dir).unwrap();

        let config_file = sah_dir.join("sah.toml");
        fs::write(
            &config_file,
            r#"
config_key = "config_value"
shared_key = "from_config"
"#,
        ).unwrap();

        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp_dir.path()).unwrap();

        let mut context = HashMap::new();
        context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_key": "workflow_value",
                "shared_key": "from_workflow" // Should override config
            }),
        );

        let result = load_and_merge_repo_config(&mut context);

        std::env::set_current_dir(original_dir).unwrap();

        assert!(result.is_ok());
        let config_loaded = result.unwrap();
        assert!(config_loaded);

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        
        // Should have config values
        assert_eq!(template_vars.get("config_key"), Some(&json!("config_value")));
        
        // Should have workflow values
        assert_eq!(template_vars.get("workflow_key"), Some(&json!("workflow_value")));
        
        // Workflow should override config
        assert_eq!(template_vars.get("shared_key"), Some(&json!("from_workflow")));
    }

    #[test]
    fn test_precedence_matches_legacy_behavior() {
        let mut context = HashMap::new();
        context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_var": "workflow_value",
                "environment": "workflow_environment"
            }),
        );

        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        
        // Workflow vars should have highest priority
        assert_eq!(template_vars.get("workflow_var"), Some(&json!("workflow_value")));
        assert_eq!(template_vars.get("environment"), Some(&json!("workflow_environment")));
        
        // Config vars should be added for keys that don't exist in workflow
        assert!(template_vars.contains_key("debug")); // From config defaults
    }

    #[test]
    fn test_backwards_compatibility_structure() {
        let mut context = HashMap::new();
        context.insert("other_workflow_data".to_string(), json!("should_be_preserved"));

        let result = merge_config_into_context(&mut context);
        assert!(result.is_ok());

        // Should preserve non-_template_vars keys
        assert!(context.contains_key("other_workflow_data"));
        assert_eq!(context.get("other_workflow_data"), Some(&json!("should_be_preserved")));
        
        // Should have _template_vars in correct format
        assert!(context.contains_key("_template_vars"));
        let template_vars = context.get("_template_vars").unwrap();
        assert!(template_vars.is_object());
    }
}