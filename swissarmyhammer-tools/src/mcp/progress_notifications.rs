//! Generic progress notification infrastructure for MCP tool operations
//!
//! This module provides notification types and utilities for sending progress updates
//! during long-running tool execution via MCP progress notifications.
//!
//! # Purpose
//!
//! `ProgressNotification` is designed for generic tool progress updates during execution,
//! separate from `FlowNotification` which is specifically for workflow state machine transitions.
//! Tools can optionally send progress notifications to provide real-time feedback to users
//! without blocking execution.
//!
//! # Design
//!
//! - **Channel-based**: Uses tokio channels for async, non-blocking notification delivery
//! - **Optional**: Progress sender is optional in ToolContext to avoid overhead when not needed
//! - **ULID Tokens**: Each operation gets a unique ULID token for tracking
//! - **Metadata Support**: Tools can include custom metadata in notifications
//!
//! # Examples
//!
//! ```ignore
//! // In a tool execute method
//! if let Some(sender) = &context.progress_sender {
//!     let token = generate_progress_token();
//!     
//!     sender.send_progress(&token, Some(0), "Starting operation")?;
//!     
//!     // Do work...
//!     sender.send_progress(&token, Some(50), "Halfway done")?;
//!     
//!     // Do more work...
//!     sender.send_progress(&token, Some(100), "Complete")?;
//! }
//! ```

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Progress notification for MCP tool operations
///
/// Represents a single progress update during tool execution. Notifications
/// are sent asynchronously via channels and serialized as MCP progress notifications.
///
/// # Fields
///
/// * `progress_token` - Unique identifier for this operation (typically a ULID)
/// * `progress` - Progress percentage (0-100), None for indeterminate progress
/// * `message` - Human-readable progress message
/// * `metadata` - Optional tool-specific metadata as JSON
///
/// # Examples
///
/// ```ignore
/// let notification = ProgressNotification {
///     progress_token: "01K7SMD16Z48DXJCQN0XJJ66N9".to_string(),
///     progress: Some(50),
///     message: "Indexing files: 50/100".to_string(),
///     metadata: Some(serde_json::json!({"files_processed": 50, "total_files": 100})),
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressNotification {
    /// Unique token for this operation (ULID)
    pub progress_token: String,

    /// Progress percentage (0-100), None for indeterminate
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress: Option<u32>,

    /// Human-readable progress message
    pub message: String,

    /// Tool-specific metadata
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<serde_json::Value>,
}

/// Error type for notification sending failures
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// Channel was closed, receiver no longer exists
    #[error("Progress notification channel closed: {0}")]
    ChannelClosed(String),
}

/// Progress notification sender with channel-based async delivery
///
/// Wraps a channel sender to provide typed notification sending with error handling.
/// The sender is cloneable and can be shared across tasks.
///
/// # Examples
///
/// ```ignore
/// let (tx, rx) = mpsc::unbounded_channel();
/// let sender = ProgressSender::new(tx);
///
/// sender.send_progress("token_123", Some(0), "Starting").unwrap();
/// ```
#[derive(Clone)]
pub struct ProgressSender {
    sender: mpsc::UnboundedSender<ProgressNotification>,
}

impl ProgressSender {
    /// Create a new progress sender
    ///
    /// # Arguments
    ///
    /// * `sender` - The underlying channel sender
    ///
    /// # Returns
    ///
    /// A new `ProgressSender` wrapping the channel
    pub fn new(sender: mpsc::UnboundedSender<ProgressNotification>) -> Self {
        Self { sender }
    }

    /// Send a progress notification through the channel
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification to send
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send(&self, notification: ProgressNotification) -> Result<(), SendError> {
        self.sender
            .send(notification)
            .map_err(|e| SendError::ChannelClosed(e.to_string()))
    }

    /// Convenience method to send progress with token, progress %, and message
    ///
    /// # Arguments
    ///
    /// * `token` - The progress token (operation identifier)
    /// * `progress` - Optional progress percentage (0-100)
    /// * `message` - Human-readable progress message
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_progress(
        &self,
        token: &str,
        progress: Option<u32>,
        message: impl Into<String>,
    ) -> Result<(), SendError> {
        self.send(ProgressNotification {
            progress_token: token.to_string(),
            progress,
            message: message.into(),
            metadata: None,
        })
    }

