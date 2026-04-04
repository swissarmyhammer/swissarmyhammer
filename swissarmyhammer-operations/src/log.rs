//! Log entry types for operation tracking

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A log entry recording an operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique ID for this log entry (ULID format)
    pub id: String,

    /// When the operation occurred
    pub timestamp: DateTime<Utc>,

    /// Canonical op string (e.g., "add task", "move task")
    pub op: String,

    /// The normalized input parameters (as JSON)
    pub input: Value,

    /// The result value or error (as JSON)
    pub output: Value,

    /// Who performed the operation (optional)
    /// Format: "user_id" or "agent_name[session_id]"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,

    /// How long the operation took (milliseconds)
    pub duration_ms: u64,
}

impl LogEntry {
    /// Create a new log entry
    pub fn new(
        op: impl Into<String>,
        input: Value,
        output: Value,
        actor: Option<String>,
        duration_ms: u64,
    ) -> Self {
        Self {
            id: ulid::Ulid::new().to_string(),
            timestamp: Utc::now(),
            op: op.into(),
            input,
            output,
            actor,
            duration_ms,
        }
    }

    /// Set the actor
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_new_creates_entry_with_fields() {
        let entry = LogEntry::new(
            "add task",
            json!({"title": "t"}),
            json!({"id": "1"}),
            None,
            42,
        );
        assert_eq!(entry.op, "add task");
        assert_eq!(entry.input, json!({"title": "t"}));
        assert_eq!(entry.output, json!({"id": "1"}));
        assert!(entry.actor.is_none());
        assert_eq!(entry.duration_ms, 42);
        assert!(!entry.id.is_empty());
    }

    #[test]
    fn test_new_with_actor_param() {
        let entry = LogEntry::new(
            "move task",
            json!({}),
            json!({}),
            Some("alice".to_string()),
            10,
        );
        assert_eq!(entry.actor, Some("alice".to_string()));
    }

    #[test]
    fn test_with_actor_sets_actor() {
        let entry = LogEntry::new("get task", json!({}), json!({}), None, 0).with_actor("bob");
        assert_eq!(entry.actor, Some("bob".to_string()));
    }

    #[test]
    fn test_with_actor_overrides_none() {
        let entry = LogEntry::new("list tasks", json!({}), json!({}), None, 5)
            .with_actor("agent[session123]");
        assert_eq!(entry.actor, Some("agent[session123]".to_string()));
        assert_eq!(entry.duration_ms, 5);
    }

    #[test]
    fn test_serialize_roundtrip() {
        let entry = LogEntry::new("add task", json!({"x": 1}), json!({"y": 2}), None, 100);
        let serialized = serde_json::to_string(&entry).unwrap();
        let deserialized: LogEntry = serde_json::from_str(&serialized).unwrap();
        assert_eq!(deserialized.op, entry.op);
        assert_eq!(deserialized.input, entry.input);
        assert_eq!(deserialized.output, entry.output);
        assert!(deserialized.actor.is_none());
    }

    #[test]
    fn test_serialize_skips_none_actor() {
        let entry = LogEntry::new("test", json!({}), json!({}), None, 0);
        let serialized = serde_json::to_string(&entry).unwrap();
        assert!(!serialized.contains("actor"));
    }

    #[test]
    fn test_serialize_includes_some_actor() {
        let entry = LogEntry::new("test", json!({}), json!({}), Some("user1".into()), 0);
        let serialized = serde_json::to_string(&entry).unwrap();
        assert!(serialized.contains("\"actor\":\"user1\""));
    }
}
