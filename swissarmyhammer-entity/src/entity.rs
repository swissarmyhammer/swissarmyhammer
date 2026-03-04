//! Dynamic entity type backed by a field-value HashMap.
//!
//! An Entity represents any domain object as a bag of named fields. The
//! entity_type and id are metadata; actual data lives in the fields HashMap.

use std::collections::HashMap;

use serde_json::Value;
use swissarmyhammer_fields::EntityTypeName;

use crate::id_types::EntityId;

/// A dynamic, field-driven entity.
///
/// The `entity_type` identifies the kind (e.g. "task", "tag") and determines
/// which EntityDef schema applies. The `id` is a ULID or slug extracted from
/// the filename. All data fields live in the `fields` HashMap.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entity {
    pub entity_type: EntityTypeName,
    pub id: EntityId,
    pub fields: HashMap<String, Value>,
}

impl Default for Entity {
    fn default() -> Self {
        Self {
            entity_type: EntityTypeName::from(""),
            id: EntityId::from(""),
            fields: HashMap::new(),
        }
    }
}

impl Entity {
    /// Create a new entity with the given type and id.
    pub fn new(entity_type: impl Into<EntityTypeName>, id: impl Into<EntityId>) -> Self {
        Self {
            entity_type: entity_type.into(),
            id: id.into(),
            fields: HashMap::new(),
        }
    }

    /// Get a field value by name.
    pub fn get(&self, field: &str) -> Option<&Value> {
        self.fields.get(field)
    }

    /// Get a field value as a string slice.
    pub fn get_str(&self, field: &str) -> Option<&str> {
        self.fields.get(field).and_then(|v| v.as_str())
    }

    /// Get a field value as i64.
    pub fn get_i64(&self, field: &str) -> Option<i64> {
        self.fields.get(field).and_then(|v| v.as_i64())
    }

    /// Get a field value as f64.
    pub fn get_f64(&self, field: &str) -> Option<f64> {
        self.fields.get(field).and_then(|v| v.as_f64())
    }

    /// Get a field value as bool.
    pub fn get_bool(&self, field: &str) -> Option<bool> {
        self.fields.get(field).and_then(|v| v.as_bool())
    }

    /// Get a field value as a list of strings (for reference arrays).
    pub fn get_string_list(&self, field: &str) -> Vec<String> {
        self.fields
            .get(field)
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Set a field value.
    pub fn set(&mut self, field: impl Into<String>, value: Value) {
        self.fields.insert(field.into(), value);
    }

    /// Remove a field, returning its previous value if any.
    pub fn remove(&mut self, field: &str) -> Option<Value> {
        self.fields.remove(field)
    }

    /// Serialize to a JSON Value with id and entity_type injected.
    ///
    /// The `id` and `entity_type` keys are always set from the entity metadata,
    /// overriding any fields with the same names.
    pub fn to_json(&self) -> Value {
        let mut map = serde_json::Map::new();
        for (k, v) in &self.fields {
            map.insert(k.clone(), v.clone());
        }
        // Insert after fields so metadata always wins
        map.insert("id".into(), Value::String(self.id.to_string()));
        map.insert(
            "entity_type".into(),
            Value::String(self.entity_type.to_string()),
        );
        Value::Object(map)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_entity_has_empty_fields() {
        let e = Entity::new("task", "01ABC");
        assert_eq!(e.entity_type, "task");
        assert_eq!(e.id, "01ABC");
        assert!(e.fields.is_empty());
    }

    #[test]
    fn set_and_get() {
        let mut e = Entity::new("task", "01ABC");
        e.set("title", Value::String("Hello".into()));
        assert_eq!(e.get_str("title"), Some("Hello"));
        assert_eq!(e.get("missing"), None);
    }

    #[test]
    fn get_typed_accessors() {
        let mut e = Entity::new("task", "01ABC");
        e.set("count", serde_json::json!(42));
        e.set("ratio", serde_json::json!(0.75));
        e.set("done", serde_json::json!(true));
        e.set("tags", serde_json::json!(["bug", "urgent"]));

        assert_eq!(e.get_i64("count"), Some(42));
        assert_eq!(e.get_f64("ratio"), Some(0.75));
        assert_eq!(e.get_bool("done"), Some(true));
        assert_eq!(e.get_string_list("tags"), vec!["bug", "urgent"]);
        assert!(e.get_string_list("missing").is_empty());
    }

    #[test]
    fn remove_field() {
        let mut e = Entity::new("task", "01ABC");
        e.set("title", Value::String("Hello".into()));
        let removed = e.remove("title");
        assert_eq!(removed, Some(Value::String("Hello".into())));
        assert_eq!(e.get("title"), None);
        assert_eq!(e.remove("missing"), None);
    }

    #[test]
    fn to_json_includes_id_and_type() {
        let mut e = Entity::new("task", "01ABC");
        e.set("title", Value::String("Hello".into()));
        e.set("count", serde_json::json!(5));

        let json = e.to_json();
        assert_eq!(json["id"], "01ABC");
        assert_eq!(json["entity_type"], "task");
        assert_eq!(json["title"], "Hello");
        assert_eq!(json["count"], 5);
    }

    #[test]
    fn to_json_metadata_overrides_field_collision() {
        let mut e = Entity::new("task", "01ABC");
        // Intentionally set fields that collide with metadata keys
        e.set("id", Value::String("WRONG".into()));
        e.set("entity_type", Value::String("WRONG".into()));
        e.set("title", Value::String("Hello".into()));

        let json = e.to_json();
        // Metadata always wins over field values
        assert_eq!(json["id"], "01ABC");
        assert_eq!(json["entity_type"], "task");
        assert_eq!(json["title"], "Hello");
    }

    #[test]
    fn default_entity() {
        let e = Entity::default();
        assert_eq!(e.entity_type, "");
        assert_eq!(e.id, "");
        assert!(e.fields.is_empty());
    }

    #[test]
    fn partial_eq() {
        let mut a = Entity::new("task", "01ABC");
        a.set("title", Value::String("Hello".into()));
        let mut b = Entity::new("task", "01ABC");
        b.set("title", Value::String("Hello".into()));
        assert_eq!(a, b);

        b.set("title", Value::String("Different".into()));
        assert_ne!(a, b);
    }

    #[test]
    fn get_string_list_non_array_returns_empty() {
        let mut e = Entity::new("task", "01ABC");
        e.set("title", Value::String("not an array".into()));
        assert!(e.get_string_list("title").is_empty());
    }
}
