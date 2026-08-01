//! [`TrackedStore`] implementation for entities.
//!
//! [`EntityTypeStore`] adapts a single entity type directory to the
//! [`TrackedStore`] trait from `swissarmyhammer-store`. It handles two on-disk
//! formats:
//!
//! - **MD+YAML** (when `entity_def.body_field` is `Some`): YAML frontmatter
//!   between two lines of exactly `---`, followed by a markdown body.
//! - **Plain YAML** (when `body_field` is `None`): a single YAML document.
//!
//! Computed fields are stripped on serialize and never written to disk.
//! Field keys are sorted alphabetically (via `BTreeMap`) for deterministic diffs.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;
use swissarmyhammer_fields::types::{EntityDef, FieldDef, FieldType};
use swissarmyhammer_store::{StoreError, TrackedStore};

use crate::entity::Entity;
use crate::frontmatter::split_frontmatter_body;
use crate::id_types::EntityId;

/// Convenience alias matching the store crate's Result type.
type StoreResult<T> = std::result::Result<T, StoreError>;

/// A [`TrackedStore`] for a single entity type directory.
///
/// Parameterized by an [`EntityDef`] (determines format: MD+YAML vs plain YAML)
/// and a list of [`FieldDef`]s (for computed field identification during
/// serialization).
#[derive(Debug)]
pub struct EntityTypeStore {
    root: PathBuf,
    entity_type_name: String,
    entity_def: Arc<EntityDef>,
    /// Pre-computed set of computed field names for O(1) lookup in
    /// [`is_computed`].
    computed_field_names: HashSet<String>,
}

impl EntityTypeStore {
    /// Create a new store for the given entity type directory.
    ///
    /// # Parameters
    ///
    /// - `root` -- the directory containing entity files for this type.
    /// - `entity_type_name` -- the entity type slug (e.g. "task", "column").
    /// - `entity_def` -- the entity definition (determines format).
    /// - `field_defs` -- field definitions for this entity type (used to
    ///   identify computed fields).
    pub fn new(
        root: impl Into<PathBuf>,
        entity_type_name: impl Into<String>,
        entity_def: Arc<EntityDef>,
        field_defs: Arc<Vec<FieldDef>>,
    ) -> Self {
        let computed_field_names = field_defs
            .iter()
            .filter(|fd| matches!(fd.type_, FieldType::Computed { .. }))
            .map(|fd| fd.name.as_str().to_string())
            .collect();
        Self {
            root: root.into(),
            entity_type_name: entity_type_name.into(),
            entity_def,
            computed_field_names,
        }
    }

    /// Check whether a field is computed (and should be stripped on serialize).
    ///
    /// Uses a pre-built `HashSet` for O(1) lookup instead of scanning all field
    /// definitions.
    fn is_computed(&self, field_name: &str) -> bool {
        self.computed_field_names.contains(field_name)
    }
}

impl swissarmyhammer_store::store::sealed::Sealed for EntityTypeStore {}

impl TrackedStore for EntityTypeStore {
    type Item = Entity;
    type ItemId = EntityId;

    fn root(&self) -> &Path {
        &self.root
    }

    fn item_id(&self, entity: &Entity) -> EntityId {
        EntityId::from(entity.id.as_str())
    }

    fn extension(&self) -> &str {
        if self.entity_def.body_field.is_some() {
            "md"
        } else {
            "yaml"
        }
    }

    fn store_name(&self) -> &str {
        &self.entity_type_name
    }

    /// Serialize an entity to its on-disk text representation.
    ///
    /// Computed fields are stripped. Field keys are sorted alphabetically for
    /// deterministic output. If `body_field` is set, produces YAML frontmatter
    /// + markdown body; otherwise produces plain YAML.
    ///
    /// Entity fields are always stored flat. Nested YAML objects are flattened
    /// on read and stay flat on write. This matches the existing io.rs behavior.
    ///
    /// Uses `BTreeMap` for deterministic alphabetical key ordering, producing
    /// clean text diffs. This differs from io.rs which uses `HashMap`
    /// (non-deterministic). The text difference is harmless -- YAML semantics
    /// are identical.
    ///
    /// The delimiter lines stay unambiguous because a three-hyphen run inside
    /// a field value is never emitted alone at column 0: it is written
    /// indented inside a block scalar, or escaped inside a quoted one. So
    /// [`split_frontmatter_body`] reads it back as frontmatter content rather
    /// than as the closing delimiter.
    fn serialize(&self, entity: &Entity) -> StoreResult<String> {
        if let Some(body_field) = &self.entity_def.body_field {
            let body = entity
                .get_str(body_field.as_str())
                .unwrap_or("")
                .to_string();

            // Build sorted frontmatter from all fields EXCEPT body and computed
            let mut frontmatter = BTreeMap::new();
            for (k, v) in &entity.fields {
                if k == body_field.as_str() {
                    continue;
                }
                if self.is_computed(k) {
                    continue;
                }
                frontmatter.insert(k.clone(), v.clone());
            }

            let frontmatter_yaml =
                serde_yaml_ng::to_string(&Value::Object(frontmatter.into_iter().collect()))?;

            Ok(format!("---\n{}---\n{}", frontmatter_yaml, body))
        } else {
            // Plain YAML -- all fields except computed
            let mut map = BTreeMap::new();
            for (k, v) in &entity.fields {
                if self.is_computed(k) {
                    continue;
                }
                map.insert(k.clone(), v.clone());
            }

            let yaml = serde_yaml_ng::to_string(&Value::Object(map.into_iter().collect()))?;
            Ok(yaml)
        }
    }

