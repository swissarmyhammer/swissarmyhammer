//! Unique identifier for undo/redo entries, wrapping a ULID.

use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// A unique identifier for an undo entry, backed by a ULID.
///
/// ULIDs are lexicographically sortable and monotonically increasing,
/// making them ideal for changelog entry IDs where ordering matters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UndoEntryId(ulid::Ulid);

impl UndoEntryId {
    /// Generate a new unique `UndoEntryId`.
    ///
    /// No `Default` impl is provided because generating a random ID as a
    /// "default" is semantically misleading -- callers should be explicit.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self(ulid::Ulid::new())
    }

    /// Wrap an existing ULID as an `UndoEntryId`.
    pub fn from_ulid(ulid: ulid::Ulid) -> Self {
        Self(ulid)
    }

    /// Return the inner ULID value.
    pub fn as_ulid(&self) -> ulid::Ulid {
        self.0
    }
}

impl fmt::Display for UndoEntryId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl FromStr for UndoEntryId {
    type Err = ulid::DecodeError;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        ulid::Ulid::from_string(s).map(Self)
    }
}

impl Serialize for UndoEntryId {
    fn serialize<S: Serializer>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> Deserialize<'de> for UndoEntryId {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ulid::Ulid::from_string(&s)
            .map(Self)
            .map_err(serde::de::Error::custom)
    }
}

/// Serialized form of a store item's ID, as stored in the undo stack.
///
/// Each store's `ItemId` associated type converts to/from this via
/// `Display`/`FromStr`. This newtype exists so the undo stack can hold
/// item IDs from heterogeneous stores (e.g. `EntityId`, `PerspectiveId`)
/// without being generic over the ID type.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StoredItemId(String);

impl StoredItemId {
    /// Create from any type that implements `Display` (all `ItemId` types do).
    pub fn from_display(id: &impl fmt::Display) -> Self {
        Self(id.to_string())
    }

    /// Get the string representation for passing to store methods.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for StoredItemId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<&str> for StoredItemId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

impl From<String> for StoredItemId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_produces_unique_ids() {
        let a = UndoEntryId::new();
        let b = UndoEntryId::new();
        assert_ne!(a, b);
    }

    #[test]
    fn display_from_str_round_trip() {
        let id = UndoEntryId::new();
        let s = id.to_string();
        let parsed: UndoEntryId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn serde_json_round_trip() {
        let id = UndoEntryId::new();
        let json = serde_json::to_string(&id).unwrap();
        let parsed: UndoEntryId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn ordering_newer_is_greater() {
        let a = UndoEntryId::new();
        // Sleep a tiny bit to ensure different ULID timestamp
        std::thread::sleep(std::time::Duration::from_millis(2));
        let b = UndoEntryId::new();
        assert!(b > a);
    }

    #[test]
    fn from_ulid_and_as_ulid() {
        let ulid = ulid::Ulid::new();
        let id = UndoEntryId::from_ulid(ulid);
        assert_eq!(id.as_ulid(), ulid);
    }
}