    /// Send progress with metadata
    ///
    /// # Arguments
    ///
    /// * `token` - The progress token (operation identifier)
    /// * `progress` - Optional progress percentage (0-100)
    /// * `message` - Human-readable progress message
    /// * `metadata` - Tool-specific metadata as JSON
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_progress_with_metadata(
        &self,
        token: &str,
        progress: Option<u32>,
        message: impl Into<String>,
        metadata: serde_json::Value,
    ) -> Result<(), SendError> {
        self.send(ProgressNotification {
            progress_token: token.to_string(),
            progress,
            message: message.into(),
            metadata: Some(metadata),
        })
    }
}

/// Generate a unique progress token
///
/// Creates a unique token that can be used as a progress token for tracking
/// an operation's progress updates. Uses a combination of timestamp and random
/// bytes to ensure uniqueness.
///
/// # Returns
///
/// A unique token string
///
/// # Examples
///
/// ```ignore
/// let token = generate_progress_token();
/// sender.send_progress(&token, Some(0), "Starting")?;
/// ```
pub fn generate_progress_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();

    let random_bytes: [u8; 8] = rand::random();
    let random_hex = random_bytes
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect::<String>();

    format!("progress_{:016x}_{}", timestamp, random_hex)
}

/// Create a progress notification for operation start
///
/// Convenience function to create a standardized start notification
/// with 0% progress.
///
/// # Arguments
///
/// * `token` - The progress token
/// * `operation` - Name or description of the operation
///
/// # Returns
///
/// A `ProgressNotification` with 0% progress
///
/// # Examples
///
/// ```ignore
/// let notification = start_notification("token_123", "Indexing files");
/// sender.send(notification)?;
/// ```
pub fn start_notification(token: &str, operation: impl Into<String>) -> ProgressNotification {
    ProgressNotification {
        progress_token: token.to_string(),
        progress: Some(0),
        message: format!("Starting: {}", operation.into()),
        metadata: None,
    }
}

