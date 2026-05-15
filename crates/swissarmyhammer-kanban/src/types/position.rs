//! Position types for task ordering using fractional indexing.
//!
//! Uses the `fractional_index` crate (Figma's algorithm) for correct,
//! unbounded fractional key generation.

use super::ids::ColumnId;
use fractional_index::FractionalIndex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;

/// Full position of a task on the board: column + ordinal
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub column: ColumnId,
    pub ordinal: Ordinal,
}

impl Position {
    /// Create a new position
    pub fn new(column: ColumnId, ordinal: Ordinal) -> Self {
        Self { column, ordinal }
    }

    /// Create a position in a column with default ordinal (at the start)
    pub fn in_column(column: ColumnId) -> Self {
        Self {
            column,
            ordinal: Ordinal::first(),
        }
    }
}

/// Ordering within a column using fractional indexing.
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
    /// The string representation of the default ordinal.
    ///
    /// Use this as the fallback when reading raw ordinal strings from entity
    /// fields (e.g. `get_str("position_ordinal").unwrap_or(Ordinal::DEFAULT_STR)`).
    /// This ensures the raw-string sort order matches `Ordinal::from_string`'s
    /// fallback, which also returns `Ordinal::first()`.
    pub const DEFAULT_STR: &'static str = "80";

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
    ///
    /// When the underlying `new_between` fails (equal or misordered inputs),
    /// falls back to `before(after)` so the result is at least less than the
    /// `after` argument — preserving the caller's intent of "place before this
    /// item" as closely as possible.
    pub fn between(before: &Ordinal, after: &Ordinal) -> Self {
        match FractionalIndex::new_between(&before.fi, &after.fi) {
            Some(fi) => Self::wrap(fi),
            None => Self::before(after),
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
    fn test_ordinal_between_equal_inputs_falls_back_to_before() {
        // When both inputs are equal (e.g. duplicate ordinals from legacy data),
        // between() must NOT silently produce a value AFTER both inputs.
        // It should produce a value before `after` so the caller's intent
        // ("place me between these two") degrades gracefully.
        let a = Ordinal::first();
        let b = Ordinal::first(); // same as a — simulates duplicate ordinals
        let result = Ordinal::between(&a, &b);
        // The result must be < b (the "after" argument), not > b
        assert!(
            result < b,
            "between(equal, equal) produced '{}' which is >= after '{}' — should be <",
            result.as_str(),
            b.as_str()
        );
    }

    #[test]
    fn test_position_new() {
        let col = ColumnId::from_string("todo");
        let ordinal = Ordinal::first();
        let pos = Position::new(col.clone(), ordinal.clone());

        assert_eq!(pos.column, col);
        assert_eq!(pos.ordinal, ordinal);
    }

    #[test]
    fn test_position_in_column() {
        let col = ColumnId::from_string("backlog");
        let pos = Position::in_column(col.clone());

        assert_eq!(pos.column, col);
        // Should use the default "first" ordinal
        assert_eq!(pos.ordinal, Ordinal::first());
    }

    #[test]
    fn test_ordinal_is_valid_with_valid_string() {
        // The default ordinal string should be valid
        let valid = Ordinal::DEFAULT_STR;
        assert!(Ordinal::is_valid(valid));
    }

    #[test]
    fn test_ordinal_is_valid_with_first_ordinal() {
        let first = Ordinal::first();
        assert!(Ordinal::is_valid(first.as_str()));
    }

    #[test]
    fn test_ordinal_is_valid_rejects_legacy() {
        // Legacy ordinals like "a0" are not valid FractionalIndex encodings
        assert!(!Ordinal::is_valid("a0"));
    }

    #[test]
    fn test_ordinal_is_valid_rejects_empty() {
        assert!(!Ordinal::is_valid(""));
    }

    #[test]
    fn test_ordinal_is_valid_rejects_arbitrary_text() {
        assert!(!Ordinal::is_valid("not-a-valid-ordinal"));
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
