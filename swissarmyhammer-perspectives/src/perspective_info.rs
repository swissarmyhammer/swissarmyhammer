//! Lightweight runtime descriptors for dynamic command generation.
//!
//! Lives here (in the perspectives crate) rather than
//! `swissarmyhammer-kanban` because the descriptors are pure
//! perspective-domain data. `PerspectiveFieldInfo` is the denormalised
//! shape consumed by the `perspective.fields` options resolver — every
//! consumer that emits perspective-aware commands needs to assemble
//! this list.
//!
//! The companion lightweight `PerspectiveInfo` descriptor moves with
//! `PerspectiveFieldInfo` in a follow-up commit so the migration stays
//! one-type-per-commit.

/// Denormalised field descriptor carried alongside a perspective's
/// runtime metadata.
///
/// Joins a perspective's `fields[].field` (field ULID) against the
/// active board's `FieldsContext` at gather-time so a downstream
/// options resolver can answer at resolve-time without re-borrowing
/// `FieldsContext`.
#[derive(Debug, Clone)]
pub struct PerspectiveFieldInfo {
    /// Field identifier (ULID) — matches the wire `value` the picker emits.
    pub id: String,
    /// Human-readable display name resolved from the field registry
    /// (caption override on the perspective field entry wins; the
    /// field definition's name is the fallback).
    pub display_name: String,
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
            display_name: "Title".into(),
        };
        assert_eq!(f.id, "01F1");
        assert_eq!(f.display_name, "Title");
    }

    /// `Clone` is derived so a consumer can fan a field list out to
    /// multiple consumers (registry + tests). Pins the trait so
    /// removing it surfaces here.
    #[test]
    fn perspective_field_info_clone() {
        let f = PerspectiveFieldInfo {
            id: "01F2".into(),
            display_name: "Status".into(),
        };
        let f2 = f.clone();
        assert_eq!(f.id, f2.id);
        assert_eq!(f.display_name, f2.display_name);
    }
}