    /// Deserialize an entity from its on-disk text representation.
    ///
    /// The `id` comes from the filename, not from file contents. Nested YAML
    /// objects are flattened one level deep with underscore-separated keys.
    ///
    /// In MD+YAML format the frontmatter runs between line-anchored `---`
    /// delimiters (see [`split_frontmatter_body`]) and the body field takes
    /// every byte after the closing delimiter line, unchanged.
    fn deserialize(&self, id: &EntityId, text: &str) -> StoreResult<Entity> {
        if let Some(body_field) = &self.entity_def.body_field {
            let Some((frontmatter, body)) = split_frontmatter_body(text) else {
                return Err(StoreError::Deserialize("invalid frontmatter format".into()));
            };

            // No trim: the slice is exactly the frontmatter bytes, and
            // trimming its trailing newline would re-chomp a block scalar that
            // ends the mapping, silently dropping that field's trailing
            // newline.
            let yaml_map: serde_json::Map<String, Value> = serde_yaml_ng::from_str(frontmatter)
                .map_err(|e| StoreError::Deserialize(e.to_string()))?;

            let mut entity = Entity::new(self.entity_type_name.as_str(), id.as_str());
            for (k, v) in yaml_map {
                flatten_into(&mut entity, &k, v);
            }
            entity.set(body_field.as_str(), Value::String(body.to_string()));

            Ok(entity)
        } else {
            let yaml_map: serde_json::Map<String, Value> = serde_yaml_ng::from_str(text)
                .map_err(|e| StoreError::Deserialize(e.to_string()))?;

            let mut entity = Entity::new(self.entity_type_name.as_str(), id.as_str());
            for (k, v) in yaml_map {
                flatten_into(&mut entity, &k, v);
            }

            Ok(entity)
        }
    }
}

