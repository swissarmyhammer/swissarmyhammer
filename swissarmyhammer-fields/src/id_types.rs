//! Newtype wrappers for field and entity type names.
//!
//! These eliminate "primitive obsession" -- passing raw `String` values for
//! semantically distinct identifiers. The `define_id!` macro is imported from
//! `swissarmyhammer-common` and used here to define `FieldName` and
//! `EntityTypeName`.

use serde::{Deserialize, Serialize};
use std::fmt;

// Import the canonical macro from common.
use swissarmyhammer_common::define_id;

// Define shared name types used across crates.
define_id!(
    FieldName,
    "A field name (e.g. \"title\", \"status\", \"body\")"
);
define_id!(
    EntityTypeName,
    "An entity type name (e.g. \"task\", \"tag\", \"column\")"
);
define_id!(FieldDefId, "A field definition ID (ULID)");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn field_name_from_str() {
        let name = FieldName::from("title");
        assert_eq!(name.as_str(), "title");
        assert_eq!(name, "title");
    }

    #[test]
    fn entity_type_name_from_string() {
        let name = EntityTypeName::from(String::from("task"));
        assert_eq!(name.as_str(), "task");
        assert_eq!(format!("{}", name), "task");
    }

    #[test]
    fn serde_transparent_json() {
        let name = FieldName::from("status");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"status\"");
        let parsed: FieldName = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, name);
    }

    #[test]
    fn serde_transparent_yaml() {
        let name = EntityTypeName::from("task");
        let yaml = serde_yaml_ng::to_string(&name).unwrap();
        let parsed: EntityTypeName = serde_yaml_ng::from_str(&yaml).unwrap();
        assert_eq!(parsed, name);
    }

    #[test]
    fn hash_map_lookup_with_borrow() {
        use std::collections::HashMap;
        let mut map: HashMap<FieldName, i32> = HashMap::new();
        map.insert(FieldName::from("title"), 42);
        // Look up using &str thanks to Borrow<str>
        assert_eq!(map.get("title"), Some(&42));
    }

    #[test]
    fn partial_eq_with_str() {
        let name = FieldName::from("body");
        assert!(name == "body");
        assert!(name == *"body");
        assert!(name != "other");
    }
}
