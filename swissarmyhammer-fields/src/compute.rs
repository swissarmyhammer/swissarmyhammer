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
    dyn Fn(
            &HashMap<String, serde_json::Value>,
        ) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>
        + Send
        + Sync,
>;

/// Read-only query interface for aggregate computed fields.
///
/// Takes an entity type string and returns a list of field maps.
/// This avoids depending on the entity crate — the caller constructs a closure
/// over `EntityContext::list()` at the call site.
pub type EntityQueryFn = Box<
    dyn Fn(&str) -> Pin<Box<dyn Future<Output = Vec<HashMap<String, serde_json::Value>>> + Send>>
        + Send
        + Sync,
>;

/// A derivation function that can query other entities.
///
/// Receives the entity's own fields plus a shared query function for reading
/// related entities. The `Arc` allows the async block to capture ownership.
/// Used for aggregate computed fields like board `percent_complete` that
/// depend on data from other entity types.
pub type AggregateFn = Box<
    dyn Fn(
            &HashMap<String, serde_json::Value>,
            std::sync::Arc<EntityQueryFn>,
        ) -> Pin<Box<dyn Future<Output = serde_json::Value> + Send>>
        + Send
        + Sync,
>;

/// Engine for computing derived field values.
///
/// Consumers register derivation functions by name. When a computed field
/// is encountered, the engine looks up the `derive` name and invokes it.
///
/// Two kinds of derivations are supported:
/// - **Simple**: receives only the entity's own fields (registered via `register`)
/// - **Aggregate**: receives the entity's fields plus a query function for
///   reading other entities (registered via `register_aggregate`)
pub struct ComputeEngine {
    derivations: HashMap<String, DeriveFn>,
    aggregations: HashMap<String, AggregateFn>,
}

impl ComputeEngine {
    /// Create a new empty compute engine.
    pub fn new() -> Self {
        Self {
            derivations: HashMap::new(),
            aggregations: HashMap::new(),
        }
    }

    /// Register a simple derivation function by name.
    pub fn register(&mut self, name: &str, f: DeriveFn) {
        self.derivations.insert(name.to_string(), f);
    }

    /// Register an aggregate derivation function by name.
    ///
    /// Aggregate derivations receive the entity's own fields plus a query
    /// function for reading other entity types. They require an `EntityQueryFn`
    /// to be passed to `derive_all`.
    pub fn register_aggregate(&mut self, name: &str, f: AggregateFn) {
        self.aggregations.insert(name.to_string(), f);
    }

    /// Derive the value of a single computed field.
    ///
    /// Returns `Ok(Null)` for non-computed fields.
    /// Returns `Err(ComputeError)` if the derive name is not registered.
    ///
    /// For aggregate derivations, `entity_query` must be `Some`. If it is
    /// `None` and the derive name is registered as an aggregate, an error
    /// is returned.
    pub async fn derive(
        &self,
        field: &FieldDef,
        entity_fields: &HashMap<String, serde_json::Value>,
        entity_query: Option<&std::sync::Arc<EntityQueryFn>>,
    ) -> Result<serde_json::Value> {
        match &field.type_ {
            FieldType::Computed { derive, .. } => {
                // Try simple derivation first
                if let Some(f) = self.derivations.get(derive.as_str()) {
                    return Ok(f(entity_fields).await);
                }
                // Try aggregate derivation
                if let Some(f) = self.aggregations.get(derive.as_str()) {
                    let query = entity_query.ok_or_else(|| FieldsError::ComputeError {
                        field: field.name.to_string(),
                        message: format!(
                            "aggregate derivation '{}' requires an entity query function",
                            derive
                        ),
                    })?;
                    return Ok(f(entity_fields, std::sync::Arc::clone(query)).await);
                }
                Err(FieldsError::ComputeError {
                    field: field.name.to_string(),
                    message: format!("unregistered derivation: {}", derive),
                })
            }
            _ => Ok(serde_json::Value::Null),
        }
    }

    /// Compute all computed fields on an entity, inserting results into `entity_fields`.
    ///
    /// # Parameters
    ///
    /// - `entity_query`: Optional shared query function for aggregate derivations.
    ///   Simple derivations work without it; aggregate derivations will error
    ///   if this is `None`.
    ///
    /// # Field ordering
    ///
    /// Fields are derived in the order they appear in `field_defs`. Each
    /// computed field's result is inserted into `entity_fields` immediately,
    /// so if computed field B reads from computed field A's output, A **must**
    /// appear before B in the slice.
    pub async fn derive_all(
        &self,
        entity_fields: &mut HashMap<String, serde_json::Value>,
        field_defs: &[FieldDef],
        entity_query: Option<&std::sync::Arc<EntityQueryFn>>,
    ) -> Result<()> {
        for field in field_defs {
            if matches!(&field.type_, FieldType::Computed { .. }) {
                let value = self.derive(field, entity_fields, entity_query).await?;
                entity_fields.insert(field.name.to_string(), value);
            }
        }
        Ok(())
    }

