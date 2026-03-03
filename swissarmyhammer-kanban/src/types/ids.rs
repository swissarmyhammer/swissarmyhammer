//! Newtype wrappers for identifiers to prevent mixing up IDs at compile time.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Macro to define ID newtypes with consistent derives and impls
macro_rules! define_id {
    ($name:ident, $doc:literal) => {
        #[doc = $doc]
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            /// Create a new ID with a fresh ULID
            pub fn new() -> Self {
                Self(ulid::Ulid::new().to_string())
            }

            /// Create an ID from an existing string
            pub fn from_string(s: impl Into<String>) -> Self {
                Self(s.into())
            }

            /// Get the inner string value
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(s: &str) -> Self {
                Self(s.to_string())
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }
    };
}

// Define all ID types
define_id!(TaskId, "ULID-based identifier for tasks");
define_id!(ColumnId, "Identifier for columns (slug-style)");
define_id!(SubtaskId, "ULID-based identifier for subtasks");
define_id!(AttachmentId, "ULID-based identifier for attachments");
define_id!(LogEntryId, "ULID-based identifier for log entries");
define_id!(SwimlaneId, "Identifier for swimlanes (slug-style)");
define_id!(ActorId, "Identifier for actors (people or agents)");
define_id!(TagId, "Identifier for tags (slug-style)");
define_id!(CommentId, "ULID-based identifier for comments");

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
