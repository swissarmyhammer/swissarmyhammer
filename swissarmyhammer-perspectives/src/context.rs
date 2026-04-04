//! PerspectiveContext -- file-backed CRUD for perspective definitions.
//!
//! Manages perspectives stored as YAML files in a `perspectives/` directory,
//! with in-memory indexes for fast lookup by ID and name.
//!
//! ```text
//! .kanban/perspectives/
//!   01JPERSP000000000000000000.yaml
//!   01JPERSP000000000000000001.yaml
//! ```

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use tokio::fs;
use tracing::debug;

use crate::error::{PerspectiveError, Result};
use crate::store::PerspectiveStore;
use crate::types::Perspective;
use crate::PerspectiveId;
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId, UndoEntryId};

/// Read cache and write coordinator for perspective definitions.
///
/// On `open()`, loads all YAML files into memory for fast lookup. When a
/// `StoreHandle` is wired in via `set_store_handle`, mutations delegate I/O
/// to the store (which handles serialization, changelog, undo/redo, and
/// change events). Without a store handle, falls back to direct file I/O.
pub struct PerspectiveContext {
    root: PathBuf,
    perspectives: Vec<Perspective>,
    id_index: HashMap<String, usize>,
    /// When set, write/delete delegate I/O to the store handle instead of
    /// doing their own atomic_write / fs::remove_file.
    store_handle: Option<Arc<StoreHandle<PerspectiveStore>>>,
    /// Shared undo/redo stack. When set, write/delete push entries.
    store_context: OnceLock<Arc<StoreContext>>,
}

impl PerspectiveContext {
    /// Open a perspectives directory, loading all YAML files into memory.
    ///
    /// Creates the directory if it does not exist. Invalid YAML files are
    /// logged and skipped.
    pub async fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let root = dir.into();
        fs::create_dir_all(&root).await?;

        let mut ctx = Self {
            root,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
        };

        ctx.load_all().await?;