/// Create a progress notification for operation completion
///
/// Convenience function to create a standardized completion notification
/// with 100% progress.
///
/// # Arguments
///
/// * `token` - The progress token
/// * `operation` - Name or description of the operation
///
/// # Returns
///
/// A `ProgressNotification` with 100% progress
///
/// # Examples
///
/// ```ignore
/// let notification = complete_notification("token_123", "Indexing files");
/// sender.send(notification)?;
/// ```
pub fn complete_notification(token: &str, operation: impl Into<String>) -> ProgressNotification {
    ProgressNotification {
        progress_token: token.to_string(),
        progress: Some(100),
        message: format!("Completed: {}", operation.into()),
        metadata: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_progress_notification_creation() {
        let notification = ProgressNotification {
            progress_token: "test_token".to_string(),
            progress: Some(50),
            message: "Half done".to_string(),
            metadata: None,
        };

        assert_eq!(notification.progress_token, "test_token");
        assert_eq!(notification.progress, Some(50));
        assert_eq!(notification.message, "Half done");
        assert!(notification.metadata.is_none());
    }

    #[test]
    fn test_progress_notification_with_metadata() {
        let metadata = json!({"files": 50, "total": 100});
        let notification = ProgressNotification {
            progress_token: "test_token".to_string(),
            progress: Some(50),
            message: "Processing".to_string(),
            metadata: Some(metadata.clone()),
        };

        assert_eq!(notification.metadata, Some(metadata));
    }

    #[test]
    fn test_progress_notification_serialization() {
        let notification = ProgressNotification {
            progress_token: "token_123".to_string(),
            progress: Some(75),
            message: "Almost done".to_string(),
            metadata: Some(json!({"status": "active"})),
        };

        let json = serde_json::to_string(&notification).unwrap();
        let deserialized: ProgressNotification = serde_json::from_str(&json).unwrap();

        assert_eq!(notification.progress_token, deserialized.progress_token);
        assert_eq!(notification.progress, deserialized.progress);
        assert_eq!(notification.message, deserialized.message);
        assert_eq!(notification.metadata, deserialized.metadata);
    }

    #[test]
    fn test_progress_notification_indeterminate() {
        let notification = ProgressNotification {
            progress_token: "token_123".to_string(),
            progress: None,
            message: "Processing...".to_string(),
            metadata: None,
        };

        assert!(notification.progress.is_none());
    }

    #[tokio::test]
    async fn test_progress_sender_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        let notification = ProgressNotification {
            progress_token: "test".to_string(),
            progress: Some(50),
            message: "Test".to_string(),
            metadata: None,
        };

        sender.send(notification.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.progress_token, notification.progress_token);
        assert_eq!(received.message, notification.message);
    }

    #[tokio::test]
    async fn test_progress_sender_send_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        sender
            .send_progress("token_123", Some(25), "Quarter done")
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.progress_token, "token_123");
        assert_eq!(notification.progress, Some(25));
        assert_eq!(notification.message, "Quarter done");
        assert!(notification.metadata.is_none());
    }

    #[tokio::test]
    async fn test_progress_sender_send_progress_with_metadata() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        let metadata = json!({"files_processed": 10});
        sender
            .send_progress_with_metadata("token_123", Some(50), "Processing", metadata.clone())
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.progress_token, "token_123");
        assert_eq!(notification.progress, Some(50));
        assert_eq!(notification.message, "Processing");
        assert_eq!(notification.metadata, Some(metadata));
    }

    #[tokio::test]
    async fn test_progress_sender_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        // Drop the receiver to close the channel
        drop(rx);

        let result = sender.send_progress("token_123", Some(50), "Test");
        assert!(result.is_err());

        match result {
            Err(SendError::ChannelClosed(_)) => {}
            _ => panic!("Expected ChannelClosed error"),
        }
    }

    #[tokio::test]
    async fn test_progress_sender_multiple_sends() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        sender.send_progress("token_123", Some(0), "Start").unwrap();
        sender
            .send_progress("token_123", Some(50), "Middle")
            .unwrap();
        sender.send_progress("token_123", Some(100), "End").unwrap();

        let notif1 = rx.recv().await.unwrap();
        assert_eq!(notif1.progress, Some(0));

        let notif2 = rx.recv().await.unwrap();
        assert_eq!(notif2.progress, Some(50));

        let notif3 = rx.recv().await.unwrap();
        assert_eq!(notif3.progress, Some(100));
    }

    #[test]
    fn test_progress_sender_clone() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        let sender_clone = sender.clone();

        sender.send_progress("token_1", Some(25), "First").unwrap();
        sender_clone
            .send_progress("token_2", Some(75), "Second")
            .unwrap();
    }

    #[test]
    fn test_generate_progress_token() {
        let token1 = generate_progress_token();
        let token2 = generate_progress_token();

        // Tokens should be non-empty strings
        assert!(!token1.is_empty());
        assert!(!token2.is_empty());

        // Tokens should be unique
        assert_ne!(token1, token2);

        // Tokens should start with "progress_"
        assert!(token1.starts_with("progress_"));
        assert!(token2.starts_with("progress_"));

        // Tokens should have the expected format: progress_<timestamp>_<random>
        let parts: Vec<&str> = token1.split('_').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "progress");
    }

    #[test]
    fn test_start_notification() {
        let notification = start_notification("token_123", "file indexing");

        assert_eq!(notification.progress_token, "token_123");
        assert_eq!(notification.progress, Some(0));
        assert_eq!(notification.message, "Starting: file indexing");
        assert!(notification.metadata.is_none());
    }

    #[test]
    fn test_complete_notification() {
        let notification = complete_notification("token_123", "file indexing");

        assert_eq!(notification.progress_token, "token_123");
        assert_eq!(notification.progress, Some(100));
        assert_eq!(notification.message, "Completed: file indexing");
        assert!(notification.metadata.is_none());
    }

    #[test]
    fn test_send_error_display() {
        let error = SendError::ChannelClosed("test error".to_string());
        let display_str = format!("{}", error);
        assert!(display_str.contains("Progress notification channel closed"));
        assert!(display_str.contains("test error"));
    }

    #[tokio::test]
    async fn test_progress_sender_indeterminate_progress() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = ProgressSender::new(tx);

        sender
            .send_progress("token_123", None, "Working...")
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.progress_token, "token_123");
        assert!(notification.progress.is_none());
        assert_eq!(notification.message, "Working...");
    }

    #[test]
    fn test_progress_notification_json_without_metadata() {
        let notification = ProgressNotification {
            progress_token: "token_123".to_string(),
            progress: Some(50),
            message: "Test".to_string(),
            metadata: None,
        };

        let json = serde_json::to_value(&notification).unwrap();

        // Should not include metadata field when None
        assert!(json.get("metadata").is_none());
        assert_eq!(json.get("progress_token").unwrap(), "token_123");
        assert_eq!(json.get("progress").unwrap(), 50);
        assert_eq!(json.get("message").unwrap(), "Test");
    }

    #[test]
    fn test_progress_notification_json_with_metadata() {
        let metadata = json!({"key": "value"});
        let notification = ProgressNotification {
            progress_token: "token_123".to_string(),
            progress: Some(50),
            message: "Test".to_string(),
            metadata: Some(metadata.clone()),
        };

        let json = serde_json::to_value(&notification).unwrap();

        // Metadata should be flattened into the top level
        assert_eq!(json.get("key").unwrap(), "value");
    }
}
