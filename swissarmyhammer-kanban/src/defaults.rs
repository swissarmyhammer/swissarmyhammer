//! Built-in field definitions and entity templates for kanban.
//!
//! Builtin YAML files are embedded from `builtin/fields/` at compile time via
//! `include_dir!`. At runtime, these are merged with local overrides from
//! `.kanban/fields/` to produce the full field registry.
//!
//! `KanbanLookup` implements `EntityLookup` for kanban entity stores,
//! enabling reference field validation to prune dangling IDs.

use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use include_dir::{include_dir, Dir};
use swissarmyhammer_entity::EntityContext;
use swissarmyhammer_fields::{ComputeEngine, EntityLookup, FieldsContext};

use crate::tag_parser;
use crate::task_helpers;

/// Builtin field definition YAML files, embedded at compile time.
///
/// Each builtin field uses a zero-padded sentinel ID (e.g. `00000000000000000000000001`)
/// that sorts before any real ULID. The last two characters encode the builtin field
/// code. See `builtin/fields/definitions/*.yaml` for the full set.
static BUILTIN_DEFINITIONS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/fields/definitions");

/// Builtin entity definition YAML files, embedded at compile time.
static BUILTIN_ENTITIES: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/fields/entities");

/// Builtin view definition YAML files, embedded at compile time.
static BUILTIN_VIEWS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/views");

/// Builtin actor entity YAML files, embedded at compile time.
static BUILTIN_ACTORS: Dir = include_dir!("$CARGO_MANIFEST_DIR/builtin/actors");

/// Load builtin field definitions as `(name, yaml_content)` pairs.
pub fn builtin_field_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_DEFINITIONS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin entity definitions as `(name, yaml_content)` pairs.
pub fn builtin_entity_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_ENTITIES
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin view definitions as `(name, yaml_content)` pairs.
pub fn builtin_view_definitions() -> Vec<(&'static str, &'static str)> {
    BUILTIN_VIEWS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Load builtin actor entity YAML as `(id, yaml_content)` pairs.
///
/// The file stem is the actor ID (e.g. `claude-code.yaml` → `"claude-code"`).
pub fn builtin_actor_entities() -> Vec<(&'static str, &'static str)> {
    BUILTIN_ACTORS
        .files()
        .filter_map(|file| {
            let name = file.path().file_stem()?.to_str()?;
            let content = file.contents_utf8()?;
            Some((name, content))
        })
        .collect()
}

/// Build a ComputeEngine with all kanban derivation functions registered.
pub fn kanban_compute_engine() -> ComputeEngine {
    let mut engine = ComputeEngine::new();

    // parse-body-tags: extract #tag patterns from the body field,
    // filtered to only include tags that actually exist as tag entities.
    engine.register_aggregate(
        "parse-body-tags",
        Box::new(|fields, query| {
            let body = fields
                .get("body")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            Box::pin(async move {
                let parsed = tag_parser::parse_tags(&body);
                let existing_tags = query("tag").await;
                let known: std::collections::HashSet<&str> = existing_tags
                    .iter()
                    .filter_map(|t| t.get("tag_name").and_then(|v| v.as_str()))
                    .collect();
                let filtered: Vec<serde_json::Value> = parsed
                    .into_iter()
                    .filter(|slug| known.contains(slug.as_str()))
                    .map(serde_json::Value::String)
                    .collect();
                serde_json::Value::Array(filtered)
            })
        }),
    );

    // parse-body-progress: parse GFM task lists from body
    engine.register(
        "parse-body-progress",
        Box::new(|fields| {
            let body = fields.get("body").and_then(|v| v.as_str()).unwrap_or("");
            let (total, completed) = task_helpers::parse_checklist_counts(body);
            let percent = if total > 0 {
                (completed as f64 / total as f64 * 100.0).round() as u32
            } else {
                0
            };
            let value = serde_json::json!({
                "total": total,
                "completed": completed,
                "percent": percent,
            });
            Box::pin(async move { value })
        }),
    );

    // board-percent-complete: aggregate — counts done tasks (terminal column) vs total
    engine.register_aggregate(
        "board-percent-complete",
        Box::new(|_fields, query| {
            Box::pin(async move {
                let columns = query("column").await;
                let tasks = query("task").await;

                // Terminal column is the one with the highest order
                let terminal_id = columns
                    .iter()
                    .max_by_key(|c| c.get("order").and_then(|v| v.as_u64()).unwrap_or(0))
                    .and_then(|c| c.get("id").and_then(|v| v.as_str()))
                    .unwrap_or("done");

                let total = tasks.len();
                let done = tasks
                    .iter()
                    .filter(|t| {
                        t.get("position_column")
                            .and_then(|v| v.as_str())
                            .unwrap_or("todo")
                            == terminal_id
                    })
                    .count();
                let percent = if total > 0 {
                    (done as f64 / total as f64 * 100.0).round() as u32
                } else {
                    0
                };

                serde_json::json!({
                    "done": done,
                    "total": total,
                    "percent": percent,
                })
            })
        }),
    );

    // compute-virtual-tags: stub — returns empty array.
    // Populated by the enrichment pipeline in a later card.
    engine.register(
        "compute-virtual-tags",
        Box::new(|_fields| Box::pin(async { serde_json::Value::Array(vec![]) })),
    );

    // compute-filter-tags: stub — returns empty array.
    // Will compute tags ∪ virtual_tags once the enrichment pipeline lands.
    engine.register(
        "compute-filter-tags",
        Box::new(|_fields| Box::pin(async { serde_json::Value::Array(vec![]) })),
    );

    engine
}

/// Entity types supported by kanban lookup.
const KNOWN_ENTITY_TYPES: &[&str] = &["task", "tag", "actor", "column", "swimlane"];

/// Entity lookup backed by kanban file storage.
///
/// Uses a bare `EntityContext` (no engines) to avoid circular dependency:
/// engines → lookup → engines. Validation lookups use raw I/O only.
pub struct KanbanLookup {
    root: PathBuf,
    fields: Arc<FieldsContext>,
}

impl KanbanLookup {
    /// Create a lookup from a root path and fields context.
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        Self {
            root: root.into(),
            fields,
        }
    }

    /// Build a bare EntityContext (no engines) for raw I/O.
    fn bare_entity_context(&self) -> EntityContext {
        EntityContext::new(&self.root, Arc::clone(&self.fields))
    }
}

