//! Validation engine — runs JavaScript validation functions on field values.
//!
//! Validation runs on both read and write (clean in, clean out). Each field
//! definition can have a `validate` property containing a JS function body.
//! Reference fields get an automatic default validation that prunes dangling IDs.

use std::collections::HashMap;

use async_trait::async_trait;
use swissarmyhammer_js::JsState;

use crate::error::Result;
use crate::types::{FieldDef, FieldType};

/// Consumer-provided entity lookup for validation and computed fields.
///
/// Implementations dispatch on `entity_type` to query the right store.
/// `swissarmyhammer-fields` owns the validation engine and `ctx.lookup`
/// plumbing; the consumer provides the actual data access.
#[async_trait]
pub trait EntityLookup: Send + Sync {
    /// Get a single entity by type and ID. Returns `None` if not found.
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value>;

    /// List all entities of a given type.
    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value>;
}

/// Validation engine that runs JS validation functions via `swissarmyhammer-js`.
pub struct ValidationEngine {
    js: JsState,
    lookup: Option<Box<dyn EntityLookup>>,
}

impl ValidationEngine {
    /// Create a new validation engine using the global JS state.
    pub fn new() -> Self {
        Self {
            js: JsState::global(),
            lookup: None,
        }
    }

    /// Register an entity lookup provider for `ctx.lookup` in validation functions.
    pub fn with_lookup(mut self, lookup: impl EntityLookup + 'static) -> Self {
        self.lookup = Some(Box::new(lookup));
        self
    }

    /// Validate a field value. Returns the (possibly transformed) value.
    ///
    /// - If the field has an explicit `validate` JS function body, it runs that.
    /// - If the field is a reference type with no explicit validate, runs default
    ///   reference validation (prune dangling IDs).
    /// - Otherwise, passes the value through unchanged.
    pub async fn validate(
        &self,
        field: &FieldDef,
        value: serde_json::Value,
        sibling_fields: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        if let Some(ref validate_fn) = field.validate {
            self.run_js_validation(validate_fn, &field.name, value, sibling_fields)
                .await
        } else if let FieldType::Reference {
            ref entity,
            multiple,
        } = field.type_
        {
            self.default_reference_validation(entity, multiple, value)
                .await
        } else {
            Ok(value)
        }
    }

    /// Run a JS validation function body.
    ///
    /// The function body receives `ctx` with: value, fields, name, lookup.
    /// It should return the (possibly transformed) value, or throw to reject.
    async fn run_js_validation(
        &self,
        validate_fn: &str,
        field_name: &str,
        value: serde_json::Value,
        sibling_fields: &HashMap<String, serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let ctx_obj = serde_json::json!({
            "value": value,
            "fields": sibling_fields,
            "name": field_name,
        });

        // Build a self-executing function that:
        // 1. Creates the ctx object
        // 2. Runs the validation function body
        // 3. Returns the result (or throws)
        let js_code = format!(
            r#"(function() {{
    var ctx = {ctx_json};
    {validate_body}
}})()"#,
            ctx_json = serde_json::to_string(&ctx_obj).unwrap_or_default(),
            validate_body = validate_fn.trim(),
        );

        let result = self
            .js
            .get(&js_code)
            .await
            .map_err(|e| crate::error::FieldsError::ValidationFailed {
                field: field_name.to_string(),
                message: e,
            })?;

        Ok(result)
    }

    /// Default validation for reference fields: prune dangling IDs.
    ///
    /// IDs that don't resolve to an existing entity are silently removed.
    /// No error thrown — broken references are cleaned up, not rejected.
    async fn default_reference_validation(
        &self,
        entity_type: &str,
        multiple: bool,
        value: serde_json::Value,
    ) -> Result<serde_json::Value> {
        let Some(ref lookup) = self.lookup else {
            // No lookup provider — pass through unchanged
            return Ok(value);
        };

        if multiple {
            // Array of IDs
            let ids = match &value {
                serde_json::Value::Array(arr) => arr
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>(),
                serde_json::Value::Null => return Ok(serde_json::Value::Array(vec![])),
                _ => return Ok(value),
            };

            let mut valid = Vec::new();
            for id in &ids {
                if lookup.get(entity_type, id).await.is_some() {
                    valid.push(serde_json::Value::String(id.clone()));
                }
            }
            Ok(serde_json::Value::Array(valid))
        } else {
            // Single ID
            match value.as_str() {
                Some(id) => {
                    if lookup.get(entity_type, id).await.is_some() {
                        Ok(value)
                    } else {
                        Ok(serde_json::Value::Null)
                    }
                }
                None => Ok(value),
            }
        }
    }
}

