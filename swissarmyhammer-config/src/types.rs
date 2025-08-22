//! Core data structures for SwissArmyHammer configuration system

use crate::{ConfigError, ConfigResult};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::{debug, trace, warn};

/// Template context for rendering prompts, workflows, and actions
///
/// This replaces the HashMap-based context system with a structured approach
/// that supports environment variable substitution and merging operations.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TemplateContext {
    /// Template variables stored as JSON values for flexibility
    vars: HashMap<String, serde_json::Value>,
}

impl TemplateContext {
    /// Create a new empty template context
    pub fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    /// Create a template context with initial variables
    pub fn with_vars(vars: HashMap<String, serde_json::Value>) -> Self {
        Self { vars }
    }

    /// Get a template variable by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.vars.get(key)
    }

    /// Set a template variable
    pub fn set(&mut self, key: String, value: serde_json::Value) {
        trace!("Setting template variable: {} = {:?}", key, value);
        self.vars.insert(key, value);
    }

    /// Get all variables as a reference to the HashMap
    pub fn vars(&self) -> &HashMap<String, serde_json::Value> {
        &self.vars
    }

    /// Get all variables as a mutable reference to the HashMap
    pub fn vars_mut(&mut self) -> &mut HashMap<String, serde_json::Value> {
        &mut self.vars
    }

    /// Merge another context into this one with precedence
    ///
    /// Variables from `other` will override variables in `self` for conflicting keys.
    pub fn merge(&mut self, other: &TemplateContext) {
        debug!(
            "Merging template context with {} variables",
            other.vars.len()
        );
        for (key, value) in &other.vars {
            trace!("Merging variable: {} = {:?}", key, value);
            self.vars.insert(key.clone(), value.clone());
        }
    }

    /// Process environment variable substitution in all template variables
    ///
    /// Supports patterns:
    /// - `${VAR}` - Replace with environment variable VAR
    /// - `${VAR:-default}` - Replace with VAR or default if VAR is unset
    pub fn substitute_env_vars(&mut self) -> ConfigResult<()> {
        debug!("Performing environment variable substitution");
        let mut updated_vars = HashMap::new();

        for (key, value) in &self.vars {
            let substituted = self.substitute_value(value)?;
            updated_vars.insert(key.clone(), substituted);
        }

        self.vars = updated_vars;
        Ok(())
    }

    /// Convert to a liquid template object for rendering
    ///
    /// This creates an object that can be used directly with the liquid template engine.
    pub fn to_liquid_object(&self) -> liquid::Object {
        debug!("Converting template context to liquid object");
        let mut object = liquid::Object::new();

        for (key, value) in &self.vars {
            if let Some(liquid_value) = Self::json_to_liquid(value) {
                object.insert(key.clone().into(), liquid_value);
            } else {
                warn!(
                    "Failed to convert variable '{}' to liquid value: {:?}",
                    key, value
                );
            }
        }

        object
    }

    /// Check if the context is empty
    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }

    /// Get the number of variables in the context
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    /// Substitute environment variables in a single JSON value
    fn substitute_value(&self, value: &serde_json::Value) -> ConfigResult<serde_json::Value> {
        match value {
            serde_json::Value::String(s) => {
                let substituted = self.substitute_env_vars_in_string(s)?;
                Ok(serde_json::Value::String(substituted))
            }
            serde_json::Value::Array(arr) => {
                let mut new_arr = Vec::new();
                for item in arr {
                    new_arr.push(self.substitute_value(item)?);
                }
                Ok(serde_json::Value::Array(new_arr))
            }
            serde_json::Value::Object(obj) => {
                let mut new_obj = serde_json::Map::new();
                for (k, v) in obj {
                    new_obj.insert(k.clone(), self.substitute_value(v)?);
                }
                Ok(serde_json::Value::Object(new_obj))
            }
            _ => Ok(value.clone()),
        }
    }

    /// Substitute environment variables in a string using ${VAR} and ${VAR:-default} patterns
    fn substitute_env_vars_in_string(&self, input: &str) -> ConfigResult<String> {
        let mut result = input.to_string();
        let mut start = 0;

        while let Some(dollar_pos) = result[start..].find("${") {
            let dollar_pos = start + dollar_pos;
            if let Some(brace_pos) = result[dollar_pos..].find('}') {
                let brace_pos = dollar_pos + brace_pos;
                let var_expr = &result[dollar_pos + 2..brace_pos];

                let (var_name, default_value) = if let Some(colon_pos) = var_expr.find(":-") {
                    let var_name = &var_expr[..colon_pos];
                    let default = &var_expr[colon_pos + 2..];
                    (var_name, Some(default))
                } else {
                    (var_expr, None)
                };

                let replacement = match std::env::var(var_name) {
                    Ok(value) => {
                        trace!(
                            "Environment variable substitution: {} = {}",
                            var_name,
                            value
                        );
                        value
                    }
                    Err(_) => {
                        if let Some(default) = default_value {
                            trace!("Using default value for {}: {}", var_name, default);
                            default.to_string()
                        } else {
                            warn!(
                                "Environment variable '{}' not found and no default provided",
                                var_name
                            );
                            return Err(ConfigError::environment_error(format!(
                                "Environment variable '{}' not found and no default provided",
                                var_name
                            )));
                        }
                    }
                };

                result.replace_range(dollar_pos..=brace_pos, &replacement);
                start = dollar_pos + replacement.len();
            } else {
                start = dollar_pos + 2;
            }
        }

        Ok(result)
    }

    /// Convert a JSON value to a liquid value
    fn json_to_liquid(value: &serde_json::Value) -> Option<liquid::model::Value> {
        match value {
            serde_json::Value::Null => Some(liquid::model::Value::Nil),
            serde_json::Value::Bool(b) => Some(liquid::model::Value::scalar(*b)),
            serde_json::Value::Number(n) => {
                if let Some(i) = n.as_i64() {
                    Some(liquid::model::Value::scalar(i))
                } else {
                    n.as_f64().map(liquid::model::Value::scalar)
                }
            }
            serde_json::Value::String(s) => Some(liquid::model::Value::scalar(s.clone())),
            serde_json::Value::Array(arr) => {
                let mut liquid_arr = Vec::new();
                for item in arr {
                    if let Some(liquid_val) = Self::json_to_liquid(item) {
                        liquid_arr.push(liquid_val);
                    }
                }
                Some(liquid::model::Value::Array(liquid_arr))
            }
            serde_json::Value::Object(obj) => {
                let mut liquid_obj = liquid::Object::new();
                for (k, v) in obj {
                    if let Some(liquid_val) = Self::json_to_liquid(v) {
                        liquid_obj.insert(k.clone().into(), liquid_val);
                    }
                }
                Some(liquid::model::Value::Object(liquid_obj))
            }
        }
    }
}

