use crate::sah_config::{ConfigValue, Configuration};
use serde_json::Value;
use std::collections::HashMap;

/// Merge sah.toml configuration values into a workflow context for template rendering
///
/// This function takes existing template variables from the workflow context and merges
/// them with sah.toml configuration values. The merge priority is:
/// 1. Repository root sah.toml (lowest priority)
/// 2. Environment variable overrides (medium priority)
/// 3. Existing workflow state variables from _template_vars (highest priority)
///
/// # Arguments
/// * `context` - Mutable reference to the workflow context HashMap
/// * `config` - The sah.toml configuration to merge
///
/// # Example
/// ```
/// use swissarmyhammer::sah_config::{Configuration, ConfigValue, merge_config_into_context};
/// use std::collections::HashMap;
/// use serde_json::{json, Value};
///
/// let mut context = HashMap::new();
/// // Existing template vars from workflow state
/// context.insert("_template_vars".to_string(), json!({"workflow_var": "workflow_value"}));
///
/// let mut config = Configuration::new();
/// config.insert("project_name".to_string(), ConfigValue::String("MyProject".to_string()));
///
/// merge_config_into_context(&mut context, &config);
///
/// // The context now contains both workflow and config variables
/// let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
/// assert_eq!(template_vars.get("project_name").unwrap(), "MyProject");
/// assert_eq!(template_vars.get("workflow_var").unwrap(), "workflow_value");
/// ```
pub fn merge_config_into_context(context: &mut HashMap<String, Value>, config: &Configuration) {
    // Get or create the _template_vars object
    let template_vars = match context.get("_template_vars") {
        Some(Value::Object(obj)) => obj.clone(),
        _ => serde_json::Map::new(),
    };

    // Convert config values to JSON values and merge
    let mut merged_vars = serde_json::Map::new();

    // First, add sah.toml configuration values (lowest priority)
    for (key, config_value) in config.values() {
        merged_vars.insert(key.clone(), config_value_to_json_value(config_value));
    }

    // TODO: Add environment variable substitution here (medium priority)
    // This would involve processing values like ${VAR_NAME:-default}
    // and replacing them with actual environment variable values

    // Finally, add existing workflow template variables (highest priority)
    // These will override any config values with the same key
    for (key, value) in template_vars {
        merged_vars.insert(key, value);
    }

    // Update the context with merged template variables
    context.insert("_template_vars".to_string(), Value::Object(merged_vars));
}

/// Convert a ConfigValue to a serde_json::Value for template rendering
///
/// This function recursively converts our sah.toml ConfigValue representation
/// to JSON values that can be used with the existing workflow template system.
fn config_value_to_json_value(config_value: &ConfigValue) -> Value {
    match config_value {
        ConfigValue::String(s) => Value::String(s.clone()),
        ConfigValue::Integer(i) => Value::Number(serde_json::Number::from(*i)),
        ConfigValue::Float(f) => {
            // Use from_f64 which returns None for non-finite values
            serde_json::Number::from_f64(*f)
                .map(Value::Number)
                .unwrap_or(Value::Null)
        }
        ConfigValue::Boolean(b) => Value::Bool(*b),
        ConfigValue::Array(arr) => {
            let json_array: Vec<Value> = arr.iter().map(config_value_to_json_value).collect();
            Value::Array(json_array)
        }
        ConfigValue::Table(table) => {
            let mut json_object = serde_json::Map::new();
            for (key, value) in table {
                json_object.insert(key.clone(), config_value_to_json_value(value));
            }
            Value::Object(json_object)
        }
    }
}

/// Load sah.toml from repository root and merge into workflow context
///
/// This is a convenience function that loads the sah.toml configuration
/// from the repository root and merges it into the provided context.
/// If no sah.toml file is found, the context is left unchanged.
///
/// # Arguments
/// * `context` - Mutable reference to the workflow context HashMap
///
/// # Returns
/// * `Result<bool, crate::sah_config::ConfigurationError>` - True if config was loaded and merged, false if no config file found
///
/// # Example
/// ```no_run
/// use swissarmyhammer::sah_config::load_and_merge_repo_config;
/// use std::collections::HashMap;
/// use serde_json::Value;
///
/// let mut context = HashMap::new();
/// let config_loaded = load_and_merge_repo_config(&mut context)?;
///
/// if config_loaded {
///     println!("sah.toml configuration merged into workflow context");
/// } else {
///     println!("No sah.toml file found");
/// }
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn load_and_merge_repo_config(
    context: &mut HashMap<String, Value>,
) -> Result<bool, crate::sah_config::loader::ConfigurationError> {
    use crate::sah_config::loader::ConfigurationLoader;

    let loader = ConfigurationLoader::new();
    match loader.load_from_repo_root()? {
        Some(config) => {
            merge_config_into_context(context, &config);
            Ok(true)
        }
        None => Ok(false),
    }
}

