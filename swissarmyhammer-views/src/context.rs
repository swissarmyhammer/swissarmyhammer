//! ViewsContext -- main API surface for the views registry.
//!
//! Manages view definitions with in-memory indexes for fast lookup by both
//! name and ID. Supports CRUD operations with disk persistence.
//!
//! Two ways to create a ViewsContext:
//!
//! 1. `from_yaml_sources()` -- from pre-loaded YAML content (VFS / embedded)
//! 2. `open().build()` -- from a directory on disk (for tests / standalone)

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use tokio::fs;
use tracing::debug;

use crate::error::{Result, ViewsError};
use crate::types::{ViewDef, ViewId};

/// Builder for `ViewsContext`. Created by `ViewsContext::open()`.
#[derive(Debug)]
pub struct ViewsContextBuilder {
    root: PathBuf,
}

impl ViewsContextBuilder {
    /// Build the context: create directory, load from disk.
    pub async fn build(self) -> Result<ViewsContext> {
        let root = self.root;
        fs::create_dir_all(&root).await?;

        let mut ctx = ViewsContext {
            root,
            views: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
        };

        ctx.load_views().await?;

        debug!(views = ctx.views.len(), "views context opened");
        Ok(ctx)
    }
}

/// Context for view definitions.
///
/// Owns a writable directory on disk:
/// ```text
/// views/
///   board.yaml
///   list.yaml
///   ...
/// ```
#[derive(Debug)]
pub struct ViewsContext {
    root: PathBuf,
    views: Vec<ViewDef>,
    id_index: HashMap<ViewId, usize>,
    name_index: HashMap<String, usize>,
}

impl ViewsContext {
    /// Build context from pre-loaded YAML content.
    ///
    /// Each entry is `(name, yaml_content)`. The writable_root is where
    /// modifications are persisted.
    pub fn from_yaml_sources(
        writable_root: impl Into<PathBuf>,
        sources: &[(&str, &str)],
    ) -> Result<ViewsContext> {
        let root = writable_root.into();
        let mut ctx = ViewsContext {
            root,
            views: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
        };

        for (name, yaml) in sources {
            match serde_yaml_ng::from_str::<ViewDef>(yaml) {
                Ok(def) => {
                    // Later entries override earlier ones (same id)
                    if let Some(&old_idx) = ctx.id_index.get(&def.id) {
                        let old_name = ctx.views[old_idx].name.clone();
                        ctx.name_index.remove(&old_name);
                        ctx.views[old_idx] = def.clone();
                        ctx.name_index.insert(def.name.clone(), old_idx);
                    } else {
                        let idx = ctx.views.len();
                        ctx.id_index.insert(def.id.clone(), idx);
                        ctx.name_index.insert(def.name.clone(), idx);
                        ctx.views.push(def);
                    }
                }
                Err(e) => {
                    tracing::warn!(name = %name, %e, "skipping invalid view definition");
                }
            }
        }

        debug!(
            views = ctx.views.len(),
            "views context built from YAML sources"
        );

        Ok(ctx)
    }

    /// Open or create a views directory. Returns a builder.
    pub fn open(root: impl Into<PathBuf>) -> ViewsContextBuilder {
        ViewsContextBuilder { root: root.into() }
    }

    // --- View lookups ---

    /// Get a view definition by ID.
    pub fn get_by_id(&self, id: &str) -> Option<&ViewDef> {
        self.id_index.get(id).map(|&i| &self.views[i])
    }

    /// Get a view definition by name.
    pub fn get_by_name(&self, name: &str) -> Option<&ViewDef> {
        self.name_index.get(name).map(|&i| &self.views[i])
    }

    /// All view definitions.
    pub fn all_views(&self) -> &[ViewDef] {
        &self.views
    }

    /// Write (create or update) a view definition. Persists to YAML immediately.
    pub async fn write_view(&mut self, def: &ViewDef) -> Result<()> {
        let yaml = serde_yaml_ng::to_string(def)?;
        let path = self.view_path(&def.id);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await?;
        }
        atomic_write(&path, yaml.as_bytes()).await?;

        // Update in-memory state
        if let Some(&idx) = self.id_index.get(&def.id) {
            let old_name = self.views[idx].name.clone();
            if old_name != def.name {
                self.name_index.remove(&old_name);
            }
            self.views[idx] = def.clone();
            self.name_index.insert(def.name.clone(), idx);
        } else {
            let idx = self.views.len();
            self.views.push(def.clone());
            self.id_index.insert(def.id.clone(), idx);
            self.name_index.insert(def.name.clone(), idx);
        }

