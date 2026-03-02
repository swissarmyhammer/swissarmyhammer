//! FieldsContext — main API surface for the fields registry.
//!
//! Manages field definitions and entity templates as YAML files under a
//! `fields/` directory. Provides in-memory indexes for fast lookup by
//! both name and ULID.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;
use ulid::Ulid;

use crate::error::{FieldsError, Result};
use crate::types::{EntityDef, FieldDef};

/// A collection of default field definitions and entity templates.
///
/// Consumers build this to pass to `FieldsContextBuilder::with_defaults()`.
/// On open, defaults that don't already exist on disk are written.
pub struct FieldDefaults {
    fields: Vec<FieldDef>,
    entities: Vec<EntityDef>,
}

impl FieldDefaults {
    pub fn new() -> Self {
        Self {
            fields: Vec::new(),
            entities: Vec::new(),
        }
    }

    /// Add a default field definition.
    pub fn field(mut self, def: FieldDef) -> Self {
        self.fields.push(def);
        self
    }

    /// Add a default entity template.
    pub fn entity(mut self, def: EntityDef) -> Self {
        self.entities.push(def);
        self
    }

    /// Access the field definitions.
    pub fn fields(&self) -> &[FieldDef] {
        &self.fields
    }

    /// Access the entity templates.
    pub fn entities(&self) -> &[EntityDef] {
        &self.entities
    }
}

impl Default for FieldDefaults {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for `FieldsContext`. Created by `FieldsContext::open()`.
pub struct FieldsContextBuilder {
    root: PathBuf,
    defaults: Option<FieldDefaults>,
}

impl FieldsContextBuilder {
    /// Provide default field definitions and entity templates.
    /// Defaults are seeded on first open; existing definitions are preserved.
    pub fn with_defaults(mut self, defaults: FieldDefaults) -> Self {
        self.defaults = Some(defaults);
        self
    }

    /// Build the context: create directories, seed defaults, load from disk.
    pub async fn build(self) -> Result<FieldsContext> {
        let root = self.root;

        // Create directory structure if missing
        let defs_dir = root.join("definitions");
        let entities_dir = root.join("entities");
        let lib_dir = root.join("lib");
        fs::create_dir_all(&defs_dir).await?;
        fs::create_dir_all(&entities_dir).await?;
        fs::create_dir_all(&lib_dir).await?;

        // Seed defaults before loading
        if let Some(defaults) = self.defaults {
            seed_defaults(&root, &defaults).await?;
        }

        let mut ctx = FieldsContext {
            root,
            fields: Vec::new(),
            entities: Vec::new(),
            name_index: HashMap::new(),
            id_index: HashMap::new(),
            entity_index: HashMap::new(),
        };

        ctx.load_definitions().await?;
        ctx.load_entities().await?;

        debug!(
            fields = ctx.fields.len(),
            entities = ctx.entities.len(),
            "fields context opened"
        );

        Ok(ctx)
    }
}

/// Seed default definitions that don't already exist on disk.
///
/// Fields are matched by ULID — if a file with that ULID exists (even if renamed),
/// the default is skipped. Entity templates are matched by name.
async fn seed_defaults(root: &Path, defaults: &FieldDefaults) -> Result<()> {
    let defs_dir = root.join("definitions");
    let entities_dir = root.join("entities");

    // Collect existing field ULIDs from disk
    let existing_ids = collect_existing_field_ids(&defs_dir).await?;

    // Seed field definitions (ULID-matched)
    for def in &defaults.fields {
        if !existing_ids.contains(&def.id) {
            let yaml = serde_yaml::to_string(def)?;
            let path = defs_dir.join(format!("{}.yaml", def.name));
            atomic_write(&path, yaml.as_bytes()).await?;
            debug!(name = %def.name, id = %def.id, "seeded default field");
        }
    }

    // Seed entity templates (name-matched)
    for def in &defaults.entities {
        let path = entities_dir.join(format!("{}.yaml", def.name));
        if !path.exists() {
            let yaml = serde_yaml::to_string(def)?;
            atomic_write(&path, yaml.as_bytes()).await?;
            debug!(name = %def.name, "seeded default entity template");
        }
    }

    Ok(())
}

/// Read all .yaml files in definitions/ and extract their ULIDs.
async fn collect_existing_field_ids(defs_dir: &Path) -> Result<Vec<Ulid>> {
    let mut ids = Vec::new();
    if !defs_dir.exists() {
        return Ok(ids);
    }
    let mut entries = fs::read_dir(defs_dir).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        if let Ok(content) = fs::read_to_string(&path).await {
            if let Ok(def) = serde_yaml::from_str::<FieldDef>(&content) {
                ids.push(def.id);
            }
        }
    }
    Ok(ids)
}

