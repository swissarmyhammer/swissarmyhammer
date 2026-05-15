//! Lightweight runtime descriptors for dynamic command generation.
//!
//! Lives here (in the perspectives crate) rather than
//! `swissarmyhammer-kanban` because the descriptors are pure
//! perspective-domain data. `PerspectiveFieldInfo` is the denormalised
//! shape consumed by the `perspective.fields` options resolver, and
//! `PerspectiveInfo` is the lightweight perspective descriptor every
//! consumer that drives the command registry off a set of perspectives
//! produces.

/// Denormalised field descriptor carried alongside a perspective's
/// runtime metadata.
///
/// Joins a perspective's `fields[].field` (field ULID) against the
/// active board's `FieldsContext` at gather-time so a downstream
/// options resolver can answer at resolve-time without re-borrowing
/// `FieldsContext`.
#[derive(Debug, Clone)]
pub struct PerspectiveFieldInfo {
    /// Field identifier (ULID). Stable across renames; used by
    /// consumers that need an identity-preserving handle.
    pub id: String,
    /// Field name (schema slug, e.g. `"assignees"`, `"tags"`). This is
    /// the wire `value` the `perspective.fields` picker emits and the
    /// key tasks use to store the corresponding value in their
    /// `fields` map. Persisted perspective YAMLs also key `group:` by
    /// this name, so the round-trip (picker → dispatch → persist →
    /// `<GroupedBoardView>`) is consistently name-shaped end-to-end.
    pub name: String,
    /// Human-readable display name resolved from the field registry
    /// (caption override on the perspective field entry wins; the
    /// field definition's name is the fallback).
    pub display_name: String,
}

/// Lightweight perspective descriptor for dynamic command generation.
///
/// Only carries the fields needed to produce a palette row that
/// dispatches `perspective.switch` with a pre-filled `perspective_id`.
/// Intentionally decoupled from the heavyweight
/// [`crate::types::Perspective`] so the dispatch path does not need
/// to load the entire perspective for every emit.
#[derive(Debug, Clone)]
pub struct PerspectiveInfo {
    /// Perspective identifier (ULID).
    pub id: String,
    /// Human-readable name (e.g. `"Active Sprint"`).
    pub name: String,
    /// View kind (e.g. `"board"`, `"grid"`).
    pub view: String,
    /// Denormalised field list for this perspective's columns.
    ///
    /// Populated at gather-time by joining the perspective's
    /// `fields[].field` ULID list against the active board's field
    /// registry. Empty when the perspective has no fields or when
    /// the join failed for every entry.
    pub fields: Vec<PerspectiveFieldInfo>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `PerspectiveFieldInfo` is plain data — fields are directly
    /// readable. Pins the shape so a future rename surfaces here
    /// instead of in every downstream consumer.
    #[test]
    fn perspective_field_info_construction() {
        let f = PerspectiveFieldInfo {
            id: "01F1".into(),
            name: "title".into(),
            display_name: "Title".into(),
        };
        assert_eq!(f.id, "01F1");
        assert_eq!(f.name, "title");
        assert_eq!(f.display_name, "Title");
    }

    /// `Clone` is derived so a consumer can fan a field list out to
    /// multiple consumers (registry + tests). Pins the trait so
    /// removing it surfaces here.
    #[test]
    fn perspective_field_info_clone() {
        let f = PerspectiveFieldInfo {
            id: "01F2".into(),
            name: "status".into(),
            display_name: "Status".into(),
        };
        let f2 = f.clone();
        assert_eq!(f.id, f2.id);
        assert_eq!(f.name, f2.name);
        assert_eq!(f.display_name, f2.display_name);
    }

    /// `PerspectiveInfo` aggregates a perspective's runtime
    /// descriptor with its denormalised field list. Pins both the
    /// shape and the contained-field-list relationship so a future
    /// rename surfaces here.
    #[test]
    fn perspective_info_with_fields() {
        let p = PerspectiveInfo {
            id: "01P".into(),
            name: "Active Sprint".into(),
            view: "grid".into(),
            fields: vec![
                PerspectiveFieldInfo {
                    id: "01F1".into(),
                    name: "title".into(),
                    display_name: "Title".into(),
                },
                PerspectiveFieldInfo {
                    id: "01F2".into(),
                    name: "status".into(),
                    display_name: "Status".into(),
                },
            ],
        };
        assert_eq!(p.id, "01P");
        assert_eq!(p.name, "Active Sprint");
        assert_eq!(p.view, "grid");
        assert_eq!(p.fields.len(), 2);
        assert_eq!(p.fields[0].id, "01F1");
        assert_eq!(p.fields[0].name, "title");
    }

    /// An empty field list is valid (the perspective is selectable
    /// but its column-picker yields no options). Pins the contract
    /// so the default-shaped `PerspectiveInfo` keeps compiling.
    #[test]
    fn perspective_info_with_empty_fields() {
        let p = PerspectiveInfo {
            id: "01P".into(),
            name: "Default".into(),
            view: "board".into(),
            fields: vec![],
        };
        assert!(p.fields.is_empty());
    }
}