/// Flatten nested objects into underscore-separated keys.
///
/// If `key` maps to a JSON object, each sub-key is expanded to `key_subkey`.
/// Non-object values are inserted as-is. Only one level of nesting is flattened.
///
/// Entity fields are always stored flat. Nested YAML objects are flattened on
/// read and stay flat on write. This matches the existing io.rs behavior.
///
/// Duplicated from io.rs intentionally -- store.rs is designed to be
/// self-contained and eventually replace io.rs.
fn flatten_into(entity: &mut Entity, key: &str, value: Value) {
    if let Value::Object(map) = &value {
        for (sub_key, sub_value) in map {
            let flat_key = format!("{}_{}", key, sub_key);
            entity.set(flat_key, sub_value.clone());
        }
    } else {
        entity.set(key, value);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_fields::types::{FieldDef, FieldType};
    use swissarmyhammer_fields::{EntityTypeName, FieldDefId, FieldName};

    /// Build a minimal EntityDef for a plain YAML entity (no body field).
    fn plain_entity_def(name: &str) -> EntityDef {
        EntityDef {
            name: EntityTypeName::from(name),
            icon: None,
            body_field: None,
            fields: vec![],
            sections: vec![],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            mention_slug_field: None,
            search_display_field: None,
        }
    }

    /// Build a minimal EntityDef with a body field (MD+YAML format).
    fn body_entity_def(name: &str, body_field: &str) -> EntityDef {
        EntityDef {
            name: EntityTypeName::from(name),
            icon: None,
            body_field: Some(FieldName::from(body_field)),
            fields: vec![],
            sections: vec![],
            validate: None,
            mention_prefix: None,
            mention_display_field: None,
            mention_slug_field: None,
            search_display_field: None,
        }
    }

    /// Build a text FieldDef.
    fn text_field(name: &str) -> FieldDef {
        FieldDef {
            id: FieldDefId::from(name),
            name: FieldName::from(name),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            width: None,
            icon: None,
            section: None,
            placeholder: None,
            validate: None,
            groupable: None,
        }
    }

    /// Build a computed FieldDef.
    fn computed_field(name: &str) -> FieldDef {
        FieldDef {
            id: FieldDefId::from(name),
            name: FieldName::from(name),
            description: None,
            type_: FieldType::Computed {
                derive: "test-derive".into(),
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
            placeholder: None,
            validate: None,
            groupable: None,
        }
    }

    /// Build an EntityTypeStore for testing.
    fn make_store(entity_def: EntityDef, field_defs: Vec<FieldDef>) -> EntityTypeStore {
        let name = entity_def.name.as_str().to_string();
        EntityTypeStore::new(
            "/tmp/test",
            name,
            Arc::new(entity_def),
            Arc::new(field_defs),
        )
    }

    #[test]
    fn test_extension_yaml() {
        let store = make_store(plain_entity_def("column"), vec![]);
        assert_eq!(store.extension(), "yaml");
    }

    #[test]
    fn test_extension_md() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        assert_eq!(store.extension(), "md");
    }

    #[test]
    fn test_item_id() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let entity = Entity::new("column", "01ABC");
        assert_eq!(store.item_id(&entity), EntityId::from("01ABC"));
    }

    #[test]
    fn test_serialize_plain_yaml() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let mut entity = Entity::new("column", "01ABC");
        entity.set("name", Value::String("Backlog".into()));
        entity.set("order", serde_json::json!(0));

        let text = store.serialize(&entity).unwrap();
        let parsed: serde_json::Map<String, Value> = serde_yaml_ng::from_str(&text).unwrap();
        assert_eq!(parsed["name"], "Backlog");
        assert_eq!(parsed["order"], 0);
    }

    #[test]
    fn test_serialize_frontmatter_body() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Fix bug".into()));
        entity.set("body", Value::String("Some markdown content".into()));

        let text = store.serialize(&entity).unwrap();
        assert!(text.starts_with("---\n"));
        assert!(text.contains("---\nSome markdown content"));
        assert!(text.contains("title: Fix bug"));
        // Body should NOT appear in frontmatter
        assert!(!text.contains("body: "));
    }

    #[test]
    fn test_deserialize_plain_yaml() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let yaml = "name: Backlog\norder: 0\n";
        let id = EntityId::from("01ABC");

        let entity = store.deserialize(&id, yaml).unwrap();
        assert_eq!(entity.entity_type, "column");
        assert_eq!(entity.id, "01ABC");
        assert_eq!(entity.get_str("name"), Some("Backlog"));
        assert_eq!(entity.get_i64("order"), Some(0));
    }

    #[test]
    fn test_deserialize_frontmatter_body() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let text = "---\ntitle: Fix bug\nstatus: open\n---\nSome markdown content";
        let id = EntityId::from("01ABC");

        let entity = store.deserialize(&id, text).unwrap();
        assert_eq!(entity.entity_type, "task");
        assert_eq!(entity.id, "01ABC");
        assert_eq!(entity.get_str("title"), Some("Fix bug"));
        assert_eq!(entity.get_str("status"), Some("open"));
        assert_eq!(entity.get_str("body"), Some("Some markdown content"));
    }

    #[test]
    fn test_round_trip_plain_yaml() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let mut entity = Entity::new("column", "01ABC");
        entity.set("name", Value::String("Backlog".into()));
        entity.set("order", serde_json::json!(0));

        let text1 = store.serialize(&entity).unwrap();
        let id = EntityId::from("01ABC");
        let restored = store.deserialize(&id, &text1).unwrap();
        let text2 = store.serialize(&restored).unwrap();

        assert_eq!(text1, text2);
    }

    #[test]
    fn test_round_trip_frontmatter() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Fix bug".into()));
        entity.set("status", Value::String("open".into()));
        entity.set("body", Value::String("Some markdown content".into()));

        let text1 = store.serialize(&entity).unwrap();
        let id = EntityId::from("01ABC");
        let restored = store.deserialize(&id, &text1).unwrap();
        let text2 = store.serialize(&restored).unwrap();

        assert_eq!(text1, text2);
    }

    #[test]
    fn test_computed_fields_stripped() {
        let field_defs = vec![text_field("title"), computed_field("task_count")];
        let store = make_store(plain_entity_def("column"), field_defs);

        let mut entity = Entity::new("column", "01ABC");
        entity.set("title", Value::String("Backlog".into()));
        entity.set("task_count", serde_json::json!(5));

        let text = store.serialize(&entity).unwrap();
        // Computed field should be absent from output
        assert!(!text.contains("task_count"));
        // Regular field should be present
        assert!(text.contains("title: Backlog"));
    }

    #[test]
    fn test_nested_object_flattening() {
        let store = make_store(plain_entity_def("widget"), vec![]);
        let yaml = "name: test\nmetadata:\n  author: alice\n  version: 2\n";
        let id = EntityId::from("w01");

        let entity = store.deserialize(&id, yaml).unwrap();
        assert_eq!(entity.get_str("name"), Some("test"));
        assert_eq!(entity.get_str("metadata_author"), Some("alice"));
        assert_eq!(entity.get_i64("metadata_version"), Some(2));
        // The nested object itself should not be present as a single field
        assert!(entity.get("metadata").is_none());
    }

    #[test]
    fn test_deterministic_ordering() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let mut entity = Entity::new("column", "01ABC");
        entity.set("zebra", Value::String("last".into()));
        entity.set("alpha", Value::String("first".into()));
        entity.set("middle", Value::String("mid".into()));

        let text1 = store.serialize(&entity).unwrap();
        let text2 = store.serialize(&entity).unwrap();
        assert_eq!(text1, text2);

        // Verify alphabetical order: alpha before middle before zebra
        let alpha_pos = text1.find("alpha").unwrap();
        let middle_pos = text1.find("middle").unwrap();
        let zebra_pos = text1.find("zebra").unwrap();
        assert!(alpha_pos < middle_pos);
        assert!(middle_pos < zebra_pos);
    }

    #[test]
    fn test_deserialize_invalid_frontmatter_returns_error() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let text = "no frontmatter delimiters here";
        let id = EntityId::from("01ABC");

        let result = store.deserialize(&id, text);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.to_string().contains("invalid frontmatter"));
    }

    #[test]
    fn test_computed_fields_stripped_from_frontmatter() {
        let field_defs = vec![text_field("title"), computed_field("parsed_tags")];
        let store = make_store(body_entity_def("task", "body"), field_defs);

        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Fix bug".into()));
        entity.set("parsed_tags", serde_json::json!(["bug", "urgent"]));
        entity.set("body", Value::String("content".into()));

        let text = store.serialize(&entity).unwrap();
        assert!(!text.contains("parsed_tags"));
        assert!(text.contains("title: Fix bug"));
        assert!(text.contains("content"));
    }

    // -- NIT 2: body containing `---` horizontal rule --

    #[test]
    fn test_body_containing_horizontal_rule() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let text = "---\ntitle: Fix bug\n---\nBefore rule\n---\nAfter rule";
        let id = EntityId::from("01ABC");

        let entity = store.deserialize(&id, text).unwrap();
        assert_eq!(entity.get_str("title"), Some("Fix bug"));
        // The frontmatter ends at the FIRST delimiter line after the opening
        // one, so every later three-hyphen line stays in the body.
        assert_eq!(entity.get_str("body"), Some("Before rule\n---\nAfter rule"));
    }

    #[test]
    fn test_comment_carrying_a_delimiter_round_trips() {
        // The "Research:" paragraph of the 2026-07-31T18:59:02 comment on
        // ^tnr56gg, recovered from that task's changelog. It quotes a
        // backticked three-hyphen run, which is what corrupted the card.
        let comment = r#"Research: `parse_frontmatter_body` takes everything after `---\n`, `format_frontmatter_body` writes it back, so a body CAN carry a trailing newline or CRLF.

---

That rule line is prose, not a delimiter."#;

        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set(
            "title",
            Value::String("remove_tag rewrites every task body".into()),
        );
        entity.set("position_column", Value::String("doing".into()));
        entity.set("position_ordinal", Value::String("8480".into()));
        entity.set(
            "comments",
            serde_json::json!([{ "actor": "claude-code", "text": comment }]),
        );
        entity.set("body", Value::String("Card description.\n".into()));

        let text = store.serialize(&entity).unwrap();
        let parsed = store.deserialize(&EntityId::from("01ABC"), &text).unwrap();

        for (key, value) in &entity.fields {
            assert_eq!(
                parsed.get(key),
                Some(value),
                "field {key} did not survive the round trip\nserialized form:\n{text}"
            );
        }
    }

    #[test]
    fn test_a_frontmatter_value_keeps_its_trailing_newline() {
        // Only one frontmatter key, so its block scalar is the last thing
        // before the closing delimiter line.
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set(
            "comments",
            serde_json::json!([{ "actor": "claude-code", "text": "first line\nsecond line\n" }]),
        );
        entity.set("body", Value::String("Card description.\n".into()));

        let text = store.serialize(&entity).unwrap();
        let parsed = store.deserialize(&EntityId::from("01ABC"), &text).unwrap();

        assert_eq!(
            parsed.get("comments"),
            entity.get("comments"),
            "serialized form:\n{text}"
        );
    }

    #[test]
    fn test_empty_frontmatter_yields_an_entity_with_only_the_body() {
        let store = make_store(body_entity_def("task", "body"), vec![]);

        let entity = store
            .deserialize(&EntityId::from("01ABC"), "---\n---\nJust a body\n")
            .unwrap();

        assert_eq!(entity.get_str("body"), Some("Just a body\n"));
    }

    #[test]
    fn test_crlf_delimiter_lines_leave_the_body_byte_exact() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let text = "---\r\ntitle: CRLF card\r\n---\r\nFirst body line\r\nSecond body line\r\n";

        let entity = store.deserialize(&EntityId::from("01ABC"), text).unwrap();

        assert_eq!(entity.get_str("title"), Some("CRLF card"));
        assert_eq!(
            entity.get_str("body"),
            Some("First body line\r\nSecond body line\r\n")
        );
    }

    #[test]
    fn test_deserialize_without_a_closing_delimiter_is_an_error() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let text = "---\ntitle: Truncated\nposition_column: doing\n";

        let err = store
            .deserialize(&EntityId::from("01ABC"), text)
            .unwrap_err();

        assert!(
            matches!(err, StoreError::Deserialize(_)),
            "expected Deserialize error, got {err:?}"
        );
    }

    // -- NIT 5: empty entity serialization --

    #[test]
    fn test_serialize_empty_plain_yaml() {
        let store = make_store(plain_entity_def("column"), vec![]);
        let entity = Entity::new("column", "01ABC");

        let text = store.serialize(&entity).unwrap();
        // An entity with no fields should produce valid YAML (empty map).
        let parsed: serde_json::Map<String, Value> = serde_yaml_ng::from_str(&text).unwrap();
        assert!(parsed.is_empty());
    }

    #[test]
    fn test_serialize_body_only_frontmatter() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("body", Value::String("Just a body".into()));

        let text = store.serialize(&entity).unwrap();
        assert!(text.starts_with("---\n"));
        assert!(text.contains("Just a body"));
        // Frontmatter section should be empty (only body field, which is
        // excluded from frontmatter).
    }

    // -- NIT 6: Debug derive verification --

    #[test]
    fn test_entity_type_store_implements_debug() {
        let store = make_store(plain_entity_def("column"), vec![]);
        // Verify Debug is implemented by formatting.
        let debug_str = format!("{:?}", store);
        assert!(debug_str.contains("EntityTypeStore"));
    }

    // -- NIT 7: edge-case body content --

    #[test]
    fn test_body_empty_string() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Empty body".into()));
        entity.set("body", Value::String(String::new()));

        let text = store.serialize(&entity).unwrap();
        let id = EntityId::from("01ABC");
        let restored = store.deserialize(&id, &text).unwrap();
        assert_eq!(restored.get_str("body"), Some(""));
    }

    #[test]
    fn test_body_with_trailing_newline() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Trailing NL".into()));
        entity.set("body", Value::String("content\n".into()));

        let text = store.serialize(&entity).unwrap();
        let id = EntityId::from("01ABC");
        let restored = store.deserialize(&id, &text).unwrap();
        assert_eq!(restored.get_str("body"), Some("content\n"));
    }

    #[test]
    fn test_body_only_whitespace() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        let mut entity = Entity::new("task", "01ABC");
        entity.set("title", Value::String("Whitespace body".into()));
        entity.set("body", Value::String("   \n  \n".into()));

        let text = store.serialize(&entity).unwrap();
        let id = EntityId::from("01ABC");
        let restored = store.deserialize(&id, &text).unwrap();
        assert_eq!(restored.get_str("body"), Some("   \n  \n"));
    }

    #[test]
    fn test_malformed_yaml_in_frontmatter_returns_error() {
        let store = make_store(body_entity_def("task", "body"), vec![]);
        // Valid frontmatter delimiters but garbage YAML between them
        let text = "---\n: : [invalid yaml {{ not closed\n---\nSome body";
        let id = EntityId::from("01ABC");

        let result = store.deserialize(&id, text);
        assert!(result.is_err());
        match result.unwrap_err() {
            StoreError::Deserialize(msg) => {
                // Should contain a YAML parse error message
                assert!(!msg.is_empty());
            }
            other => panic!("expected StoreError::Deserialize, got {:?}", other),
        }
    }
}
