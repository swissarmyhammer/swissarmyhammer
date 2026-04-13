//! DeriveHandler trait and registry for computed field read/write.
//!
//! Computed fields have a `derive` rule (e.g. `"parse-body-tags"`) that defines
//! how to compute the value from the entity (read) and how to apply a desired
//! value back to the entity (write).
//!
//! Handlers are registered by derive rule name in a `DeriveRegistry`. The
//! consumer (e.g. kanban) registers its handlers at startup.

use std::collections::HashMap;

use crate::types::EntityDef;

/// Error type for derive handler operations.
#[derive(Debug, thiserror::Error)]
pub enum DeriveError {
    #[error("computed field is read-only")]
    ReadOnly,
    #[error("invalid value: {0}")]
    InvalidValue(String),
    #[error("{0}")]
    Other(String),
}

/// A handler for a computed field's derive rule.
///
/// Implements both the read path (compute the field value from entity fields)
/// and the write path (apply a desired value by mutating entity fields).
///
/// Handlers receive the entity's fields as a mutable HashMap and the entity
/// schema. They do NOT receive the Entity struct directly since this crate
/// does not depend on swissarmyhammer-entity.
pub trait DeriveHandler: Send + Sync {
    /// Compute the field value from the entity's stored fields (read path).
    fn compute(
        &self,
        fields: &HashMap<String, serde_json::Value>,
        schema: &EntityDef,
    ) -> serde_json::Value;

    /// Apply a desired value by mutating the entity's stored fields (write path).
    ///
    /// The handler receives the desired final state and makes it so — how it
    /// gets there (diffing, replacing, etc.) is an implementation detail.
    fn apply(
        &self,
        fields: &mut HashMap<String, serde_json::Value>,
        schema: &EntityDef,
        desired: &serde_json::Value,
    ) -> Result<(), DeriveError>;

    /// Whether this computed field supports writes. Default: true.
    fn writable(&self) -> bool {
        true
    }
}

/// Registry mapping derive rule names to handler implementations.
pub struct DeriveRegistry {
    handlers: HashMap<String, Box<dyn DeriveHandler>>,
}

impl DeriveRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler for a derive rule name.
    pub fn register(&mut self, name: impl Into<String>, handler: Box<dyn DeriveHandler>) {
        self.handlers.insert(name.into(), handler);
    }

    /// Look up a handler by derive rule name.
    pub fn get(&self, name: &str) -> Option<&dyn DeriveHandler> {
        self.handlers.get(name).map(|b| b.as_ref())
    }

    /// Check whether a handler is registered for a derive rule name.
    pub fn has(&self, name: &str) -> bool {
        self.handlers.contains_key(name)
    }
}

