//! Newtype wrappers for entity identifiers.
//!
//! These eliminate "primitive obsession" for entity-layer IDs that are
//! distinct from the name types defined in `swissarmyhammer-fields`.
//!
//! Uses the `define_id!` macro from `swissarmyhammer-common`.

use serde::{Deserialize, Serialize};
use std::fmt;

// Import the canonical macro from common.
use swissarmyhammer_common::define_id;

define_id!(EntityId, "An entity instance ID (ULID or slug)");
define_id!(ChangeEntryId, "A changelog entry ULID");
define_id!(TransactionId, "A transaction ULID");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn entity_id_from_str() {
        let id = EntityId::from("01ABC");
        assert_eq!(id.as_str(), "01ABC");
        assert_eq!(id, "01ABC");
    }

    #[test]
    fn change_entry_id_new() {
        let id = ChangeEntryId::new();
        assert!(!id.as_str().is_empty());
    }

    #[test]
    fn transaction_id_display() {
        let id = TransactionId::from("TX001");
        assert_eq!(format!("{}", id), "TX001");
    }

    #[test]
    fn serde_transparent() {
        let id = EntityId::from("test-id");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test-id\"");
        let parsed: EntityId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }

    #[test]
    fn hash_map_lookup() {
        use std::collections::HashMap;
        let mut map: HashMap<EntityId, i32> = HashMap::new();
        map.insert(EntityId::from("id1"), 42);
        assert_eq!(map.get("id1"), Some(&42));
    }
}