/// Raw configuration values from files
///
/// This represents the unprocessed configuration data loaded from files
/// before environment variable substitution and context processing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawConfig {
    /// Flattened configuration values
    #[serde(flatten)]
    pub values: HashMap<String, serde_json::Value>,
}

impl RawConfig {
    /// Create a new empty raw config
    pub fn new() -> Self {
        Self {
            values: HashMap::new(),
        }
    }

    /// Create raw config from a HashMap
    pub fn from_map(values: HashMap<String, serde_json::Value>) -> Self {
        Self { values }
    }

    /// Convert to a TemplateContext
    pub fn to_template_context(self) -> TemplateContext {
        TemplateContext::with_vars(self.values)
    }

    /// Check if the raw config is empty
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

impl Default for RawConfig {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_template_context_new() {
        let ctx = TemplateContext::new();
        assert!(ctx.is_empty());
        assert_eq!(ctx.len(), 0);
    }

    #[test]
    fn test_template_context_with_vars() {
        let mut vars = HashMap::new();
        vars.insert(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );
        vars.insert("key2".to_string(), serde_json::Value::Number(42.into()));

        let ctx = TemplateContext::with_vars(vars);
        assert!(!ctx.is_empty());
        assert_eq!(ctx.len(), 2);
        assert_eq!(
            ctx.get("key1"),
            Some(&serde_json::Value::String("value1".to_string()))
        );
        assert_eq!(ctx.get("key2"), Some(&serde_json::Value::Number(42.into())));
    }

