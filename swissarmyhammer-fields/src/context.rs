//! FieldsContext -- main API surface for the fields registry.
//!
//! Manages field definitions and entity templates. Provides in-memory
//! indexes for fast lookup by both name and ID.
//!
//! Two ways to create a FieldsContext:
//!
//! 1. `from_yaml_sources()` -- from pre-loaded YAML content (VFS / embedded)
//! 2. `open().build()` -- from a directory on disk (for tests / standalone)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

use crate::error::{FieldsError, Result};
use crate::id_types::{EntityTypeName, FieldDefId, FieldName};
use crate::types::{EntityDef, FieldDef};

/// Builder for `FieldsContext`. Created by `FieldsContext::open()`.
pub struct FieldsContextBuilder {
    root: PathBuf,
}

impl FieldsContextBuilder {
    /// Build the context: create directories, load from disk.
    pub async fn build(self) -> Result<FieldsContext> {
        let root = self.root;

        // Create directory structure if missing
        let defs_dir = root.join("definitions");
        let entities_dir = root.join("entities");
        let lib_dir = root.join("lib");
        fs::create_dir_all(&defs_dir).await?;
        fs::create_dir_all(&entities_dir).await?;
        fs::create_dir_all(&lib_dir).await?;

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

/// Context for field definitions and entity templates.
///
/// Owns a writable directory on disk with the structure:
/// ```text
/// fields/
///   definitions/    <- one .yaml per field
///   entities/       <- one .yaml per entity type
///   lib/            <- JS modules for validation
/// ```
pub struct FieldsContext {
    root: PathBuf,
    fields: Vec<FieldDef>,
    entities: Vec<EntityDef>,
    name_index: HashMap<FieldName, usize>,
    id_index: HashMap<FieldDefId, usize>,
    entity_index: HashMap<EntityTypeName, usize>,
}

impl FieldsContext {
    /// Build context from pre-loaded YAML content.
    ///
    /// Each entry is `(name, yaml_content)`. Definitions and entities are
    /// provided separately. The writable_root is where modifications are persisted.
    ///
    /// ```rust,ignore
    /// let ctx = FieldsContext::from_yaml_sources(
    ///     root.join("fields"),
    ///     &[("title", title_yaml), ("body", body_yaml)],
    ///     &[("task", task_yaml)],
    /// )?;
    /// ```
    pub fn from_yaml_sources(
        writable_root: impl Into<PathBuf>,
        definitions: &[(&str, &str)],
        entities: &[(&str, &str)],
    ) -> Result<FieldsContext> {
        let root = writable_root.into();
        let mut ctx = FieldsContext {
            root,
            fields: Vec::new(),
            entities: Vec::new(),
            name_index: HashMap::new(),
            id_index: HashMap::new(),
            entity_index: HashMap::new(),
        };

        for (name, yaml) in definitions {
            match serde_yaml::from_str::<FieldDef>(yaml) {
                Ok(def) => {
                    let idx = ctx.fields.len();
                    // Later entries override earlier ones (same name)
                    if let Some(&old_idx) = ctx.name_index.get(&def.name) {
                        let old_id = ctx.fields[old_idx].id.clone();
                        ctx.id_index.remove(&old_id);
                        ctx.fields[old_idx] = def.clone();
                        ctx.id_index.insert(def.id.clone(), old_idx);
                    } else {
                        ctx.name_index.insert(def.name.clone(), idx);
                        ctx.id_index.insert(def.id.clone(), idx);
                        ctx.fields.push(def);
                    }
                }
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid field definition");
                }
            }
        }

        for (name, yaml) in entities {
            match serde_yaml::from_str::<EntityDef>(yaml) {
                Ok(def) => {
                    let idx = ctx.entities.len();
                    if let Some(&old_idx) = ctx.entity_index.get(&def.name) {
                        ctx.entities[old_idx] = def;
                    } else {
                        ctx.entity_index.insert(def.name.clone(), idx);
                        ctx.entities.push(def);
                    }
                }
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid entity definition");
                }
            }
        }

        debug!(
            fields = ctx.fields.len(),
            entities = ctx.entities.len(),
            "fields context built from YAML sources"
        );

        Ok(ctx)
    }

    /// Open or create a fields directory. Returns a builder.
    ///
    /// ```rust,ignore
    /// let ctx = FieldsContext::open(path).build().await?;
    /// ```
    pub fn open(root: impl Into<PathBuf>) -> FieldsContextBuilder {
        FieldsContextBuilder { root: root.into() }
    }

