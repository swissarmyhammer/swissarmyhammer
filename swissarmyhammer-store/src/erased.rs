//! Object-safe wrapper trait for heterogeneous store dispatch.
//!
//! [`ErasedStore`] erases the concrete `TrackedStore` type so that a
//! [`StoreContext`](crate::context::StoreContext) can hold multiple stores
//! of different item types in a single collection.

use std::path::Path;

use async_trait::async_trait;

use crate::error::Result;
use crate::event::ChangeEvent;
use crate::handle::StoreHandle;
use crate::id::{StoredItemId, UndoEntryId};
use crate::store::TrackedStore;

/// Object-safe wrapper for heterogeneous store dispatch.
///
/// Any `StoreHandle<S>` implements this trait, allowing the `StoreContext`
/// to operate on stores without knowing their concrete item types.
#[async_trait]
pub trait ErasedStore: Send + Sync {
    /// The root directory this store manages.
    fn root(&self) -> &Path;

    /// A human-readable name for this store (e.g. "task", "column").
    fn store_name(&self) -> &str;

    /// Scan for changes and produce change events.
    async fn flush_changes(&self) -> Vec<ChangeEvent>;

    /// Check whether this store owns the given changelog entry.
    ///
    /// The `item_id` identifies which per-item changelog to search.
    async fn has_entry(&self, id: &UndoEntryId, item_id: &StoredItemId) -> bool;

    /// Undo an operation, discarding the typed return value.
    ///
    /// The `item_id` identifies which per-item changelog contains the entry.
    async fn undo_erased(&self, id: &UndoEntryId, item_id: &StoredItemId) -> Result<()>;

    /// Redo an operation, discarding the typed return value.
    ///
    /// The `item_id` identifies which per-item changelog contains the entry.
    async fn redo_erased(&self, id: &UndoEntryId, item_id: &StoredItemId) -> Result<()>;
}

#[async_trait]
impl<S: TrackedStore> ErasedStore for StoreHandle<S> {
    fn root(&self) -> &Path {
        self.store.root()
    }

    fn store_name(&self) -> &str {
        self.store.store_name()
    }

    async fn flush_changes(&self) -> Vec<ChangeEvent> {
        StoreHandle::flush_changes(self).await
    }

    async fn has_entry(&self, id: &UndoEntryId, item_id: &StoredItemId) -> bool {
        StoreHandle::has_entry(self, id, item_id).await
    }

    async fn undo_erased(&self, id: &UndoEntryId, item_id: &StoredItemId) -> Result<()> {
        self.undo(id, item_id).await?;
        Ok(())
    }

    async fn redo_erased(&self, id: &UndoEntryId, item_id: &StoredItemId) -> Result<()> {
        self.redo(id, item_id).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    /// A simple mock store for testing. Items are plain strings where the
    /// first line is the ID.
    struct MockStore {
        root: PathBuf,
    }

    impl TrackedStore for MockStore {
        type Item = String;
        type ItemId = String;

        fn root(&self) -> &Path {
            &self.root
        }

        fn item_id(&self, item: &String) -> String {
            item.lines().next().unwrap_or("unknown").to_string()
        }

        fn serialize(&self, item: &String) -> Result<String> {
            Ok(item.clone())
        }

        fn deserialize(&self, _id: &String, text: &str) -> Result<String> {
            Ok(text.to_string())
        }

        fn extension(&self) -> &str {
            "txt"
        }
    }

    fn setup() -> (tempfile::TempDir, Arc<StoreHandle<MockStore>>) {
        let dir = tempfile::TempDir::new().unwrap();
        let store = Arc::new(MockStore {
            root: dir.path().to_path_buf(),
        });
        let handle = Arc::new(StoreHandle::new(store));
        (dir, handle)
    }

    #[tokio::test]
    async fn has_entry_through_dyn_erased_store() {
        let (_dir, handle) = setup();
        let item = "item1\ndata".to_string();
        let entry_id = handle.write(&item).await.unwrap().unwrap();
        let item_id = StoredItemId::from("item1");

        let erased: Arc<dyn ErasedStore> = handle;
        assert!(erased.has_entry(&entry_id, &item_id).await);

        // Unknown entry returns false
        let unknown = UndoEntryId::new();
        assert!(!erased.has_entry(&unknown, &item_id).await);
    }

    #[tokio::test]
    async fn undo_erased_through_dyn_erased_store() {
        let (_dir, handle) = setup();
        let v1 = "item1\nversion1".to_string();
        let v2 = "item1\nversion2".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        // Verify v2 is on disk
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v2);

        // Undo through the erased trait
        let erased: Arc<dyn ErasedStore> = handle;
        erased
            .undo_erased(&update_id, &StoredItemId::from("item1"))
            .await
            .unwrap();

        // File should be reverted to v1
        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v1);
    }

    #[tokio::test]
    async fn redo_erased_through_dyn_erased_store() {
        let (_dir, handle) = setup();
        let v1 = "item1\nversion1".to_string();
        let v2 = "item1\nversion2".to_string();

        handle.write(&v1).await.unwrap();
        let update_id = handle.write(&v2).await.unwrap().unwrap();

        let item_id = StoredItemId::from("item1");

        // Undo through erased trait
        let erased: Arc<dyn ErasedStore> = handle;
        erased.undo_erased(&update_id, &item_id).await.unwrap();

        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v1);

        // Redo through erased trait
        erased.redo_erased(&update_id, &item_id).await.unwrap();

        let content = std::fs::read_to_string(_dir.path().join("item1.txt")).unwrap();
        assert_eq!(content, v2);
    }
}
