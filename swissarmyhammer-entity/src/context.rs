//! EntityContext — root-aware I/O coordinator for dynamic entities.
//!
//! Given a storage root and a FieldsContext, this handles all directory
//! resolution, file I/O, and changelog management. Consumers (like kanban)
//! create an EntityContext and delegate all entity I/O to it.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use swissarmyhammer_fields::{ComputeEngine, EntityDef, FieldType, FieldsContext, ValidationEngine};

use crate::changelog::{self, ChangeEntry, FieldChange};
use crate::entity::Entity;
use crate::error::{EntityError, Result};
use crate::io;

/// Root-aware I/O coordinator for dynamic entities.
///
/// Maps entity types to storage directories under a root path,
/// handles read/write/delete/list, and manages per-entity changelogs.
pub struct EntityContext {
    root: PathBuf,
    fields: Arc<FieldsContext>,
    validation: Option<Arc<ValidationEngine>>,
    compute: Option<Arc<ComputeEngine>>,
}

impl EntityContext {
    /// Create a new EntityContext.
    ///
    /// - `root`: the storage root (e.g. `.kanban/`)
    /// - `fields`: the field registry containing EntityDefs
    pub fn new(root: impl Into<PathBuf>, fields: Arc<FieldsContext>) -> Self {
        Self {
            root: root.into(),
            fields,
            validation: None,
            compute: None,
        }
    }

    /// Attach a validation engine. Enables field validation on write.
    pub fn with_validation(mut self, engine: Arc<ValidationEngine>) -> Self {
        self.validation = Some(engine);
        self
    }

    /// Attach a compute engine. Enables computed field derivation on read.
    pub fn with_compute(mut self, engine: Arc<ComputeEngine>) -> Self {
        self.compute = Some(engine);
        self
    }

    /// Get the storage root path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Get the FieldsContext.
    pub fn fields(&self) -> &FieldsContext {
        &self.fields
    }

    /// Look up the EntityDef for an entity type.
    pub fn entity_def(&self, entity_type: &str) -> Result<&EntityDef> {
        self.fields.get_entity(entity_type).ok_or_else(|| {
            EntityError::UnknownEntityType {
                entity_type: entity_type.into(),
            }
        })
    }

    /// Get the storage directory for an entity type.
    ///
    /// Maps entity type → `{root}/{type}s/` (e.g. "task" → "tasks/",
    /// "board" → "boards/").
    pub fn entity_dir(&self, entity_type: &str) -> PathBuf {
        self.root.join(format!("{}s", entity_type))
    }

    /// Get the file path for a specific entity.
    ///
    /// Includes the correct extension (.md or .yaml) based on the EntityDef.
    pub fn entity_path(&self, entity_type: &str, id: &str) -> Result<PathBuf> {
        let def = self.entity_def(entity_type)?;
        Ok(io::entity_file_path(&self.entity_dir(entity_type), id, def))
    }

    /// Get the changelog path for a specific entity.
    pub fn changelog_path(&self, entity_type: &str, id: &str) -> Result<PathBuf> {
        let path = self.entity_path(entity_type, id)?;
        Ok(path.with_extension("jsonl"))
    }

    /// Get the trash directory for an entity type.
    ///
    /// Maps entity type → `{root}/.trash/{type}s/` (e.g. "task" → ".trash/tasks/").
    pub fn trash_dir(&self, entity_type: &str) -> PathBuf {
        self.root.join(".trash").join(format!("{}s", entity_type))
    }

    /// Read a single entity by type and ID.
    ///
    /// If a `ComputeEngine` is attached, computed fields are derived after reading.
    pub async fn read(&self, entity_type: &str, id: &str) -> Result<Entity> {
        let def = self.entity_def(entity_type)?;
        let path = io::entity_file_path(&self.entity_dir(entity_type), id, def);
        let mut entity = io::read_entity(&path, entity_type, id, def).await?;
        self.apply_compute(entity_type, &mut entity).await?;
        Ok(entity)
    }

