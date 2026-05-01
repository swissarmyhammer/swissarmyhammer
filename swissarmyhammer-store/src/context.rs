//! Central coordinator for multiple stores with shared undo/redo.
//!
//! The [`StoreContext`] holds an [`UndoStack`] and a collection of
//! [`ErasedStore`] instances. It dispatches undo/redo to the correct store
//! and aggregates change events from all stores.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use tokio::sync::RwLock;
use tracing;

use crate::erased::ErasedStore;
use crate::error::{Result, StoreError};
use crate::event::ChangeEvent;
use crate::id::{StoredItemId, UndoEntryId};
use crate::stack::UndoStack;

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
    pub store_name: String,
    /// Identifier of the item whose state was reversed or reapplied.
    pub item_id: StoredItemId,
}

/// Central coordinator for multiple file-backed stores.
///
/// Manages a shared undo/redo stack and dispatches operations to the
/// correct store based on changelog entry ownership.
pub struct StoreContext {
    stack: RwLock<UndoStack>,
    stores: RwLock<Vec<Arc<dyn ErasedStore>>>,
    root: PathBuf,
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
        }
    }

    /// Register a store with this context.
    pub async fn register(&self, store: Arc<dyn ErasedStore>) {
        self.stores.write().await.push(store);
    }

    /// Push an entry onto the undo stack and persist to disk.
    ///
    /// The `item_id` records which item's per-item changelog contains this
    /// entry, so that undo/redo can look it up without scanning all files.
    pub async fn push(&self, id: UndoEntryId, label: String, item_id: StoredItemId) {
        let mut stack = self.stack.write().await;
        stack.push(id, label, item_id);
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
        let (target_id, item_id) = {
            let stack = self.stack.read().await;
            let entry = stack
                .undo_target()
                .ok_or_else(|| StoreError::NotFound("nothing to undo".into()))?;
            (entry.id, entry.item_id.clone())
        };

        // Clone matching store out of the lock, then release it before awaiting
        let store = {
            let stores = self.stores.read().await;
            let mut found = None;
            for s in stores.iter() {
                if s.has_entry(&target_id, &item_id).await {
                    found = Some(Arc::clone(s));
                    break;
                }
            }
            found
        };
        // Lock released here

        let Some(store) = store else {
            return Err(StoreError::NoProvider(target_id.to_string()));
        };

        store.undo_erased(&target_id, &item_id).await?;
        let store_name = store.store_name().to_string();

        let mut stack = self.stack.write().await;
        stack.record_undo();
        if let Err(e) = stack.save(&self.root.join("undo_stack.yaml")) {
            tracing::warn!(error = %e, "failed to save undo stack");
        }

        Ok(UndoOutcome {
            store_name,
            item_id,
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
        let (target_id, item_id) = {
            let stack = self.stack.read().await;
            let entry = stack
                .redo_target()
                .ok_or_else(|| StoreError::NotFound("nothing to redo".into()))?;
            (entry.id, entry.item_id.clone())
        };

        // Clone matching store out of the lock, then release it before awaiting
        let store = {
            let stores = self.stores.read().await;
            let mut found = None;
            for s in stores.iter() {
                if s.has_entry(&target_id, &item_id).await {
                    found = Some(Arc::clone(s));
                    break;
                }
            }
            found
        };
        // Lock released here

        let Some(store) = store else {
            return Err(StoreError::NoProvider(target_id.to_string()));
        };

        store.redo_erased(&target_id, &item_id).await?;
        let store_name = store.store_name().to_string();

        let mut stack = self.stack.write().await;
        stack.record_redo();
        if let Err(e) = stack.save(&self.root.join("undo_stack.yaml")) {
            tracing::warn!(error = %e, "failed to save undo stack");
        }

        Ok(UndoOutcome {
            store_name,
            item_id,
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