        Ok(())
    }

    /// Delete a view definition by ID.
    pub async fn delete_view(&mut self, id: &str) -> Result<()> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| ViewsError::ViewNotFound { id: id.to_string() })?;

        let path = self.view_path(id);
        let _ = fs::remove_file(&path).await;

        let name = self.views[idx].name.clone();
        let view_id = self.views[idx].id.clone();
        self.name_index.remove(&name);
        self.id_index.remove(&view_id);

        // Swap-remove and fix indexes
        self.views.swap_remove(idx);
        if idx < self.views.len() {
            let moved = &self.views[idx];
            self.name_index.insert(moved.name.clone(), idx);
            self.id_index.insert(moved.id.clone(), idx);
        }

        Ok(())
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- Internal ---

    fn view_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.yaml"))
    }

    async fn load_views(&mut self) -> Result<()> {
        if !self.root.exists() {
            return Ok(());
        }
        let mut entries = fs::read_dir(&self.root).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let content = fs::read_to_string(&path).await?;
            match serde_yaml_ng::from_str::<ViewDef>(&content) {
                Ok(def) => {
                    let idx = self.views.len();
                    self.id_index.insert(def.id.clone(), idx);
                    self.name_index.insert(def.name.clone(), idx);
                    self.views.push(def);
                }
                Err(e) => {
                    tracing::warn!(?path, %e, "skipping invalid view definition");
                }
            }
        }
        Ok(())
    }
}

/// Load YAML files from a directory as `(name, content)` pairs.
///
/// Note: identical copies exist in `swissarmyhammer-fields` and
/// `swissarmyhammer-commands`. The function is trivial and the crates are
/// independent (no shared dependency path that avoids a heavy import),
/// so the duplication is intentional.
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

