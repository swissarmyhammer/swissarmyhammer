//! Computed field derivation engine.
//!
//! Derivation functions are native Rust functions registered by name.
//! The consumer (e.g. kanban) registers its derivations at startup.
//! The engine looks up the `derive` name from a `Computed` field type
//! and invokes the matching function.

use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use crate::error::{FieldsError, Result};
use crate::types::{FieldDef, FieldType};

/// A native derivation function.
///
/// Receives the entity's field values as a HashMap and returns the derived value.
pub type DeriveFn = Box<
    dyn Fn(&HashMap<String, serde_json::Value>) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>
        + Send
        + Sync,
>;

/// Engine for computing derived field values.
///
/// Consumers register derivation functions by name. When a computed field
/// is encountered, the engine looks up the `derive` name and invokes it.
pub struct ComputeEngine {
    derivations: HashMap<String, DeriveFn>,
}

impl ComputeEngine {
    /// Create a new empty compute engine.
    pub fn new() -> Self {
        Self {
            derivations: HashMap::new(),
        }
    }

    /// Register a derivation function by name.
    pub fn register(&mut self, name: &str, f: DeriveFn) {
        self.derivations.insert(name.to_string(), f);
    }

    /// Derive the value of a single computed field.
    ///
    /// Returns `Ok(Null)` for non-computed fields.
    /// Returns `Err(ComputeError)` if the derive name is not registered.
    pub async fn derive(
        &self,
        field: &FieldDef,
        entity_fields: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        match &field.type_ {
            FieldType::Computed { derive } => {
                let f = self.derivations.get(derive.as_str()).ok_or_else(|| {
                    FieldsError::ComputeError {
                        field: field.name.clone(),
                        message: format!("unregistered derivation: {}", derive),
                    }
                })?;
                Ok(f(entity_fields).await)
            }
            _ => Ok(serde_json::Value::Null),
        }
    }

    /// Compute all computed fields on an entity, inserting results into entity_fields.
    pub async fn derive_all(
        &self,
        entity_fields: &mut HashMap<String, serde_json::Value>,
        field_defs: &[FieldDef],
    ) -> Result<()> {
        for field in field_defs {
            if matches!(&field.type_, FieldType::Computed { .. }) {
                let value = self.derive(field, entity_fields).await?;
                entity_fields.insert(field.name.clone(), value);
            }
        }
        Ok(())
    }

    /// Check whether a derivation name is registered.
    pub fn has(&self, name: &str) -> bool {
        self.derivations.contains_key(name)
    }
}

impl Default for ComputeEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ulid::Ulid;

    fn make_computed_field(name: &str, derive: &str) -> FieldDef {
        FieldDef {
            id: Ulid::new(),
            name: name.to_string(),
            description: None,
            type_: FieldType::Computed {
                derive: derive.to_string(),
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            validate: None,
        }
    }

    fn make_text_field(name: &str) -> FieldDef {
        FieldDef {
            id: Ulid::new(),
            name: name.to_string(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            validate: None,
        }
    }

    #[tokio::test]
    async fn register_and_derive() {
        let mut engine = ComputeEngine::new();
        engine.register(
            "double-title",
            Box::new(|fields| {
                let title = fields
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let doubled = format!("{}{}", title, title);
                Box::pin(async move { serde_json::Value::String(doubled) })
            }),
        );

        let field = make_computed_field("doubled", "double-title");
        let mut fields = HashMap::new();
        fields.insert(
            "title".to_string(),
            serde_json::Value::String("Hello".to_string()),
        );

        let result = engine.derive(&field, &fields).await.unwrap();
        assert_eq!(result, serde_json::json!("HelloHello"));
    }

    #[tokio::test]
    async fn unregistered_derive_errors() {
        let engine = ComputeEngine::new();
        let field = make_computed_field("tags", "parse-body-tags");
        let fields = HashMap::new();

        let result = engine.derive(&field, &fields).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unregistered derivation"));
        assert!(err.contains("parse-body-tags"));
    }

    #[tokio::test]
    async fn derive_non_computed_returns_null() {
        let engine = ComputeEngine::new();
        let field = make_text_field("title");
        let fields = HashMap::new();

        let result = engine.derive(&field, &fields).await.unwrap();
        assert_eq!(result, serde_json::Value::Null);
    }

    #[tokio::test]
    async fn derive_all_computes_all_computed_fields() {
        let mut engine = ComputeEngine::new();
        engine.register(
            "upper-title",
            Box::new(|fields| {
                let title = fields
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_uppercase();
                Box::pin(async move { serde_json::Value::String(title) })
            }),
        );
        engine.register(
            "title-len",
            Box::new(|fields| {
                let len = fields
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.len())
                    .unwrap_or(0);
                Box::pin(async move { serde_json::json!(len) })
            }),
        );

        let field_defs = vec![
            make_text_field("title"),
            make_computed_field("upper", "upper-title"),
            make_computed_field("length", "title-len"),
        ];

        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("hello"));

        engine.derive_all(&mut fields, &field_defs).await.unwrap();

        assert_eq!(fields.get("upper").unwrap(), &serde_json::json!("HELLO"));
        assert_eq!(fields.get("length").unwrap(), &serde_json::json!(5));
        // Original field unchanged
        assert_eq!(fields.get("title").unwrap(), &serde_json::json!("hello"));
    }

    #[tokio::test]
    async fn derive_all_errors_on_unregistered() {
        let engine = ComputeEngine::new();
        let field_defs = vec![make_computed_field("tags", "parse-body-tags")];
        let mut fields = HashMap::new();

        let result = engine.derive_all(&mut fields, &field_defs).await;
        assert!(result.is_err());
    }

    #[test]
    fn has_checks_registration() {
        let mut engine = ComputeEngine::new();
        assert!(!engine.has("test"));
        engine.register(
            "test",
            Box::new(|_| Box::pin(async { serde_json::Value::Null })),
        );
        assert!(engine.has("test"));
    }
}
