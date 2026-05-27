//! ViewsContext -- main API surface for the views registry.
//!
//! Manages view definitions with in-memory indexes for fast lookup by both
//! name and ID. Supports CRUD operations with disk persistence.
//!
//! Two ways to create a ViewsContext:
//!
//! 1. `from_yaml_sources()` -- from pre-loaded YAML content (VFS / embedded)
//! 2. `open().build()` -- from a directory on disk (for tests / standalone)
//!
//! When a [`StoreHandle<ViewStore>`] is wired in via
//! [`ViewsContext::set_store_handle`], mutations delegate I/O to the store
//! handle (which provides changelog, undo/redo, and change events) and
//! [`ViewsContext::write_view`] / [`ViewsContext::delete_view`] push entries
//! onto the shared undo stack when a [`StoreContext`] is also attached.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

use tokio::fs;
use tokio::sync::broadcast;
use tracing::debug;

use crate::error::{Result, ViewsError};
use crate::events::ViewEvent;
use crate::store::ViewStore;
use crate::types::{ViewDef, ViewId};
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId, UndoEntryId};

/// Default capacity for the view event broadcast channel.
///
/// Sized smaller than the entity cache channel (256) because view mutations
/// are infrequent — a handful per session, not hundreds per second.
const EVENT_CHANNEL_CAPACITY: usize = 64;

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

        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut ctx = ViewsContext {
            root,
            views: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
            event_sender,
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
///
/// When a `StoreHandle` is wired in via `set_store_handle`, mutations delegate
/// I/O to the store (which handles serialization, changelog, undo/redo, and
/// change events). Without a store handle, falls back to direct file I/O.
pub struct ViewsContext {
    root: PathBuf,
    views: Vec<ViewDef>,
    id_index: HashMap<ViewId, usize>,
    name_index: HashMap<String, usize>,
    /// When set, write/delete delegate I/O to the store handle instead of
    /// doing their own atomic_write / fs::remove_file.
    store_handle: Option<Arc<StoreHandle<ViewStore>>>,
    /// Shared undo/redo stack. When set, write/delete push entries.
    store_context: OnceLock<Arc<StoreContext>>,
    /// Broadcast channel for view change events.
    ///
    /// Consumers (e.g. the Tauri bridge) subscribe to this channel to learn
    /// about view mutations without coupling to the views crate.
    event_sender: broadcast::Sender<ViewEvent>,
}

impl std::fmt::Debug for ViewsContext {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ViewsContext")
            .field("root", &self.root)
            .field("views_count", &self.views.len())
            .field("has_store_handle", &self.store_handle.is_some())
            .field("has_store_context", &self.store_context.get().is_some())
            .finish()
    }
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
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut ctx = ViewsContext {
            root,
            views: Vec::new(),
            id_index: HashMap::new(),
            name_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
            event_sender,
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

    /// Write (create or update) a view definition.
    ///
    /// When a `StoreHandle` is wired in, delegates file I/O to it (which
    /// provides changelog, undo/redo, and change events). Otherwise falls
    /// back to direct atomic file writes.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle recorded the change,
    /// or `Ok(None)` for idempotent writes or the legacy fallback path.
    pub async fn write_view(&mut self, def: &ViewDef) -> Result<Option<UndoEntryId>> {
        // Snapshot the old state for diff computation.
        let old = self.get_by_id(&def.id).cloned();

        // Persist to disk — delegate to StoreHandle when available.
        let entry_id = if let Some(ref sh) = self.store_handle {
            sh.write(def).await?
        } else {
            let yaml = serde_yaml_ng::to_string(def)?;
            let path = self.view_path(&def.id);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).await?;
            }
            atomic_write(&path, yaml.as_bytes()).await?;
            None
        };

        // Push onto the shared undo stack if a StoreContext is available.
        if let (Some(sc), Some(eid)) = (self.store_context.get(), &entry_id) {
            let is_create = old.is_none();
            let op = if is_create { "create" } else { "update" };
            let label = format!("{} view {}", op, def.id);
            let item_id = StoredItemId::from(def.id.as_str());
            sc.push(*eid, label, item_id).await;
        }

        // Update in-memory cache.
        self.cache_upsert(def.clone());

        // Broadcast the change event. Compute the field-level diff so
        // consumers know which fields actually changed.
        let is_create = old.is_none();
        let changed_fields = diff_view(old.as_ref(), def);
        if !changed_fields.is_empty() {
            let _ = self.event_sender.send(ViewEvent::ViewChanged {
                id: def.id.clone(),
                changed_fields,
                is_create,
            });
        }

        Ok(entry_id)
    }

    /// Delete a view definition by ID.
    ///
    /// When a `StoreHandle` is wired in, delegates file removal to it (which
    /// trashes the file for undo support and records a change event). Otherwise
    /// falls back to direct `fs::remove_file`.
    ///
    /// Returns `Ok(Some(entry_id))` when a store handle recorded the deletion,
    /// or `Ok(None)` for the legacy fallback path.
    ///
    /// Returns `ViewsError::ViewNotFound` if no view with that ID exists.
    pub async fn delete_view(&mut self, id: &str) -> Result<Option<UndoEntryId>> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| ViewsError::ViewNotFound { id: id.to_string() })?;

        // Remove from disk — delegate to StoreHandle when available.
        let entry_id = if let Some(ref sh) = self.store_handle {
            let vid: ViewId = id.to_string();
            let entry_id = sh.delete(&vid).await?;
            // Push onto the shared undo stack if a StoreContext is available.
            if let Some(sc) = self.store_context.get() {
                let label = format!("delete view {}", id);
                let item_id = StoredItemId::from(id);
                sc.push(entry_id, label, item_id).await;
            }
            Some(entry_id)
        } else {
            let path = self.view_path(id);
            match fs::remove_file(&path).await {
                Ok(_) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(ViewsError::Io(e));
                }
            }
            None
        };

        // Update in-memory cache.
        let deleted = self.cache_remove_at(idx);

        // Broadcast the deletion event.
        let _ = self
            .event_sender
            .send(ViewEvent::ViewDeleted { id: deleted.id });

        Ok(entry_id)
    }

    /// Wire in a `StoreHandle` for delegated I/O.
    ///
    /// When set, `write_view()` and `delete_view()` delegate file operations
    /// to the store handle, which provides changelog, undo/redo, and change
    /// events.
    pub fn set_store_handle(&mut self, handle: Arc<StoreHandle<ViewStore>>) {
        self.store_handle = Some(handle);
    }

    /// Set the shared undo/redo stack.
    ///
    /// When set, `write_view()` and `delete_view()` push entries onto the stack.
    /// Can be called through a shared reference (uses `OnceLock`).
    pub fn set_store_context(&self, ctx: Arc<StoreContext>) {
        let _ = self.store_context.set(ctx);
    }

    /// Return the `Arc<StoreContext>` previously installed via
    /// [`set_store_context`], if any.
    ///
    /// Exposed so substrate-guard tests can verify via `Arc::ptr_eq` that the
    /// views context shares the single app-wide `StoreContext`. Production
    /// code paths reach the context through the setter side and do not need
    /// to read it back.
    pub fn store_context(&self) -> Option<Arc<StoreContext>> {
        self.store_context.get().cloned()
    }

    /// Subscribe to view change events.
    ///
    /// Returns a receiver that will get all events emitted after this call.
    /// Missed events (due to slow consumption) result in `RecvError::Lagged`.
    pub fn subscribe(&self) -> broadcast::Receiver<ViewEvent> {
        self.event_sender.subscribe()
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Refresh a single view's in-memory entry from disk.
    ///
    /// Used by post-undo / post-redo reconciliation after the store layer has
    /// rewritten the on-disk YAML without going through [`write_view`](Self::write_view).
    /// Mirrors `PerspectiveContext::reload_from_disk`:
    ///
    /// - If the file exists and parses, replace the cached entry and emit
    ///   [`ViewEvent::ViewChanged`] with `is_create: false` so downstream
    ///   subscribers (Tauri bridge, frontend refresh) react. The
    ///   `changed_fields` list is left empty to signal "unspecified — full
    ///   refresh may be needed" because the pre-undo state in memory may have
    ///   already been overwritten by the disk rewrite, so a meaningful field
    ///   diff is not reliably computable here.
    /// - If the file is absent (undo of a create, redo of a delete), evict
    ///   the cached entry and emit [`ViewEvent::ViewDeleted`].
    /// - If the file is absent and the cache also has no entry, this is a
    ///   no-op — nothing to reconcile and no event is emitted.
    ///
    /// Parse failures on an existing file return an error. In-memory cache
    /// state is not mutated when parsing fails.
    pub async fn reload_from_disk(&mut self, id: &str) -> Result<()> {
        let path = self.view_path(id);
        if path.exists() {
            let content = fs::read_to_string(&path).await?;
            let view: ViewDef = serde_yaml_ng::from_str(&content)?;
            self.cache_upsert(view.clone());
            let _ = self.event_sender.send(ViewEvent::ViewChanged {
                id: view.id,
                // Empty list signals "unspecified — consumers should treat
                // this as a full refresh."
                changed_fields: Vec::new(),
                is_create: false,
            });
        } else if let Some(&idx) = self.id_index.get(id) {
            let _deleted = self.cache_remove_at(idx);
            let _ = self
                .event_sender
                .send(ViewEvent::ViewDeleted { id: id.to_string() });
        }
        Ok(())
    }

    // --- Internal ---

    fn view_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.yaml"))
    }

    /// Insert or replace a view in the in-memory cache.
    ///
    /// When the id is already known, overwrites the existing slot and updates
    /// the name index if the name changed. When it is new, appends and records
    /// both indexes. Shared by [`write_view`](Self::write_view) and
    /// [`reload_from_disk`](Self::reload_from_disk) to keep the replace /
    /// append logic in one place.
    fn cache_upsert(&mut self, view: ViewDef) {
        if let Some(&idx) = self.id_index.get(&view.id) {
            let old_name = self.views[idx].name.clone();
            if old_name != view.name {
                self.name_index.remove(&old_name);
            }
            self.views[idx] = view.clone();
            self.name_index.insert(view.name.clone(), idx);
        } else {
            let idx = self.views.len();
            self.id_index.insert(view.id.clone(), idx);
            self.name_index.insert(view.name.clone(), idx);
            self.views.push(view);
        }
    }

    /// Remove the view at the given in-cache index, returning the removed value.
    ///
    /// Uses `swap_remove` for O(1) removal and fixes up both indexes for the
    /// element that was swapped into the vacated slot. Shared by
    /// [`delete_view`](Self::delete_view) and
    /// [`reload_from_disk`](Self::reload_from_disk) to keep the swap-remove
    /// index-fixup logic in one place.
    fn cache_remove_at(&mut self, idx: usize) -> ViewDef {
        let removed = self.views.swap_remove(idx);
        self.id_index.remove(&removed.id);
        self.name_index.remove(&removed.name);
        if idx < self.views.len() {
            let moved = &self.views[idx];
            self.id_index.insert(moved.id.clone(), idx);
            self.name_index.insert(moved.name.clone(), idx);
        }
        removed
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

/// Compute which view fields changed between the old and new state.
///
/// Returns a list of field names that differ. When `old` is `None` (a create),
/// all fields are listed. Returns an empty vec only when both states are
/// byte-identical (a no-op write).
fn diff_view(old: Option<&ViewDef>, new: &ViewDef) -> Vec<String> {
    let Some(old) = old else {
        // Brand-new view — every field counts as changed.
        // NOTE: keep this list in sync with the `ViewDef` struct fields.
        // If a new field is added to `ViewDef`, add it here AND add a
        // comparison branch in the update diff below.
        return vec![
            "name".into(),
            "icon".into(),
            "kind".into(),
            "entity_type".into(),
            "card_fields".into(),
            "commands".into(),
        ];
    };

    let mut changed = Vec::new();
    if old.name != new.name {
        changed.push("name".into());
    }
    if old.icon != new.icon {
        changed.push("icon".into());
    }
    if old.kind != new.kind {
        changed.push("kind".into());
    }
    if old.entity_type != new.entity_type {
        changed.push("entity_type".into());
    }
    if old.card_fields != new.card_fields {
        changed.push("card_fields".into());
    }
    if old.commands != new.commands {
        changed.push("commands".into());
    }
    changed
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
    use std::sync::Arc;
    use swissarmyhammer_store::StoreHandle;
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

    /// Build a ViewsContext wired to a StoreHandle (the production path).
    async fn setup_with_store(dir: &Path) -> (ViewsContext, Arc<StoreHandle<ViewStore>>) {
        tokio::fs::create_dir_all(dir).await.unwrap();
        let store = Arc::new(ViewStore::new(dir));
        let handle = Arc::new(StoreHandle::new(store));
        let mut ctx = ViewsContext::open(dir).build().await.unwrap();
        ctx.set_store_handle(Arc::clone(&handle));
        (ctx, handle)
    }

    /// Build a ViewsContext with StoreHandle + StoreContext for undo tests.
    async fn setup_with_undo(
        dir: &Path,
    ) -> (ViewsContext, Arc<StoreHandle<ViewStore>>, Arc<StoreContext>) {
        tokio::fs::create_dir_all(dir).await.unwrap();
        let store = Arc::new(ViewStore::new(dir));
        let handle = Arc::new(StoreHandle::new(store));
        let store_context = Arc::new(StoreContext::new(dir.parent().unwrap().to_path_buf()));
        store_context.register(handle.clone()).await;
        let mut ctx = ViewsContext::open(dir).build().await.unwrap();
        ctx.set_store_handle(Arc::clone(&handle));
        ctx.set_store_context(Arc::clone(&store_context));
        (ctx, handle, store_context)
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

    #[tokio::test]
    async fn load_views_skips_non_yaml_files() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        std::fs::create_dir_all(&root).unwrap();

        std::fs::write(
            root.join("board.yaml"),
            "id: 01B\nname: Board\nkind: board\n",
        )
        .unwrap();
        std::fs::write(root.join("readme.md"), "# ignore me").unwrap();
        std::fs::write(root.join("Makefile"), "all: build").unwrap();

        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01B").is_some());
    }

    #[tokio::test]
    async fn load_views_skips_invalid_yaml_on_disk() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        std::fs::create_dir_all(&root).unwrap();

        std::fs::write(
            root.join("good.yaml"),
            "id: 01GOOD\nname: Good\nkind: board\n",
        )
        .unwrap();
        std::fs::write(root.join("bad.yaml"), "not valid: [[[").unwrap();

        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01GOOD").is_some());
    }

    #[tokio::test]
    async fn write_view_update_same_name() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        let mut updated = view.clone();
        updated.kind = ViewKind::Grid;
        ctx.write_view(&updated).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01ABC").unwrap().kind, ViewKind::Grid);
        assert!(ctx.get_by_name("Test").is_some());
    }

    #[test]
    fn get_by_id_returns_none_for_unknown() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.get_by_id("nonexistent").is_none());
    }

    #[test]
    fn get_by_name_returns_none_for_unknown() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.get_by_name("nonexistent").is_none());
    }

    #[tokio::test]
    async fn root_returns_expected_path() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let ctx = ViewsContext::open(&root).build().await.unwrap();
        assert_eq!(ctx.root(), root.as_path());
    }

    #[test]
    fn from_yaml_sources_empty_sources() {
        let ctx = ViewsContext::from_yaml_sources(PathBuf::from("/tmp/test"), &[]).unwrap();
        assert!(ctx.all_views().is_empty());
    }

    #[tokio::test]
    async fn delete_last_view_no_index_fixup() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        ctx.write_view(&make_test_view("01A", "A")).await.unwrap();
        ctx.write_view(&make_test_view("01B", "B")).await.unwrap();

        ctx.delete_view("01B").await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert!(ctx.get_by_id("01A").is_some());
        assert!(ctx.get_by_id("01B").is_none());
        assert!(ctx.get_by_name("B").is_none());
    }

    #[tokio::test]
    async fn delete_view_missing_file_still_removes_from_memory() {
        let tmp = TempDir::new().unwrap();
        let root = tmp.path().join("views");
        let mut ctx = ViewsContext::open(&root).build().await.unwrap();

        let view = make_test_view("01ABC", "Test");
        ctx.write_view(&view).await.unwrap();

        let path = root.join("01ABC.yaml");
        std::fs::remove_file(&path).unwrap();

        ctx.delete_view("01ABC").await.unwrap();
        assert!(ctx.all_views().is_empty());
    }

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

        let new_view = make_test_view("01NEW", "New View");
        ctx.write_view(&new_view).await.unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01BOARD").is_some());
        assert!(ctx.get_by_id("01NEW").is_some());
        assert!(ctx.get_by_name("New View").is_some());

        assert!(root.join("01NEW.yaml").exists());
    }

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

        let mut updated = ctx.get_by_id("01BOARD").unwrap().clone();
        updated.name = "Updated Board".into();
        ctx.write_view(&updated).await.unwrap();

        assert_eq!(ctx.all_views().len(), 1);
        assert_eq!(ctx.get_by_id("01BOARD").unwrap().name, "Updated Board");
        assert!(ctx.get_by_name("Board").is_none());
        assert!(ctx.get_by_name("Updated Board").is_some());
    }

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

        ctx.delete_view("01A").await.unwrap();

        assert_eq!(ctx.all_views().len(), 2);
        assert!(ctx.get_by_id("01A").is_none());
        assert!(ctx.get_by_name("A").is_none());
        assert!(ctx.get_by_id("01B").is_some());
        assert!(ctx.get_by_id("01C").is_some());
    }

    // =========================================================================
    // Store handle / undo stack integration tests — mirror perspectives.
    // =========================================================================

    #[tokio::test]
    async fn write_view_returns_entry_id_with_store() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("views");
        let (mut ctx, _handle, _sc) = setup_with_undo(&dir).await;

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Board View");
        let entry_id = ctx.write_view(&v).await.unwrap();

        assert!(entry_id.is_some(), "create must return an UndoEntryId");
    }

    /// `write_view` pushes the change onto the shared undo stack.
    ///
    /// Regression equivalent to
    /// `swissarmyhammer_perspectives::context::write_pushes_onto_undo_stack`.
    #[tokio::test]
    async fn write_view_pushes_onto_undo_stack() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("views");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        assert!(!sc.can_undo().await, "nothing to undo before any writes");

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Board View");
        ctx.write_view(&v).await.unwrap();

        assert!(sc.can_undo().await, "undo must be available after write");
    }

    /// `delete_view` pushes the deletion onto the shared undo stack.
    ///
    /// Mirrors `swissarmyhammer_perspectives::context::delete_pushes_onto_undo_stack`.
    #[tokio::test]
    async fn delete_view_pushes_onto_undo_stack() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("views");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Delete Me");
        ctx.write_view(&v).await.unwrap();

        assert!(sc.can_undo().await);
        let entry_id = ctx.delete_view("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert!(
            entry_id.is_some(),
            "delete with store handle must return entry ID"
        );
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        sc.undo().await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
    }

    /// Undo of a view create must remove the file, leave the cache empty, and
    /// fire a `ViewDeleted` event once `reload_from_disk` is called.
    #[tokio::test]
    async fn undo_view_create_round_trip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("views");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Ephemeral");

        let mut rx = ctx.subscribe();

        ctx.write_view(&v).await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_some());

        let create_evt = rx.try_recv().unwrap();
        match create_evt {
            ViewEvent::ViewChanged {
                ref id, is_create, ..
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(is_create);
            }
            other => panic!("expected ViewChanged is_create=true, got {other:?}"),
        }

        sc.undo().await.unwrap();
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        ctx.reload_from_disk("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap();

        assert!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none(),
            "cache must be empty after reload_from_disk on a missing file"
        );

        let undo_evt = rx
            .try_recv()
            .expect("undo of create must emit ViewDeleted via reload_from_disk");
        match undo_evt {
            ViewEvent::ViewDeleted { ref id } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            }
            other => panic!("expected ViewDeleted from undo of create, got {other:?}"),
        }
    }

    /// Undo of a view delete must restore the file, restore the cache, and
    /// fire a `ViewChanged { is_create: false }` event once `reload_from_disk`
    /// is called.
    ///
    /// Mirrors `swissarmyhammer-kanban/tests/undo_cross_cutting.rs:1031
    /// perspective_delete_undo_restores_cache_and_emits_event` at the
    /// context level.
    #[tokio::test]
    async fn undo_view_delete_round_trip() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("views");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write_view(&v).await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        let mut rx = ctx.subscribe();

        ctx.delete_view("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();
        assert!(!dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none());

        let delete_evt = rx.try_recv().unwrap();
        match delete_evt {
            ViewEvent::ViewDeleted { ref id } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            }
            other => panic!("expected ViewDeleted from delete, got {other:?}"),
        }

        sc.undo().await.unwrap();
        assert!(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml").exists());

        ctx.reload_from_disk("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap();

        let restored = ctx
            .get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .expect("cache must contain the restored view after reload_from_disk");
        assert_eq!(restored.name, "Doomed");

        let undo_evt = rx
            .try_recv()
            .expect("undo of delete must emit ViewChanged via reload_from_disk");
        match undo_evt {
            ViewEvent::ViewChanged {
                ref id, is_create, ..
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(
                    !is_create,
                    "undo of delete emits is_create=false — the view was \
                     previously created, not re-created"
                );
            }
            other => panic!("expected ViewChanged from undo of delete, got {other:?}"),
        }
    }

    // =========================================================================
    // Broadcast event tests
    // =========================================================================

    #[tokio::test]
    async fn write_emits_view_changed_event_on_create() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ViewsContext::open(tmp.path().join("views"))
            .build()
            .await
            .unwrap();
        let mut rx = ctx.subscribe();

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "New View");
        ctx.write_view(&v).await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            ViewEvent::ViewChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(is_create, "first write must be flagged as create");
                assert!(changed_fields.contains(&"name".to_string()));
                assert!(changed_fields.contains(&"kind".to_string()));
            }
            other => panic!("expected ViewChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_emits_view_changed_with_correct_diff() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ViewsContext::open(tmp.path().join("views"))
            .build()
            .await
            .unwrap();

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Original");
        ctx.write_view(&v).await.unwrap();

        let mut rx = ctx.subscribe();

        let mut updated = v.clone();
        updated.kind = ViewKind::Grid;
        ctx.write_view(&updated).await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            ViewEvent::ViewChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(!is_create, "update must not be flagged as create");
                assert_eq!(changed_fields, vec!["kind".to_string()]);
            }
            other => panic!("expected ViewChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn delete_emits_view_deleted_event() {
        let tmp = TempDir::new().unwrap();
        let (mut ctx, _handle) = setup_with_store(&tmp.path().join("views")).await;

        let v = make_test_view("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write_view(&v).await.unwrap();

        let mut rx = ctx.subscribe();

        ctx.delete_view("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            ViewEvent::ViewDeleted { id } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            }
            other => panic!("expected ViewDeleted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn reload_from_disk_noop_when_absent_and_uncached() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = ViewsContext::open(tmp.path().join("views"))
            .build()
            .await
            .unwrap();
        let mut rx = ctx.subscribe();

        ctx.reload_from_disk("01NONEXISTENT").await.unwrap();
        assert!(
            rx.try_recv().is_err(),
            "no-op reload must not emit an event"
        );
    }
}