/// Write to a temp file then rename for atomic persistence.
async fn atomic_write(path: &Path, data: &[u8]) -> Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::InvalidInput, "no parent dir"))?;
    let tmp = dir.join(format!(".tmp_{}", ulid::Ulid::new()));
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ViewDef, ViewKind};
    use tempfile::TempDir;

    fn make_test_view(id: &str, name: &str) -> ViewDef {
        ViewDef {
            id: id.into(),
            name: name.into(),
            icon: None,
            kind: ViewKind::Board,
            entity_type: None,
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[test]
    fn from_yaml_sources_parses_views() {
        let board_yaml = r#"
id: "01BOARD"
name: Board
kind: board
entity_type: task
"#;
        let list_yaml = r#"
id: "01LIST"
name: List
kind: list
"#;
        let ctx = ViewsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("board", board_yaml), ("list", list_yaml)],
        )
        .unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01BOARD").is_some());
        assert!(ctx.get_by_name("Board").is_some());
        assert!(ctx.get_by_name("List").is_some());
    }

    #[test]
    fn from_yaml_sources_later_overrides_earlier() {
        let v1 = r#"
id: "01BOARD"
name: Board
kind: board
"#;
        let v2 = r#"
id: "01BOARD"
name: Board V2
kind: board
"#;
        let ctx = ViewsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("board", v1), ("board2", v2)],
        )
        .unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01BOARD").unwrap().name, "Board V2");
    }

    #[test]
    fn from_yaml_sources_skips_invalid() {
        let good = r#"
id: "01BOARD"
name: Board
kind: board
"#;
        let bad = "not valid: [[[";
        let ctx = ViewsContext::from_yaml_sources(
            PathBuf::from("/tmp/test"),
            &[("board", good), ("bad", bad)],
        )
        .unwrap();

        assert_eq!(ctx.all_views().len(), 1);
    }

    #[tokio::test]
    async fn open_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let _ctx = ViewsContext::open(&root).build().await.unwrap();
        assert!(root.is_dir());
    }

    #[tokio::test]
    async fn write_and_read_view() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01ABC").unwrap().name, "Test");
        assert_eq!(ctx.get_by_name("Test").unwrap().id, "01ABC");

        let path = root.join("01ABC.yaml");
        assert!(path.exists());
    }

    #[tokio::test]
    async fn update_view() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let mut view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        view.name = "Updated".into();
        ctx.write_view(&view).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01ABC").unwrap().name, "Updated");
        assert!(ctx.get_by_name("Test").is_none());
        assert!(ctx.get_by_name("Updated").is_some());
    }

    #[tokio::test]
    async fn delete_view() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();
        ctx.delete_view("01ABC").await.unwrap();

        assert!(ctx.all_views().is_empty());
        assert!(ctx.get_by_id("01ABC").is_none());
        assert!(!root.join("01ABC.yaml").exists());
    }

    #[tokio::test]
    async fn delete_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();
        assert!(ctx.delete_view("nonexistent").await.is_err());
    }

    #[tokio::test]
    async fn persistence_survives_reopen() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");

        {
            let mut ctx = ViewsContext::open(&root).build().await.unwrap();
            ctx.write_view(&make_test_view("01A", "View A"))
                .await
                .unwrap();
            ctx.write_view(&make_test_view("01B", "View B"))
                .await
                .unwrap();
        }

        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01A").is_some());
        assert!(ctx.get_by_id("01B").is_some());
    }

    #[tokio::test]
    async fn delete_middle_fixes_indexes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        ctx.write_view(&make_test_view("01A", "A")).await.unwrap();
        ctx.write_view(&make_test_view("01B", "B")).await.unwrap();
        ctx.write_view(&make_test_view("01C", "C")).await.unwrap();

        ctx.delete_view("01B").await.unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_name("A").is_some());
        assert!(ctx.get_by_name("B").is_none());
        assert!(ctx.get_by_name("C").is_some());
    }

    #[test]
    fn load_yaml_dir_reads_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(
            dir.join("board.yaml"),
            "id: 01B\nname: Board\nkind: board\n",
        )
        .unwrap();
        std::fs::write(dir.join("readme.md"), "# ignore").unwrap();

        let entries = load_yaml_dir(dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "board");
    }

    #[test]
    fn load_yaml_dir_nonexistent_returns_empty() {
        let entries = load_yaml_dir(Path::new("/nonexistent/path"));
        assert!(entries.is_empty());
    }

    /// Cover load_views path that skips non-YAML files on disk.
    #[tokio::test]
    async fn load_views_skips_non_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        std::fs::create_dir_all(&root).unwrap();

        // Write a valid YAML view file
        std::fs::write(
            root.join("board.yaml"),
            "id: 01B\nname: Board\nkind: board\n",
        )
        .unwrap();
        // Write a non-YAML file that should be skipped
        std::fs::write(root.join("readme.md"), "# ignore me").unwrap();
        // Write a file with no extension that should be skipped
        std::fs::write(root.join("Makefile"), "all: build").unwrap();

        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01B").is_some());
    }

    /// Cover load_views path that warns on invalid YAML files on disk.
    #[tokio::test]
    async fn load_views_skips_invalid_yaml_on_disk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        std::fs::create_dir_all(&root).unwrap();

        // Write a valid view
        std::fs::write(
            root.join("good.yaml"),
            "id: 01GOOD\nname: Good\nkind: board\n",
        )
        .unwrap();
        // Write an invalid YAML file (should be skipped with warning)
        std::fs::write(root.join("bad.yaml"), "not valid: [[[").unwrap();

        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01GOOD").is_some());
    }

    /// Cover write_view update path where name stays the same (no name_index removal).
    #[tokio::test]
    async fn write_view_update_same_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        // Update the same view with the same name but different kind
        let mut updated = view.clone();
        updated.kind = ViewKind::Grid;
        ctx.write_view(&updated).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01ABC").unwrap().kind, ViewKind::Grid);
        // Name lookup still works
        assert!(ctx.get_by_name("Test").is_some());
    }

    /// Cover load_yaml_dir when directory exists but read_dir fails
    /// (e.g., a file where a directory is expected).
    #[test]
    fn load_yaml_dir_file_instead_of_dir() {
        let tmp = TempDir::new().unwrap();
        let file_path = tmp.path().join("not_a_dir");
        // Create a file instead of a directory
        std::fs::write(&file_path, "I am a file").unwrap();

        let entries = load_yaml_dir(&file_path);
        assert!(entries.is_empty());
    }

    /// Cover get_by_id returning None for unknown ID.
    #[test]
    fn get_by_id_returns_none_for_unknown() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.get_by_id("nonexistent").is_none());
    }

    /// Cover get_by_name returning None for unknown name.
    #[test]
    fn get_by_name_returns_none_for_unknown() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.get_by_name("nonexistent").is_none());
    }

    /// Cover root() accessor.
    #[tokio::test]
    async fn root_returns_expected_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.root(), root.as_path());
    }

    /// Cover from_yaml_sources with empty sources list.
    #[test]
    fn from_yaml_sources_empty_sources() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.all_views().is_empty());
    }

    /// Cover delete_view when the deleted element is the last in the vec
    /// (swap_remove with idx == views.len(), so no index fixup needed).
    #[tokio::test]
    async fn delete_last_view_no_index_fixup() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        ctx.write_view(&make_test_view("01A", "A")).await.unwrap();
        ctx.write_view(&make_test_view("01B", "B")).await.unwrap();

        // Delete the last element -- swap_remove won't need to fix indexes
        ctx.delete_view("01B").await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01A").is_some());
        assert!(ctx.get_by_id("01B").is_none());
        assert!(ctx.get_by_name("B").is_none());
    }

    /// Cover delete_view removing the file from disk even if it doesn't exist
    /// (the `let _ = fs::remove_file` path that ignores errors).
    #[tokio::test]
    async fn delete_view_missing_file_still_removes_from_memory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        // Manually remove the file before calling delete_view
        let path = root.join("01ABC.yaml");
        std::fs::remove_file(&path).unwrap();

        // delete_view should still succeed (ignores file removal error)
        ctx.delete_view("01ABC").await.unwrap();
        assert!(ctx.all_views().is_empty());
    }

    /// Cover write_view adding a new view to a context built via from_yaml_sources.
    #[tokio::test]
    async fn write_view_new_on_yaml_sources_context() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");

        let board_yaml = r#"
