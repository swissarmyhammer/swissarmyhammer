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