/// Substitute environment variables in configuration values
///
/// This function processes configuration values and replaces patterns like
/// `${VAR_NAME}` and `${VAR_NAME:-default_value}` with actual environment variable values.
///
/// # Arguments
/// * `config` - Mutable reference to the configuration to process
///
/// # Example
/// ```
/// use swissarmyhammer::sah_config::{Configuration, ConfigValue, substitute_env_vars};
/// use std::env;
///
/// env::set_var("TEST_VAR", "test_value");
///
/// let mut config = Configuration::new();
/// config.insert("message".to_string(), ConfigValue::String("Hello ${TEST_VAR}!".to_string()));
///
/// substitute_env_vars(&mut config);
///
/// let message = config.get("message").unwrap();
/// assert_eq!(message, &ConfigValue::String("Hello test_value!".to_string()));
///
/// env::remove_var("TEST_VAR");
/// ```
pub fn substitute_env_vars(config: &mut Configuration) {
    // Create a new configuration with processed values
    let mut new_values = HashMap::new();

    for (key, value) in config.values() {
        new_values.insert(key.clone(), substitute_env_vars_in_value(value.clone()));
    }

    // Replace the configuration with the processed version
    *config = Configuration::with_values(new_values, config.file_path().cloned());
}

/// Substitute environment variables in a single ConfigValue
fn substitute_env_vars_in_value(value: ConfigValue) -> ConfigValue {
    match value {
        ConfigValue::String(s) => ConfigValue::String(substitute_env_vars_in_string(&s)),
        ConfigValue::Array(arr) => {
            let new_arr = arr.into_iter().map(substitute_env_vars_in_value).collect();
            ConfigValue::Array(new_arr)
        }
        ConfigValue::Table(table) => {
            let new_table = table
                .into_iter()
                .map(|(k, v)| (k, substitute_env_vars_in_value(v)))
                .collect();
            ConfigValue::Table(new_table)
        }
        // Other types don't need environment variable substitution
        other => other,
    }
}

