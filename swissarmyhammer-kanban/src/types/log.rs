//! Log entry types for activity tracking

use super::ids::LogEntryId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A log entry recording an operation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogEntry {
    /// Unique ID for this log entry
    pub id: LogEntryId,

    /// When the operation occurred
    pub timestamp: DateTime<Utc>,

    /// Canonical op string (e.g., "add task")
    pub op: String,

    /// The normalized input parameters
    pub input: Value,

    /// The result (or error)
    pub output: Value,

    /// Who performed the operation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub actor: Option<String>,

    /// How long the operation took
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
            id: LogEntryId::new(),
            timestamp: Utc::now(),
            op: op.into(),
            input,
            output,
            actor,
            duration_ms,
        }
    }

    /// Create a log entry for a successful operation
    pub fn success(op: impl Into<String>, input: Value, output: Value, duration_ms: u64) -> Self {
        Self::new(op, input, output, None, duration_ms)
    }

    /// Create a log entry for a failed operation
    pub fn failure(op: impl Into<String>, input: Value, error: &str, duration_ms: u64) -> Self {
        Self::new(
            op,
            input,
            serde_json::json!({ "error": error }),
            None,
            duration_ms,
        )
    }

    /// Set the actor
    pub fn with_actor(mut self, actor: impl Into<String>) -> Self {
        self.actor = Some(actor.into());
        self
    }
}

/// Result of an operation execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperationResult {
    /// The operation ID
    pub op_id: LogEntryId,

    /// Whether the operation succeeded
    pub ok: bool,

    /// The response payload (if successful)
    pub data: Value,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// How long the operation took
    pub duration_ms: u64,

    /// How many retry attempts were made
    #[serde(default)]
    pub retries: usize,
}

impl OperationResult {
    /// Create a successful result
    pub fn success(op_id: LogEntryId, data: Value, duration_ms: u64) -> Self {
        Self {
            op_id,
            ok: true,
            data,
            error: None,
            duration_ms,
            retries: 0,
        }
    }

    /// Create a failed result
    pub fn failure(op_id: LogEntryId, error: impl Into<String>, duration_ms: u64) -> Self {
        Self {
            op_id,
            ok: false,
            data: Value::Null,
            error: Some(error.into()),
            duration_ms,
            retries: 0,
        }
    }

    /// Set the retry count
    pub fn with_retries(mut self, retries: usize) -> Self {
        self.retries = retries;
        self
    }

    /// Get the ID from the data if present
    pub fn get_id(&self) -> Option<&str> {
        self.data.get("id").and_then(|v| v.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_entry_creation() {
        let entry = LogEntry::success(
            "add task",
            serde_json::json!({"title": "Test"}),
            serde_json::json!({"id": "abc123"}),
            50,
        );

        assert_eq!(entry.op, "add task");
        assert_eq!(entry.duration_ms, 50);
        assert!(entry.actor.is_none());
    }

    #[test]
    fn test_log_entry_with_actor() {
        let entry = LogEntry::success("add task", Value::Null, Value::Null, 10)
            .with_actor("claude[session123]");

        assert_eq!(entry.actor, Some("claude[session123]".into()));
    }

    #[test]
    fn test_operation_result() {
        let result = OperationResult::success(
            LogEntryId::new(),
            serde_json::json!({"id": "task123", "title": "Test"}),
            25,
        );

        assert!(result.ok);
        assert_eq!(result.get_id(), Some("task123"));
    }

    #[test]
    fn test_operation_result_failure() {
        let result =
            OperationResult::failure(LogEntryId::new(), "Task not found", 5).with_retries(2);

        assert!(!result.ok);
        assert_eq!(result.error, Some("Task not found".into()));
        assert_eq!(result.retries, 2);
    }
}