#[async_trait]
impl EntityLookup for KanbanLookup {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value> {
        if !KNOWN_ENTITY_TYPES.contains(&entity_type) {
            return None;
        }
        let ectx = self.bare_entity_context();
        ectx.read(entity_type, id).await.ok().map(|e| e.to_json())
    }

    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value> {
        if !KNOWN_ENTITY_TYPES.contains(&entity_type) {
            return Vec::new();
        }
        let ectx = self.bare_entity_context();
        ectx.list(entity_type)
            .await
            .unwrap_or_default()
            .iter()
            .map(|e| e.to_json())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use swissarmyhammer_fields::{EntityDef, FieldDef, FieldName};

    #[test]
    fn builtin_view_definitions_load() {
        let defs = builtin_view_definitions();
        assert!(
            !defs.is_empty(),
            "expected at least 1 builtin view definition"
        );
    }

    #[test]
    fn builtin_views_parse_as_view_def() {
        for (name, yaml) in builtin_view_definitions() {
            let result: Result<swissarmyhammer_views::ViewDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse view '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_board_view_exists() {
        let defs = builtin_view_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "board").unwrap();
        let view: swissarmyhammer_views::ViewDef = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(view.name, "Board");
        assert_eq!(view.kind, swissarmyhammer_views::ViewKind::Board);
        assert!(view.entity_type.as_deref() == Some("task"));
        assert!(!view.card_fields.is_empty());
        assert!(!view.commands.is_empty());
    }

    #[test]
    fn builtin_field_definitions_load() {
        let defs = builtin_field_definitions();
        assert_eq!(defs.len(), 19, "expected 19 builtin field definitions");
    }

    #[test]
    fn builtin_entity_definitions_load() {
        let defs = builtin_entity_definitions();
        assert_eq!(defs.len(), 6, "expected 6 builtin entity definitions");
    }

    #[test]
    fn builtin_fields_parse_as_field_def() {
        for (name, yaml) in builtin_field_definitions() {
            let result: Result<FieldDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse field '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_entities_parse_as_entity_def() {
        for (name, yaml) in builtin_entity_definitions() {
            let result: Result<EntityDef, _> = serde_yaml_ng::from_str(yaml);
            assert!(
                result.is_ok(),
                "Failed to parse entity '{}': {}",
                name,
                result.unwrap_err()
            );
        }
    }

    #[test]
    fn builtin_field_names_are_unique() {
        let defs = builtin_field_definitions();
        let mut names: Vec<_> = defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.name
            })
            .collect();
        let orig_len = names.len();
        names.sort();
        names.dedup();
        assert_eq!(orig_len, names.len(), "duplicate field names in builtins");
    }

