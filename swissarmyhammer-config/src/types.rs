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

    /// Create from configuration values only
    pub fn from_config(config_vars: HashMap<String, serde_json::Value>) -> Self {
        debug!("Creating TemplateContext from config with {} variables", config_vars.len());
        Self::with_vars(config_vars)
    }

    /// Get a template variable by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.vars.get(key)
    }

    /// Get variable as string if possible
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.vars.get(key).and_then(|v| {
            match v {
                serde_json::Value::String(s) => Some(s.clone()),
                serde_json::Value::Number(n) => Some(n.to_string()),
                serde_json::Value::Bool(b) => Some(b.to_string()),
                _ => None,
            }
        })
    }

    /// Get variable as boolean if possible
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.vars.get(key).and_then(|v| {
            match v {
                serde_json::Value::Bool(b) => Some(*b),
                serde_json::Value::String(s) => {
                    match s.to_lowercase().as_str() {
                        "true" | "yes" | "1" => Some(true),
                        "false" | "no" | "0" => Some(false),
                        _ => None,
                    }
                }
                serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0),
                _ => None,
            }
        })
    }

    /// Get variable as number if possible
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.vars.get(key).and_then(|v| {
            match v {
                serde_json::Value::Number(n) => n.as_f64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            }
        })
    }

    /// Set a template variable
    pub fn set<K, V>(&mut self, key: K, value: V) 
    where 
        K: Into<String>,
        V: Into<serde_json::Value>
    {
        let key = key.into();
        let value = value.into();
        trace!("Setting template variable: {} = {:?}", key, value);
        self.vars.insert(key, value);
    }

    /// Insert multiple variables
    pub fn extend(&mut self, vars: HashMap<String, serde_json::Value>) {
        debug!("Extending TemplateContext with {} variables", vars.len());
        for (key, value) in vars {
            self.vars.insert(key, value);
        }
    }

    /// Check if variable exists
    pub fn contains_key(&self, key: &str) -> bool {
        self.vars.contains_key(key)
    }

    /// Get all variable keys
    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.vars.keys()
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

    /// Merge with configuration context (config has lower priority)
    pub fn merge_config(&mut self, config_vars: HashMap<String, serde_json::Value>) {
        debug!("Merging config variables with lower priority: {} variables", config_vars.len());
        // Insert config vars first, then existing vars will override them
        let mut merged = config_vars;
        for (key, value) in &self.vars {
            merged.insert(key.clone(), value.clone());
        }
        self.vars = merged;
    }

    /// Merge with workflow variables (workflow has higher priority)
    pub fn merge_workflow(&mut self, workflow_vars: HashMap<String, serde_json::Value>) {
        debug!("Merging workflow variables with higher priority: {} variables", workflow_vars.len());
        // Workflow vars override existing vars
        for (key, value) in workflow_vars {
            trace!("Workflow variable override: {} = {:?}", key, value);
            self.vars.insert(key, value);
        }
    }

    /// Create merged context without modifying self
    pub fn merged_with(&self, other: &TemplateContext) -> TemplateContext {
        let mut result = self.clone();
        result.merge(other);
        result
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

    /// Get context with environment variables substituted (non-mutating)
    pub fn with_env_substitution(&self) -> ConfigResult<TemplateContext> {
        debug!("Creating context with environment variable substitution");
        let mut result = self.clone();
        result.substitute_env_vars()?;
        Ok(result)
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

    /// Extract as HashMap for compatibility with existing code
    pub fn as_hashmap(&self) -> &HashMap<String, serde_json::Value> {
        &self.vars
    }

    /// Convert to HashMap (consuming)
    pub fn into_hashmap(self) -> HashMap<String, serde_json::Value> {
        self.vars
    }

    /// Get variables in the legacy `_template_vars` format
    pub fn as_legacy_context(&self) -> HashMap<String, serde_json::Value> {
        let mut legacy = HashMap::new();
        legacy.insert("_template_vars".to_string(), serde_json::Value::Object(
            self.vars.iter().map(|(k, v)| (k.clone(), v.clone())).collect()
        ));
        legacy
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

/// Conversion trait implementations for TemplateContext
impl From<HashMap<String, serde_json::Value>> for TemplateContext {
    fn from(vars: HashMap<String, serde_json::Value>) -> Self {
        Self::with_vars(vars)
    }
}

impl From<TemplateContext> for HashMap<String, serde_json::Value> {
    fn from(ctx: TemplateContext) -> Self {
        ctx.vars
    }
}

impl From<TemplateContext> for liquid::Object {
    fn from(ctx: TemplateContext) -> Self {
        ctx.to_liquid_object()
    }
}

// TODO: Implement From<liquid::Object> for TemplateContext once we understand the liquid::Value API better
// This will be added in a follow-up implementation

impl TemplateContext {
    /// Create TemplateContext from liquid::Object (manual implementation)
    pub fn from_liquid_object(_obj: liquid::Object) -> Self {
        // For now, we'll implement this as a no-op and return empty context
        // This will be properly implemented once we understand the liquid::Value enum
        warn!("from_liquid_object is not yet fully implemented");
        Self::new()
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

    #[test]
    fn test_template_context_from_config() {
        let mut config_vars = HashMap::new();
        config_vars.insert(
            "project_name".to_string(),
            serde_json::Value::String("TestProject".to_string()),
        );
        config_vars.insert("debug".to_string(), serde_json::Value::Bool(true));

        let ctx = TemplateContext::from_config(config_vars);
        assert_eq!(ctx.len(), 2);
        assert_eq!(
            ctx.get("project_name"),
            Some(&serde_json::Value::String("TestProject".to_string()))
        );
        assert_eq!(ctx.get("debug"), Some(&serde_json::Value::Bool(true)));
    }

    #[test]
    fn test_template_context_typed_getters() {
        let mut ctx = TemplateContext::new();
        ctx.set("string_val", "test_string");
        ctx.set("bool_val", true);
        ctx.set("number_val", 42.5);
        ctx.set("bool_string_true", "true");
        ctx.set("bool_string_false", "false");
        ctx.set("number_string", "123.45");

        // Test string getter
        assert_eq!(ctx.get_string("string_val"), Some("test_string".to_string()));
        assert_eq!(ctx.get_string("bool_val"), Some("true".to_string()));
        assert_eq!(ctx.get_string("number_val"), Some("42.5".to_string()));
        assert_eq!(ctx.get_string("nonexistent"), None);

        // Test bool getter
        assert_eq!(ctx.get_bool("bool_val"), Some(true));
        assert_eq!(ctx.get_bool("bool_string_true"), Some(true));
        assert_eq!(ctx.get_bool("bool_string_false"), Some(false));
        assert_eq!(ctx.get_bool("string_val"), None);
        assert_eq!(ctx.get_bool("nonexistent"), None);

        // Test number getter
        assert_eq!(ctx.get_number("number_val"), Some(42.5));
        assert_eq!(ctx.get_number("number_string"), Some(123.45));
        assert_eq!(ctx.get_number("string_val"), None);
        assert_eq!(ctx.get_number("nonexistent"), None);
    }

    #[test]
    fn test_template_context_extend() {
        let mut ctx = TemplateContext::new();
        ctx.set("existing", "value");

        let mut new_vars = HashMap::new();
        new_vars.insert(
            "new_key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );
        new_vars.insert(
            "new_key2".to_string(),
            serde_json::Value::Number(42.into()),
        );

        ctx.extend(new_vars);

        assert_eq!(ctx.len(), 3);
        assert!(ctx.contains_key("existing"));
        assert!(ctx.contains_key("new_key1"));
        assert!(ctx.contains_key("new_key2"));
        assert!(!ctx.contains_key("nonexistent"));
    }

    #[test]
    fn test_template_context_keys() {
        let mut ctx = TemplateContext::new();
        ctx.set("key1", "value1");
        ctx.set("key2", "value2");
        ctx.set("key3", "value3");

        let keys: Vec<&String> = ctx.keys().collect();
        assert_eq!(keys.len(), 3);
        assert!(keys.contains(&&"key1".to_string()));
        assert!(keys.contains(&&"key2".to_string()));
        assert!(keys.contains(&&"key3".to_string()));
    }

    #[test]
    fn test_template_context_merge_config() {
        let mut ctx = TemplateContext::new();
        ctx.set("existing_var", "existing_value");
        ctx.set("override_me", "workflow_value");

        let mut config_vars = HashMap::new();
        config_vars.insert(
            "config_var".to_string(),
            serde_json::Value::String("config_value".to_string()),
        );
        config_vars.insert(
            "override_me".to_string(),
            serde_json::Value::String("config_value".to_string()),
        );

        ctx.merge_config(config_vars);

        // Existing workflow vars should override config
        assert_eq!(
            ctx.get("override_me"),
            Some(&serde_json::Value::String("workflow_value".to_string()))
        );
        // New config vars should be added
        assert_eq!(
            ctx.get("config_var"),
            Some(&serde_json::Value::String("config_value".to_string()))
        );
        // Existing vars should remain
        assert_eq!(
            ctx.get("existing_var"),
            Some(&serde_json::Value::String("existing_value".to_string()))
        );
    }

    #[test]
    fn test_template_context_merge_workflow() {
        let mut ctx = TemplateContext::new();
        ctx.set("config_var", "config_value");
        ctx.set("override_me", "config_value");

        let mut workflow_vars = HashMap::new();
        workflow_vars.insert(
            "workflow_var".to_string(),
            serde_json::Value::String("workflow_value".to_string()),
        );
        workflow_vars.insert(
            "override_me".to_string(),
            serde_json::Value::String("workflow_value".to_string()),
        );

        ctx.merge_workflow(workflow_vars);

        // Workflow vars should override config
        assert_eq!(
            ctx.get("override_me"),
            Some(&serde_json::Value::String("workflow_value".to_string()))
        );
        // New workflow vars should be added
        assert_eq!(
            ctx.get("workflow_var"),
            Some(&serde_json::Value::String("workflow_value".to_string()))
        );
        // Existing config vars should remain
        assert_eq!(
            ctx.get("config_var"),
            Some(&serde_json::Value::String("config_value".to_string()))
        );
    }

    #[test]
    fn test_template_context_merged_with() {
        let mut ctx1 = TemplateContext::new();
        ctx1.set("key1", "value1");
        ctx1.set("shared", "original");

        let mut ctx2 = TemplateContext::new();
        ctx2.set("key2", "value2");
        ctx2.set("shared", "override");

        let merged = ctx1.merged_with(&ctx2);

        // Original contexts should be unchanged
        assert_eq!(
            ctx1.get("shared"),
            Some(&serde_json::Value::String("original".to_string()))
        );
        assert_eq!(ctx1.get("key2"), None);

        // Merged context should have all values with proper precedence
        assert_eq!(
            merged.get("key1"),
            Some(&serde_json::Value::String("value1".to_string()))
        );
        assert_eq!(
            merged.get("key2"),
            Some(&serde_json::Value::String("value2".to_string()))
        );
        assert_eq!(
            merged.get("shared"),
            Some(&serde_json::Value::String("override".to_string()))
        );
    }

    #[test]
    fn test_template_context_with_env_substitution() {
        std::env::set_var("IMMUTABLE_TEST", "immutable_value");

        let mut ctx = TemplateContext::new();
        ctx.set("var1", "${IMMUTABLE_TEST}");
        ctx.set("var2", "unchanged");

        let new_ctx = ctx.with_env_substitution().unwrap();

        // Original context should be unchanged
        assert_eq!(
            ctx.get("var1"),
            Some(&serde_json::Value::String("${IMMUTABLE_TEST}".to_string()))
        );

        // New context should have substitution
        assert_eq!(
            new_ctx.get("var1"),
            Some(&serde_json::Value::String("immutable_value".to_string()))
        );
        assert_eq!(
            new_ctx.get("var2"),
            Some(&serde_json::Value::String("unchanged".to_string()))
        );

        std::env::remove_var("IMMUTABLE_TEST");
    }

    #[test]
    fn test_template_context_compatibility_layer() {
        let mut ctx = TemplateContext::new();
        ctx.set("key1", "value1");
        ctx.set("key2", 42);

        // Test as_hashmap
        let hashmap_ref = ctx.as_hashmap();
        assert_eq!(hashmap_ref.len(), 2);
        assert!(hashmap_ref.contains_key("key1"));

        // Test into_hashmap
        let ctx_copy = ctx.clone();
        let hashmap = ctx_copy.into_hashmap();
        assert_eq!(hashmap.len(), 2);
        assert_eq!(
            hashmap.get("key1"),
            Some(&serde_json::Value::String("value1".to_string()))
        );

        // Test legacy context format
        let legacy = ctx.as_legacy_context();
        assert!(legacy.contains_key("_template_vars"));
        if let Some(serde_json::Value::Object(template_vars)) = legacy.get("_template_vars") {
            assert_eq!(template_vars.len(), 2);
            assert!(template_vars.contains_key("key1"));
            assert!(template_vars.contains_key("key2"));
        } else {
            panic!("Expected _template_vars to be an object");
        }
    }

    #[test]
    fn test_template_context_conversion_traits() {
        let mut original_map = HashMap::new();
        original_map.insert(
            "key1".to_string(),
            serde_json::Value::String("value1".to_string()),
        );
        original_map.insert("key2".to_string(), serde_json::Value::Number(42.into()));

        // Test From<HashMap>
        let ctx: TemplateContext = original_map.clone().into();
        assert_eq!(ctx.len(), 2);
        assert_eq!(
            ctx.get("key1"),
            Some(&serde_json::Value::String("value1".to_string()))
        );

        // Test Into<HashMap>
        let back_to_map: HashMap<String, serde_json::Value> = ctx.into();
        assert_eq!(back_to_map.len(), 2);
        assert_eq!(back_to_map, original_map);
    }
}
