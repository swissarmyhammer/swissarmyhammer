//! The `TrackedStore` trait defining the contract for file-backed stores.
//!
//! Implementors provide only serialization logic and directory location.
//! The [`StoreHandle`](crate::handle::StoreHandle) wraps any `TrackedStore`
//! to add write, delete, undo, redo, changelog, and change detection.

use std::fmt::Display;
use std::hash::Hash;
use std::path::Path;
use std::str::FromStr;

use crate::error::Result;

/// Sealing module for [`TrackedStore`].
///
/// The `Sealed` supertrait lives in a public module so that sibling workspace
/// crates (swissarmyhammer-entity, swissarmyhammer-perspectives) can implement
/// it, but the trait is `#[doc(hidden)]` so downstream consumers cannot
/// discover or implement it.
pub mod sealed {
    /// Marker trait that seals [`TrackedStore`](super::TrackedStore).
    ///
    /// Implement this for any type that should be allowed to implement
    /// `TrackedStore`. This prevents arbitrary downstream types from
    /// implementing the trait, preserving semver freedom to add methods.
    #[doc(hidden)]
    pub trait Sealed {}
}

/// A file-backed store managing one directory.
///
/// Implementors provide only serialization; the `StoreHandle` blanket impl
/// provides write, delete, undo, redo, changelog, and change detection.
///
/// This trait is sealed and cannot be implemented outside this workspace.
pub trait TrackedStore: sealed::Sealed + Send + Sync + 'static {
    /// The item type this store manages.
    type Item: Send + Sync;

    /// The item's ID type.
    ///
    /// Clone (not Copy) because some IDs are String-based slugs.
    type ItemId: Send + Sync + Clone + Eq + Hash + Display + FromStr;

    /// The single directory this store manages.
    fn root(&self) -> &Path;

    /// Extract the item's unique ID.
    fn item_id(&self, item: &Self::Item) -> Self::ItemId;

    /// Serialize an item to its on-disk text representation.
    ///
    /// The returned text is exactly what gets written to the file.
    /// Returns an error if serialization fails.
    fn serialize(&self, item: &Self::Item) -> Result<String>;

    /// Deserialize an item from its on-disk text representation.
    ///
    /// The ID comes from the filename, not from within the text.
    fn deserialize(&self, id: &Self::ItemId, text: &str) -> Result<Self::Item>;

    /// File extension for items in this store (e.g. "yaml", "md").
    fn extension(&self) -> &str;

    /// A human-readable name for this store, used in change events.
    ///
    /// For entity stores this is the entity type name (e.g. "task", "column").
    /// The default implementation infers the name from the directory basename.
    fn store_name(&self) -> &str {
        self.root()
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
    }
}