/// Substitute environment variables in a string value
///
/// Supports patterns:
/// - `${VAR_NAME}` - Replace with environment variable value, empty string if not set
/// - `${VAR_NAME:-default}` - Replace with environment variable value, or default if not set
fn substitute_env_vars_in_string(s: &str) -> String {
    // Use regex to find and replace environment variable patterns
    thread_local! {
        static ENV_VAR_REGEX: regex::Regex = regex::Regex::new(r"\$\{([^}:]+)(?::-([^}]*))?\}")
            .expect("Failed to compile environment variable regex");
    }

    ENV_VAR_REGEX.with(|re| {
        re.replace_all(s, |caps: &regex::Captures| {
            let var_name = &caps[1];
            match std::env::var(var_name) {
                Ok(value) => value,
                Err(_) => {
                    // Check if we have a default value (pattern was ${VAR:-default})
                    if let Some(default_match) = caps.get(2) {
                        default_match.as_str().to_string()
                    } else {
                        String::new() // No default, return empty string
                    }
                }
            }
        })
        .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::env;

    #[test]
    fn test_merge_config_into_context_empty_context() {
        let mut context = HashMap::new();
        let mut config = Configuration::new();
        config.insert(
            "project_name".to_string(),
            ConfigValue::String("TestProject".to_string()),
        );
        config.insert("debug".to_string(), ConfigValue::Boolean(true));

        merge_config_into_context(&mut context, &config);

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert_eq!(template_vars.get("project_name").unwrap(), "TestProject");
        assert_eq!(template_vars.get("debug").unwrap(), &json!(true));
    }

    #[test]
    fn test_merge_config_into_context_existing_vars() {
        let mut context = HashMap::new();
        context.insert(
            "_template_vars".to_string(),
            json!({
                "workflow_var": "workflow_value",
                "project_name": "WorkflowProject" // This should override config
            }),
        );

        let mut config = Configuration::new();
        config.insert(
            "project_name".to_string(),
            ConfigValue::String("ConfigProject".to_string()),
        );
        config.insert(
            "config_var".to_string(),
            ConfigValue::String("config_value".to_string()),
        );

        merge_config_into_context(&mut context, &config);

        let template_vars = context.get("_template_vars").unwrap().as_object().unwrap();
        assert_eq!(template_vars.get("workflow_var").unwrap(), "workflow_value");
        assert_eq!(
            template_vars.get("project_name").unwrap(),
            "WorkflowProject"
        ); // Workflow overrides config
        assert_eq!(template_vars.get("config_var").unwrap(), "config_value");
    }

    #[test]
    fn test_config_value_to_json_value_conversions() {
        // Test string
        let string_val = ConfigValue::String("test".to_string());
        assert_eq!(config_value_to_json_value(&string_val), json!("test"));

        // Test integer
        let int_val = ConfigValue::Integer(42);
        assert_eq!(config_value_to_json_value(&int_val), json!(42));

        // Test float
        let float_val = ConfigValue::Float(3.15); // Using 3.15 to avoid clippy PI warning
        assert_eq!(config_value_to_json_value(&float_val), json!(3.15));

        // Test boolean
        let bool_val = ConfigValue::Boolean(true);
        assert_eq!(config_value_to_json_value(&bool_val), json!(true));

        // Test array
        let array_val = ConfigValue::Array(vec![
            ConfigValue::String("item1".to_string()),
            ConfigValue::Integer(2),
        ]);
        assert_eq!(config_value_to_json_value(&array_val), json!(["item1", 2]));

        // Test table
        let mut table = HashMap::new();
        table.insert(
            "key1".to_string(),
            ConfigValue::String("value1".to_string()),
        );
        table.insert("key2".to_string(), ConfigValue::Integer(42));
        let table_val = ConfigValue::Table(table);
        assert_eq!(
            config_value_to_json_value(&table_val),
            json!({"key1": "value1", "key2": 42})
        );
    }

    #[test]
    fn test_substitute_env_vars_in_string() {
        env::set_var("TEST_VAR", "test_value");
        env::set_var("ANOTHER_VAR", "another_value");

        // Test simple substitution
        let result = substitute_env_vars_in_string("Hello ${TEST_VAR}!");
        assert_eq!(result, "Hello test_value!");

        // Test with default value (should use env var)
        let result = substitute_env_vars_in_string("Hello ${TEST_VAR:-default}!");
        assert_eq!(result, "Hello test_value!");

        // Test with missing var and default
        let result = substitute_env_vars_in_string("Hello ${MISSING_VAR:-default_value}!");
        assert_eq!(result, "Hello default_value!");

        // Test with missing var and no default
        let result = substitute_env_vars_in_string("Hello ${MISSING_VAR}!");
        assert_eq!(result, "Hello !");

        // Test multiple substitutions
        let result = substitute_env_vars_in_string("${TEST_VAR} and ${ANOTHER_VAR}");
        assert_eq!(result, "test_value and another_value");

        env::remove_var("TEST_VAR");
        env::remove_var("ANOTHER_VAR");
    }

    #[test]
    fn test_substitute_env_vars() {
        env::set_var("PROJECT_NAME", "MyProject");
        env::set_var("VERSION", "1.0.0");

        let mut config = Configuration::new();
        config.insert(
            "title".to_string(),
            ConfigValue::String("${PROJECT_NAME} v${VERSION}".to_string()),
        );
        config.insert("debug".to_string(), ConfigValue::Boolean(true)); // Should remain unchanged
        config.insert(
            "servers".to_string(),
            ConfigValue::Array(vec![
                ConfigValue::String("${PROJECT_NAME}-server1".to_string()),
                ConfigValue::String("${PROJECT_NAME}-server2".to_string()),
            ]),
        );

        substitute_env_vars(&mut config);

        assert_eq!(
            config.get("title").unwrap(),
            &ConfigValue::String("MyProject v1.0.0".to_string())
        );
        assert_eq!(config.get("debug").unwrap(), &ConfigValue::Boolean(true));

        if let Some(ConfigValue::Array(servers)) = config.get("servers") {
            assert_eq!(
                servers[0],
                ConfigValue::String("MyProject-server1".to_string())
            );
            assert_eq!(
                servers[1],
                ConfigValue::String("MyProject-server2".to_string())
            );
        } else {
            panic!("Expected servers array");
        }

        env::remove_var("PROJECT_NAME");
        env::remove_var("VERSION");
    }
}