        debug!(
            perspectives = ctx.perspectives.len(),
            "perspective context opened"
        );
        Ok(ctx)
    }

    /// Write (create or update) a perspective.
    ///
    /// When a `StoreHandle` is wired in, delegates file I/O to it (which
    /// provides changelog, undo/redo, and change events). Otherwise falls
    /// back to direct atomic file writes.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle recorded the change,
    /// or `Ok(None)` for idempotent writes or the legacy fallback path.
    ///
    /// Multiple perspectives may share the same name — only IDs are unique.
    pub async fn write(&mut self, perspective: &Perspective) -> Result<Option<UndoEntryId>> {
        // Persist to disk — delegate to StoreHandle when available.
        let entry_id = if let Some(ref sh) = self.store_handle {
            sh.write(perspective).await?
        } else {
            let yaml = serde_yaml_ng::to_string(perspective)?;
            let path = self.perspective_path(&perspective.id);
            atomic_write(&path, yaml.as_bytes()).await?;
            None
        };

        // Push onto the shared undo stack if a StoreContext is available.
        if let (Some(sc), Some(eid)) = (self.store_context.get(), &entry_id) {
            let is_create = !self.id_index.contains_key(&perspective.id);
            let op = if is_create { "create" } else { "update" };
            let label = format!("{} perspective {}", op, perspective.id);
            let item_id = StoredItemId::from(perspective.id.as_str());
            sc.push(*eid, label, item_id).await;
        }

        // Update in-memory cache.
        if let Some(&idx) = self.id_index.get(&perspective.id) {
            self.perspectives[idx] = perspective.clone();
        } else {
            let idx = self.perspectives.len();
            self.perspectives.push(perspective.clone());
            self.id_index.insert(perspective.id.clone(), idx);
        }

        Ok(entry_id)
    }

    /// Look up a perspective by its ULID.
    pub fn get_by_id(&self, id: &str) -> Option<&Perspective> {
        self.id_index.get(id).map(|&i| &self.perspectives[i])
    }

    /// Look up a perspective by name (linear scan, returns first match).
    ///
    /// Names are **not unique** — multiple perspectives may share the same name.
    /// When duplicates exist this returns an arbitrary match (whichever appears
    /// first in insertion order). For reliable lookup, use [`get_by_id`](Self::get_by_id)
    /// with the perspective's ULID instead.
    pub fn get_by_name(&self, name: &str) -> Option<&Perspective> {
        self.perspectives.iter().find(|p| p.name == name)
    }

    /// All loaded perspectives.
    pub fn all(&self) -> &[Perspective] {
        &self.perspectives
    }

    /// Delete a perspective by ID and return the deleted value plus undo entry.
    ///
    /// When a `StoreHandle` is wired in, delegates file removal to it (which
    /// trashes the file for undo support and records a change event). Otherwise
    /// falls back to direct `fs::remove_file`.
    ///
    /// Returns `(deleted_perspective, Some(entry_id))` when a store handle
    /// recorded the deletion, or `(deleted_perspective, None)` for the legacy
    /// fallback path.
    ///
    /// Returns `PerspectiveError::NotFound` if no perspective with that ID exists.
    pub async fn delete(&mut self, id: &str) -> Result<(Perspective, Option<UndoEntryId>)> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| PerspectiveError::NotFound {
                resource: "perspective".to_string(),
                id: id.to_string(),
            })?;

        // Remove from disk — delegate to StoreHandle when available.
        let entry_id = if let Some(ref sh) = self.store_handle {
            let pid = PerspectiveId::from(id);
            let entry_id = sh.delete(&pid).await?;
            // Push onto the shared undo stack if a StoreContext is available.
            if let Some(sc) = self.store_context.get() {
                let label = format!("delete perspective {}", id);
                let item_id = StoredItemId::from(id);
                sc.push(entry_id, label, item_id).await;
            }
            Some(entry_id)
        } else {
            let path = self.perspective_path(id);
            match fs::remove_file(&path).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(PerspectiveError::Io(e));
                }
            }
            None
        };

        // Update in-memory cache.
        let deleted = self.perspectives.swap_remove(idx);
        self.id_index.remove(&deleted.id);

        // Fix the index of the element that was swapped into `idx`
        if idx < self.perspectives.len() {
            let moved = &self.perspectives[idx];
            self.id_index.insert(moved.id.clone(), idx);
        }

        Ok((deleted, entry_id))
    }

    /// Wire in a `StoreHandle` for delegated I/O.
    ///
    /// When set, `write()` and `delete()` delegate file operations to the
    /// store handle, which provides changelog, undo/redo, and change events.
    pub fn set_store_handle(&mut self, handle: Arc<StoreHandle<PerspectiveStore>>) {
        self.store_handle = Some(handle);
    }

    /// Set the shared undo/redo stack.
    ///
    /// When set, `write()` and `delete()` push entries onto the stack.
    /// Can be called through a shared reference (uses `OnceLock`).
    pub fn set_store_context(&self, ctx: Arc<StoreContext>) {
        let _ = self.store_context.set(ctx);
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    // --- Internal ---

    /// Path to a perspective's YAML file.
    fn perspective_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.yaml"))
    }

    /// Load all YAML files from the root directory into memory.
    async fn load_all(&mut self) -> Result<()> {
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
            match serde_yaml_ng::from_str::<Perspective>(&content) {
                Ok(p) => {
                    let idx = self.perspectives.len();
                    self.id_index.insert(p.id.clone(), idx);
                    self.perspectives.push(p);
                }
                Err(e) => {
                    tracing::warn!(?path, %e, "skipping invalid perspective file");
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
    let tmp = dir.join(format!(".tmp_{}", ulid::Ulid::new()));
    fs::write(&tmp, data).await?;
    fs::rename(&tmp, path).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::PerspectiveStore;
    use crate::types::{PerspectiveFieldEntry, SortDirection, SortEntry};
    use std::sync::Arc;
    use swissarmyhammer_store::StoreHandle;
    use tempfile::TempDir;

    /// Create a PerspectiveContext wired to a StoreHandle (the production path).
    async fn setup_with_store(
        dir: &Path,
    ) -> (PerspectiveContext, Arc<StoreHandle<PerspectiveStore>>) {
        tokio::fs::create_dir_all(dir).await.unwrap();
        let store = Arc::new(PerspectiveStore::new(dir));
        let handle = Arc::new(StoreHandle::new(store));
        let mut ctx = PerspectiveContext::open(dir).await.unwrap();
        ctx.set_store_handle(Arc::clone(&handle));
        (ctx, handle)
    }

    /// Create a PerspectiveContext with StoreHandle + StoreContext for undo tests.
    async fn setup_with_undo(
        dir: &Path,
    ) -> (
        PerspectiveContext,
        Arc<StoreHandle<PerspectiveStore>>,
        Arc<swissarmyhammer_store::StoreContext>,
    ) {
        tokio::fs::create_dir_all(dir).await.unwrap();
        let store = Arc::new(PerspectiveStore::new(dir));
        let handle = Arc::new(StoreHandle::new(store));
        let store_context = Arc::new(swissarmyhammer_store::StoreContext::new(
            dir.parent().unwrap().to_path_buf(),
        ));
        store_context.register(handle.clone()).await;
        let mut ctx = PerspectiveContext::open(dir).await.unwrap();
        ctx.set_store_handle(Arc::clone(&handle));
        ctx.set_store_context(Arc::clone(&store_context));
        (ctx, handle, store_context)
    }

    /// Helper to create a test perspective with sensible defaults.
    fn make_perspective(id: &str, name: &str) -> Perspective {
        Perspective {
            id: id.to_string(),
            name: name.to_string(),
            view: "board".to_string(),
            fields: vec![],
            filter: None,
            group: None,
            sort: vec![],
        }
    }

    /// Helper to create a rich perspective with all optional fields populated.
    fn make_rich_perspective(id: &str, name: &str) -> Perspective {
        Perspective {
            id: id.to_string(),
            name: name.to_string(),
            view: "grid".to_string(),
            fields: vec![PerspectiveFieldEntry {
                field: "01JMTASK0000000000TITLE00".to_string(),
                caption: Some("Title".to_string()),
                width: Some(200),
                editor: None,
                display: None,
                sort_comparator: None,
            }],
            filter: Some("(e) => e.Status !== \"Done\"".to_string()),
            group: Some("(e) => e.Assignee".to_string()),
            sort: vec![SortEntry {
                field: "01JMTASK0000000000PRIORTY".to_string(),
                direction: SortDirection::Asc,
            }],
        }
    }

    #[tokio::test]
    async fn write_and_read_by_id() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint View");
        ctx.write(&p).await.unwrap();

        let found = ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap();
        assert_eq!(found.name, "Sprint View");
        assert_eq!(found.view, "board");
    }

    #[tokio::test]
    async fn write_and_read_by_name() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "My Grid");
        ctx.write(&p).await.unwrap();

        let found = ctx.get_by_name("My Grid").unwrap();
        assert_eq!(found.id, "01BBBBBBBBBBBBBBBBBBBBBBBB");
        assert_eq!(found.fields.len(), 1);
        assert_eq!(
            found.filter.as_deref(),
            Some("(e) => e.Status !== \"Done\"")
        );
    }

    #[tokio::test]
    async fn list_all_perspectives() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        assert!(ctx.all().is_empty());

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "View A"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "View B"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "View C"))
            .await
            .unwrap();

        assert_eq!(ctx.all().len(), 3);
    }

    #[tokio::test]
    async fn delete_perspective() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();
        assert_eq!(ctx.all().len(), 1);

        let (deleted, entry_id) = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Doomed");
        assert!(entry_id.is_none(), "no store handle means no undo entry");
        assert!(ctx.all().is_empty());
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());
        assert!(ctx.get_by_name("Doomed").is_none());

        // File should be gone
        let file = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        assert!(!file.exists());
    }

    #[tokio::test]
    async fn delete_tolerates_already_removed_file() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Ghost");
        ctx.write(&p).await.unwrap();

        // Remove the file externally before calling delete
        let file = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        fs::remove_file(&file).await.unwrap();
        assert!(!file.exists());

        // delete should still succeed (NotFound is tolerated)
        let (deleted, _) = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Ghost");
        assert!(ctx.all().is_empty());
    }

    #[tokio::test]
    async fn delete_nonexistent_errors() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let err = ctx.delete("01ZZZZZZZZZZZZZZZZZZZZZZZZ").await;
        assert!(err.is_err());
        let msg = format!("{}", err.unwrap_err());
        assert!(msg.contains("perspective"));
        assert!(msg.contains("not found"));
    }

    #[tokio::test]
    async fn persistence_survives_reopen() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Write two perspectives
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
                .await
                .unwrap();
            ctx.write(&make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
                .await
                .unwrap();
            assert_eq!(ctx.all().len(), 2);
        }

        // Reopen and verify everything is still there
        {
            let ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert_eq!(ctx.all().len(), 2);

            let alpha = ctx.get_by_name("Alpha").unwrap();
            assert_eq!(alpha.id, "01AAAAAAAAAAAAAAAAAAAAAAAA");

            let beta = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
            assert_eq!(beta.name, "Beta");
            assert_eq!(beta.fields.len(), 1);
            assert!(beta.filter.is_some());
        }
    }

    #[tokio::test]
    async fn write_updates_existing() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let mut p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Original");
        ctx.write(&p).await.unwrap();

        // Update name
        p.name = "Renamed".to_string();
        ctx.write(&p).await.unwrap();

        assert_eq!(ctx.all().len(), 1);
        assert!(ctx.get_by_name("Original").is_none());
        assert!(ctx.get_by_name("Renamed").is_some());
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().name,
            "Renamed"
        );
    }

    #[tokio::test]
    async fn rename_to_own_name_succeeds() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Stable");
        ctx.write(&p).await.unwrap();

        // Writing the same perspective with the same name should succeed (no-op rename)
        ctx.write(&p).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
    }

    /// Deleting the first element (not the last) exercises the swap_remove index
    /// fixup at lines 156-159. After deletion the remaining perspectives must
    /// still be findable by both ID and name.
    #[tokio::test]
    async fn delete_first_element_fixes_swap_remove_indexes() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        // Insert three perspectives -- internal order is [A, B, C]
        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Gamma"))
            .await
            .unwrap();
        assert_eq!(ctx.all().len(), 3);

        // Delete the FIRST perspective -- swap_remove swaps last element into idx 0
        let (deleted, _) = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Alpha");
        assert_eq!(ctx.all().len(), 2);

        // The deleted perspective must be gone from both indexes
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());
        assert!(ctx.get_by_name("Alpha").is_none());

        // Beta and Gamma must still be reachable by both ID and name
        let beta_by_id = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB");
        assert!(beta_by_id.is_some(), "Beta must be findable by ID");
        assert_eq!(beta_by_id.unwrap().name, "Beta");

        let beta_by_name = ctx.get_by_name("Beta");
        assert!(beta_by_name.is_some(), "Beta must be findable by name");
        assert_eq!(beta_by_name.unwrap().id, "01BBBBBBBBBBBBBBBBBBBBBBBB");

        let gamma_by_id = ctx.get_by_id("01CCCCCCCCCCCCCCCCCCCCCCCC");
        assert!(gamma_by_id.is_some(), "Gamma must be findable by ID");
        assert_eq!(gamma_by_id.unwrap().name, "Gamma");

        let gamma_by_name = ctx.get_by_name("Gamma");
        assert!(gamma_by_name.is_some(), "Gamma must be findable by name");
        assert_eq!(gamma_by_name.unwrap().id, "01CCCCCCCCCCCCCCCCCCCCCCCC");
    }

    /// Deleting the middle element also exercises swap_remove index fixup.
    /// The last element is swapped into the middle slot.
    #[tokio::test]
    async fn delete_middle_element_fixes_swap_remove_indexes() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
            .await
            .unwrap();
        ctx.write(&make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Gamma"))
            .await
            .unwrap();

        // Delete the middle element
        let (deleted, _) = ctx.delete("01BBBBBBBBBBBBBBBBBBBBBBBB").await.unwrap();
        assert_eq!(deleted.name, "Beta");
        assert_eq!(ctx.all().len(), 2);

        assert!(ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").is_none());
        assert!(ctx.get_by_name("Beta").is_none());

        // Alpha and Gamma must still be reachable
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().name,
            "Alpha"
        );
        assert_eq!(
            ctx.get_by_name("Alpha").unwrap().id,
            "01AAAAAAAAAAAAAAAAAAAAAAAA"
        );
        assert_eq!(
            ctx.get_by_id("01CCCCCCCCCCCCCCCCCCCCCCCC").unwrap().name,
            "Gamma"
        );
        assert_eq!(
            ctx.get_by_name("Gamma").unwrap().id,
            "01CCCCCCCCCCCCCCCCCCCCCCCC"
        );
    }

    /// When remove_file hits a non-NotFound IO error, delete must propagate it
    /// as `PerspectiveError::Io`. We induce this by replacing the YAML file with
    /// a directory -- `remove_file` on a directory yields an OS error.
    #[tokio::test]
    async fn delete_propagates_non_not_found_io_error() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();

        // Replace the YAML file with a directory so remove_file fails with a
        // non-NotFound error (IsADirectory / PermissionDenied depending on OS).
        let file_path = tmp
            .path()
            .join("perspectives")
            .join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml");
        fs::remove_file(&file_path).await.unwrap();
        fs::create_dir(&file_path).await.unwrap();

        let err = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await;
        assert!(err.is_err(), "delete should fail with IO error");
        let msg = err.unwrap_err().to_string();
        assert!(msg.contains("IO error"), "expected IO error, got: {msg}");
    }

    #[tokio::test]
    async fn load_all_ignores_non_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Bootstrap with a valid perspective
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Valid"))
                .await
                .unwrap();
        }

        // Drop non-.yaml files into the directory
        fs::write(dir.join("notes.txt"), b"some text")
            .await
            .unwrap();
        fs::write(dir.join("config.json"), b"{}").await.unwrap();
        fs::write(dir.join("noext"), b"data").await.unwrap();

        // Reopen -- non-yaml files must be silently ignored
        let ctx = PerspectiveContext::open(&dir).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
        assert_eq!(
            ctx.get_by_name("Valid").unwrap().id,
            "01AAAAAAAAAAAAAAAAAAAAAAAA"
        );
    }

    #[tokio::test]
    async fn load_all_skips_malformed_yaml() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Bootstrap with a valid perspective
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Good"))
                .await
                .unwrap();
        }

        // Write a malformed YAML file that will fail deserialization
        fs::write(
            dir.join("01BADBADBADBADBADBADBADBAD.yaml"),
            b"not: [valid: yaml: {{",
        )
        .await
        .unwrap();

        // Reopen -- malformed file must be skipped, valid file still loads
        let ctx = PerspectiveContext::open(&dir).await.unwrap();
        assert_eq!(ctx.all().len(), 1);
        assert_eq!(
            ctx.get_by_name("Good").unwrap().id,
            "01AAAAAAAAAAAAAAAAAAAAAAAA"
        );
    }

    #[tokio::test]
    async fn open_fresh_directory_creates_it_and_returns_empty() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("brand_new_perspectives");
        assert!(!dir.exists(), "directory must not exist before open");

        let ctx = PerspectiveContext::open(&dir).await.unwrap();

        assert!(dir.exists(), "open() must create the directory");
        assert!(dir.is_dir(), "created path must be a directory");
        assert!(
            ctx.all().is_empty(),
            "fresh context must have no perspectives"
        );
        assert_eq!(ctx.root(), dir);
    }

    #[tokio::test]
    async fn open_loads_preexisting_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        fs::create_dir_all(&dir).await.unwrap();

        // Write YAML files directly (not via PerspectiveContext) to simulate pre-existing data
        let p1 = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Pre A");
        let p2 = make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Pre B");
        let p3 = make_perspective("01CCCCCCCCCCCCCCCCCCCCCCCC", "Pre C");

        for p in [&p1, &p2, &p3] {
            let yaml = serde_yaml_ng::to_string(p).unwrap();
            fs::write(dir.join(format!("{}.yaml", p.id)), yaml.as_bytes())
                .await
                .unwrap();
        }

        let ctx = PerspectiveContext::open(&dir).await.unwrap();

        assert_eq!(
            ctx.all().len(),
            3,
            "all three pre-existing perspectives must load"
        );
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_some());
        assert!(ctx.get_by_name("Pre B").is_some());
        // Verify rich fields survived the round-trip
        let b = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
        assert_eq!(b.view, "grid");
        assert_eq!(b.fields.len(), 1);
        assert!(b.filter.is_some());
        assert_eq!(b.sort.len(), 1);
    }

    #[tokio::test]
    async fn open_write_close_reopen_round_trip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("round_trip_perspectives");
        assert!(!dir.exists());

        // Phase 1: open fresh, write, drop (close)
        {
            let mut ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert!(dir.exists(), "directory created on first open");
            assert!(ctx.all().is_empty());

            ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Alpha"))
                .await
                .unwrap();
            ctx.write(&make_rich_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Beta"))
                .await
                .unwrap();
            assert_eq!(ctx.all().len(), 2);
        }
        // ctx dropped here -- simulates close

        // Phase 2: reopen and verify persistence
        {
            let ctx = PerspectiveContext::open(&dir).await.unwrap();
            assert_eq!(ctx.all().len(), 2, "both perspectives must survive reopen");

            let alpha = ctx.get_by_name("Alpha").unwrap();
            assert_eq!(alpha.id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            assert_eq!(alpha.view, "board");

            let beta = ctx.get_by_id("01BBBBBBBBBBBBBBBBBBBBBBBB").unwrap();
            assert_eq!(beta.name, "Beta");
            assert_eq!(beta.view, "grid");
            assert_eq!(beta.fields.len(), 1);
            assert!(beta.filter.is_some());
            assert!(beta.group.is_some());
            assert_eq!(beta.sort.len(), 1);
        }

        // Verify YAML files exist on disk
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
        assert!(dir.join("01BBBBBBBBBBBBBBBBBBBBBBBB.yaml").exists());
    }

    #[tokio::test]
    async fn load_all_returns_ok_when_root_missing() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");

        // Create and immediately remove the directory so load_all hits the early return
        fs::create_dir_all(&dir).await.unwrap();
        fs::remove_dir(&dir).await.unwrap();
        assert!(!dir.exists());

        // Build the context manually to bypass open()'s create_dir_all
        let mut ctx = PerspectiveContext {
            root: dir,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
        };

        // load_all should return Ok with zero perspectives
        ctx.load_all().await.unwrap();
        assert!(ctx.all().is_empty());
    }

    // =========================================================================
    // Store-delegated I/O tests
    // =========================================================================

    #[tokio::test]
    async fn write_delegates_to_store_handle_and_produces_event() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, handle) = setup_with_store(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Board View");
        ctx.write(&p).await.unwrap();

        // File must exist on disk (written by StoreHandle)
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        // In-memory cache must be updated
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().name,
            "Board View"
        );

        // StoreHandle must have a pending event
        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-created");
        assert_eq!(events[0].payload["store"], "perspective");
        assert_eq!(events[0].payload["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[tokio::test]
    async fn update_via_store_handle_produces_changed_event() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, handle) = setup_with_store(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Original");
        ctx.write(&p).await.unwrap();
        handle.flush_changes().await; // drain create event

        let mut updated = p.clone();
        updated.name = "Renamed".to_string();
        ctx.write(&updated).await.unwrap();

        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-changed");
        assert_eq!(events[0].payload["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
    }

    #[tokio::test]
    async fn delete_delegates_to_store_handle_and_produces_event() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, handle) = setup_with_store(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();
        handle.flush_changes().await; // drain create event

        let (deleted, entry_id) = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert_eq!(deleted.name, "Doomed");
        assert!(
            entry_id.is_some(),
            "store handle delete must return entry ID"
        );

        // File must be gone (trashed by StoreHandle)
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        // In-memory cache must be updated
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());

        // StoreHandle must have a pending event
        let events = handle.flush_changes().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_name, "item-removed");
        assert_eq!(events[0].payload["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
    }

    // =========================================================================
    // Undo/redo tests — matching EntityContext pattern
    // =========================================================================

    #[tokio::test]
    async fn write_returns_undo_entry_id() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle, _sc) = setup_with_undo(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Board View");
        let entry_id = ctx.write(&p).await.unwrap();

        // First write (create) must return Some
        assert!(entry_id.is_some(), "create must return an UndoEntryId");
    }

    #[tokio::test]
    async fn write_pushes_onto_undo_stack() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        assert!(!sc.can_undo().await, "nothing to undo before any writes");

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Board View");
        ctx.write(&p).await.unwrap();

        assert!(sc.can_undo().await, "undo must be available after write");
    }

    #[tokio::test]
    async fn undo_reverts_perspective_create() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Undo Me");
        ctx.write(&p).await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        sc.undo().await.unwrap();

        // File must be gone after undo
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
    }

    #[tokio::test]
    async fn delete_pushes_onto_undo_stack() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Delete Me");
        ctx.write(&p).await.unwrap();

        // Reset undo stack state by undoing+redoing (or just check after delete)
        assert!(sc.can_undo().await);
        // Now delete
        let (_deleted, entry_id) = ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert!(
            entry_id.is_some(),
            "delete with store handle must return entry ID"
        );
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        // Undo the delete — file must be restored
        sc.undo().await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
    }

    #[tokio::test]
    async fn duplicate_names_allowed() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle) = setup_with_store(&dir).await;

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint"))
            .await
            .unwrap();
        // Same name, different ID — must succeed
        ctx.write(&make_perspective("01BBBBBBBBBBBBBBBBBBBBBBBB", "Sprint"))
            .await
            .unwrap();

        assert_eq!(ctx.all().len(), 2);
    }
}