/// Context for field definitions and entity templates.
///
/// Owns a directory on disk with the structure:
/// ```text
/// fields/
///   definitions/    ← one .yaml per field
///   entities/       ← one .yaml per entity type
///   lib/            ← JS modules for validation
/// ```
pub struct FieldsContext {
    root: PathBuf,
    fields: Vec<FieldDef>,
    entities: Vec<EntityDef>,
    name_index: HashMap<String, usize>,
    id_index: HashMap<Ulid, usize>,
    entity_index: HashMap<String, usize>,
}

impl FieldsContext {
    /// Open or create a fields directory. Returns a builder for optional configuration.
    ///
    /// ```rust,ignore
    /// // Simple open:
    /// let ctx = FieldsContext::open(path).build().await?;
    ///
    /// // With defaults:
    /// let ctx = FieldsContext::open(path)
    ///     .with_defaults(my_defaults())
    ///     .build()
    ///     .await?;
    /// ```
    pub fn open(root: impl Into<PathBuf>) -> FieldsContextBuilder {
        FieldsContextBuilder {
            root: root.into(),
            defaults: None,
        }
    }

    // --- Field definitions ---

    /// Get a field definition by name.
    pub fn get_field_by_name(&self, name: &str) -> Option<&FieldDef> {
        self.name_index.get(name).map(|&i| &self.fields[i])
    }

    /// Get a field definition by ULID.
    pub fn get_field_by_id(&self, id: &Ulid) -> Option<&FieldDef> {
        self.id_index.get(id).map(|&i| &self.fields[i])
    }

    /// All field definitions.
    pub fn all_fields(&self) -> &[FieldDef] {
        &self.fields
    }

    /// Write (create or update) a field definition. Persists to YAML immediately.
    pub async fn write_field(&mut self, def: &FieldDef) -> Result<()> {
        let yaml = serde_yaml::to_string(def)?;
        let path = self.definition_path(&def.name);
        atomic_write(&path, yaml.as_bytes()).await?;

        // Update in-memory state
        if let Some(&idx) = self.id_index.get(&def.id) {
            // Existing field — might be renamed
            let old_name = self.fields[idx].name.clone();
            if old_name != def.name {
                self.name_index.remove(&old_name);
                // Remove old file if name changed
                let old_path = self.definition_path(&old_name);
                let _ = fs::remove_file(&old_path).await;
            }
            self.fields[idx] = def.clone();
            self.name_index.insert(def.name.clone(), idx);
        } else {
            // New field
            let idx = self.fields.len();
            self.fields.push(def.clone());
            self.name_index.insert(def.name.clone(), idx);
            self.id_index.insert(def.id, idx);
        }

        Ok(())
    }