    /// Check whether a derivation name is registered (simple or aggregate).
    pub fn has(&self, name: &str) -> bool {
        self.derivations.contains_key(name) || self.aggregations.contains_key(name)
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
    use crate::id_types::FieldDefId;

    fn make_computed_field(name: &str, derive: &str) -> FieldDef {
        FieldDef {
            id: FieldDefId::new(),
            name: name.into(),
            description: None,
            type_: FieldType::Computed {
                derive: derive.to_string(),
                depends_on: vec![],
                entity: None,
                commit_display_names: false,
            },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        }
    }

    fn make_text_field(name: &str) -> FieldDef {
        FieldDef {
            id: FieldDefId::new(),
            name: name.into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            validate: None,
        }
    }

    #[tokio::test]
    async fn register_and_derive() {
        let mut engine = ComputeEngine::new();
        engine.register(
            "double-title",
            Box::new(|fields| {
                let title = fields.get("title").and_then(|v| v.as_str()).unwrap_or("");
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

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result, serde_json::json!("HelloHello"));
    }

    #[tokio::test]
    async fn unregistered_derive_errors() {
        let engine = ComputeEngine::new();
        let field = make_computed_field("tags", "parse-body-tags");
        let fields = HashMap::new();

        let result = engine.derive(&field, &fields, None).await;
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

        let result = engine.derive(&field, &fields, None).await.unwrap();
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

        engine
            .derive_all(&mut fields, &field_defs, None)
            .await
            .unwrap();

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

        let result = engine.derive_all(&mut fields, &field_defs, None).await;
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

    #[tokio::test]
    async fn aggregate_derive_queries_entities() {
        use std::sync::Arc;

        let mut engine = ComputeEngine::new();
        // Register an aggregate derivation that counts entities of type "task"
        engine.register_aggregate(
            "count-tasks",
            Box::new(|_fields, query| {
                Box::pin(async move {
                    let tasks = query("task").await;
                    serde_json::json!(tasks.len())
                })
            }),
        );

        let field = make_computed_field("task_count", "count-tasks");
        let field_defs = vec![field];
        let mut fields = HashMap::new();

        // Provide a query fn that returns 3 fake tasks
        let query: Arc<EntityQueryFn> = Arc::new(Box::new(|entity_type: &str| {
            let entity_type = entity_type.to_string();
            Box::pin(async move {
                if entity_type == "task" {
                    vec![
                        HashMap::from([("title".to_string(), serde_json::json!("Task 1"))]),
                        HashMap::from([("title".to_string(), serde_json::json!("Task 2"))]),
                        HashMap::from([("title".to_string(), serde_json::json!("Task 3"))]),
                    ]
                } else {
                    vec![]
                }
            })
        }));

        engine
            .derive_all(&mut fields, &field_defs, Some(&query))
            .await
            .unwrap();

        assert_eq!(fields.get("task_count").unwrap(), &serde_json::json!(3));
    }

    #[tokio::test]
    async fn derive_all_with_no_query_still_works() {
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

        let field_defs = vec![make_computed_field("upper", "upper-title")];
        let mut fields = HashMap::new();
        fields.insert("title".to_string(), serde_json::json!("hello"));

        // No query fn — regular derives still work
        engine
            .derive_all(&mut fields, &field_defs, None)
            .await
            .unwrap();

        assert_eq!(fields.get("upper").unwrap(), &serde_json::json!("HELLO"));
    }

    #[tokio::test]
    async fn aggregate_without_query_fn_errors() {
        let mut engine = ComputeEngine::new();
        engine.register_aggregate(
            "count-tasks",
            Box::new(|_fields, query| {
                Box::pin(async move {
                    let tasks = query("task").await;
                    serde_json::json!(tasks.len())
                })
            }),
        );

        let field_defs = vec![make_computed_field("task_count", "count-tasks")];
        let mut fields = HashMap::new();

        // No query fn but aggregate derive needs one — should error
        let result = engine.derive_all(&mut fields, &field_defs, None).await;
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("aggregate"));
    }

    #[test]
    fn compute_engine_default_creates_empty() {
        let engine = ComputeEngine::default();
        assert!(!engine.has("anything"));
    }
}
