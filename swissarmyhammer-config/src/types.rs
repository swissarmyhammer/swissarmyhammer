//! Core data structures for SwissArmyHammer configuration system

use crate::ConfigResult;
use liquid::ValueView;
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
        debug!(
            "Creating TemplateContext from config with {} variables",
            config_vars.len()
        );
        Self::with_vars(config_vars)
    }

    /// Get a template variable by key
    pub fn get(&self, key: &str) -> Option<&serde_json::Value> {
        self.vars.get(key)
    }

    /// Get variable as string if possible
    pub fn get_string(&self, key: &str) -> Option<String> {
        self.vars.get(key).and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            serde_json::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        })
    }

    /// Get variable as boolean if possible
    pub fn get_bool(&self, key: &str) -> Option<bool> {
        self.vars.get(key).and_then(|v| match v {
            serde_json::Value::Bool(b) => Some(*b),
            serde_json::Value::String(s) => match s.to_lowercase().as_str() {
                "true" | "yes" | "1" => Some(true),
                "false" | "no" | "0" => Some(false),
                _ => None,
            },
            serde_json::Value::Number(n) => n.as_i64().map(|i| i != 0),
            _ => None,
        })
    }

    /// Get variable as number if possible
    pub fn get_number(&self, key: &str) -> Option<f64> {
        self.vars.get(key).and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_f64(),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        })
    }

    /// Set a template variable
    pub fn set<K, V>(&mut self, key: K, value: V)
    where
        K: Into<String>,
        V: Into<serde_json::Value>,
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
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    ///
    /// let mut base_ctx = TemplateContext::new();
    /// base_ctx.set("app_name", serde_json::Value::String("MyApp".to_string()));
    /// base_ctx.set("version", serde_json::Value::String("1.0.0".to_string()));
    ///
    /// let mut override_ctx = TemplateContext::new();
    /// override_ctx.set("version", serde_json::Value::String("2.0.0".to_string()));
    /// override_ctx.set("debug", serde_json::Value::Bool(true));
    ///
    /// base_ctx.merge(&override_ctx);
    ///
    /// // `version` is overridden, `app_name` is preserved, `debug` is added
    /// assert_eq!(base_ctx.get_string("app_name"), Some("MyApp".to_string()));
    /// assert_eq!(base_ctx.get_string("version"), Some("2.0.0".to_string()));
    /// assert_eq!(base_ctx.get_bool("debug"), Some(true));
    /// ```
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
    ///
    /// Configuration variables are added only if they don't conflict with existing variables.
    /// Existing template variables have higher priority and will not be overridden.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::collections::HashMap;
    ///
    /// let mut ctx = TemplateContext::new();
    /// ctx.set("user_setting", serde_json::Value::String("custom".to_string()));
    ///
    /// let mut config_vars = HashMap::new();
    /// config_vars.insert("user_setting".to_string(), serde_json::Value::String("default".to_string()));
    /// config_vars.insert("global_timeout".to_string(), serde_json::Value::Number(30.into()));
    ///
    /// ctx.merge_config(config_vars);
    ///
    /// // user_setting keeps its original value (higher priority)
    /// assert_eq!(ctx.get_string("user_setting"), Some("custom".to_string()));
    /// // global_timeout is added from config
    /// assert_eq!(ctx.get_number("global_timeout"), Some(30.0));
    /// ```
    pub fn merge_config(&mut self, config_vars: HashMap<String, serde_json::Value>) {
        debug!(
            "Merging config variables with lower priority: {} variables",
            config_vars.len()
        );
        // Insert config vars first, then existing vars will override them
        let mut merged = config_vars;
        for (key, value) in &self.vars {
            merged.insert(key.clone(), value.clone());
        }
        self.vars = merged;
    }

    /// Merge with workflow variables (workflow has higher priority)
    ///
    /// Workflow variables override any existing variables with the same keys.
    /// This is typically used to inject runtime state into template contexts.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::collections::HashMap;
    ///
    /// let mut ctx = TemplateContext::new();
    /// ctx.set("environment", serde_json::Value::String("development".to_string()));
    /// ctx.set("debug_mode", serde_json::Value::Bool(false));
    ///
    /// let mut workflow_vars = HashMap::new();
    /// workflow_vars.insert("environment".to_string(), serde_json::Value::String("production".to_string()));
    /// workflow_vars.insert("workflow_id".to_string(), serde_json::Value::String("deploy-001".to_string()));
    ///
    /// ctx.merge_workflow(workflow_vars);
    ///
    /// // environment is overridden by workflow
    /// assert_eq!(ctx.get_string("environment"), Some("production".to_string()));
    /// // debug_mode is preserved
    /// assert_eq!(ctx.get_bool("debug_mode"), Some(false));
    /// // workflow_id is added
    /// assert_eq!(ctx.get_string("workflow_id"), Some("deploy-001".to_string()));
    /// ```
    pub fn merge_workflow(&mut self, workflow_vars: HashMap<String, serde_json::Value>) {
        debug!(
            "Merging workflow variables with higher priority: {} variables",
            workflow_vars.len()
        );
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
    /// Uses the legacy-compatible behavior where missing environment variables
    /// without defaults return empty strings rather than errors.
    ///
    /// Supports patterns:
    /// - `${VAR}` - Replace with environment variable VAR, empty string if not set
    /// - `${VAR:-default}` - Replace with VAR or default if VAR is unset
    ///
    /// Substitution works recursively through nested JSON objects and arrays.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::env;
    ///
    /// // Set up environment variables
    /// env::set_var("DATABASE_HOST", "localhost");
    /// env::set_var("DATABASE_PORT", "5432");
    ///
    /// let mut ctx = TemplateContext::new();
    /// ctx.set("connection", serde_json::json!({
    ///     "url": "postgresql://${DATABASE_HOST}:${DATABASE_PORT}/myapp",
    ///     "timeout": "${CONNECTION_TIMEOUT:-30}",
    ///     "pool_size": "${POOL_SIZE}"  // Will be empty string if not set
    /// }));
    ///
    /// ctx.substitute_env_vars().unwrap();
    ///
    /// let connection = ctx.get("connection").unwrap();
    /// assert_eq!(connection["url"], "postgresql://localhost:5432/myapp");
    /// assert_eq!(connection["timeout"], "30"); // default used
    /// assert_eq!(connection["pool_size"], ""); // empty string for missing var
    ///
    /// // Clean up
    /// env::remove_var("DATABASE_HOST");
    /// env::remove_var("DATABASE_PORT");
    /// ```
    pub fn substitute_env_vars(&mut self) -> ConfigResult<()> {
        debug!("Performing environment variable substitution (legacy compatible mode)");
        crate::env_substitution::LEGACY_PROCESSOR
            .with(|processor| processor.substitute_vars(&mut self.vars))
    }

    /// Process environment variable substitution in strict mode
    ///
    /// In strict mode, missing environment variables without defaults will return errors
    /// rather than empty strings. This provides better validation but may break compatibility
    /// with existing templates that expect empty string behavior.
    ///
    /// # Arguments
    /// * None
    ///
    /// # Returns
    /// * `ConfigResult<()>` - Ok if successful, Err if any environment variables are missing
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::env;
    ///
    /// let mut ctx = TemplateContext::new();
    /// ctx.set("config", serde_json::Value::String("${MISSING_VAR}".to_string()));
    ///
    /// // This will return an error because MISSING_VAR is not set and has no default
    /// let result = ctx.substitute_env_vars_strict();
    /// assert!(result.is_err());
    /// ```
    pub fn substitute_env_vars_strict(&mut self) -> ConfigResult<()> {
        debug!("Performing environment variable substitution (strict mode)");
        crate::env_substitution::STRICT_PROCESSOR
            .with(|processor| processor.substitute_vars(&mut self.vars))
    }

    /// Get context with environment variables substituted (non-mutating, legacy mode)
    ///
    /// Creates a new context with environment variables substituted without modifying the original.
    /// Uses legacy-compatible behavior where missing variables without defaults become empty strings.
    /// This is useful for functional-style programming where immutability is preferred.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::env;
    ///
    /// env::set_var("API_KEY", "secret123");
    ///
    /// let mut original_ctx = TemplateContext::new();
    /// original_ctx.set("api_url", serde_json::Value::String("https://api.example.com/${API_KEY}".to_string()));
    /// original_ctx.set("missing", serde_json::Value::String("${MISSING_VAR}".to_string()));
    ///
    /// let substituted_ctx = original_ctx.with_env_substitution().unwrap();
    ///
    /// // Original context is unchanged
    /// assert_eq!(original_ctx.get_string("api_url"), Some("https://api.example.com/${API_KEY}".to_string()));
    ///
    /// // New context has substituted values
    /// assert_eq!(substituted_ctx.get_string("api_url"), Some("https://api.example.com/secret123".to_string()));
    /// assert_eq!(substituted_ctx.get_string("missing"), Some("".to_string())); // empty string for missing var
    ///
    /// env::remove_var("API_KEY");
    /// ```
    pub fn with_env_substitution(&self) -> ConfigResult<TemplateContext> {
        debug!("Creating context with environment variable substitution (legacy compatible mode)");
        let mut result = self.clone();
        result.substitute_env_vars()?;
        Ok(result)
    }

    /// Get context with environment variables substituted in strict mode (non-mutating)
    ///
    /// Creates a new context with environment variables substituted without modifying the original.
    /// Uses strict mode where missing variables without defaults return errors.
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::env;
    ///
    /// env::set_var("API_KEY", "secret123");
    ///
    /// let mut original_ctx = TemplateContext::new();
    /// original_ctx.set("api_url", serde_json::Value::String("https://api.example.com/${API_KEY}".to_string()));
    /// original_ctx.set("missing", serde_json::Value::String("${MISSING_VAR}".to_string()));
    ///
    /// // This will return an error because MISSING_VAR is not set
    /// let result = original_ctx.with_env_substitution_strict();
    /// assert!(result.is_err());
    ///
    /// env::remove_var("API_KEY");
    /// ```
    pub fn with_env_substitution_strict(&self) -> ConfigResult<TemplateContext> {
        debug!("Creating context with environment variable substitution (strict mode)");
        let mut result = self.clone();
        result.substitute_env_vars_strict()?;
        Ok(result)
    }

    /// Substitute environment variables in a specific variable
    ///
    /// This method allows selective substitution of individual template variables,
    /// useful for optimization or when only specific variables need processing.
    ///
    /// # Arguments
    /// * `key` - The key of the variable to process
    /// * `strict` - Whether to use strict mode (errors for missing vars) or legacy mode (empty strings)
    ///
    /// # Example
    ///
    /// ```rust
    /// use swissarmyhammer_config::types::TemplateContext;
    /// use std::env;
    ///
    /// env::set_var("SELECTIVE_VAR", "selected");
    ///
    /// let mut ctx = TemplateContext::new();
    /// ctx.set("var1", serde_json::Value::String("${SELECTIVE_VAR}".to_string()));
    /// ctx.set("var2", serde_json::Value::String("${OTHER_VAR}".to_string()));
    ///
    /// // Only substitute var1
    /// ctx.substitute_var("var1", false).unwrap();
    ///
    /// assert_eq!(ctx.get_string("var1"), Some("selected".to_string()));
    /// assert_eq!(ctx.get_string("var2"), Some("${OTHER_VAR}".to_string())); // unchanged
    ///
    /// env::remove_var("SELECTIVE_VAR");
    /// ```
    pub fn substitute_var(&mut self, key: &str, strict: bool) -> ConfigResult<()> {
        if let Some(value) = self.vars.get_mut(key) {
            if strict {
                crate::env_substitution::STRICT_PROCESSOR
                    .with(|processor| processor.substitute_value(value))
            } else {
                crate::env_substitution::LEGACY_PROCESSOR
                    .with(|processor| processor.substitute_value(value))
            }
        } else {
            Ok(()) // Variable doesn't exist, nothing to do
        }
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
        legacy.insert(
            "_template_vars".to_string(),
            serde_json::Value::Object(
                self.vars
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            ),
        );
        legacy
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

impl From<liquid::Object> for TemplateContext {
    fn from(obj: liquid::Object) -> Self {
        Self::from_liquid_object(obj)
    }
}

impl TemplateContext {
    /// Create TemplateContext from liquid::Object
    pub fn from_liquid_object(obj: liquid::Object) -> Self {
        debug!("Converting liquid object to template context");
        let mut vars = HashMap::new();

        for (key, value) in obj.iter() {
            if let Some(json_value) = Self::liquid_to_json(value) {
                vars.insert(key.to_string(), json_value);
            } else {
                warn!(
                    "Failed to convert liquid variable '{}' to JSON value: {:?}",
                    key, value
                );
            }
        }

        Self::with_vars(vars)
    }

    /// Convert a liquid value to a JSON value
    fn liquid_to_json(value: &liquid::model::Value) -> Option<serde_json::Value> {
        match value {
            liquid::model::Value::Nil => Some(serde_json::Value::Null),
            liquid::model::Value::Scalar(scalar) => {
                // Handle scalar values - try to convert to appropriate JSON type
                if let Some(b) = scalar.to_bool() {
                    Some(serde_json::Value::Bool(b))
                } else if let Some(i) = scalar.to_integer() {
                    Some(serde_json::Value::Number(serde_json::Number::from(i)))
                } else if let Some(f) = scalar.to_float() {
                    serde_json::Number::from_f64(f).map(serde_json::Value::Number)
                } else {
                    // Fallback to string representation
                    Some(serde_json::Value::String(scalar.to_kstr().to_string()))
                }
            }
            liquid::model::Value::Array(arr) => {
                let mut json_arr = Vec::new();
                for item in arr {
                    if let Some(json_val) = Self::liquid_to_json(item) {
                        json_arr.push(json_val);
                    }
                }
                Some(serde_json::Value::Array(json_arr))
            }
            liquid::model::Value::Object(obj) => {
                let mut json_obj = serde_json::Map::new();
                for (k, v) in obj.iter() {
                    if let Some(json_val) = Self::liquid_to_json(v) {
                        json_obj.insert(k.to_string(), json_val);
                    }
                }
                Some(serde_json::Value::Object(json_obj))
            }
            // Handle any other liquid value types by converting to string
            _ => {
                warn!("Unknown liquid value type, converting to string");
                Some(serde_json::Value::String(format!("{:?}", value)))
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
    fn test_env_var_substitution_missing_no_default_legacy_mode() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_key".to_string(),
            serde_json::Value::String("${NONEXISTENT_VAR}".to_string()),
        );

        // Legacy mode should return empty string for missing vars
        ctx.substitute_env_vars().unwrap();
        assert_eq!(
            ctx.get("config_key"),
            Some(&serde_json::Value::String("".to_string()))
        );
    }

    #[test]
    fn test_env_var_substitution_missing_no_default_strict_mode() {
        let mut ctx = TemplateContext::new();
        ctx.set(
            "config_key".to_string(),
            serde_json::Value::String("${NONEXISTENT_VAR}".to_string()),
        );

        // Strict mode should return error for missing vars
        let result = ctx.substitute_env_vars_strict();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("NONEXISTENT_VAR"));
    }

    #[test]
    fn test_env_var_substitution_in_nested_structures() {
        std::env::set_var("NESTED_TEST", "nested_value");

        let mut ctx = TemplateContext::new();
        let nested_obj = serde_json::json!({
            "inner": "${NESTED_TEST}",
            "array": ["${NESTED_TEST}", "static", "${MISSING_VAR}"]
        });
        ctx.set("nested".to_string(), nested_obj);

        ctx.substitute_env_vars().unwrap();

        let expected = serde_json::json!({
            "inner": "nested_value",
            "array": ["nested_value", "static", ""] // empty string for missing var in legacy mode
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
        assert_eq!(
            ctx.get_string("string_val"),
            Some("test_string".to_string())
        );
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
        new_vars.insert("new_key2".to_string(), serde_json::Value::Number(42.into()));

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
        ctx.set("var3", "${MISSING_VAR}"); // Should become empty string in legacy mode

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
        assert_eq!(
            new_ctx.get("var3"),
            Some(&serde_json::Value::String("".to_string()))
        ); // empty string for missing var

        std::env::remove_var("IMMUTABLE_TEST");
    }

    #[test]
    fn test_template_context_with_env_substitution_strict() {
        std::env::set_var("STRICT_TEST", "strict_value");

        let mut ctx = TemplateContext::new();
        ctx.set("var1", "${STRICT_TEST}");
        ctx.set("var2", "unchanged");
        ctx.set("var3", "${MISSING_VAR}"); // Should cause error in strict mode

        // This should fail because MISSING_VAR is not set
        let result = ctx.with_env_substitution_strict();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("MISSING_VAR"));

        // But without the missing var, it should work
        ctx.vars_mut().remove("var3");
        let new_ctx = ctx.with_env_substitution_strict().unwrap();

        // New context should have substitution
        assert_eq!(
            new_ctx.get("var1"),
            Some(&serde_json::Value::String("strict_value".to_string()))
        );
        assert_eq!(
            new_ctx.get("var2"),
            Some(&serde_json::Value::String("unchanged".to_string()))
        );

        std::env::remove_var("STRICT_TEST");
    }

    #[test]
    fn test_substitute_var_selective() {
        std::env::set_var("SELECTIVE_TEST", "selective_value");

        let mut ctx = TemplateContext::new();
        ctx.set("var1", "${SELECTIVE_TEST}");
        ctx.set("var2", "${SELECTIVE_TEST}");
        ctx.set("var3", "${MISSING_VAR}");

        // Only substitute var1 in legacy mode
        ctx.substitute_var("var1", false).unwrap();

        // Only substitute var2 in strict mode (should fail)
        let result = ctx.substitute_var("var3", true);
        assert!(result.is_err());

        // var1 should be substituted, others unchanged
        assert_eq!(
            ctx.get("var1"),
            Some(&serde_json::Value::String("selective_value".to_string()))
        );
        assert_eq!(
            ctx.get("var2"),
            Some(&serde_json::Value::String("${SELECTIVE_TEST}".to_string()))
        );
        assert_eq!(
            ctx.get("var3"),
            Some(&serde_json::Value::String("${MISSING_VAR}".to_string()))
        );

        // Nonexistent variable should not cause error
        ctx.substitute_var("nonexistent", false).unwrap();

        std::env::remove_var("SELECTIVE_TEST");
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
