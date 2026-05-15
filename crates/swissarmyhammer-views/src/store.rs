//! [`TrackedStore`] implementation for views.
//!
//! [`ViewStore`] adapts the views directory to the [`TrackedStore`] trait from
//! `swissarmyhammer-store`. Views are stored as plain YAML files, one per view
//! definition.

use std::path::{Path, PathBuf};

use swissarmyhammer_store::{StoreError, TrackedStore};

use crate::types::{ViewDef, ViewId};

/// Convenience alias matching the store crate's Result type.
type StoreResult<T> = std::result::Result<T, StoreError>;

/// Store name returned by [`ViewStore::store_name`].
///
/// Exposed as a constant so downstream code that needs to key off the
/// store name (notably `app.undo` / `app.redo` reconciliation in the
/// kanban crate) can reference the same literal the store exposes.
/// Without this, the two sites drift silently: if either side changes the
/// spelling, cache reconciliation becomes a no-op with no compile-time
/// signal, and view undo regresses to a state where the disk file is
/// rewritten but the cache and broadcast channel never learn about it.
pub const VIEW_STORE_NAME: &str = "view";

/// A [`TrackedStore`] for view definitions.
///
/// Stores views as YAML files in a single directory, one file per view.
/// The filename is the view's ULID with a `.yaml` extension.
#[derive(Debug)]
pub struct ViewStore {
    root: PathBuf,
}

impl ViewStore {
    /// Create a new store rooted at the given directory.
    ///
    /// The directory is the `views/` subdirectory inside the `.kanban` root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl swissarmyhammer_store::store::sealed::Sealed for ViewStore {}

impl TrackedStore for ViewStore {
    type Item = ViewDef;
    type ItemId = ViewId;

    fn root(&self) -> &Path {
        &self.root
    }

    fn item_id(&self, view: &ViewDef) -> ViewId {
        view.id.clone()
    }

    fn serialize(&self, view: &ViewDef) -> StoreResult<String> {
        serde_yaml_ng::to_string(view).map_err(StoreError::Yaml)
    }

    fn deserialize(&self, id: &ViewId, text: &str) -> StoreResult<ViewDef> {
        let mut view: ViewDef = serde_yaml_ng::from_str(text).map_err(StoreError::Yaml)?;
        // Ensure the ID matches the filename, not whatever is in the YAML body.
        view.id = id.clone();
        Ok(view)
    }

    fn extension(&self) -> &str {
        "yaml"
    }

    fn store_name(&self) -> &str {
        VIEW_STORE_NAME
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ViewDef, ViewKind};

    /// Build a minimal test view definition.
    fn test_view(id: &str, name: &str) -> ViewDef {
        ViewDef {
            id: id.to_string(),
            name: name.to_string(),
            icon: None,
            kind: ViewKind::Board,
            entity_type: None,
            card_fields: Vec::new(),
            commands: Vec::new(),
        }
    }

    #[test]
    fn extension_is_yaml() {
        let store = ViewStore::new("/tmp/views");
        assert_eq!(store.extension(), "yaml");
    }

    #[test]
    fn store_name_is_view() {
        let store = ViewStore::new("/tmp/views");
        assert_eq!(store.store_name(), "view");
        // Also assert the constant matches — if someone edits either side this
        // test catches the drift before it reaches reconciliation.
        assert_eq!(store.store_name(), VIEW_STORE_NAME);
    }

    #[test]
    fn item_id_extraction() {
        let store = ViewStore::new("/tmp/views");
        let v = test_view("01JVIEW000000000000000000", "Board");
        let id = store.item_id(&v);
        assert_eq!(id, "01JVIEW000000000000000000");
    }

    #[test]
    fn serialize_round_trip() {
        let store = ViewStore::new("/tmp/views");
        let v = ViewDef {
            id: "01JVIEW000000000000000000".into(),
            name: "Board".into(),
            icon: Some("kanban".into()),
            kind: ViewKind::Board,
            entity_type: Some("task".into()),
            card_fields: vec!["title".into(), "tags".into()],
            commands: Vec::new(),
        };

        let text = store.serialize(&v).unwrap();
        let restored = store.deserialize(&v.id, &text).unwrap();

        assert_eq!(v, restored);
    }

    #[test]
    fn deserialize_overrides_id_from_filename() {
        let store = ViewStore::new("/tmp/views");
        let v = test_view("wrong-id", "Test");
        let text = store.serialize(&v).unwrap();

        let correct_id: ViewId = "correct-id".to_string();
        let restored = store.deserialize(&correct_id, &text).unwrap();
        assert_eq!(restored.id, "correct-id");
    }
}