    /// Write an entity, automatically computing and logging field-level changes.
    ///
    /// If a `ValidationEngine` is attached, fields are validated/transformed
    /// before writing. Computed fields are stripped (they are derived on read).
    /// If a previous version exists, diffs against it and appends a changelog
    /// entry. On creation (no previous version), all fields are logged as `Set`.
    pub async fn write(&self, entity: &Entity) -> Result<()> {
        let def = self.entity_def(&entity.entity_type)?;
        let dir = self.entity_dir(&entity.entity_type);

        // Apply validation and strip computed fields
        let mut entity = entity.clone();
        let entity_type = entity.entity_type.clone();
        self.apply_validation(&entity_type, &mut entity).await?;

        let path = io::entity_file_path(&dir, &entity.id, def);

        // Read previous state for diffing (if it exists)
        let previous =
            io::read_entity(&path, &entity.entity_type, &entity.id, def)
                .await
                .ok();

        // Write the entity
        io::write_entity(&path, &entity, def).await?;

        // Compute and append changelog
        let changes = match &previous {
            Some(old) => changelog::diff_entities(old, &entity),
            None => {
                // Creation — all fields are Set
                let mut changes: Vec<_> = entity
                    .fields
                    .iter()
                    .map(|(k, v)| (k.clone(), FieldChange::Set { value: v.clone() }))
                    .collect();
                changes.sort_by(|a, b| a.0.cmp(&b.0));
                changes
            }
        };

        if !changes.is_empty() {
            let op = if previous.is_some() { "update" } else { "create" };
            let entry = ChangeEntry::new(op, changes);
            let log_path = path.with_extension("jsonl");
            changelog::append_changelog(&log_path, &entry).await?;
        }

        Ok(())
    }

    /// Delete an entity by type and ID.
    ///
    /// Logs a "delete" changelog entry with all fields as `Removed`,
    /// then moves the data file and changelog to the trash directory
    /// (`{root}/.trash/{type}s/`). The entity is no longer listed or
    /// readable, but its files are preserved for recovery.
    pub async fn delete(&self, entity_type: &str, id: &str) -> Result<()> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let path = io::entity_file_path(&dir, id, def);

        // Read current state to log deletion
        if let Ok(old) = io::read_entity(&path, entity_type, id, def).await {
            let mut changes: Vec<_> = old
                .fields
                .iter()
                .map(|(k, v)| {
                    (
                        k.clone(),
                        FieldChange::Removed {
                            old_value: v.clone(),
                        },
                    )
                })
                .collect();
            changes.sort_by(|a, b| a.0.cmp(&b.0));

            if !changes.is_empty() {
                let entry = ChangeEntry::new("delete", changes);
                let log_path = path.with_extension("jsonl");
                changelog::append_changelog(&log_path, &entry).await?;
            }
        }

