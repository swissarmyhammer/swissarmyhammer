//! Newtype wrappers for identifiers to prevent mixing up IDs at compile time.

use serde::{Deserialize, Serialize};
use std::fmt;

// Import the canonical macro from common.
use swissarmyhammer_common::define_id;

// Define all ID types
define_id!(TaskId, "ULID-based identifier for tasks");
define_id!(ColumnId, "Identifier for columns (slug-style)");
define_id!(LogEntryId, "ULID-based identifier for log entries");
define_id!(ActorId, "Identifier for actors (people or agents)");
define_id!(TagId, "ULID-based identifier for tags");
define_id!(ProjectId, "Identifier for projects (slug-style)");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_creation() {
        let task_id = TaskId::new();
        assert!(!task_id.0.is_empty());

        let column_id = ColumnId::from_string("todo");
        assert_eq!(column_id.as_str(), "todo");
    }

    #[test]
    fn test_id_display() {
        let id = TaskId::from_string("test-id");
        assert_eq!(format!("{}", id), "test-id");
    }

    #[test]
    fn test_id_serialization() {
        let id = TaskId::from_string("test-id");
        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, "\"test-id\"");

        let parsed: TaskId = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, id);
    }
}
