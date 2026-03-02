//! Built-in field definitions and entity templates for kanban.
//!
//! Builtin YAML files are embedded from `builtin/fields/` at compile time via
//! `include_dir!`. At runtime, these are merged with local overrides from
//! `.kanban/fields/` to produce the full field registry.
//!
//! `KanbanLookup` implements `EntityLookup` for kanban entity stores,
//! enabling reference field validation to prune dangling IDs.

use std::path::PathBuf;

use async_trait::async_trait;
use include_dir::{include_dir, Dir};
use swissarmyhammer_fields::EntityLookup;

use crate::context::KanbanContext;

/// Builtin field definition YAML files, embedded at compile time.
static BUILTIN_DEFINITIONS: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/builtin/fields/definitions");

/// Builtin entity definition YAML files, embedded at compile time.
static BUILTIN_ENTITIES: Dir =
    include_dir!("$CARGO_MANIFEST_DIR/builtin/fields/entities");

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

/// Entity lookup backed by kanban file storage.
///
/// Reads entities from the `.kanban/` directory structure. Each entity type
/// dispatches to the appropriate subdirectory (tasks/, tags/, actors/, etc.).
pub struct KanbanLookup {
    root: PathBuf,
}

impl KanbanLookup {
    /// Create a lookup for a kanban root directory.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Create a lookup from a KanbanContext.
    pub fn from_context(ctx: &KanbanContext) -> Self {
        Self {
            root: ctx.root().to_path_buf(),
        }
    }
}

#[async_trait]
impl EntityLookup for KanbanLookup {
    async fn get(&self, entity_type: &str, id: &str) -> Option<serde_json::Value> {
        let ctx = KanbanContext::new(&self.root);
        match entity_type {
            "task" => {
                let task_id = crate::types::TaskId::from_string(id);
                ctx.read_task(&task_id).await.ok().map(|t| {
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "tag" => {
                let tag_id = crate::types::TagId::from_string(id);
                ctx.read_tag(&tag_id).await.ok().map(|t| {
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "actor" => {
                let actor_id = crate::types::ActorId::from_string(id);
                ctx.read_actor(&actor_id).await.ok().map(|a| {
                    let mut v = serde_json::to_value(&a).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "column" => {
                let col_id = crate::types::ColumnId::from_string(id);
                ctx.read_column(&col_id).await.ok().map(|c| {
                    let mut v = serde_json::to_value(&c).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            "swimlane" => {
                let sl_id = crate::types::SwimlaneId::from_string(id);
                ctx.read_swimlane(&sl_id).await.ok().map(|s| {
                    let mut v = serde_json::to_value(&s).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id.to_string()));
                    }
                    v
                })
            }
            _ => None,
        }
    }

    async fn list(&self, entity_type: &str) -> Vec<serde_json::Value> {
        let ctx = KanbanContext::new(&self.root);
        match entity_type {
            "task" => ctx
                .read_all_tasks()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|t| {
                    let id = t.id.to_string();
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "tag" => ctx
                .read_all_tags()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|t| {
                    let id = t.id.to_string();
                    let mut v = serde_json::to_value(&t).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "actor" => ctx
                .read_all_actors()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|a| {
                    let id = a.id().to_string();
                    let mut v = serde_json::to_value(&a).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "column" => ctx
                .read_all_columns()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|c| {
                    let id = c.id.to_string();
                    let mut v = serde_json::to_value(&c).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            "swimlane" => ctx
                .read_all_swimlanes()
                .await
                .unwrap_or_default()
                .into_iter()
                .map(|s| {
                    let id = s.id.to_string();
                    let mut v = serde_json::to_value(&s).unwrap_or_default();
                    if let serde_json::Value::Object(ref mut map) = v {
                        map.insert("id".into(), serde_json::Value::String(id));
                    }
                    v
                })
                .collect(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use swissarmyhammer_fields::{EntityDef, FieldDef};

    #[test]
    fn builtin_field_definitions_load() {
        let defs = builtin_field_definitions();
        assert_eq!(defs.len(), 21, "expected 21 builtin field definitions");
    }

    #[test]
    fn builtin_entity_definitions_load() {
        let defs = builtin_entity_definitions();
        assert_eq!(defs.len(), 7, "expected 7 builtin entity definitions");
    }

    #[test]
    fn builtin_fields_parse_as_field_def() {
        for (name, yaml) in builtin_field_definitions() {
            let result: Result<FieldDef, _> = serde_yaml::from_str(yaml);
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
            let result: Result<EntityDef, _> = serde_yaml::from_str(yaml);
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
                let def: FieldDef = serde_yaml::from_str(yaml).unwrap();
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
                let def: FieldDef = serde_yaml::from_str(yaml).unwrap();
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
        let entity: EntityDef = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(entity.name, "task");
        assert_eq!(entity.body_field, Some("body".into()));
        assert!(entity.fields.contains(&"title".to_string()));
        assert!(entity.fields.contains(&"position_column".to_string()));
        assert!(entity.fields.contains(&"position_swimlane".to_string()));
        assert!(entity.fields.contains(&"position_ordinal".to_string()));
        assert!(entity.fields.contains(&"attachments".to_string()));
        assert!(entity.fields.contains(&"progress".to_string()));
    }

    #[test]
    fn builtin_board_entity_exists() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "board").unwrap();
        let entity: EntityDef = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(entity.name, "board");
        assert!(entity.fields.contains(&"name".to_string()));
        assert!(entity.fields.contains(&"description".to_string()));
    }

    #[test]
    fn builtin_attachment_entity_exists() {
        let defs = builtin_entity_definitions();
        let (_, yaml) = defs.iter().find(|(n, _)| *n == "attachment").unwrap();
        let entity: EntityDef = serde_yaml::from_str(yaml).unwrap();

        assert_eq!(entity.name, "attachment");
        assert!(entity.fields.contains(&"attachment_name".to_string()));
        assert!(entity.fields.contains(&"attachment_path".to_string()));
        assert!(entity.fields.contains(&"attachment_mime_type".to_string()));
        assert!(entity.fields.contains(&"attachment_size".to_string()));
        assert!(!entity.fields.contains(&"attachment_task".to_string()));
    }

    #[test]
    fn builtin_entity_fields_reference_existing_field_defs() {
        let field_defs = builtin_field_definitions();
        let field_names: Vec<String> = field_defs
            .iter()
            .map(|(_, yaml)| {
                let def: FieldDef = serde_yaml::from_str(yaml).unwrap();
                def.name
            })
            .collect();

        let entity_defs = builtin_entity_definitions();
        for (ename, eyaml) in &entity_defs {
            let entity: EntityDef = serde_yaml::from_str(eyaml).unwrap();
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

        assert_eq!(ctx.all_fields().len(), 21);
        assert_eq!(ctx.all_entities().len(), 7);
        assert!(ctx.get_field_by_name("title").is_some());
        assert!(ctx.get_entity("task").is_some());
        assert_eq!(ctx.fields_for_entity("task").len(), 11);
    }
}