    #[test]
    fn test_template_context_set_get() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "test_key".to_string(),
            serde_json::Value::String("test_value".to_string()),
        );

        assert_eq!(
            ctx.get("test_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );
        assert_eq!(ctx.get("nonexistent"), None);
    }

    #[test]
    fn test_template_context_merge() {
        let mut ctx1 = TemplateContext::new();
        ctx1.set(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );
        ctx1.set(
            "key2".to_string(),
            serde_json::Value::String("original".to_string()),
        );

        let mut ctx2 = TemplateContext::new();
        ctx2.set(
            "key2".to_string(),
            serde_json::Value::String("overridden".to_string()),
        );
        ctx2.set(
            "key3".to_string(),
            serde_json::Value::String("value3".to_string()),
        );

        ctx1.merge(&ctx2);

        assert_eq!(
            ctx1.get("key1"),
            Some(&serde_json::Value::String("value1".to_string()))
        );
        assert_eq!(
            ctx1.get("key2"),
            Some(&serde_json::Value::String("overridden".to_string()))
        );
        assert_eq!(
            ctx1.get("key3"),
            Some(&serde_json::Value::String("value3".to_string()))
        );
        assert_eq!(ctx1.len(), 3);
    }

    #[test]
    fn test_env_var_substitution_simple() {
        std::env::set_var("TEST_VAR", "test_value");

        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_key".to_string(),
            serde_json::Value::String("${TEST_VAR}".to_string()),
        );

        ctx.substitute_env_vars().unwrap();

        assert_eq!(
            ctx.get("config_key"),
            Some(&serde_json::Value::String("test_value".to_string()))
        );

        std::env::remove_var("TEST_VAR");
    }

    #[test]
    fn test_env_var_substitution_with_default() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_key".to_string(),
            serde_json::Value::String("${NONEXISTENT_VAR:-default_value}".to_string()),
        );

        ctx.substitute_env_vars().unwrap();

        assert_eq!(
            ctx.get("config_key"),
            Some(&serde_json::Value::String("default_value".to_string()))
        );
    }

    #[test]
    fn test_env_var_substitution_missing_no_default() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_key".to_string(),
            serde_json::Value::String("${NONEXISTENT_VAR}".to_string()),
        );

        let result = ctx.substitute_env_vars();
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigError::EnvironmentError { .. }
        ));
    }

    #[test]
    fn test_env_var_substitution_in_nested_structures() {
        std::env::set_var("NESTED_TEST", "nested_value");

        let mut ctx = TemplateContext::new();
        let nested_obj = serde_json::json!({
            "inner": "${NESTED_TEST}",
            "array": ["${NESTED_TEST}", "static"]
        });
        ctx.set("nested".to_string(), nested_obj);

        ctx.substitute_env_vars().unwrap();

        let expected = serde_json::json!({
            "inner": "nested_value",
            "array": ["nested_value", "static"]
        });
        assert_eq!(ctx.get("nested"), Some(&expected));

        std::env::remove_var("NESTED_TEST");
    }

    #[test]
    fn test_raw_config_new() {
        let config = RawConfig::new();
        assert!(config.is_empty());
    }

    #[test]
    fn test_raw_config_from_map() {
        let mut values = HashMap::new();
        values.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let config = RawConfig::from_map(values);
        assert!(!config.is_empty());
        assert_eq!(
            config.values.get("key"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }

    #[test]
    fn test_raw_config_to_template_context() {
        let mut values = HashMap::new();
        values.insert(
            "key".to_string(),
            serde_json::Value::String("value".to_string()),
        );

        let config = RawConfig::from_map(values);
        let ctx = config.to_template_context();

        assert_eq!(
            ctx.get("key"),
            Some(&serde_json::Value::String("value".to_string()))
        );
    }
}
