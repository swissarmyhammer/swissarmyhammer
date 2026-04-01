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
