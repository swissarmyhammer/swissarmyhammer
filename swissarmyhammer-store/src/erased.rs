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
use crate::id::UndoEntryId;
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
    async fn has_entry(&self, id: &UndoEntryId) -> bool;

    /// Undo an operation, discarding the typed return value.
    async fn undo_erased(&self, id: &UndoEntryId) -> Result<()>;

    /// Redo an operation, discarding the typed return value.
    async fn redo_erased(&self, id: &UndoEntryId) -> Result<()>;
}

#[async_trait]
impl<S: TrackedStore> ErasedStore for StoreHandle<S> {
    fn root(&self) -> &Path {
        self.store.root()
    }

    async fn flush_changes(&self) -> Vec<ChangeEvent> {
        StoreHandle::flush_changes(self).await
    }

    async fn has_entry(&self, id: &UndoEntryId) -> bool {
        StoreHandle::has_entry(self, id).await
    }

    async fn undo_erased(&self, id: &UndoEntryId) -> Result<()> {
        self.undo(id).await?;
        Ok(())
    }

    async fn redo_erased(&self, id: &UndoEntryId) -> Result<()> {
        self.redo(id).await?;
        Ok(())
    }
}