impl std::fmt::Debug for DeriveRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeriveRegistry")
            .field("handlers", &self.handlers.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Default for DeriveRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A trivial handler for testing: computes uppercase title, applies by lowercasing.
    struct UpperTitle;

    impl DeriveHandler for UpperTitle {
        fn compute(
            &self,
            fields: &HashMap<String, serde_json::Value>,
            _schema: &EntityDef,
        ) -> serde_json::Value {
            let title = fields.get("title").and_then(|v| v.as_str()).unwrap_or("");
            serde_json::Value::String(title.to_uppercase())
        }

        fn apply(
            &self,
            fields: &mut HashMap<String, serde_json::Value>,
            _schema: &EntityDef,
            desired: &serde_json::Value,
        ) -> Result<(), DeriveError> {
            let text = desired
                .as_str()
                .ok_or_else(|| DeriveError::InvalidValue("expected string".into()))?;
            fields.insert("title".to_string(), serde_json::json!(text.to_lowercase()));
            Ok(())
        }
    }

    /// A read-only handler for testing.
    struct ReadOnlyField;

    impl DeriveHandler for ReadOnlyField {
        fn compute(
            &self,
            _fields: &HashMap<String, serde_json::Value>,
            _schema: &EntityDef,
        ) -> serde_json::Value {
            serde_json::json!(42)
        }

        fn apply(
            &self,
            _fields: &mut HashMap<String, serde_json::Value>,
            _schema: &EntityDef,
            _desired: &serde_json::Value,
        ) -> Result<(), DeriveError> {
            Err(DeriveError::ReadOnly)
        }

        fn writable(&self) -> bool {
            false
        }
    }

    fn test_schema() -> EntityDef {
        EntityDef {
            name: "test".into(),
            icon: None,
            body_field: Some("body".into()),
            fields: vec!["title".into(), "body".into()],
            sections: vec![],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            mention_slug_field: None,
            search_display_field: None,
            commands: vec![],
        }
    }

    #[test]
    fn registry_lookup_returns_handler() {
        let mut registry = DeriveRegistry::new();
        registry.register("upper-title", Box::new(UpperTitle));

        assert!(registry.has("upper-title"));
        assert!(registry.get("upper-title").is_some());
    }

    #[test]
    fn registry_missing_handler_returns_none() {
        let registry = DeriveRegistry::new();

        assert!(!registry.has("nonexistent"));
        assert!(registry.get("nonexistent").is_none());
    }

    #[test]
    fn handler_compute() {
        let handler = UpperTitle;
        let schema = test_schema();
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("hello world"));

        let result = handler.compute(&fields, &schema);
        assert_eq!(result, serde_json::json!("HELLO WORLD"));
    }

    #[test]
    fn handler_apply() {
        let handler = UpperTitle;
        let schema = test_schema();
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("HELLO"));

        handler
            .apply(&mut fields, &schema, &serde_json::json!("GOODBYE"))
            .unwrap();
        assert_eq!(fields["title"], serde_json::json!("goodbye"));
    }

    #[test]
    fn handler_apply_invalid_value() {
        let handler = UpperTitle;
        let schema = test_schema();
        let mut fields = HashMap::new();

        let result = handler.apply(&mut fields, &schema, &serde_json::json!(123));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("expected string"));
    }

    #[test]
    fn read_only_handler() {
        let handler = ReadOnlyField;
        let schema = test_schema();
        let fields = HashMap::new();

        assert!(!handler.writable());
        assert_eq!(handler.compute(&fields, &schema), serde_json::json!(42));

        let mut fields_mut = HashMap::new();
        let result = handler.apply(&mut fields_mut, &schema, &serde_json::json!("anything"));
        assert!(matches!(result, Err(DeriveError::ReadOnly)));
    }

    #[test]
    fn registry_via_trait_object() {
        let mut registry = DeriveRegistry::new();
        registry.register("upper-title", Box::new(UpperTitle));
        registry.register("read-only", Box::new(ReadOnlyField));

        let schema = test_schema();
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("test"));

        // Compute via registry
        let handler = registry.get("upper-title").unwrap();
        assert_eq!(handler.compute(&fields, &schema), serde_json::json!("TEST"));

        // Apply via registry
        handler
            .apply(&mut fields, &schema, &serde_json::json!("NEW"))
            .unwrap();
        assert_eq!(fields["title"], serde_json::json!("new"));

        // Read-only via registry
        let ro = registry.get("read-only").unwrap();
        assert!(!ro.writable());
    }

    #[test]
    fn default_writable_returns_true_via_trait_default() {
        // UpperTitle does not override writable(), so it relies on the default.
        let handler = UpperTitle;
        assert!(
            handler.writable(),
            "DeriveHandler::writable() default should return true"
        );
    }

    #[test]
    fn registry_debug_shows_handler_names() {
        let mut registry = DeriveRegistry::new();
        registry.register("alpha", Box::new(UpperTitle));
        registry.register("beta", Box::new(ReadOnlyField));

        let debug_str = format!("{:?}", registry);
        assert!(debug_str.contains("DeriveRegistry"));
        assert!(debug_str.contains("alpha"));
        assert!(debug_str.contains("beta"));
    }

    #[test]
    fn registry_default_creates_empty() {
        let registry = DeriveRegistry::default();
        assert!(!registry.has("anything"));
        assert!(registry.get("anything").is_none());
    }
}