id: "01BOARD"
name: Board
kind: board
"#;
        let mut ctx = ViewsContext::from_yaml_sources(&root, &[("board", board_yaml)]).unwrap();

        // Write a brand new view (not an update)
        let new_view = make_test_view("01NEW", "New View");
        ctx.write_view(&new_view).await.unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01BOARD").is_some());
        assert!(ctx.get_by_id("01NEW").is_some());
        assert!(ctx.get_by_name("New View").is_some());

        // Verify it was persisted to disk
        assert!(root.join("01NEW.yaml").exists());
    }

    /// Cover write_view updating an existing view loaded from from_yaml_sources.
    #[tokio::test]
    async fn write_view_update_on_yaml_sources_context() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");

        let board_yaml = r#"
id: "01BOARD"
name: Board
kind: board
"#;
        let mut ctx = ViewsContext::from_yaml_sources(&root, &[("board", board_yaml)]).unwrap();

        // Update the existing view with a new name
        let mut updated = ctx.get_by_id("01BOARD").unwrap().clone();
        updated.name = "Updated Board".into();
        ctx.write_view(&updated).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01BOARD").unwrap().name, "Updated Board");
        assert!(ctx.get_by_name("Board").is_none());
        assert!(ctx.get_by_name("Updated Board").is_some());
    }

    /// Cover delete_view on a from_yaml_sources context with swap-remove index fixup.
    #[tokio::test]
    async fn delete_from_yaml_sources_fixes_indexes() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");

        let yaml_a = "id: 01A\nname: A\nkind: board\n";
        let yaml_b = "id: 01B\nname: B\nkind: list\n";
        let yaml_c = "id: 01C\nname: C\nkind: grid\n";
        let mut ctx =
            ViewsContext::from_yaml_sources(&root, &[("a", yaml_a), ("b", yaml_b), ("c", yaml_c)])
                .unwrap();

        // Delete the first element (swap_remove replaces it with last)
        ctx.delete_view("01A").await.unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01A").is_none());
        assert!(ctx.get_by_name("A").is_none());
        assert!(ctx.get_by_id("01B").is_some());
        assert!(ctx.get_by_id("01C").is_some());
    }

    /// Cover load_yaml_dir skipping non-YAML files (exercises the extension filter).
    #[test]
    fn load_yaml_dir_skips_non_yaml() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        std::fs::write(dir.join("view.yaml"), "id: 01V\nname: View\nkind: board\n").unwrap();
        std::fs::write(dir.join("notes.txt"), "some notes").unwrap();
        std::fs::write(dir.join("data.json"), "{}").unwrap();

        let entries = load_yaml_dir(dir);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "view");
    }
}
