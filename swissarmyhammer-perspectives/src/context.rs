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
use tokio::sync::broadcast;
use tracing::debug;

use crate::error::{PerspectiveError, Result};
use crate::events::PerspectiveEvent;
use crate::store::PerspectiveStore;
use crate::types::Perspective;
use crate::PerspectiveId;
use swissarmyhammer_store::{StoreContext, StoreHandle, StoredItemId, UndoEntryId};

/// Default capacity for the perspective event broadcast channel.
///
/// Sized smaller than the entity cache channel (256) because perspective
/// mutations are infrequent — a handful per session, not hundreds per second.
const EVENT_CHANNEL_CAPACITY: usize = 64;

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
    /// Broadcast channel for perspective change events.
    ///
    /// Consumers (e.g. the Tauri bridge) subscribe to this channel to learn
    /// about perspective mutations without coupling to the perspectives crate.
    event_sender: broadcast::Sender<PerspectiveEvent>,
}

impl PerspectiveContext {
    /// Open a perspectives directory, loading all YAML files into memory.
    ///
    /// Creates the directory if it does not exist. Invalid YAML files are
    /// logged and skipped.
    pub async fn open(dir: impl Into<PathBuf>) -> Result<Self> {
        let root = dir.into();
        fs::create_dir_all(&root).await?;

        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut ctx = Self {
            root,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
            event_sender,
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
        // Snapshot the old state for diff computation.
        let old = self.get_by_id(&perspective.id).cloned();

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
            let is_create = old.is_none();
            let op = if is_create { "create" } else { "update" };
            let label = format!("{} perspective {}", op, perspective.id);
            let item_id = StoredItemId::from(perspective.id.as_str());
            sc.push(*eid, label, item_id).await;
        }

        // Update in-memory cache.
        self.cache_upsert(perspective.clone());

        // Broadcast the change event. Compute the field-level diff so
        // consumers know which fields actually changed.
        let is_create = old.is_none();
        let changed_fields = diff_perspective(old.as_ref(), perspective);
        if !changed_fields.is_empty() {
            let _ = self
                .event_sender
                .send(PerspectiveEvent::PerspectiveChanged {
                    id: perspective.id.clone(),
                    changed_fields,
                    is_create,
                });
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

    /// Rename a perspective atomically.
    ///
    /// Looks up the perspective by ID, changes its name, and writes it back
    /// in a single operation. This avoids the non-atomic delete + create pattern.
    ///
    /// Returns the updated perspective on success.
    pub async fn rename(&mut self, id: &str, new_name: impl Into<String>) -> Result<Perspective> {
        let idx = self
            .id_index
            .get(id)
            .copied()
            .ok_or_else(|| PerspectiveError::NotFound {
                resource: "perspective".to_string(),
                id: id.to_string(),
            })?;

        let mut updated = self.perspectives[idx].clone();
        updated.name = new_name.into();
        self.write(&updated).await?;
        Ok(updated)
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
        let deleted = self.cache_remove_at(idx);

        // Broadcast the deletion event.
        let _ = self
            .event_sender
            .send(PerspectiveEvent::PerspectiveDeleted {
                id: deleted.id.clone(),
            });

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

    /// Subscribe to perspective change events.
    ///
    /// Returns a receiver that will get all events emitted after this call.
    /// Missed events (due to slow consumption) result in `RecvError::Lagged`.
    pub fn subscribe(&self) -> broadcast::Receiver<PerspectiveEvent> {
        self.event_sender.subscribe()
    }

    /// The root directory path.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Refresh a single perspective's in-memory entry from disk.
    ///
    /// Used by post-undo / post-redo reconciliation after the store layer
    /// has rewritten the on-disk YAML without going through [`write`](Self::write).
    /// Mirrors the `EntityContext::sync_entity_cache_from_disk` contract
    /// at the entity layer:
    ///
    /// - If the file exists and parses, replace the cached entry and emit
    ///   [`PerspectiveEvent::PerspectiveChanged`] so downstream subscribers
    ///   (Tauri bridge, frontend refresh) react. The `changed_fields` list
    ///   is left empty to signal "unspecified — full refresh may be needed"
    ///   because the pre-undo state in memory may have already been
    ///   overwritten by the disk rewrite, so a meaningful field diff is
    ///   not reliably computable here.
    /// - If the file is absent (undo of a create, redo of a delete), evict
    ///   the cached entry and emit [`PerspectiveEvent::PerspectiveDeleted`].
    /// - If the file is absent and the cache also has no entry, this is a
    ///   no-op — nothing to reconcile and no event is emitted.
    ///
    /// Parse failures on an existing file return an error. In-memory cache
    /// state is not mutated when parsing fails.
    pub async fn reload_from_disk(&mut self, id: &str) -> Result<()> {
        let path = self.perspective_path(id);
        if path.exists() {
            let content = fs::read_to_string(&path).await?;
            let perspective: Perspective = serde_yaml_ng::from_str(&content)?;
            self.cache_upsert(perspective.clone());
            let _ = self
                .event_sender
                .send(PerspectiveEvent::PerspectiveChanged {
                    id: perspective.id,
                    // Empty list signals "unspecified — consumers should
                    // treat this as a full refresh."
                    changed_fields: Vec::new(),
                    is_create: false,
                });
        } else if let Some(&idx) = self.id_index.get(id) {
            let _deleted = self.cache_remove_at(idx);
            let _ = self
                .event_sender
                .send(PerspectiveEvent::PerspectiveDeleted { id: id.to_string() });
        }
        Ok(())
    }

    // --- Internal ---

    /// Path to a perspective's YAML file.
    fn perspective_path(&self, id: &str) -> PathBuf {
        self.root.join(format!("{id}.yaml"))
    }

    /// Insert or replace a perspective in the in-memory cache.
    ///
    /// When the id is already known, overwrites the existing slot. When it
    /// is new, appends and records the index. Shared by [`write`](Self::write)
    /// and [`reload_from_disk`](Self::reload_from_disk) to keep the replace /
    /// append logic in one place.
    fn cache_upsert(&mut self, perspective: Perspective) {
        if let Some(&idx) = self.id_index.get(&perspective.id) {
            self.perspectives[idx] = perspective;
        } else {
            let idx = self.perspectives.len();
            self.id_index.insert(perspective.id.clone(), idx);
            self.perspectives.push(perspective);
        }
    }

    /// Remove the perspective at the given in-cache index, returning the
    /// removed value.
    ///
    /// Uses `swap_remove` for O(1) removal and fixes up the id-index of the
    /// element that was swapped into the vacated slot. Shared by
    /// [`delete`](Self::delete) and [`reload_from_disk`](Self::reload_from_disk)
    /// to keep the swap-remove index-fixup logic in one place.
    fn cache_remove_at(&mut self, idx: usize) -> Perspective {
        let removed = self.perspectives.swap_remove(idx);
        self.id_index.remove(&removed.id);
        if idx < self.perspectives.len() {
            let moved = &self.perspectives[idx];
            self.id_index.insert(moved.id.clone(), idx);
        }
        removed
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

/// Compute which perspective fields changed between the old and new state.
///
/// Returns a list of field names that differ. When `old` is `None` (a create),
/// all fields are listed. Returns an empty vec only when both states are
/// byte-identical (a no-op write).
fn diff_perspective(old: Option<&Perspective>, new: &Perspective) -> Vec<String> {
    let Some(old) = old else {
        // Brand-new perspective — every field counts as changed.
        // NOTE: keep this list in sync with the `Perspective` struct fields.
        // If a new field is added to `Perspective`, add it here AND add a
        // comparison branch in the update diff below.
        return vec![
            "name".into(),
            "view".into(),
            "fields".into(),
            "filter".into(),
            "group".into(),
            "sort".into(),
        ];
    };

    let mut changed = Vec::new();
    if old.name != new.name {
        changed.push("name".into());
    }
    if old.view != new.view {
        changed.push("view".into());
    }
    if old.fields != new.fields {
        changed.push("fields".into());
    }
    if old.filter != new.filter {
        changed.push("filter".into());
    }
    if old.group != new.group {
        changed.push("group".into());
    }
    if old.sort != new.sort {
        changed.push("sort".into());
    }
    changed
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
        let (event_sender, _) = broadcast::channel(EVENT_CHANNEL_CAPACITY);
        let mut ctx = PerspectiveContext {
            root: dir,
            perspectives: Vec::new(),
            id_index: HashMap::new(),
            store_handle: None,
            store_context: OnceLock::new(),
            event_sender,
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
        assert_eq!(events[0].event_name(), "item-created");
        assert_eq!(events[0].payload()["store"], "perspective");
        assert_eq!(events[0].payload()["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
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
        assert_eq!(events[0].event_name(), "item-changed");
        assert_eq!(events[0].payload()["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
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
        assert_eq!(events[0].event_name(), "item-removed");
        assert_eq!(events[0].payload()["id"], "01AAAAAAAAAAAAAAAAAAAAAAAA");
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

    // =========================================================================
    // Rename tests
    // =========================================================================

    #[tokio::test]
    async fn rename_updates_name_atomically() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle) = setup_with_store(&dir).await;

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Before"))
            .await
            .unwrap();

        let updated = ctx
            .rename("01AAAAAAAAAAAAAAAAAAAAAAAA", "After")
            .await
            .unwrap();
        assert_eq!(updated.name, "After");
        assert_eq!(updated.id, "01AAAAAAAAAAAAAAAAAAAAAAAA");

        // In-memory cache should reflect the rename
        assert!(ctx.get_by_name("Before").is_none());
        assert!(ctx.get_by_name("After").is_some());
        assert_eq!(ctx.all().len(), 1);
    }

    #[tokio::test]
    async fn rename_nonexistent_returns_not_found() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle) = setup_with_store(&dir).await;

        let err = ctx.rename("01ZZZZZZZZZZZZZZZZZZZZZZZZ", "New").await;
        assert!(err.is_err());
    }

    // =========================================================================
    // Broadcast event tests
    // =========================================================================

    #[tokio::test]
    async fn write_emits_perspective_changed_event_on_create() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();
        let mut rx = ctx.subscribe();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "New View");
        ctx.write(&p).await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            PerspectiveEvent::PerspectiveChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(is_create, "first write must be flagged as create");
                // Create emits all fields
                assert!(changed_fields.contains(&"name".to_string()));
                assert!(changed_fields.contains(&"view".to_string()));
                assert!(changed_fields.contains(&"filter".to_string()));
            }
            other => panic!("expected PerspectiveChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_emits_perspective_changed_with_correct_diff() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Original");
        ctx.write(&p).await.unwrap();

        // Subscribe AFTER the create so we only see the update event
        let mut rx = ctx.subscribe();

        let mut updated = p.clone();
        updated.filter = Some("#bug".to_string());
        ctx.write(&updated).await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            PerspectiveEvent::PerspectiveChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(!is_create, "update must not be flagged as create");
                assert_eq!(changed_fields, vec!["filter".to_string()]);
            }
            other => panic!("expected PerspectiveChanged, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn write_does_not_emit_event_on_noop() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Stable");
        ctx.write(&p).await.unwrap();

        let mut rx = ctx.subscribe();

        // Write the exact same perspective — no fields changed
        ctx.write(&p).await.unwrap();

        assert!(rx.try_recv().is_err(), "no-op write must not emit an event");
    }

    #[tokio::test]
    async fn delete_emits_perspective_deleted_event() {
        let tmp = TempDir::new().unwrap();
        let mut ctx = PerspectiveContext::open(tmp.path().join("perspectives"))
            .await
            .unwrap();

        let p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Doomed");
        ctx.write(&p).await.unwrap();

        let mut rx = ctx.subscribe();

        ctx.delete("01AAAAAAAAAAAAAAAAAAAAAAAA").await.unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            PerspectiveEvent::PerspectiveDeleted { id } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            }
            other => panic!("expected PerspectiveDeleted, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rename_emits_perspective_changed_with_name_field() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle) = setup_with_store(&dir).await;

        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Before"))
            .await
            .unwrap();

        let mut rx = ctx.subscribe();

        ctx.rename("01AAAAAAAAAAAAAAAAAAAAAAAA", "After")
            .await
            .unwrap();

        let evt = rx.try_recv().unwrap();
        match evt {
            PerspectiveEvent::PerspectiveChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(!is_create, "rename is an update, not a create");
                assert_eq!(changed_fields, vec!["name".to_string()]);
            }
            other => panic!("expected PerspectiveChanged, got {other:?}"),
        }
    }

    // =========================================================================
    // reload_from_disk tests — post-undo/redo cache reconciliation
    // =========================================================================

    /// After an external rewrite of a perspective's YAML (simulating what
    /// `PerspectiveStore::undo_erased` does during an undo), `reload_from_disk`
    /// must:
    ///   1. Replace the in-memory cache entry with the on-disk state.
    ///   2. Emit a `PerspectiveChanged` event so downstream subscribers
    ///      (e.g. the Tauri bridge) can forward a refresh to the frontend.
    #[tokio::test]
    async fn reload_from_disk_syncs_cache_and_emits_event_on_file_change() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let mut ctx = PerspectiveContext::open(&dir).await.unwrap();

        // Seed the cache with a perspective that has no group set.
        let mut seeded = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint");
        seeded.group = None;
        ctx.write(&seeded).await.unwrap();

        // Rewrite the YAML on disk directly — bypasses write() so the cache
        // is now stale. This simulates what undo does when it reverses a
        // previously-persisted state.
        let mut rewritten = seeded.clone();
        rewritten.group = Some("status".to_string());
        let yaml = serde_yaml_ng::to_string(&rewritten).unwrap();
        fs::write(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml"), yaml.as_bytes())
            .await
            .unwrap();

        // Cache still shows the stale state.
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().group,
            None,
            "cache must be stale before reload"
        );

        let mut rx = ctx.subscribe();

        // Reload — cache must pick up the on-disk state.
        ctx.reload_from_disk("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap();

        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA")
                .unwrap()
                .group
                .as_deref(),
            Some("status"),
            "cache must be refreshed to match disk after reload"
        );

        // Event must have fired with is_create=false.
        let evt = rx.try_recv().expect("reload must emit an event");
        match evt {
            PerspectiveEvent::PerspectiveChanged {
                id,
                changed_fields,
                is_create,
            } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
                assert!(
                    !is_create,
                    "reload is never a create — the file already existed"
                );
                assert!(
                    changed_fields.is_empty(),
                    "reload emits empty changed_fields as the full-refresh marker"
                );
            }
            other => panic!("expected PerspectiveChanged, got {other:?}"),
        }
    }

    /// When the YAML file is deleted externally (simulating what
    /// `PerspectiveStore::undo_erased` does when undoing a create),
    /// `reload_from_disk` must:
    ///   1. Evict the in-memory cache entry.
    ///   2. Emit a `PerspectiveDeleted` event.
    #[tokio::test]
    async fn reload_from_disk_evicts_cache_and_emits_deleted_on_file_absence() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let mut ctx = PerspectiveContext::open(&dir).await.unwrap();

        // Seed a perspective.
        ctx.write(&make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Ghost"))
            .await
            .unwrap();
        assert!(ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_some());

        // Remove the file behind the cache's back.
        fs::remove_file(dir.join("01AAAAAAAAAAAAAAAAAAAAAAAA.yaml"))
            .await
            .unwrap();

        let mut rx = ctx.subscribe();

        ctx.reload_from_disk("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap();

        assert!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").is_none(),
            "cache entry must be evicted after reload when file is gone"
        );

        let evt = rx.try_recv().expect("reload must emit an event");
        match evt {
            PerspectiveEvent::PerspectiveDeleted { id } => {
                assert_eq!(id, "01AAAAAAAAAAAAAAAAAAAAAAAA");
            }
            other => panic!("expected PerspectiveDeleted, got {other:?}"),
        }
    }

    /// When both the cache and disk have no entry for the id, `reload_from_disk`
    /// is a no-op: no cache mutation, no event emitted. Prevents spurious
    /// deleted events for ids that were never present.
    #[tokio::test]
    async fn reload_from_disk_is_noop_when_file_and_cache_both_absent() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let mut ctx = PerspectiveContext::open(&dir).await.unwrap();

        let mut rx = ctx.subscribe();

        // id is unknown to both cache and disk — must be a no-op.
        ctx.reload_from_disk("01ZZZZZZZZZZZZZZZZZZZZZZZZ")
            .await
            .unwrap();

        assert!(
            rx.try_recv().is_err(),
            "reload must not emit an event when there is nothing to reconcile"
        );
        assert!(ctx.all().is_empty(), "cache must remain empty");
    }

    /// End-to-end undo reconciliation: write a perspective, mutate it, then
    /// roll back on disk via the store's undo path and call `reload_from_disk`.
    /// The cache must see the pre-mutation state afterwards.
    #[tokio::test]
    async fn reload_from_disk_reflects_store_undo() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("perspectives");
        let (mut ctx, _handle, sc) = setup_with_undo(&dir).await;

        // Write the initial state.
        let mut p = make_perspective("01AAAAAAAAAAAAAAAAAAAAAAAA", "Sprint");
        p.group = None;
        ctx.write(&p).await.unwrap();

        // Mutate — add a group.
        p.group = Some("status".to_string());
        ctx.write(&p).await.unwrap();

        // Confirm the cache reflects the mutation.
        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA")
                .unwrap()
                .group
                .as_deref(),
            Some("status"),
            "cache must reflect the second write before undo"
        );

        // Undo via the store layer — this rewrites the YAML to the pre-mutation
        // state but does not touch the in-memory cache.
        sc.undo().await.unwrap();

        // Cache is now stale: it still has group=Some("status") while disk
        // has group=None. reload_from_disk should fix the cache.
        ctx.reload_from_disk("01AAAAAAAAAAAAAAAAAAAAAAAA")
            .await
            .unwrap();

        assert_eq!(
            ctx.get_by_id("01AAAAAAAAAAAAAAAAAAAAAAAA").unwrap().group,
            None,
            "cache must reflect the post-undo on-disk state after reload"
        );
    }
}
