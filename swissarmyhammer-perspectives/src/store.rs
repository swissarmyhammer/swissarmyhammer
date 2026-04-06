//! [`TrackedStore`] implementation for perspectives.
//!
//! [`PerspectiveStore`] adapts the perspectives directory to the
//! [`TrackedStore`] trait from `swissarmyhammer-store`. Perspectives are
//! stored as plain YAML files, one per perspective.

use std::path::{Path, PathBuf};

use swissarmyhammer_store::{StoreError, TrackedStore};

use crate::types::Perspective;
use crate::PerspectiveId;

/// Convenience alias matching the store crate's Result type.
type StoreResult<T> = std::result::Result<T, StoreError>;

/// A [`TrackedStore`] for perspective definitions.
///
/// Stores perspectives as YAML files in a single directory, one file per
/// perspective. The filename is the perspective's ULID with a `.yaml`
/// extension.
#[derive(Debug)]
pub struct PerspectiveStore {
    root: PathBuf,
}

impl PerspectiveStore {
    /// Create a new store rooted at the given directory.
    ///
    /// The directory is the `perspectives/` subdirectory inside the
    /// `.kanban` root.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

impl swissarmyhammer_store::store::sealed::Sealed for PerspectiveStore {}

impl TrackedStore for PerspectiveStore {
    type Item = Perspective;
    type ItemId = PerspectiveId;

    fn root(&self) -> &Path {
        &self.root
    }

    fn item_id(&self, perspective: &Perspective) -> PerspectiveId {
        PerspectiveId::from(perspective.id.as_str())
    }

    fn serialize(&self, perspective: &Perspective) -> StoreResult<String> {
        serde_yaml_ng::to_string(perspective).map_err(StoreError::Yaml)
    }

    fn deserialize(&self, id: &PerspectiveId, text: &str) -> StoreResult<Perspective> {
        let mut perspective: Perspective =
            serde_yaml_ng::from_str(text).map_err(StoreError::Yaml)?;
        // Ensure the ID matches the filename, not whatever is in the YAML body.
        perspective.id = id.to_string();
        Ok(perspective)
    }

    fn extension(&self) -> &str {
        "yaml"
    }

    fn store_name(&self) -> &str {
        "perspective"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Perspective, PerspectiveFieldEntry, SortDirection, SortEntry};

    /// Build a minimal test perspective.
    fn test_perspective(id: &str, name: &str) -> Perspective {
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

    #[test]
    fn extension_is_yaml() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        assert_eq!(store.extension(), "yaml");
    }

    #[test]
    fn store_name_is_perspective() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        assert_eq!(store.store_name(), "perspective");
    }

    #[test]
    fn item_id_extraction() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        let p = test_perspective("01JPERSP000000000000000000", "Sprint View");
        let id = store.item_id(&p);
        assert_eq!(id.as_str(), "01JPERSP000000000000000000");
    }

    #[test]
    fn serialize_round_trip() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        let p = Perspective {
            id: "01JPERSP000000000000000000".to_string(),
            name: "Active Sprint".to_string(),
            view: "board".to_string(),
            fields: vec![PerspectiveFieldEntry {
                field: "01JMTASK0000000000TITLE00".to_string(),
                caption: Some("Title".to_string()),
                width: Some(200),
                editor: None,
                display: None,
                sort_comparator: None,
            }],
            filter: Some("(e) => e.Status !== \"Done\"".to_string()),
            group: Some("(e) => e.Status".to_string()),
            sort: vec![SortEntry {
                field: "01JMTASK0000000000PRIORTY".to_string(),
                direction: SortDirection::Asc,
            }],
        };

        let text = store.serialize(&p).unwrap();
        let id = PerspectiveId::from("01JPERSP000000000000000000");
        let restored = store.deserialize(&id, &text).unwrap();

        assert_eq!(p.name, restored.name);
        assert_eq!(p.view, restored.view);
        assert_eq!(p.fields, restored.fields);
        assert_eq!(p.filter, restored.filter);
        assert_eq!(p.group, restored.group);
        assert_eq!(p.sort, restored.sort);
    }

    #[test]
    fn deserialize_overrides_id_from_filename() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        let p = test_perspective("wrong-id", "Test");
        let text = store.serialize(&p).unwrap();

        let correct_id = PerspectiveId::from("correct-id");
        let restored = store.deserialize(&correct_id, &text).unwrap();
        assert_eq!(restored.id, "correct-id");
    }

    #[test]
    fn serialize_minimal_perspective() {
        let store = PerspectiveStore::new("/tmp/perspectives");
        let p = test_perspective("01AAA", "Minimal");
        let text = store.serialize(&p).unwrap();

        // Minimal perspective should contain name and view but not optional sections
        assert!(text.contains("name: Minimal"));
        assert!(text.contains("view: board"));
        assert!(!text.contains("filter:"));
        assert!(!text.contains("group:"));
    }
}
