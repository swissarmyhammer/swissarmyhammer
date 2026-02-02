//! Plan notification infrastructure for MCP tools
//!
//! This module provides notification types for sending task plan updates
//! that can be converted to ACP Plan format by agents.
//!
//! # Purpose
//!
//! When task management tools (like kanban) modify tasks, they emit plan notifications
//! containing the complete current task list. Agents subscribing to these notifications
//! can convert them to ACP Plan format for client communication.
//!
//! # Design
//!
//! - **Channel-based**: Uses tokio channels for async, non-blocking notification delivery
//! - **Complete Plan**: Always sends the full task list (ACP requires complete plan replacement)
//! - **Generic Format**: Tasks are in a generic format that can map to ACP or other protocols
//!
//! # ACP Compliance
//!
//! Per ACP spec: "Complete plan lists must be resent with each update; clients will replace
//! prior plans entirely."

use serde::{Deserialize, Serialize};
use swissarmyhammer_common::{ErrorSeverity, Severity};
use tokio::sync::mpsc;

/// Status of a plan entry
///
/// Maps to ACP PlanEntryStatus: pending, in_progress, completed
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanEntryStatus {
    /// Entry is pending (todo column)
    Pending,
    /// Entry is in progress (doing column)
    InProgress,
    /// Entry is completed (done column)
    Completed,
}

/// Priority of a plan entry
///
/// Maps to ACP PlanEntryPriority: high, medium, low
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PlanEntryPriority {
    High,
    Medium,
    Low,
}

/// A single entry in a plan notification
///
/// Designed to map cleanly to ACP PlanEntry format while remaining generic.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanEntry {
    /// Unique identifier (maps to ACP meta.id)
    pub id: String,

    /// Human-readable task description (maps to ACP content)
    pub content: String,

    /// Task status derived from kanban column position
    pub status: PlanEntryStatus,

    /// Task priority (derived from position ordinal or explicit)
    pub priority: PlanEntryPriority,

    /// Optional notes/description
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,

    /// Source column for debugging/context
    #[serde(skip_serializing_if = "Option::is_none")]
    pub column: Option<String>,
}

impl PlanEntry {
    /// Create a new plan entry
    pub fn new(
        id: impl Into<String>,
        content: impl Into<String>,
        status: PlanEntryStatus,
        priority: PlanEntryPriority,
    ) -> Self {
        Self {
            id: id.into(),
            content: content.into(),
            status,
            priority,
            notes: None,
            column: None,
        }
    }

    /// Add notes to this entry
    pub fn with_notes(mut self, notes: impl Into<String>) -> Self {
        self.notes = Some(notes.into());
        self
    }

    /// Add column info to this entry
    pub fn with_column(mut self, column: impl Into<String>) -> Self {
        self.column = Some(column.into());
        self
    }
}

/// Plan notification containing complete task list
///
/// When emitted, contains ALL current tasks from the kanban board.
/// Per ACP spec, clients replace prior plans entirely on each update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNotification {
    /// All plan entries (complete replacement)
    pub entries: Vec<PlanEntry>,

    /// Operation that triggered this notification
    pub trigger: String,

    /// Optional task ID that was affected (for add, update, delete, move, complete)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affected_task_id: Option<String>,

    /// Source of this plan (e.g., "kanban")
    pub source: String,
}

impl PlanNotification {
    /// Create a new plan notification
    pub fn new(entries: Vec<PlanEntry>, trigger: impl Into<String>) -> Self {
        Self {
            entries,
            trigger: trigger.into(),
            affected_task_id: None,
            source: "kanban".to_string(),
        }
    }

    /// Set the affected task ID
    pub fn with_affected_task(mut self, task_id: impl Into<String>) -> Self {
        self.affected_task_id = Some(task_id.into());
        self
    }

    /// Set the source
    pub fn with_source(mut self, source: impl Into<String>) -> Self {
        self.source = source.into();
        self
    }
}

/// Error type for plan notification sending failures
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// Channel was closed, receiver no longer exists
    #[error("Plan notification channel closed: {0}")]
    ChannelClosed(String),
}