    /// Delete a field definition by ULID.
    pub async fn delete_field(&mut self, id: &Ulid) -> Result<()> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| FieldsError::FieldNotFoundById { id: id.to_string() })?;

        let def = &self.fields[idx];
        let path = self.definition_path(&def.name);
        let _ = fs::remove_file(&path).await;

        let name = def.name.clone();
        self.name_index.remove(&name);
        self.id_index.remove(id);

        // Swap-remove and fix indexes
        self.fields.swap_remove(idx);
        if idx < self.fields.len() {
            let moved = &self.fields[idx];
            self.name_index.insert(moved.name.clone(), idx);
            self.id_index.insert(moved.id, idx);
        }

        Ok(())
    }

    // --- Entity templates ---

    /// Get an entity template by name.
    pub fn get_entity(&self, name: &str) -> Option<&EntityDef> {
        self.entity_index.get(name).map(|&i| &self.entities[i])
    }

    /// All entity templates.
    pub fn all_entities(&self) -> &[EntityDef] {
        &self.entities
    }

    /// Write (create or update) an entity template. Persists to YAML immediately.
    pub async fn write_entity(&mut self, def: &EntityDef) -> Result<()> {
        let yaml = serde_yaml::to_string(def)?;
        let path = self.entity_path(&def.name);
        atomic_write(&path, yaml.as_bytes()).await?;

        if let Some(&idx) = self.entity_index.get(&def.name) {
            self.entities[idx] = def.clone();
        } else {
            let idx = self.entities.len();
            self.entities.push(def.clone());
            self.entity_index.insert(def.name.clone(), idx);
        }

        Ok(())
    }

    // --- Lookup helpers ---

    /// Resolve field definitions for an entity template, in template order.
    pub fn fields_for_entity(&self, entity_name: &str) -> Vec<&FieldDef> {
        let Some(entity) = self.get_entity(entity_name) else {
            return Vec::new();
        };
        entity
            .fields
            .iter()
            .filter_map(|name| self.get_field_by_name(name))
            .collect()
    }

    /// Resolve a field name to its ULID.
    pub fn resolve_name_to_id(&self, name: &str) -> Option<Ulid> {
        self.get_field_by_name(name).map(|f| f.id)
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- Internal ---

    fn definition_path(&self, name: &str) -> PathBuf {
        self.root.join("definitions").join(format!("{name}.yaml"))
    }

    fn entity_path(&self, name: &str) -> PathBuf {
        self.root.join("entities").join(format!("{name}.yaml"))
    }

    async fn load_definitions(&mut self) -> Result<()> {
        let defs_dir = self.root.join("definitions");
        let mut entries = fs::read_dir(&defs_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).await?;
            match serde_yaml::from_str::<FieldDef>(&content) {
                Ok(def) => {
                    let idx = self.fields.len();
                    self.name_index.insert(def.name.clone(), idx);
                    self.id_index.insert(def.id, idx);
                    self.fields.push(def);
                }
                Err(e) => {
                    tracing::warn!(?path, %e, "skipping invalid field definition");
                }
            }
        }
        Ok(())
    }

    async fn load_entities(&mut self) -> Result<()> {
        let entities_dir = self.root.join("entities");
        let mut entries = fs::read_dir(&entities_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).await?;
            match serde_yaml::from_str::<EntityDef>(&content) {
                Ok(def) => {
                    let idx = self.entities.len();
                    self.entity_index.insert(def.name.clone(), idx);
                    self.entities.push(def);
                }
                Err(e) => {
                    tracing::warn!(?path, %e, "skipping invalid entity definition");
                }
            }
        }
        Ok(())
    }
}