impl Default for ValidationEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::FieldType;
    use ulid::Ulid;

    /// Test lookup that stores entities in memory
    struct MockLookup {
        entities: HashMap<String, Vec<serde_json::Value>>,
    }

    impl MockLookup {
        fn new() -> Self {
            Self {
                entities: HashMap::new(),
            }
        }

        fn with_entities(mut self, entity_type: &str, entities: Vec<serde_json::Value>) -> Self {
            self.entities.insert(entity_type.to_string(), entities);
            self
        }
    }

    #[async_trait]
    impl EntityLookup for MockLookup {
        async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value> {
            self.entities.get(entity_type).and_then(|list| {
                list.iter()
                    .find(|e| e.get("id").and_then(|v| v.as_str()) == Some(id))
                    .cloned()
            })
        }

        async fn list(&self, entity_type: &str) -> Vec<serde_json::Value> {
            self.entities
                .get(entity_type)
                .cloned()
                .unwrap_or_default()
        }
    }

    fn make_field(name: &str, type_: FieldType) -> FieldDef {
        FieldDef {
            id: Ulid::new(),
            name: name.to_string(),
            description: None,
            type_,
            default: None,
            editor: None,
            display: None,
            sort: None,
            filter: None,
            group: None,
            validate: None,
        }
    }

    #[tokio::test]
    async fn no_validate_passes_through() {
        let engine = ValidationEngine::new();
        let field = make_field("title", FieldType::Text { single_line: true });
        let value = serde_json::json!("Hello World");
        let siblings = HashMap::new();

        let result = engine.validate(&field, value.clone(), &siblings).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), value);
    }

    #[tokio::test]
    async fn tag_name_validation() {
        let engine = ValidationEngine::new();
        let mut field = make_field("tag_name", FieldType::Text { single_line: true });
        field.validate = Some(
            r#"
            const { value } = ctx;
            let v = value.trim().replace(/ +/g, "_").replace(/\0/g, "");
            if (v.length === 0) throw new Error("tag_name cannot be empty");
            return v;
            "#
            .to_string(),
        );

        // Normal value — trims and replaces spaces
        let result = engine
            .validate(
                &field,
                serde_json::json!("  hello world  "),
                &HashMap::new(),
            )
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("hello_world"));

        // Empty value — throws
        let result = engine
            .validate(&field, serde_json::json!("   "), &HashMap::new())
            .await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn reference_validation_prunes_dangling_ids() {
        let lookup = MockLookup::new().with_entities(
            "task",
            vec![
                serde_json::json!({"id": "task_001", "title": "First"}),
                serde_json::json!({"id": "task_003", "title": "Third"}),
            ],
        );
        let engine = ValidationEngine::new().with_lookup(lookup);

        let field = make_field(
            "depends_on",
            FieldType::Reference {
                entity: "task".into(),
                multiple: true,
            },
        );

        // task_002 doesn't exist — should be pruned
        let value = serde_json::json!(["task_001", "task_002", "task_003"]);
        let result = engine.validate(&field, value, &HashMap::new()).await;
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            serde_json::json!(["task_001", "task_003"])
        );
    }

    #[tokio::test]
    async fn single_reference_validation() {
        let lookup = MockLookup::new().with_entities(
            "actor",
            vec![serde_json::json!({"id": "alice", "name": "Alice"})],
        );
        let engine = ValidationEngine::new().with_lookup(lookup);

        let field = make_field(
            "assignee",
            FieldType::Reference {
                entity: "actor".into(),
                multiple: false,
            },
        );

        // Existing entity — passes through
        let result = engine
            .validate(&field, serde_json::json!("alice"), &HashMap::new())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("alice"));

        // Non-existing entity — returns null
        let result = engine
            .validate(&field, serde_json::json!("bob"), &HashMap::new())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::Value::Null);
    }

    #[tokio::test]
    async fn reference_no_lookup_passes_through() {
        // No lookup configured — reference validation passes through
        let engine = ValidationEngine::new();
        let field = make_field(
            "depends_on",
            FieldType::Reference {
                entity: "task".into(),
                multiple: true,
            },
        );

        let value = serde_json::json!(["task_001", "task_002"]);
        let result = engine.validate(&field, value.clone(), &HashMap::new()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), value);
    }

    #[tokio::test]
    async fn validation_with_sibling_fields() {
        let engine = ValidationEngine::new();
        let mut field = make_field("priority", FieldType::Text { single_line: true });
        field.validate = Some(
            r#"
            const { value, fields } = ctx;
            if (fields.status === "Done" && value !== "P0") {
                return "P3";
            }
            return value;
            "#
            .to_string(),
        );

        let mut siblings = HashMap::new();
        siblings.insert("status".to_string(), serde_json::json!("Done"));

        let result = engine
            .validate(&field, serde_json::json!("P1"), &siblings)
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!("P3"));
    }

    #[tokio::test]
    async fn null_reference_array_returns_empty() {
        let lookup = MockLookup::new();
        let engine = ValidationEngine::new().with_lookup(lookup);

        let field = make_field(
            "depends_on",
            FieldType::Reference {
                entity: "task".into(),
                multiple: true,
            },
        );

        let result = engine
            .validate(&field, serde_json::Value::Null, &HashMap::new())
            .await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), serde_json::json!([]));
    }

    #[tokio::test]
    async fn explicit_validate_overrides_default_reference() {
        let engine = ValidationEngine::new();
        let mut field = make_field(
            "special_ref",
            FieldType::Reference {
                entity: "task".into(),
                multiple: true,
            },
        );
        // Explicit validate — runs instead of default reference validation
        field.validate = Some(
            r#"
            return ctx.value;
            "#
            .to_string(),
        );

        let value = serde_json::json!(["any", "ids", "pass"]);
        let result = engine.validate(&field, value.clone(), &HashMap::new()).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), value);
    }
}