impl Severity for SendError {
    fn severity(&self) -> ErrorSeverity {
        // Warning: Notification failures should not block operations
        match self {
            SendError::ChannelClosed(_) => ErrorSeverity::Warning,
        }
    }
}

/// Plan notification sender with channel-based async delivery
///
/// Wraps a channel sender to provide typed notification sending.
/// The sender is cloneable and can be shared across tasks.
#[derive(Clone)]
pub struct PlanSender {
    sender: mpsc::UnboundedSender<PlanNotification>,
}

impl PlanSender {
    /// Create a new plan sender
    pub fn new(sender: mpsc::UnboundedSender<PlanNotification>) -> Self {
        Self { sender }
    }

    /// Send a plan notification through the channel
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification to send
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send(&self, notification: PlanNotification) -> Result<(), SendError> {
        self.sender
            .send(notification)
            .map_err(|e| SendError::ChannelClosed(e.to_string()))
    }

    /// Send a plan update with entries
    ///
    /// Convenience method for common use case.
    pub fn send_plan(
        &self,
        entries: Vec<PlanEntry>,
        trigger: &str,
        affected_task_id: Option<&str>,
    ) -> Result<(), SendError> {
        let mut notification = PlanNotification::new(entries, trigger);
        if let Some(id) = affected_task_id {
            notification = notification.with_affected_task(id);
        }
        self.send(notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_entry_creation() {
        let entry = PlanEntry::new("task-1", "Do something", PlanEntryStatus::Pending, PlanEntryPriority::Medium);
        assert_eq!(entry.id, "task-1");
        assert_eq!(entry.content, "Do something");
        assert_eq!(entry.status, PlanEntryStatus::Pending);
        assert_eq!(entry.priority, PlanEntryPriority::Medium);
    }

    #[test]
    fn test_plan_entry_with_notes() {
        let entry = PlanEntry::new("task-1", "Do something", PlanEntryStatus::InProgress, PlanEntryPriority::High)
            .with_notes("Important task")
            .with_column("doing");

        assert_eq!(entry.notes, Some("Important task".to_string()));
        assert_eq!(entry.column, Some("doing".to_string()));
    }

    #[test]
    fn test_plan_notification_creation() {
        let entries = vec![
            PlanEntry::new("task-1", "First task", PlanEntryStatus::Completed, PlanEntryPriority::Low),
            PlanEntry::new("task-2", "Second task", PlanEntryStatus::Pending, PlanEntryPriority::Medium),
        ];

        let notification = PlanNotification::new(entries, "add task")
            .with_affected_task("task-2");

        assert_eq!(notification.entries.len(), 2);
        assert_eq!(notification.trigger, "add task");
        assert_eq!(notification.affected_task_id, Some("task-2".to_string()));
        assert_eq!(notification.source, "kanban");
    }

    #[tokio::test]
    async fn test_plan_sender_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = PlanSender::new(tx);

        let entries = vec![
            PlanEntry::new("task-1", "Test task", PlanEntryStatus::Pending, PlanEntryPriority::High),
        ];

        sender.send_plan(entries, "add task", Some("task-1")).unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.entries.len(), 1);
        assert_eq!(notification.trigger, "add task");
        assert_eq!(notification.affected_task_id, Some("task-1".to_string()));
    }

    #[tokio::test]
    async fn test_plan_sender_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        let sender = PlanSender::new(tx);

        drop(rx);

        let result = sender.send_plan(vec![], "test", None);
        assert!(result.is_err());
    }

    #[test]
    fn test_plan_entry_serialization() {
        let entry = PlanEntry::new("task-1", "Test", PlanEntryStatus::InProgress, PlanEntryPriority::High);
        let json = serde_json::to_value(&entry).unwrap();

        assert_eq!(json["id"], "task-1");
        assert_eq!(json["status"], "in_progress");
        assert_eq!(json["priority"], "high");
    }

    #[test]
    fn test_send_error_severity() {
        let error = SendError::ChannelClosed("test".to_string());
        assert_eq!(error.severity(), ErrorSeverity::Warning);
    }
}