    // --- Field definitions ---

    /// Get a field definition by name.
    pub fn get_field_by_name(&self, name: &str) -> Option<&FieldDef> {
        self.name_index.get(name).map(|&i| &self.fields[i])
    }

    /// Get a field definition by ID.
    pub fn get_field_by_id(&self, id: impl AsRef<str>) -> Option<&FieldDef> {
        self.id_index.get(id.as_ref()).map(|&i| &self.fields[i])
    }

    /// All field definitions.
    pub fn all_fields(&self) -> &[FieldDef] {
        &self.fields
    }

    /// Write (create or update) a field definition. Persists to YAML immediately.
    pub async fn write_field(&mut self, def: &FieldDef) -> Result<()> {
        let yaml = serde_yaml::to_string(def)?;
        let path = self.definition_path(&def.name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        atomic_write(&path, yaml.as_bytes()).await?;

        // Update in-memory state
        if let Some(&idx) = self.id_index.get(&def.id) {
            // Existing field -- might be renamed
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
            self.id_index.insert(def.id.clone(), idx);
        }

        Ok(())
    }

    /// Delete a field definition by ID.
    pub async fn delete_field(&mut self, id: impl AsRef<str>) -> Result<()> {
        let id = id.as_ref();
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| FieldsError::FieldNotFoundById { id: id.to_string() })?;

        let def = &self.fields[idx];
        let path = self.definition_path(&def.name);
        let _ = fs::remove_file(&path).await;

        let name = def.name.clone();
        let field_id = def.id.clone();
        self.name_index.remove(&name);
        self.id_index.remove(&field_id);

        // Swap-remove and fix indexes
        self.fields.swap_remove(idx);
        if idx < self.fields.len() {
            let moved = &self.fields[idx];
            self.name_index.insert(moved.name.clone(), idx);
            self.id_index.insert(moved.id.clone(), idx);
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
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
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
            .filter_map(|name| self.get_field_by_name(name.as_str()))
            .collect()
    }

    /// Resolve a field name to its ID.
    pub fn resolve_name_to_id(&self, name: &str) -> Option<&FieldDefId> {
        self.get_field_by_name(name).map(|f| &f.id)
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- Internal ---

    fn definition_path(&self, name: impl AsRef<str>) -> PathBuf {
        let name = name.as_ref();
        self.root.join("definitions").join(format!("{name}.yaml"))
    }

    fn entity_path(&self, name: impl AsRef<str>) -> PathBuf {
        let name = name.as_ref();
        self.root.join("entities").join(format!("{name}.yaml"))
    }

    async fn load_definitions(&mut self) -> Result<()> {
        let defs_dir = self.root.join("definitions");
        if !defs_dir.exists() {
            return Ok(());
        }
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
                    self.id_index.insert(def.id.clone(), idx);
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
        if !entities_dir.exists() {
            return Ok(());
        }
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
    let tmp = dir.join(format!(".tmp_{}", FieldDefId::new()));
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

/// Load YAML files from a directory as `(name, content)` pairs.
///
/// Scans for `.yaml` files, returns the file stem as the name.
/// Silently skips non-existent directories.
///
/// # Why synchronous
///
/// This function intentionally uses `std::fs` (blocking I/O) rather than
/// `tokio::fs`. It is called during context construction via
/// `FieldsContext::from_yaml_sources`, which follows a synchronous builder
/// pattern. The target directories are small (a handful of YAML files), so
/// the blocking cost is negligible. Converting to async would require the
/// builder itself to be async, adding complexity for no practical benefit.
pub fn load_yaml_dir(dir: &Path) -> Vec<(String, String)> {
    let mut entries = Vec::new();
    if !dir.exists() {
        return entries;
    }
    let Ok(read_dir) = std::fs::read_dir(dir) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
            continue;
        }
        let name = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if let Ok(content) = std::fs::read_to_string(&path) {
            entries.push((name, content));
        }
    }
    entries
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id_types::FieldDefId;
    use crate::types::{Editor, EntityDef, FieldDef, FieldType};
    use tempfile::TempDir;

    fn make_test_field(name: &str) -> FieldDef {
        FieldDef {
            id: FieldDefId::new(),
            name: name.into(),
            description: None,
            type_: FieldType::Text { single_line: true },
            default: None,
            editor: Some(Editor::Markdown),
            display: None,
            sort: None,
            width: None,
            section: None,
            validate: None,
        }
    }

    // --- from_yaml_sources tests ---

    #[test]
    fn from_yaml_sources_parses_definitions() {
        let title_yaml = r#"
id: "00000000000000000000000001"
name: title
type:
  kind: text
  single_line: true
"#;
        let body_yaml = r#"
id: "00000000000000000000000002"
name: body
type:
  kind: markdown
  single_line: false
"#;
        let task_yaml = r#"
name: task
body_field: body
fields:
  - title
  - body
"#;

        let ctx = FieldsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("title", title_yaml), ("body", body_yaml)],
            &[("task", task_yaml)],
        )
        .unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("title").is_some());
        assert!(ctx.get_field_by_name("body").is_some());
        assert_eq!(ctx.all_entities().len(), 1);
        assert!(ctx.get_entity("task").is_some());
        assert_eq!(ctx.fields_for_entity("task").len(), 2);
    }

    #[test]
    fn from_yaml_sources_later_overrides_earlier() {
        let v1 = r#"
id: "00000000000000000000000001"
name: title
description: "Version 1"
type:
  kind: text
  single_line: true
"#;
        let v2 = r#"
id: "00000000000000000000000099"
name: title
description: "Version 2"
type:
  kind: text
  single_line: true
"#;

        let ctx = FieldsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("title", v1), ("title", v2)],
            &[],
        )
        .unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
        assert_eq!(
            ctx.get_field_by_name("title").unwrap().description,
            Some("Version 2".into())
        );
    }

    #[test]
    fn from_yaml_sources_skips_invalid_yaml() {
        let good = r#"
id: "00000000000000000000000001"
name: title
type:
  kind: text
  single_line: true
"#;
        let bad = "this is not valid yaml: [[[";

        let ctx = FieldsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("title", good), ("bad", bad)],
            &[],
        )
        .unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
    }

    // --- load_yaml_dir tests ---

    #[test]
    fn load_yaml_dir_reads_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("title.yaml"), "name: title").unwrap();
        std::fs::write(dir.join("body.yaml"), "name: body").unwrap();
        std::fs::write(dir.join("readme.md"), "# ignore me").unwrap();

        let entries = load_yaml_dir(dir);
        assert_eq!(entries.len(), 2);
        let names: Vec<&str> = entries.iter().map(|(n, _)| n.as_str()).collect();
        assert!(names.contains(&"title"));
        assert!(names.contains(&"body"));
    }

    #[test]
    fn load_yaml_dir_nonexistent_returns_empty() {
        let entries = load_yaml_dir(Path::new("/nonexistent/path"));
        assert!(entries.is_empty());
    }

    // --- Basic context tests ---

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
        let id = field.id.clone();
        ctx.write_field(&field).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 1);
        assert_eq!(ctx.get_field_by_name("status").unwrap().id, id);
        assert_eq!(ctx.get_field_by_id(&id).unwrap().name, "status");
        assert_eq!(ctx.resolve_name_to_id("status"), Some(&id));

        let path = root.join("definitions/status.yaml");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn write_field_update_preserves_id() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("fields");
        let mut ctx = FieldsContext::open(&root).build().await.unwrap();

        let mut field = make_test_field("status");
        let id = field.id.clone();
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
        let id = field.id.clone();
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
        let id = field.id.clone();
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

        let result = ctx.delete_field(&FieldDefId::new()).await;
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
            validate: None,
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
            validate: None,
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
                validate: None,
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
                id: FieldDefId::new(),
                name: FieldName::from(*n),
                description: None,
                type_: FieldType::Text { single_line: true },
                default: None,
                editor: None,
                display: None,
                sort: None,
                width: None,
                section: None,
                validate: None,
            })
            .collect();

        for f in &fields {
            ctx.write_field(f).await.unwrap();
        }

        assert_eq!(ctx.all_fields().len(), 5);
        for f in &fields {
            assert_eq!(ctx.get_field_by_name(f.name.as_str()).unwrap().id, f.id);
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
        let id2 = f2.id.clone();

        ctx.write_field(&f1).await.unwrap();
        ctx.write_field(&f2).await.unwrap();
        ctx.write_field(&f3).await.unwrap();

        ctx.delete_field(&id2).await.unwrap();

        assert_eq!(ctx.all_fields().len(), 2);
        assert!(ctx.get_field_by_name("a").is_some());
        assert!(ctx.get_field_by_name("b").is_none());
        assert!(ctx.get_field_by_name("c").is_some());
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
