//! Central coordinator for multiple stores with shared undo/redo.
//!
//! The [`StoreContext`] holds an [`UndoStack`] and a collection of
//! [`ErasedStore`] instances. It dispatches undo/redo to the correct store
//! and aggregates change events from all stores.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use tokio::sync::RwLock;
use tokio::task;
use tracing;

use crate::changelog::ChangelogEntry;
use crate::erased::ErasedStore;
use crate::error::{Result, StoreError};
use crate::event::ChangeEvent;
use crate::id::{StoredItemId, UndoEntryId};
use crate::stack::UndoStack;

/// Key used to scope ambient transaction state by tokio task.
///
/// Wraps `Option<task::Id>` so non-tokio callers (sync code, or a test
/// not running inside `tokio::test`) share a single fallback slot
/// rather than panicking. Inside an async task the variant carries the
/// real task id, and concurrent tasks get distinct keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum AmbientKey {
    Task(task::Id),
    NoTask,
}

impl AmbientKey {
    /// Construct the key for the calling site.
    ///
    /// Uses `tokio::task::try_id()` so a non-async caller falls back to
    /// the [`AmbientKey::NoTask`] variant instead of panicking. Inside
    /// `#[tokio::test]` and any spawned task this returns
    /// [`AmbientKey::Task`] with the real task id.
    fn current() -> Self {
        match task::try_id() {
            Some(id) => AmbientKey::Task(id),
            None => AmbientKey::NoTask,
        }
    }
}

/// The concrete target an undo or redo touched.
///
/// Returned from [`StoreContext::undo`] / [`StoreContext::redo`] so callers
/// that maintain caches parallel to the on-disk store (e.g. the entity-layer
/// cache) can synchronize the single affected item without re-scanning the
/// whole store.
///
/// `store_name` is the underlying store's human-readable name (e.g.
/// `"task"`, `"tag"`) as returned by [`ErasedStore::store_name`].
/// `item_id` is the identifier of the item whose changelog was rewound /
/// replayed.
#[derive(Debug, Clone)]
pub struct UndoOutcome {
    /// Name of the store that owned the entry (e.g. `"task"`, `"tag"`).
    ///
    /// For a multi-entry undo group, this is the store of the last entry
    /// processed (kept as a single field for backward compatibility with
    /// existing single-entry consumers). Every store the group touched is
    /// enumerated in [`items`].
    pub store_name: String,
    /// Identifier of the item whose state was reversed or reapplied.
    ///
    /// For a multi-entry undo group, this is the last item processed; full
    /// per-item details are in [`items`].
    pub item_id: StoredItemId,
    /// Every (store_name, item_id) pair affected by this undo/redo call.
    ///
    /// For a single-entry undo this contains exactly one pair matching
    /// `(store_name, item_id)`. For a group undo it contains every entry
    /// that was reversed/reapplied, in processing order. Callers that
    /// maintain caches mirroring on-disk state must iterate this list so
    /// every touched item is resynced — not just the representative.
    pub items: Vec<(String, StoredItemId)>,
}

/// Central coordinator for multiple file-backed stores.
///
/// Manages a shared undo/redo stack and dispatches operations to the
/// correct store based on changelog entry ownership.
///
/// # Substrate invariant
///
/// **There is exactly one `Arc<StoreContext>` per app, and therefore one
/// `undo_stack.yaml` per board.** Every [`TrackedStore`](crate::TrackedStore)
/// (entity-type stores, the perspective store, the view store, …) must
/// register into the *same* `StoreContext` via `Arc::clone` of the one the
/// app constructed at board-open time. Sharing the `Arc` is how undo/redo
/// reverts across heterogeneous stores on a single LIFO stack.
///
/// Never construct a second `StoreContext` for the same board — that would
/// fork the undo stack: writes to one set of stores would land on one
/// stack, writes to the other on a second stack, and an `undo` would
/// silently revert only the half the caller happened to dispatch to.
///
/// In the kanban app this invariant is set up in `BoardHandle::open`
/// (`apps/kanban-app/src/state.rs`) and pinned by the substrate guard test
/// at `apps/kanban-app/tests/substrate_guard.rs`, which `Arc::ptr_eq`-
/// compares the context each subsystem holds against the one the board
/// owns. If anything splits the substrate, that test fails loudly.
pub struct StoreContext {
    stack: RwLock<UndoStack>,
    stores: RwLock<Vec<Arc<dyn ErasedStore>>>,
    root: PathBuf,
    /// Per-task ambient transaction slots.
    ///
    /// Each entry maps a tokio task id to the active `UndoEntryId` that
    /// every `push` from that task should stamp onto its undo-stack
    /// entries. Different tokio tasks running concurrent transactions
    /// each have their own slot, so they cannot interfere — the
    /// kanban-app's command pipeline pins its transaction to the task
    /// that opened it.
    ///
    /// The slot is set by [`begin_undo_group`] / [`with_transaction`]
    /// and cleared by the returned guard or `with_transaction`'s
    /// scope-end. When no slot exists for the current task, `push`
    /// records the entry without a group id (the legacy per-write
    /// behavior).
    ///
    /// `Mutex` is the right primitive here — every mutation is short
    /// and synchronous (HashMap insert / remove); we never hold the
    /// lock across an `.await`.
    ambient_txn: Mutex<HashMap<AmbientKey, UndoEntryId>>,
}