        let trash = self.trash_dir(entity_type);
        io::trash_entity_files(&path, &trash).await?;
        Ok(())
    }

    /// List all entities of a given type.
    ///
    /// If a `ComputeEngine` is attached, computed fields are derived for each entity.
    pub async fn list(&self, entity_type: &str) -> Result<Vec<Entity>> {
        let def = self.entity_def(entity_type)?;
        let dir = self.entity_dir(entity_type);
        let mut entities = io::read_entity_dir(&dir, entity_type, def).await?;
        for entity in &mut entities {
            self.apply_compute(entity_type, entity).await?;
        }
        Ok(entities)
    }

    /// Read the changelog for an entity.
    pub async fn read_changelog(&self, entity_type: &str, id: &str) -> Result<Vec<ChangeEntry>> {
        let log_path = self.changelog_path(entity_type, id)?;
        changelog::read_changelog(&log_path).await
    }

    // =========================================================================
    // Internal: validation and computation
    // =========================================================================

    /// Validate fields on write and strip computed fields.
    ///
    /// For each field defined on the entity type:
    /// - Skip `Computed` fields (remove from entity — they are derived on read).
    /// - If a validation engine is present, validate and possibly transform the value.
    /// - If a field has a default and is missing from the entity, insert the default.
    async fn apply_validation(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        let field_defs = self.fields.fields_for_entity(entity_type);
        if field_defs.is_empty() {
            return Ok(());
        }

        // Strip computed fields — they must never be persisted.
        for fd in &field_defs {
            if matches!(&fd.type_, FieldType::Computed { .. }) {
                entity.fields.remove(&fd.name);
            }
        }

        // Apply defaults for missing fields
        for fd in &field_defs {
            if matches!(&fd.type_, FieldType::Computed { .. }) {
                continue;
            }
            if !entity.fields.contains_key(&fd.name) {
                if let Some(ref default) = fd.default {
                    entity.set(
                        fd.name.clone(),
                        serde_json::Value::String(default.clone()),
                    );
                }
            }
        }

        // Validate fields
        let Some(ref engine) = self.validation else {
            return Ok(());
        };

        // Collect field names to validate (avoid borrowing entity.fields while mutating)
        let names_to_validate: Vec<String> = field_defs
            .iter()
            .filter(|fd| !matches!(&fd.type_, FieldType::Computed { .. }))
            .filter(|fd| entity.fields.contains_key(&fd.name))
            .map(|fd| fd.name.clone())
            .collect();

        // Snapshot sibling fields once before the loop — validation functions
        // see a consistent view of the entity, not partially-validated state.
        let siblings = entity.fields.clone();

        for name in &names_to_validate {
            let fd = field_defs.iter().find(|f| &f.name == name).unwrap();
            let value = entity.fields.get(name).cloned().unwrap();
            let validated = engine
                .validate(fd, value, &siblings)
                .await
                .map_err(|e| EntityError::ValidationFailed {
                    field: name.clone(),
                    message: e.to_string(),
                })?;
            entity.set(name.clone(), validated);
        }

        Ok(())
    }

    /// Derive computed fields after reading.
    async fn apply_compute(&self, entity_type: &str, entity: &mut Entity) -> Result<()> {
        let Some(ref engine) = self.compute else {
            return Ok(());
        };
        let field_defs = self.fields.fields_for_entity(entity_type);
        let owned_defs: Vec<_> = field_defs.into_iter().cloned().collect();
        engine
            .derive_all(&mut entity.fields, &owned_defs)
            .await
            .map_err(|e| {
                // Extract field name from the inner FieldsError if available
                let (field, message) = match &e {
                    swissarmyhammer_fields::FieldsError::ComputeError { field, message } => {
                        (field.clone(), message.clone())
                    }
                    other => (String::new(), other.to_string()),
                };
                EntityError::ComputeError { field, message }
            })?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::sync::Arc;
    use tempfile::TempDir;

    fn test_fields_context() -> Arc<FieldsContext> {
        let defs = vec![
            (
                "tag_name",
                "id: 00000000000000000000000TAG\nname: tag_name\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "color",
                "id: 00000000000000000000000COL\nname: color\ntype:\n  kind: color\n",
            ),
            (
                "title",
                "id: 00000000000000000000000TTL\nname: title\ntype:\n  kind: text\n  single_line: true\n",
            ),
            (
                "body",
                "id: 00000000000000000000000BDY\nname: body\ntype:\n  kind: markdown\n",
            ),
        ];
        let entities = vec![
            ("tag", "name: tag\nfields:\n  - tag_name\n  - color\n"),
            ("task", "name: task\nbody_field: body\nfields:\n  - title\n  - body\n"),
        ];

        let dir = TempDir::new().unwrap();
        Arc::new(FieldsContext::from_yaml_sources(dir.path(), &defs, &entities).unwrap())
    }

    #[tokio::test]
    async fn entity_dir_pluralizes() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(ctx.entity_dir("task"), dir.path().join("tasks"));
        assert_eq!(ctx.entity_dir("tag"), dir.path().join("tags"));
        assert_eq!(ctx.entity_dir("board"), dir.path().join("boards"));
    }

    #[tokio::test]
    async fn entity_path_uses_correct_extension() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        // task has body_field → .md
        let p = ctx.entity_path("task", "01ABC").unwrap();
        assert_eq!(p, dir.path().join("tasks").join("01ABC.md"));

        // tag has no body_field → .yaml
        let p = ctx.entity_path("tag", "bug").unwrap();
        assert_eq!(p, dir.path().join("tags").join("bug.yaml"));
    }

    #[tokio::test]
    async fn unknown_entity_type_errors() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert!(ctx.entity_path("unicorn", "x").is_err());
        assert!(ctx.read("unicorn", "x").await.is_err());
    }

    #[tokio::test]
    async fn round_trip_plain_yaml() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        tag.set("color", json!("#ff0000"));

        ctx.write(&tag).await.unwrap();

        let loaded = ctx.read("tag", "bug").await.unwrap();
        assert_eq!(loaded.get_str("tag_name"), Some("Bug"));
        assert_eq!(loaded.get_str("color"), Some("#ff0000"));
    }

    #[tokio::test]
    async fn round_trip_with_body() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut task = Entity::new("task", "01ABC");
        task.set("title", json!("Fix bug"));
        task.set("body", json!("Details here.\n\n- [ ] Step 1"));

        ctx.write(&task).await.unwrap();

        let loaded = ctx.read("task", "01ABC").await.unwrap();
        assert_eq!(loaded.get_str("title"), Some("Fix bug"));
        assert!(loaded.get_str("body").unwrap().contains("Step 1"));
    }

    #[tokio::test]
    async fn list_entities() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut t1 = Entity::new("tag", "bug");
        t1.set("tag_name", json!("Bug"));
        let mut t2 = Entity::new("tag", "feature");
        t2.set("tag_name", json!("Feature"));

        ctx.write(&t1).await.unwrap();
        ctx.write(&t2).await.unwrap();

        let tags = ctx.list("tag").await.unwrap();
        assert_eq!(tags.len(), 2);
    }

    #[tokio::test]
    async fn delete_moves_to_trash() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        assert!(ctx.read("tag", "bug").await.is_ok());
        ctx.delete("tag", "bug").await.unwrap();

        // No longer readable from live storage
        assert!(ctx.read("tag", "bug").await.is_err());

        // Files moved to trash
        let trash_dir = dir.path().join(".trash").join("tags");
        assert!(trash_dir.join("bug.yaml").exists());
        assert!(trash_dir.join("bug.jsonl").exists());

        // Changelog in trash includes the delete entry
        let log_content = tokio::fs::read_to_string(trash_dir.join("bug.jsonl"))
            .await
            .unwrap();
        assert!(log_content.contains("\"delete\""));
    }

    #[tokio::test]
    async fn trash_dir_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        assert_eq!(ctx.trash_dir("tag"), dir.path().join(".trash").join("tags"));
        assert_eq!(
            ctx.trash_dir("task"),
            dir.path().join(".trash").join("tasks")
        );
    }

    #[tokio::test]
    async fn write_creates_changelog_on_create() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(log[0].op, "create");
        assert!(log[0]
            .changes
            .iter()
            .all(|(_, c)| matches!(c, FieldChange::Set { .. })));
    }

    #[tokio::test]
    async fn write_creates_changelog_on_update() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let mut tag = Entity::new("tag", "bug");
        tag.set("tag_name", json!("Bug"));
        ctx.write(&tag).await.unwrap();

        // Update
        tag.set("tag_name", json!("Bug Report"));
        tag.set("color", json!("#ff0000"));
        ctx.write(&tag).await.unwrap();

        let log = ctx.read_changelog("tag", "bug").await.unwrap();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].op, "create");
        assert_eq!(log[1].op, "update");
    }

    #[tokio::test]
    async fn changelog_path_correct() {
        let dir = TempDir::new().unwrap();
        let fields = test_fields_context();
        let ctx = EntityContext::new(dir.path(), fields.clone());

        let p = ctx.changelog_path("tag", "bug").unwrap();
        assert_eq!(p, dir.path().join("tags").join("bug.jsonl"));
    }
}