/// Write to a temp file then rename for atomic persistence.
async fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent dir"))?;
    let tmp = dir.join(format!(".tmp_{}", Ulid::new()));
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Editor, EntityDef, FieldDef, FieldType, SelectOption};
    use tempfile::TempDir;

    fn make_test_field(name: &str) -> FieldDef {
        FieldDef {
            id: Ulid::new(),
            name: name.to_string(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: None,
            sort: None,
            filter: None,
            group: None,
            validate: None,
        }
    }

    fn sample_defaults() -> FieldDefaults {
        let status_id = Ulid::from_string("00000000000000000000000001").unwrap();
        let title_id = Ulid::from_string("00000000000000000000000002").unwrap();

        FieldDefaults::new()
            .field(FieldDef {
                id: status_id,
                name: "status".into(),
                description: Some("Current workflow state".into()),
                type_: FieldType::Select {
                    options: vec![SelectOption {
                        value: "Backlog".into(),
                        label: None,
                        color: Some("gray".into()),
                        icon: None,
                        order: 0,
                    }],
                },
                default: Some("Backlog".into()),
                editor: Some(Editor::Select),
                display: None,
                sort: None,
                filter: None,
                group: None,
                validate: None,
            })
            .field(FieldDef {
                id: title_id,
                name: "title".into(),
                description: None,
                type_: FieldType::Text { single_line: true },
                default: None,
                editor: Some(Editor::Markdown),
                display: None,
                sort: None,
                filter: None,
                group: None,
                validate: None,
            })
            .entity(EntityDef {
                name: "task".into(),
                body_field: Some("body".into()),
                fields: vec!["title".into(), "status".into()],
            })
    }

    // --- Basic context tests (updated for builder pattern) ---

    #[tokio::test]
    async fn open_creates_directories() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let _ctx = FieldsContext::open(&root).build().await.unwrap();
        assert!(root.join("definitions").is_dir());
        assert!(root.join("entities").is_dir());
        assert!(root.join("lib").is_dir());
    }

    #[tokio::test]
    async fn open_empty_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let ctx = FieldsContext::open(&root).build().await.unwrap();
        assert!(ctx.all_fields().is_empty());
        assert!(ctx.all_entities().is_empty());
    }

    #[tokio::test]
    async fn write_and_read_field() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let field = make_test_field("status");
        let id = field.id;
        ctx.write_field(&field).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
        assert_eq!(ctx.get_field_by_name("status").unwrap().id, id);
        assert_eq!(ctx.get_field_by_id(&id).unwrap().name, "status");
        assert_eq!(ctx.resolve_name_to_id("status"), Some(id));

        let path = root.join("definitions/status.yaml");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn write_field_update_preserves_id() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let mut field = make_test_field("status");
        let id = field.id;
        ctx.write_field(&field).await.unwrap();

        field.description = Some("Updated".into());
        ctx.write_field(&field).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
        assert_eq!(
            ctx.get_field_by_id(&id).unwrap().description,
            Some("Updated".into())
        );
    }

    #[tokio::test]
    async fn write_field_rename() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let mut field = make_test_field("status");
        let id = field.id;
        ctx.write_field(&field).await.unwrap();

        field.name = "state".into();
        ctx.write_field(&field).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
        assert!(ctx.get_field_by_name("status").is_none());
        assert_eq!(ctx.get_field_by_name("state").unwrap().id, id);
        assert!(!root.join("definitions/status.yaml").exists());
        assert!(root.join("definitions/state.yaml").exists());
    }

    #[tokio::test]
    async fn delete_field() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let field = make_test_field("status");
        let id = field.id;
        ctx.write_field(&field).await.unwrap();
        ctx.delete_field(&id).await.unwrap();

        assert!(ctx.all_fields().is_empty());
        assert!(ctx.get_field_by_name("status").is_none());
        assert!(ctx.get_field_by_id(&id).is_none());
        assert!(!root.join("definitions/status.yaml").exists());
    }

    #[tokio::test]
    async fn delete_nonexistent_field_errors() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let result = ctx.delete_field(&Ulid::new()).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn write_and_read_entity() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let entity = EntityDef {
            name: "task".into(),
            body_field: Some("body".into()),
            fields: vec!["title".into(), "status".into()],
        };
        ctx.write_entity(&entity).await.unwrap();

        assert_eq!(ctx.all_entities().len(), 1);
        let loaded = ctx.get_entity("task").unwrap();
        assert_eq!(loaded.body_field, Some("body".into()));
        assert_eq!(loaded.fields.len(), 2);
    }

    #[tokio::test]
    async fn fields_for_entity_resolves() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let f1 = make_test_field("title");
        let f2 = make_test_field("status");
        ctx.write_field(&f1).await.unwrap();
        ctx.write_field(&f2).await.unwrap();

        let entity = EntityDef {
            name: "task".into(),
            body_field: None,
            fields: vec!["title".into(), "status".into(), "missing".into()],
        };
        ctx.write_entity(&entity).await.unwrap();

        let resolved = ctx.fields_for_entity("task");
        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].name, "title");
        assert_eq!(resolved[1].name, "status");
    }

    #[tokio::test]
    async fn persistence_survives_reopen() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        {
            let mut ctx = FieldsContext::open(&root).build().await.unwrap();
            ctx.write_field(&make_test_field("title")).await.unwrap();
            ctx.write_field(&make_test_field("status")).await.unwrap();
            ctx.write_entity(&EntityDef {
                name: "task".into(),
                body_field: Some("body".into()),
                fields: vec!["title".into(), "status".into()],
            })
            .await
            .unwrap();
        }

        let ctx = FieldsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("title").is_some());
        assert!(ctx.get_field_by_name("status").is_some());
        assert_eq!(ctx.all_entities().len(), 1);
        assert!(ctx.get_entity("task").is_some());
    }

    #[tokio::test]
    async fn multiple_fields_index_correctly() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let fields: Vec<_> = ["title", "status", "priority", "due", "body"]
            .iter()
            .map(|n| FieldDef {
                id: Ulid::new(),
                name: n.to_string(),
                description: None,
                type_: FieldType::Text { single_line: true },
                default: None,
                editor: None,
                display: None,
                sort: None,
                filter: None,
                group: None,
                validate: None,
            })
            .collect();

        for f in &fields {
            ctx.write_field(f).await.unwrap();
        }

        assert_eq!(ctx.all_fields().len(), 5);
        for f in &fields {
            assert_eq!(ctx.get_field_by_name(&f.name).unwrap().id, f.id);
            assert_eq!(ctx.get_field_by_id(&f.id).unwrap().name, f.name);
        }
    }

    #[tokio::test]
    async fn delete_middle_field_fixes_indexes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let f1 = make_test_field("a");
        let f2 = make_test_field("b");
        let f3 = make_test_field("c");
        let id2 = f2.id;

        ctx.write_field(&f1).await.unwrap();
        ctx.write_field(&f2).await.unwrap();
        ctx.write_field(&f3).await.unwrap();

        ctx.delete_field(&id2).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("a").is_some());
        assert!(ctx.get_field_by_name("b").is_none());
        assert!(ctx.get_field_by_name("c").is_some());
    }

    // --- with_defaults() seeding tests ---

    #[tokio::test]
    async fn first_open_seeds_all_defaults() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        let ctx = FieldsContext::open(&root)
            .with_defaults(sample_defaults())
            .build()
            .await
            .unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("status").is_some());
        assert!(ctx.get_field_by_name("title").is_some());
        assert_eq!(ctx.all_entities().len(), 1);
        assert!(ctx.get_entity("task").is_some());

        // Files exist on disk
        assert!(root.join("definitions/status.yaml").exists());
        assert!(root.join("definitions/title.yaml").exists());
        assert!(root.join("entities/task.yaml").exists());
    }

    #[tokio::test]
    async fn subsequent_open_skips_existing() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        // First open seeds
        let _ctx = FieldsContext::open(&root)
            .with_defaults(sample_defaults())
            .build()
            .await
            .unwrap();

        // Second open with same defaults — should not duplicate
        let ctx = FieldsContext::open(&root)
            .with_defaults(sample_defaults())
            .build()
            .await
            .unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert_eq!(ctx.all_entities().len(), 1);
    }

    #[tokio::test]
    async fn user_modified_definitions_preserved() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        let status_id = Ulid::from_string("00000000000000000000000001").unwrap();

        // First open seeds defaults
        let mut ctx = FieldsContext::open(&root)
            .with_defaults(sample_defaults())
            .build()
            .await
            .unwrap();

        // User renames "status" to "state"
        let mut status = ctx.get_field_by_id(&status_id).unwrap().clone();
        status.name = "state".into();
        ctx.write_field(&status).await.unwrap();
        drop(ctx);

        // Reopen with defaults — renamed field should NOT be overwritten
        let ctx = FieldsContext::open(&root)
            .with_defaults(sample_defaults())
            .build()
            .await
            .unwrap();

        assert!(ctx.get_field_by_name("state").is_some());
        assert!(ctx.get_field_by_name("status").is_none());
        assert_eq!(ctx.get_field_by_id(&status_id).unwrap().name, "state");
    }

    #[tokio::test]
    async fn new_defaults_added_on_reopen() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        // First open with just status
        let defaults_v1 = FieldDefaults::new().field(FieldDef {
            id: Ulid::from_string("00000000000000000000000001").unwrap(),
            name: "status".into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: None,
            display: None,
            sort: None,
            filter: None,
            group: None,
            validate: None,
        });

        let _ctx = FieldsContext::open(&root)
            .with_defaults(defaults_v1)
            .build()
            .await
            .unwrap();

        // Second open adds a new default field
        let defaults_v2 = FieldDefaults::new()
            .field(FieldDef {
                id: Ulid::from_string("00000000000000000000000001").unwrap(),
                name: "status".into(),
                description: None,
                type_: FieldType::Text { single_line: true },
                default: None,
                editor: None,
                display: None,
                sort: None,
                filter: None,
                group: None,
                validate: None,
            })
            .field(FieldDef {
                id: Ulid::from_string("00000000000000000000000003").unwrap(),
                name: "priority".into(),
                description: None,
                type_: FieldType::Text { single_line: true },
                default: None,
                editor: None,
                display: None,
                sort: None,
                filter: None,
                group: None,
                validate: None,
            });

        let ctx = FieldsContext::open(&root)
            .with_defaults(defaults_v2)
            .build()
            .await
            .unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("status").is_some());
        assert!(ctx.get_field_by_name("priority").is_some());
    }

    #[tokio::test]
    async fn open_without_defaults_works() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");

        let ctx = FieldsContext::open(&root).build().await.unwrap();
        assert!(ctx.all_fields().is_empty());
        assert!(ctx.all_entities().is_empty());
    }
}
