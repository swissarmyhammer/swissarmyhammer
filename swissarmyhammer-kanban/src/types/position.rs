//! Position types for task ordering using fractional indexing.
//!
//! Uses the `fractional_index` crate (Figma's algorithm) for correct,
//! unbounded fractional key generation.

use super::ids::{ColumnId, SwimlaneId};
use fractional_index::FractionalIndex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Full position of a task on the board: column + optional swimlane + ordinal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub column: ColumnId,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub swimlane: Option<SwimlaneId>,
    pub ordinal: Ordinal,
}

impl Position {
    /// Create a new position
    pub fn new(column: ColumnId, swimlane: Option<SwimlaneId>, ordinal: Ordinal) -> Self {
        Self {
            column,
            swimlane,
            ordinal,
        }
    }

    /// Create a position in a column with default ordinal (at the start)
    pub fn in_column(column: ColumnId) -> Self {
        Self {
            column,
            swimlane: None,
            ordinal: Ordinal::first(),
        }
    }
}

/// Ordering within a column/swimlane cell using fractional indexing.
///
/// Backed by the `fractional_index` crate (Figma's algorithm).
/// Legacy ordinals are migrated on board open — no dual-algorithm support.
#[derive(Debug, Clone)]
pub struct Ordinal {
    fi: FractionalIndex,
    /// Cached string form for serialization and comparison.
    str_repr: String,
}

impl PartialEq for Ordinal {
    fn eq(&self, other: &Self) -> bool {
        self.str_repr == other.str_repr
    }
}

impl Eq for Ordinal {}

impl std::hash::Hash for Ordinal {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.str_repr.hash(state);
    }
}

impl Ordinal {
    fn wrap(fi: FractionalIndex) -> Self {
        let str_repr = fi.to_string();
        Self { fi, str_repr }
    }

    /// Default first ordinal.
    pub fn first() -> Self {
        Self::wrap(FractionalIndex::default())
    }

    /// Ordinal that sorts after `last`.
    pub fn after(last: &Ordinal) -> Self {
        Self::wrap(FractionalIndex::new_after(&last.fi))
    }

    /// Ordinal that sorts before `first`.
    pub fn before(first: &Ordinal) -> Self {
        Self::wrap(FractionalIndex::new_before(&first.fi))
    }

    /// Ordinal that sorts between `before` and `after`.
    pub fn between(before: &Ordinal, after: &Ordinal) -> Self {
        match FractionalIndex::new_between(&before.fi, &after.fi) {
            Some(fi) => Self::wrap(fi),
            None => Self::after(before),
        }
    }

    /// Create from a FractionalIndex string.
    ///
    /// If the string is not a valid FractionalIndex encoding (e.g. legacy
    /// ordinals like "a0"), returns `Ordinal::first()`. Call
    /// `migrate_ordinals` on board open to rewrite legacy data.
    pub fn from_string(s: &str) -> Self {
        match FractionalIndex::from_string(s) {
            Ok(fi) => Self::wrap(fi),
            Err(_) => Self::first(),
        }
    }

    /// Check if a string is a valid FractionalIndex encoding.
    pub fn is_valid(s: &str) -> bool {
        FractionalIndex::from_string(s).is_ok()
    }

    /// Get the string representation for persistence.
    pub fn as_str(&self) -> &str {
        &self.str_repr
    }
}

impl PartialOrd for Ordinal {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordinal {
    fn cmp(&self, other: &Self) -> Ordering {
        self.str_repr.cmp(&other.str_repr)
    }
}

impl Default for Ordinal {
    fn default() -> Self {
        Self::first()
    }
}

impl Serialize for Ordinal {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.str_repr)
    }
}

impl<'de> Deserialize<'de> for Ordinal {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_string(&s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ordinal_first() {
        let ord = Ordinal::first();
        assert!(!ord.as_str().is_empty());
    }

    #[test]
    fn test_ordinal_after() {
        let first = Ordinal::first();
        let second = Ordinal::after(&first);
        assert!(second > first);

        let third = Ordinal::after(&second);
        assert!(third > second);
        assert!(third > first);
    }

    #[test]
    fn test_ordinal_before() {
        let first = Ordinal::first();
        let before = Ordinal::before(&first);
        assert!(
            before < first,
            "'{}' should be < '{}'",
            before.as_str(),
            first.as_str()
        );
    }

    #[test]
    fn test_ordinal_between() {
        let first = Ordinal::first();
        let third = Ordinal::after(&Ordinal::after(&first));

        let second = Ordinal::between(&first, &third);
        assert!(second > first);
        assert!(second < third);
    }

    #[test]
    fn test_ordinal_between_adjacent() {
        let a = Ordinal::first();
        let b = Ordinal::after(&a);
        let mid = Ordinal::between(&a, &b);
        assert!(mid > a, "'{}' should be > '{}'", mid.as_str(), a.as_str());
        assert!(mid < b, "'{}' should be < '{}'", mid.as_str(), b.as_str());
    }

    #[test]
    fn test_ordinal_repeated_before() {
        // Repeatedly prepending should always produce smaller ordinals
        let mut current = Ordinal::first();
        for _ in 0..20 {
            let prev = Ordinal::before(&current);
            assert!(
                prev < current,
                "'{}' should be < '{}'",
                prev.as_str(),
                current.as_str()
            );
            current = prev;
        }
    }

    #[test]
    fn test_ordinal_repeated_after() {
        // Repeatedly appending should always produce larger ordinals
        let mut current = Ordinal::first();
        for _ in 0..20 {
            let next = Ordinal::after(&current);
            assert!(
                next > current,
                "'{}' should be > '{}'",
                next.as_str(),
                current.as_str()
            );
            current = next;
        }
    }

    #[test]
    fn test_ordinal_repeated_between() {
        // Repeatedly inserting between should always produce valid ordinals
        let mut lo = Ordinal::first();
        let hi = Ordinal::after(&Ordinal::after(&lo));
        for _ in 0..20 {
            let mid = Ordinal::between(&lo, &hi);
            assert!(mid > lo, "'{}' should be > '{}'", mid.as_str(), lo.as_str());
            assert!(mid < hi, "'{}' should be < '{}'", mid.as_str(), hi.as_str());
            lo = mid;
        }
    }

    #[test]
    fn test_ordinal_ordering() {
        let a = Ordinal::first();
        let b = Ordinal::after(&a);
        let c = Ordinal::after(&b);

        assert!(a < b);
        assert!(b < c);
        assert!(a < c);
    }

    #[test]
    fn test_ordinal_from_legacy_string() {
        // Legacy ordinals like "a0" can't be parsed — should fall back to default
        let ord = Ordinal::from_string("a0");
        assert!(!ord.as_str().is_empty());
    }

    #[test]
    fn test_fractional_index_string_format() {
        // Check what format the crate uses
        let a = Ordinal::first();
        eprintln!("first: '{}'", a.as_str());
        let b = Ordinal::after(&a);
        eprintln!("after first: '{}'", b.as_str());
        let c = Ordinal::after(&b);
        eprintln!("after after: '{}'", c.as_str());
        let before = Ordinal::before(&a);
        eprintln!("before first: '{}'", before.as_str());

        // Verify round-trip
        let s = a.as_str().to_string();
        let parsed = Ordinal::from_string(&s);
        assert_eq!(
            parsed.as_str(),
            a.as_str(),
            "round-trip should preserve ordinal"
        );
    }
}