/// RAII guard that ends the active undo group when dropped.
///
/// Returned by [`StoreContext::begin_undo_group`]. Dropping the guard
/// clears the per-task ambient transaction id so subsequent writes
/// from that task are pushed as independent undo entries again.
pub struct UndoGroupGuard<'a> {
    ctx: &'a StoreContext,
    /// The ambient-slot key this guard ends. Captured at
    /// `begin_undo_group` time so a guard dropped from a different task
    /// still clears the slot it actually set.
    owner: Option<AmbientKey>,
}

impl<'a> UndoGroupGuard<'a> {
    /// Explicitly end the group. Equivalent to dropping the guard.
    pub async fn end(mut self) {
        if let Some(id) = self.owner.take() {
            self.ctx.clear_ambient_for(id);
        }
        std::mem::forget(self);
    }
}

impl<'a> Drop for UndoGroupGuard<'a> {
    fn drop(&mut self) {
        if let Some(id) = self.owner.take() {
            self.ctx.clear_ambient_for(id);
        }
    }
}

impl StoreContext {
    /// Create a new `StoreContext` rooted at the given directory.
    ///
    /// Loads an existing `undo_stack.yaml` from the root if present.
    pub fn new(root: PathBuf) -> Self {
        let stack_path = root.join("undo_stack.yaml");
        let stack = match UndoStack::load(&stack_path) {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!(path = %stack_path.display(), error = %e, "failed to load undo stack, using default");
                UndoStack::default()
            }
        };
        Self {
            stack: RwLock::new(stack),
            stores: RwLock::new(Vec::new()),
            root,
            ambient_txn: Mutex::new(HashMap::new()),
        }
    }

    /// Begin a multi-write undo group bound to the current tokio task.
    ///
    /// Every `push` call from this task — between this point and the
    /// returned guard going out of scope (or `.end()` being called) —
    /// is stamped with a shared `group_id`. A single `undo()` then
    /// reverses every entry in the group as one step.
    ///
    /// Calling this while a group is already open on the same task
    /// returns a guard that reuses the current group id; the prior
    /// (outer) guard remains responsible for clearing the slot, so
    /// nested calls do not create sub-groups.
    ///
    /// Different tokio tasks each get their own ambient slot, so two
    /// transactions opened concurrently from different tasks do not
    /// interfere — each task's `push` reads only its own task's slot.
    pub async fn begin_undo_group(&self) -> UndoGroupGuard<'_> {
        let key = AmbientKey::current();
        let mut slots = self.ambient_txn.lock().expect("ambient_txn poisoned");
        let already_open = slots.contains_key(&key);
        if !already_open {
            slots.insert(key, UndoEntryId::new());
        }
        UndoGroupGuard {
            ctx: self,
            // Only the outer call owns clearing the slot; nested calls
            // leave `owner` as `None` so their guard drop is a no-op.
            owner: if already_open { None } else { Some(key) },
        }
    }

    /// Begin a transaction and return the freshly allocated group id.
    ///
    /// This is the public entry point used by the `store` MCP server's
    /// `BeginTransaction` verb. The ambient slot for the current task
    /// is set to the returned id, so every subsequent `push` from this
    /// task (until [`end_transaction`] or task exit) is stamped with
    /// it. Like [`begin_undo_group`], the slot is per-task — concurrent
    /// transactions opened from different tokio tasks do not interfere.
    ///
    /// Unlike [`begin_undo_group`], this does not return an RAII guard
    /// — the caller is responsible for invoking [`end_transaction`]
    /// with the returned id when the transaction is finished. This
    /// shape mirrors the MCP wire protocol, where `BeginTransaction`
    /// and `EndTransaction` are two separate calls.
    ///
    /// Calling this while a group is already open on the same task
    /// returns the existing id and does not allocate a new one, so
    /// nested calls do not create sub-groups.
    pub fn begin_transaction(&self) -> UndoEntryId {
        let key = AmbientKey::current();
        let mut slots = self.ambient_txn.lock().expect("ambient_txn poisoned");
        *slots.entry(key).or_insert_with(UndoEntryId::new)
    }

    /// End the transaction with the given id on the current task.
    ///
    /// Clears the ambient slot only when the id matches the slot's
    /// current value — guarding against a stale end on a recycled
    /// task id or a confused caller. The MCP `EndTransaction` verb
    /// dispatches here; legacy `begin_undo_group` users do not need
    /// this method because their guard handles cleanup.
    pub fn end_transaction(&self, id: UndoEntryId) {
        let key = AmbientKey::current();
        let mut slots = self.ambient_txn.lock().expect("ambient_txn poisoned");
        if matches!(slots.get(&key), Some(current) if *current == id) {
            slots.remove(&key);
        }
    }

    /// Clear the active undo group for the current task.
    ///
    /// Kept for backwards compatibility with callers that used to pair
    /// [`begin_undo_group`] with an explicit `end_undo_group()`.
    /// Equivalent to dropping the guard returned by `begin_undo_group`.
    pub async fn end_undo_group(&self) {
        self.clear_ambient_for(AmbientKey::current());
    }

    /// Drop the ambient slot for a specific key.
    ///
    /// Internal helper used by both [`end_undo_group`] and
    /// [`UndoGroupGuard::Drop`].
    fn clear_ambient_for(&self, key: AmbientKey) {
        if let Ok(mut slots) = self.ambient_txn.lock() {
            slots.remove(&key);
        }
    }

    /// Return the active ambient transaction id for the current task,
    /// if any. Exposed primarily for tests that need to probe the slot;
    /// production code reads it through [`push`].
    pub fn current_transaction(&self) -> Option<UndoEntryId> {
        let key = AmbientKey::current();
        self.ambient_txn
            .lock()
            .expect("ambient_txn poisoned")
            .get(&key)
            .copied()
    }

    /// Register a store with this context.
    pub async fn register(&self, store: Arc<dyn ErasedStore>) {
        self.stores.write().await.push(store);
    }

    /// Push an entry onto the undo stack and persist to disk.
    ///
    /// The `item_id` records which item's per-item changelog contains this
    /// entry, so that undo/redo can look it up without scanning all files.
    ///
    /// If a transaction is currently open on the calling tokio task via
    /// [`begin_undo_group`] or [`begin_transaction`], the entry is stamped
    /// with that group id so it will be undone/redone together with its
    /// siblings.
    pub async fn push(&self, id: UndoEntryId, label: String, item_id: StoredItemId) {
        let group_id = self.current_transaction();
        let mut stack = self.stack.write().await;
        stack.push_with_group(id, label, item_id, group_id);
        if let Err(e) = stack.save(&self.root.join("undo_stack.yaml")) {
            tracing::warn!(error = %e, "failed to save undo stack");
        }
    }

    /// Undo the most recent operation.
    ///
    /// Finds the store that owns the undo target entry and dispatches
    /// the undo to it. Updates the stack pointer and persists.
    /// Minimizes the scope of the stores read lock by cloning the matching
    /// `Arc<dyn ErasedStore>` before awaiting the undo operation.
    ///
    /// Returns an [`UndoOutcome`] identifying the store and item whose
    /// on-disk state was reversed, so the caller can reconcile any
    /// higher-level caches it maintains over those files.
    pub async fn undo(&self) -> Result<UndoOutcome> {
        // Snapshot the full set of entries to undo as one group. The group
        // is `[start, end)` on the stack; we reverse them in reverse-push
        // order so each store sees the file state the entry was diffed
        // against.
        let group_entries: Vec<(UndoEntryId, StoredItemId)> = {
            let stack = self.stack.read().await;
            let range = stack
                .group_undo_range()
                .ok_or_else(|| StoreError::NotFound("nothing to undo".into()))?;
            stack.entries()[range]
                .iter()
                .map(|e| (e.id, e.item_id.clone()))
                .collect()
        };

        let stores_snapshot: Vec<Arc<dyn ErasedStore>> = {
            let stores = self.stores.read().await;
            stores.iter().map(Arc::clone).collect()
        };

        // Reverse the group from newest to oldest so each store sees the
        // disk state the entry was diffed against when it was written.
        let mut items: Vec<(String, StoredItemId)> = Vec::with_capacity(group_entries.len());
        for (target_id, item_id) in group_entries.iter().rev() {
            let mut owning = None;
            for s in &stores_snapshot {
                if s.has_entry(target_id, item_id).await {
                    owning = Some(Arc::clone(s));
                    break;
                }
            }
            let Some(store) = owning else {
                return Err(StoreError::NoProvider(target_id.to_string()));
            };
            store.undo_erased(target_id, item_id).await?;
            items.push((store.store_name().to_string(), item_id.clone()));
        }

        let popped = group_entries.len();
        let mut stack = self.stack.write().await;
        stack.record_undo_n(popped);
        if let Err(e) = stack.save(&self.root.join("undo_stack.yaml")) {
            tracing::warn!(error = %e, "failed to save undo stack");
        }

        // `items` is non-empty here — `group_undo_range` returned `Some`.
        let (store_name, item_id) = items.last().cloned().expect("at least one entry processed");
        Ok(UndoOutcome {
            store_name,
            item_id,
            items,
        })
    }

    /// Redo the most recently undone operation.
    ///
    /// Finds the store that owns the redo target entry and dispatches
    /// the redo to it. Updates the stack pointer and persists.
    /// Minimizes the scope of the stores read lock by cloning the matching
    /// `Arc<dyn ErasedStore>` before awaiting the redo operation.
    ///
    /// Returns an [`UndoOutcome`] identifying the store and item whose
    /// on-disk state was reapplied, so the caller can reconcile any
    /// higher-level caches it maintains over those files.
    pub async fn redo(&self) -> Result<UndoOutcome> {
        let group_entries: Vec<(UndoEntryId, StoredItemId)> = {
            let stack = self.stack.read().await;
            let range = stack
                .group_redo_range()
                .ok_or_else(|| StoreError::NotFound("nothing to redo".into()))?;
            stack.entries()[range]
                .iter()
                .map(|e| (e.id, e.item_id.clone()))
                .collect()
        };

        let stores_snapshot: Vec<Arc<dyn ErasedStore>> = {
            let stores = self.stores.read().await;
            stores.iter().map(Arc::clone).collect()
        };

        // Reapply in original push order so the disk state matches what
        // the command produced the first time.
        let mut items: Vec<(String, StoredItemId)> = Vec::with_capacity(group_entries.len());
        for (target_id, item_id) in group_entries.iter() {
            let mut owning = None;
            for s in &stores_snapshot {
                if s.has_entry(target_id, item_id).await {
                    owning = Some(Arc::clone(s));
                    break;
                }
            }
            let Some(store) = owning else {
                return Err(StoreError::NoProvider(target_id.to_string()));
            };
            store.redo_erased(target_id, item_id).await?;
            items.push((store.store_name().to_string(), item_id.clone()));
        }

        let pushed = group_entries.len();
        let mut stack = self.stack.write().await;
        stack.record_redo_n(pushed);
        if let Err(e) = stack.save(&self.root.join("undo_stack.yaml")) {
            tracing::warn!(error = %e, "failed to save undo stack");
        }

        let (store_name, item_id) = items.last().cloned().expect("at least one entry processed");
        Ok(UndoOutcome {
            store_name,
            item_id,
            items,
        })
    }

    /// Whether there is an operation that can be undone.
    pub async fn can_undo(&self) -> bool {
        self.stack.read().await.can_undo()
    }

    /// Whether there is an operation that can be redone.
    pub async fn can_redo(&self) -> bool {
        self.stack.read().await.can_redo()
    }

    /// Number of entries currently available to undo.
    ///
    /// Equivalent to counting how many successful `undo()` calls could be
    /// performed right now without error. Exposed primarily for tests that
    /// need a cheap read-only probe of stack depth — driving real
    /// `undo`/`redo` round-trips to measure depth is correct but fragile
    /// (if any probed `redo` fails the stack is left inconsistent).
    pub async fn undo_depth(&self) -> usize {
        self.stack.read().await.pointer()
    }

    /// Flush changes from all registered stores and aggregate events.
    pub async fn flush_all(&self) -> Vec<ChangeEvent> {
        let store_clones: Vec<Arc<dyn ErasedStore>> = {
            let stores = self.stores.read().await;
            stores.iter().map(Arc::clone).collect()
        };
        let mut all_events = Vec::new();
        for store in &store_clones {
            let events = store.flush_changes().await;
            all_events.extend(events);
        }
        all_events
    }

    /// Return the root paths of all registered stores.
    ///
    /// Used by the file watcher to discover which directories to watch without
    /// hardcoding a fixed list of subdirectory names.
    pub async fn watched_roots(&self) -> Vec<PathBuf> {
        let stores = self.stores.read().await;
        stores.iter().map(|s| s.root().to_path_buf()).collect()
    }

    /// Find the store whose root is a prefix of the given path.
    pub async fn store_for_path(&self, path: &Path) -> Option<Arc<dyn ErasedStore>> {
        let stores = self.stores.read().await;
        for store in stores.iter() {
            if path.starts_with(store.root()) {
                return Some(Arc::clone(store));
            }
        }
        None
    }

    /// Find a registered store by its human-readable name.
    ///
    /// Returns `None` when no registered store reports the given name from
    /// [`ErasedStore::store_name`]. Used by the `store` MCP server's
    /// store-scoped verbs (`History`, `GetItem`) to dispatch by name.
    pub async fn store_by_name(&self, name: &str) -> Option<Arc<dyn ErasedStore>> {
        let stores = self.stores.read().await;
        stores
            .iter()
            .find(|s| s.store_name() == name)
            .map(Arc::clone)
    }

    /// Return the names of every registered store.
    ///
    /// Used by the `store` MCP server's `ListStores` verb to expose the
    /// set of stores that can be addressed by name. The order matches
    /// the order of `register` calls.
    pub async fn store_names(&self) -> Vec<String> {
        let stores = self.stores.read().await;
        stores.iter().map(|s| s.store_name().to_string()).collect()
    }

    /// Read the current serialized bytes for an item in the named store.
    ///
    /// Returns `Err(StoreError::NotFound)` when no store reports the
    /// given name. Returns `Ok(None)` when the store exists but the item
    /// does not (never written, or trashed / archived).
    pub async fn get_item_bytes(
        &self,
        store_name: &str,
        item_id: &StoredItemId,
    ) -> Result<Option<String>> {
        let store = self
            .store_by_name(store_name)
            .await
            .ok_or_else(|| StoreError::NotFound(format!("unknown store: {store_name}")))?;
        store.get_item_bytes(item_id).await
    }

    /// Read every changelog entry for an item in the named store.
    ///
    /// Returns `Err(StoreError::NotFound)` when no store reports the
    /// given name. Returns an empty `Vec` when the store exists but the
    /// item has never been written.
    pub async fn read_changelog(
        &self,
        store_name: &str,
        item_id: &StoredItemId,
    ) -> Result<Vec<ChangelogEntry>> {
        let store = self
            .store_by_name(store_name)
            .await
            .ok_or_else(|| StoreError::NotFound(format!("unknown store: {store_name}")))?;
        store.read_changelog(item_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::handle::StoreHandle;
    use crate::store::TrackedStore;
    use std::path::Path;
    use tempfile::TempDir;

    struct MockStore {
        root: PathBuf,
    }

    impl crate::store::sealed::Sealed for MockStore {}

    impl TrackedStore for MockStore {
        type Item = String;
        type ItemId = String;

        fn root(&self) -> &Path {
            &self.root
        }
        fn item_id(&self, item: &String) -> String {
            item.lines().next().unwrap_or("unknown").to_string()
        }
        fn serialize(&self, item: &String) -> crate::error::Result<String> {
            Ok(item.clone())
        }
        fn deserialize(&self, _id: &String, text: &str) -> crate::error::Result<String> {
            Ok(text.to_string())
        }
        fn extension(&self) -> &str {
            "txt"
        }
    }

    fn make_handle(dir: &Path) -> Arc<StoreHandle<MockStore>> {
        let store = Arc::new(MockStore {
            root: dir.to_path_buf(),
        });
        Arc::new(StoreHandle::new(store))
    }

    #[tokio::test]
    async fn watched_roots_returns_all_store_roots() {
        let dir = TempDir::new().unwrap();
        let store1_dir = dir.path().join("tasks");
        let store2_dir = dir.path().join("perspectives");
        std::fs::create_dir_all(&store1_dir).unwrap();
        std::fs::create_dir_all(&store2_dir).unwrap();

        let handle1 = make_handle(&store1_dir);
        let handle2 = make_handle(&store2_dir);

        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle1).await;
        ctx.register(handle2).await;

        let roots = ctx.watched_roots().await;
        assert_eq!(roots.len(), 2);
        assert!(roots.contains(&store1_dir));
        assert!(roots.contains(&store2_dir));
    }

    #[tokio::test]
    async fn register_and_undo_dispatches_correctly() {
        let dir = TempDir::new().unwrap();
        let store_dir = dir.path().join("store1");
        std::fs::create_dir_all(&store_dir).unwrap();

        let handle = make_handle(&store_dir);
        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle.clone()).await;

        // Write an item through the handle
        let item = "item1\ndata".to_string();
        let entry_id = handle.write(&item).await.unwrap().unwrap();
        ctx.push(
            entry_id,
            "create item1".to_string(),
            StoredItemId::from("item1"),
        )
        .await;

        assert!(ctx.can_undo().await);
        assert!(!ctx.can_redo().await);

        // Undo should dispatch to the correct store
        ctx.undo().await.unwrap();
        assert!(!store_dir.join("item1.txt").exists());
        assert!(!ctx.can_undo().await);
        assert!(ctx.can_redo().await);
    }

    #[tokio::test]
    async fn redo_dispatches_correctly() {
        let dir = TempDir::new().unwrap();
        let store_dir = dir.path().join("store1");
        std::fs::create_dir_all(&store_dir).unwrap();

        let handle = make_handle(&store_dir);
        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle.clone()).await;

        let item = "item1\ndata".to_string();
        let entry_id = handle.write(&item).await.unwrap().unwrap();
        ctx.push(
            entry_id,
            "create item1".to_string(),
            StoredItemId::from("item1"),
        )
        .await;

        ctx.undo().await.unwrap();
        assert!(!store_dir.join("item1.txt").exists());

        ctx.redo().await.unwrap();
        assert!(store_dir.join("item1.txt").exists());
    }

    #[tokio::test]
    async fn flush_all_aggregates_events() {
        let dir = TempDir::new().unwrap();
        let store1_dir = dir.path().join("store1");
        let store2_dir = dir.path().join("store2");
        std::fs::create_dir_all(&store1_dir).unwrap();
        std::fs::create_dir_all(&store2_dir).unwrap();

        let handle1 = make_handle(&store1_dir);
        let handle2 = make_handle(&store2_dir);

        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle1.clone()).await;
        ctx.register(handle2.clone()).await;

        // Write items through the handles so pending events are recorded
        handle1.write(&"a\na content".to_string()).await.unwrap();
        handle2.write(&"b\nb content".to_string()).await.unwrap();

        let events = ctx.flush_all().await;
        assert_eq!(events.len(), 2);
    }

    #[tokio::test]
    async fn store_for_path_finds_matching_store() {
        let dir = TempDir::new().unwrap();
        let store_dir = dir.path().join("mystore");
        std::fs::create_dir_all(&store_dir).unwrap();

        let handle = make_handle(&store_dir);
        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle).await;

        let found = ctx.store_for_path(&store_dir.join("item1.txt")).await;
        assert!(found.is_some());

        let not_found = ctx
            .store_for_path(&dir.path().join("other").join("file.txt"))
            .await;
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn can_undo_can_redo_reflects_state() {
        let dir = TempDir::new().unwrap();
        let ctx = StoreContext::new(dir.path().to_path_buf());

        assert!(!ctx.can_undo().await);
        assert!(!ctx.can_redo().await);

        let id = UndoEntryId::new();
        ctx.push(id, "op1".to_string(), StoredItemId::from("item1"))
            .await;
        assert!(ctx.can_undo().await);
        assert!(!ctx.can_redo().await);
    }

    #[tokio::test]
    async fn undo_dispatches_to_second_store() {
        let dir = TempDir::new().unwrap();
        let store1_dir = dir.path().join("store1");
        let store2_dir = dir.path().join("store2");
        std::fs::create_dir_all(&store1_dir).unwrap();
        std::fs::create_dir_all(&store2_dir).unwrap();

        let handle1 = make_handle(&store1_dir);
        let handle2 = make_handle(&store2_dir);

        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle1.clone()).await;
        ctx.register(handle2.clone()).await;

        // Write to store1
        let item1 = "s1item\ndata1".to_string();
        let id1 = handle1.write(&item1).await.unwrap().unwrap();
        ctx.push(
            id1,
            "store1 create".to_string(),
            StoredItemId::from("s1item"),
        )
        .await;

        // Write to store2
        let item2 = "s2item\ndata2".to_string();
        let id2 = handle2.write(&item2).await.unwrap().unwrap();
        ctx.push(
            id2,
            "store2 create".to_string(),
            StoredItemId::from("s2item"),
        )
        .await;

        // Undo should target store2 (most recent)
        ctx.undo().await.unwrap();
        assert!(!store2_dir.join("s2item.txt").exists());
        assert!(store1_dir.join("s1item.txt").exists());

        // Undo again should target store1
        ctx.undo().await.unwrap();
        assert!(!store1_dir.join("s1item.txt").exists());
    }

    /// `flush_all()` event payloads include the correct `store` name and item `id` fields.
    ///
    /// This test verifies that the aggregated events from multiple stores carry the
    /// store name and item ID in the payload — not just that the event count is correct.
    #[tokio::test]
    async fn flush_all_event_payloads_have_store_and_id() {
        let dir = TempDir::new().unwrap();
        // Use directory names that match the expected store_name (default impl returns basename)
        let store1_dir = dir.path().join("widgets");
        let store2_dir = dir.path().join("gadgets");
        std::fs::create_dir_all(&store1_dir).unwrap();
        std::fs::create_dir_all(&store2_dir).unwrap();

        let handle1 = make_handle(&store1_dir);
        let handle2 = make_handle(&store2_dir);

        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle1.clone()).await;
        ctx.register(handle2.clone()).await;

        // Write through each handle so pending events are produced
        handle1
            .write(&"widget1\nsome content".to_string())
            .await
            .unwrap();
        handle2
            .write(&"gadget1\nother content".to_string())
            .await
            .unwrap();

        let events = ctx.flush_all().await;
        assert_eq!(events.len(), 2);

        // Find the event for each store by `store` field
        let widget_event = events
            .iter()
            .find(|e| e.payload()["store"] == "widgets")
            .expect("expected event with store=widgets");
        let gadget_event = events
            .iter()
            .find(|e| e.payload()["store"] == "gadgets")
            .expect("expected event with store=gadgets");

        assert_eq!(widget_event.event_name(), "item-created");
        assert_eq!(widget_event.payload()["id"], "widget1");

        assert_eq!(gadget_event.event_name(), "item-created");
        assert_eq!(gadget_event.payload()["id"], "gadget1");
    }

    #[tokio::test]
    async fn undo_with_no_provider_returns_error() {
        let dir = TempDir::new().unwrap();
        let store_dir = dir.path().join("store1");
        std::fs::create_dir_all(&store_dir).unwrap();

        let handle = make_handle(&store_dir);
        let ctx = StoreContext::new(dir.path().to_path_buf());
        ctx.register(handle).await;

        // Push an entry ID that no store owns
        let orphan_id = UndoEntryId::new();
        ctx.push(
            orphan_id,
            "orphan op".to_string(),
            StoredItemId::from("orphan_item"),
        )
        .await;

        let result = ctx.undo().await;
        assert!(result.is_err());
        match result {
            Err(StoreError::NoProvider(_)) => {} // expected
            other => panic!("expected NoProvider, got {:?}", other),
        }
    }
}