    #[test]
    fn builtin_field_ulids_are_unique() {
        let defs = builtin_field_definitions();
        let mut ids: Vec<_> = defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.id
            })
            .collect();
        let orig_len = ids.len();
        ids.sort();
        ids.dedup();
        assert_eq!(orig_len, ids.len(), "duplicate ULIDs in builtins");
    }

    #[test]
    fn builtin_task_entity_has_expected_fields() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "task").unwrap();
        let entity: EntityDef = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert_eq!(entity.mention_prefix, Some("^".to_string()));
        assert_eq!(entity.mention_display_field, Some("title".into()));
        assert!(entity.fields.iter().any(|f| f == "title"));
        assert!(entity.fields.iter().any(|f| f == "position_column"));
        assert!(entity.fields.iter().any(|f| f == "position_swimlane"));
        assert!(entity.fields.iter().any(|f| f == "position_ordinal"));
        assert!(entity.fields.iter().any(|f| f == "attachments"));
        assert!(entity.fields.iter().any(|f| f == "progress"));
    }

    #[test]
    fn builtin_board_entity_exists() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "board").unwrap();
        let entity: EntityDef = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(entity.name, "board");
        assert!(entity.fields.iter().any(|f| f == "name"));
        assert!(entity.fields.iter().any(|f| f == "description"));
    }

    #[test]
    fn builtin_entity_fields_reference_existing_field_defs() {
        let field_defs = builtin_field_definitions();
        let field_names: Vec<FieldName> = field_defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
                def.name
            })
            .collect();

        let entity_defs = builtin_entity_definitions();
        for (ename, eyaml) in &entity_defs {
            let entity: EntityDef = serde_yaml_ng::from_str(eyaml).unwrap();
            for field_ref in &entity.fields {
                assert!(
                    field_names.contains(field_ref),
                    "Entity '{}' references field '{}' which has no builtin definition",
                    ename,
                    field_ref
                );
            }
        }
    }

    #[test]
    fn from_yaml_sources_builds_valid_context() {
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();

        let ctx = swissarmyhammer_fields::FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();

        assert_eq!(ctx.all_fields().len(), 19);
        assert_eq!(ctx.all_entities().len(), 6);
        assert!(ctx.get_field_by_name("title").is_some());
        assert!(ctx.get_entity("task").is_some());
        assert_eq!(ctx.fields_for_entity("task").len(), 12);
    }

    #[test]
    fn builtin_attachment_field_round_trips_through_yaml() {
        let defs = builtin_field_definitions();
        let entities = builtin_entity_definitions();

        let ctx = swissarmyhammer_fields::FieldsContext::from_yaml_sources(
            std::path::PathBuf::from("/tmp/test"),
            &defs,
            &entities,
        )
        .unwrap();

        let field = ctx
            .get_field_by_name("attachments")
            .expect("builtin 'attachments' field should exist in FieldsContext");

        match &field.type_ {
            swissarmyhammer_fields::FieldType::Attachment {
                multiple,
                max_bytes,
            } => {
                assert!(multiple, "attachments field should have multiple: true");
                assert_eq!(
                    *max_bytes, 104_857_600,
                    "attachments max_bytes should be 100 MB"
                );
            }
            other => panic!("expected FieldType::Attachment, got {:?}", other),
        }
    }

    #[test]
    fn kanban_compute_engine_registers_all_derivations() {
        let engine = kanban_compute_engine();
        assert!(engine.has("parse-body-tags"));
        assert!(engine.has("parse-body-progress"));
    }

    /// Helper: build a query function that returns known tags.
    fn tag_query(
        tag_names: Vec<&'static str>,
    ) -> std::sync::Arc<swissarmyhammer_fields::EntityQueryFn> {
        std::sync::Arc::new(Box::new(move |entity_type: &str| {
            let names = tag_names.clone();
            let entity_type = entity_type.to_string();
            Box::pin(async move {
                if entity_type == "tag" {
                    names
                        .iter()
                        .map(|n| {
                            let mut m = HashMap::new();
                            m.insert(
                                "tag_name".to_string(),
                                serde_json::Value::String(n.to_string()),
                            );
                            m
                        })
                        .collect()
                } else {
                    Vec::new()
                }
            })
        }))
    }

    #[tokio::test]
    async fn parse_body_tags_derivation() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-tags".to_string(),
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
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Fix the #bug in #login module"),
        );

        let query = tag_query(vec!["bug", "login"]);
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        let tags: Vec<String> = serde_json::from_value(result).unwrap();
        assert_eq!(tags, vec!["bug", "login"]);
    }

    #[tokio::test]
    async fn parse_body_tags_filters_nonexistent() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "tags".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-tags".to_string(),
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
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Fix #bug, and #tag, not real #valid here"),
        );

        // "bug," and "tag," are parsed as-is (with comma) — neither matches "bug" or "valid".
        // Only #valid (followed by space) parses cleanly and matches the known tag.
        let query = tag_query(vec!["valid"]);
        let result = engine.derive(&field, &fields, Some(&query)).await.unwrap();
        let tags: Vec<String> = serde_json::from_value(result).unwrap();
        assert_eq!(tags, vec!["valid"]);
    }

    #[tokio::test]
    async fn parse_body_progress_derivation() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "progress".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-progress".to_string(),
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
        };

        let mut fields = HashMap::new();
        fields.insert(
            "body".to_string(),
            serde_json::json!("Tasks:\n- [x] First\n- [ ] Second\n- [x] Third\n- [ ] Fourth"),
        );

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result["total"], 4);
        assert_eq!(result["completed"], 2);
        assert_eq!(result["percent"], 50);
    }

    // parse_checklist_counts tests live in task_helpers module

    #[test]
    fn all_builtin_computed_fields_have_registered_derivations() {
        let engine = kanban_compute_engine();
        let defs = builtin_field_definitions();

        for (filename, yaml) in &defs {
            let field: swissarmyhammer_fields::FieldDef = serde_yaml_ng::from_str(yaml).unwrap();
            if let swissarmyhammer_fields::FieldType::Computed { derive, .. } = &field.type_ {
                assert!(
                    engine.has(derive),
                    "Builtin computed field '{}' (file: {}) references derive '{}' which is not registered in kanban_compute_engine()",
                    field.name, filename, derive
                );
            }
        }
    }

    #[tokio::test]
    async fn parse_body_progress_empty_body() {
        let engine = kanban_compute_engine();
        let field = swissarmyhammer_fields::FieldDef {
            id: swissarmyhammer_fields::FieldDefId::new(),
            name: "progress".into(),
            description: None,
            type_: swissarmyhammer_fields::FieldType::Computed {
                derive: "parse-body-progress".to_string(),
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
        };

        let fields = HashMap::new(); // No body field

        let result = engine.derive(&field, &fields, None).await.unwrap();
        assert_eq!(result["total"], 0);
        assert_eq!(result["completed"], 0);
        assert_eq!(result["percent"], 0);
    }
}
