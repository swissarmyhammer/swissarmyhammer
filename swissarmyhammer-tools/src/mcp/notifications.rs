//! Flow notification infrastructure for MCP progress updates
//!
//! This module provides notification types and utilities for sending progress updates
//! during long-running workflow execution via MCP notifications.

use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;

/// Flow notification for MCP progress updates
///
/// Represents a single notification event during workflow execution. Notifications
/// are sent asynchronously via channels and serialized as MCP progress notifications.
///
/// # Examples
///
/// ```ignore
/// let notification = FlowNotification::flow_start(
///     "run_12345",
///     "implement",
///     serde_json::json!({"issue": "bug-123"}),
///     "parse_issue"
/// );
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowNotification {
    /// Workflow run ID for tracking this specific execution
    pub token: String,

    /// Progress percentage (0-100), None for errors
    pub progress: Option<u32>,

    /// Human-readable progress message
    pub message: String,

    /// Structured metadata about the notification
    #[serde(flatten)]
    pub metadata: FlowNotificationMetadata,
}

/// Metadata for different types of flow notifications
///
/// Each variant represents a specific point in the workflow lifecycle and includes
/// relevant context for that event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FlowNotificationMetadata {
    /// Workflow execution started
    FlowStart {
        /// Name of the workflow being executed
        flow_name: String,

        /// Parameters passed to the workflow
        parameters: serde_json::Value,

        /// Initial state of the workflow
        initial_state: String,
    },

    /// Entering a workflow state
    StateStart {
        /// Name of the workflow
        flow_name: String,

        /// ID of the state being entered
        state_id: String,

        /// Description of what this state does
        state_description: String,
    },

    /// Completed a workflow state
    StateComplete {
        /// Name of the workflow
        flow_name: String,

        /// ID of the state that completed
        state_id: String,

        /// Next state to transition to, if any
        next_state: Option<String>,
    },

    /// Workflow completed successfully
    FlowComplete {
        /// Name of the workflow
        flow_name: String,

        /// Final status of the workflow
        status: String,

        /// Final state reached
        final_state: String,
    },

    /// Workflow encountered an error
    FlowError {
        /// Name of the workflow
        flow_name: String,

        /// Status at time of error
        status: String,

        /// State where error occurred
        error_state: String,

        /// Error message
        error: String,
    },
}

impl FlowNotification {
    /// Create a flow start notification
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow being executed
    /// * `parameters` - Workflow parameters as JSON value
    /// * `initial_state` - Starting state of the workflow
    ///
    /// # Returns
    ///
    /// A notification indicating workflow start with 0% progress
    pub fn flow_start(
        run_id: &str,
        flow_name: &str,
        parameters: serde_json::Value,
        initial_state: &str,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(0),
            message: format!("Starting workflow: {}", flow_name),
            metadata: FlowNotificationMetadata::FlowStart {
                flow_name: flow_name.to_string(),
                parameters,
                initial_state: initial_state.to_string(),
            },
        }
    }

    /// Create a state start notification
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `state_id` - ID of the state being entered
    /// * `state_description` - Human-readable description of the state
    /// * `progress` - Current progress percentage (0-100)
    ///
    /// # Returns
    ///
    /// A notification indicating entry into a workflow state
    pub fn state_start(
        run_id: &str,
        flow_name: &str,
        state_id: &str,
        state_description: &str,
        progress: u32,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(progress),
            message: format!("Entering state: {}", state_id),
            metadata: FlowNotificationMetadata::StateStart {
                flow_name: flow_name.to_string(),
                state_id: state_id.to_string(),
                state_description: state_description.to_string(),
            },
        }
    }

    /// Create a state complete notification
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `state_id` - ID of the state that completed
    /// * `next_state` - Next state to transition to, if any
    /// * `progress` - Current progress percentage (0-100)
    ///
    /// # Returns
    ///
    /// A notification indicating completion of a workflow state
    pub fn state_complete(
        run_id: &str,
        flow_name: &str,
        state_id: &str,
        next_state: Option<&str>,
        progress: u32,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(progress),
            message: format!("Completed state: {}", state_id),
            metadata: FlowNotificationMetadata::StateComplete {
                flow_name: flow_name.to_string(),
                state_id: state_id.to_string(),
                next_state: next_state.map(|s| s.to_string()),
            },
        }
    }

    /// Create a flow complete notification
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `status` - Final status of the workflow
    /// * `final_state` - Final state reached
    ///
    /// # Returns
    ///
    /// A notification indicating successful workflow completion with 100% progress
    pub fn flow_complete(run_id: &str, flow_name: &str, status: &str, final_state: &str) -> Self {
        Self {
            token: run_id.to_string(),
            progress: Some(100),
            message: format!("Completed workflow: {}", flow_name),
            metadata: FlowNotificationMetadata::FlowComplete {
                flow_name: flow_name.to_string(),
                status: status.to_string(),
                final_state: final_state.to_string(),
            },
        }
    }

    /// Create a flow error notification
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `status` - Status at time of error
    /// * `error_state` - State where error occurred
    /// * `error` - Error message
    ///
    /// # Returns
    ///
    /// A notification indicating workflow failure with no progress value
    pub fn flow_error(
        run_id: &str,
        flow_name: &str,
        status: &str,
        error_state: &str,
        error: &str,
    ) -> Self {
        Self {
            token: run_id.to_string(),
            progress: None,
            message: format!("Workflow failed: {}", flow_name),
            metadata: FlowNotificationMetadata::FlowError {
                flow_name: flow_name.to_string(),
                status: status.to_string(),
                error_state: error_state.to_string(),
                error: error.to_string(),
            },
        }
    }
}

/// Error type for notification sending failures
#[derive(Debug, Clone, thiserror::Error)]
pub enum SendError {
    /// Channel was closed, receiver no longer exists
    #[error("Notification channel closed: {0}")]
    ChannelClosed(String),
}

/// Notification sender for flow progress updates
///
/// Wraps a channel sender to provide typed notification sending with error handling.
/// The sender is cloneable and can be shared across tasks.
///
/// # Examples
///
/// ```ignore
/// let (tx, rx) = mpsc::unbounded_channel();
/// let sender = NotificationSender::new(tx);
///
/// sender.send_flow_start("run_123", "implement", json!({}), "start").await?;
/// ```
#[derive(Clone)]
pub struct NotificationSender {
    sender: mpsc::UnboundedSender<FlowNotification>,
}

impl NotificationSender {
    /// Create a new notification sender
    ///
    /// # Arguments
    ///
    /// * `sender` - The underlying channel sender
    ///
    /// # Returns
    ///
    /// A new `NotificationSender` wrapping the channel
    pub fn new(sender: mpsc::UnboundedSender<FlowNotification>) -> Self {
        Self { sender }
    }

    /// Send a notification through the channel
    ///
    /// # Arguments
    ///
    /// * `notification` - The notification to send
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send(&self, notification: FlowNotification) -> Result<(), SendError> {
        self.sender
            .send(notification)
            .map_err(|e| SendError::ChannelClosed(e.to_string()))
    }

    /// Send a flow start notification
    ///
    /// Convenience method that creates and sends a flow start notification.
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow being executed
    /// * `parameters` - Workflow parameters as JSON value
    /// * `initial_state` - Starting state of the workflow
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_flow_start(
        &self,
        run_id: &str,
        flow_name: &str,
        parameters: serde_json::Value,
        initial_state: &str,
    ) -> Result<(), SendError> {
        let notification =
            FlowNotification::flow_start(run_id, flow_name, parameters, initial_state);
        self.send(notification)
    }

    /// Send a state start notification
    ///
    /// Convenience method that creates and sends a state start notification.
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `state_id` - ID of the state being entered
    /// * `state_description` - Human-readable description of the state
    /// * `progress` - Current progress percentage (0-100)
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_state_start(
        &self,
        run_id: &str,
        flow_name: &str,
        state_id: &str,
        state_description: &str,
        progress: u32,
    ) -> Result<(), SendError> {
        let notification =
            FlowNotification::state_start(run_id, flow_name, state_id, state_description, progress);
        self.send(notification)
    }

    /// Send a state complete notification
    ///
    /// Convenience method that creates and sends a state complete notification.
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `state_id` - ID of the state that completed
    /// * `next_state` - Next state to transition to, if any
    /// * `progress` - Current progress percentage (0-100)
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_state_complete(
        &self,
        run_id: &str,
        flow_name: &str,
        state_id: &str,
        next_state: Option<&str>,
        progress: u32,
    ) -> Result<(), SendError> {
        let notification =
            FlowNotification::state_complete(run_id, flow_name, state_id, next_state, progress);
        self.send(notification)
    }

    /// Send a flow complete notification
    ///
    /// Convenience method that creates and sends a flow complete notification.
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `status` - Final status of the workflow
    /// * `final_state` - Final state reached
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_flow_complete(
        &self,
        run_id: &str,
        flow_name: &str,
        status: &str,
        final_state: &str,
    ) -> Result<(), SendError> {
        let notification = FlowNotification::flow_complete(run_id, flow_name, status, final_state);
        self.send(notification)
    }

    /// Send a flow error notification
    ///
    /// Convenience method that creates and sends a flow error notification.
    ///
    /// # Arguments
    ///
    /// * `run_id` - Unique identifier for this workflow execution
    /// * `flow_name` - Name of the workflow
    /// * `status` - Status at time of error
    /// * `error_state` - State where error occurred
    /// * `error` - Error message
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, `Err(SendError)` if the channel is closed
    pub fn send_flow_error(
        &self,
        run_id: &str,
        flow_name: &str,
        status: &str,
        error_state: &str,
        error: &str,
    ) -> Result<(), SendError> {
        let notification =
            FlowNotification::flow_error(run_id, flow_name, status, error_state, error);
        self.send(notification)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_flow_notification_flow_start() {
        let notification = FlowNotification::flow_start(
            "run_123",
            "implement",
            json!({"issue": "bug-456"}),
            "parse_issue",
        );

        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(0));
        assert_eq!(notification.message, "Starting workflow: implement");

        match notification.metadata {
            FlowNotificationMetadata::FlowStart {
                flow_name,
                parameters,
                initial_state,
            } => {
                assert_eq!(flow_name, "implement");
                assert_eq!(parameters, json!({"issue": "bug-456"}));
                assert_eq!(initial_state, "parse_issue");
            }
            _ => panic!("Expected FlowStart metadata"),
        }
    }

    #[test]
    fn test_flow_notification_state_start() {
        let notification = FlowNotification::state_start(
            "run_123",
            "implement",
            "parse_issue",
            "Parse the issue specification",
            25,
        );

        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(25));
        assert_eq!(notification.message, "Entering state: parse_issue");

        match notification.metadata {
            FlowNotificationMetadata::StateStart {
                flow_name,
                state_id,
                state_description,
            } => {
                assert_eq!(flow_name, "implement");
                assert_eq!(state_id, "parse_issue");
                assert_eq!(state_description, "Parse the issue specification");
            }
            _ => panic!("Expected StateStart metadata"),
        }
    }

    #[test]
    fn test_flow_notification_state_complete() {
        let notification = FlowNotification::state_complete(
            "run_123",
            "implement",
            "parse_issue",
            Some("generate_code"),
            50,
        );

        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(50));
        assert_eq!(notification.message, "Completed state: parse_issue");

        match notification.metadata {
            FlowNotificationMetadata::StateComplete {
                flow_name,
                state_id,
                next_state,
            } => {
                assert_eq!(flow_name, "implement");
                assert_eq!(state_id, "parse_issue");
                assert_eq!(next_state, Some("generate_code".to_string()));
            }
            _ => panic!("Expected StateComplete metadata"),
        }
    }

    #[test]
    fn test_flow_notification_state_complete_no_next() {
        let notification =
            FlowNotification::state_complete("run_123", "implement", "final_state", None, 100);

        match notification.metadata {
            FlowNotificationMetadata::StateComplete { next_state, .. } => {
                assert_eq!(next_state, None);
            }
            _ => panic!("Expected StateComplete metadata"),
        }
    }

    #[test]
    fn test_flow_notification_flow_complete() {
        let notification =
            FlowNotification::flow_complete("run_123", "implement", "completed", "done");

        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(100));
        assert_eq!(notification.message, "Completed workflow: implement");

        match notification.metadata {
            FlowNotificationMetadata::FlowComplete {
                flow_name,
                status,
                final_state,
            } => {
                assert_eq!(flow_name, "implement");
                assert_eq!(status, "completed");
                assert_eq!(final_state, "done");
            }
            _ => panic!("Expected FlowComplete metadata"),
        }
    }

    #[test]
    fn test_flow_notification_flow_error() {
        let notification = FlowNotification::flow_error(
            "run_123",
            "implement",
            "failed",
            "generate_code",
            "Compilation failed",
        );

        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, None);
        assert_eq!(notification.message, "Workflow failed: implement");

        match notification.metadata {
            FlowNotificationMetadata::FlowError {
                flow_name,
                status,
                error_state,
                error,
            } => {
                assert_eq!(flow_name, "implement");
                assert_eq!(status, "failed");
                assert_eq!(error_state, "generate_code");
                assert_eq!(error, "Compilation failed");
            }
            _ => panic!("Expected FlowError metadata"),
        }
    }

    #[test]
    fn test_flow_notification_serialization() {
        let notification =
            FlowNotification::flow_start("run_123", "implement", json!({"test": "value"}), "start");

        let json = serde_json::to_string(&notification).unwrap();
        let deserialized: FlowNotification = serde_json::from_str(&json).unwrap();

        assert_eq!(notification.token, deserialized.token);
        assert_eq!(notification.progress, deserialized.progress);
        assert_eq!(notification.message, deserialized.message);
    }

    #[tokio::test]
    async fn test_notification_sender_send() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let notification = FlowNotification::flow_start("run_123", "test", json!({}), "start");

        sender.send(notification.clone()).unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.token, notification.token);
        assert_eq!(received.message, notification.message);
    }

    #[tokio::test]
    async fn test_notification_sender_send_flow_start() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_flow_start("run_123", "test", json!({"key": "value"}), "start")
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(0));
        assert_eq!(notification.message, "Starting workflow: test");
    }

    #[tokio::test]
    async fn test_notification_sender_send_state_start() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_state_start("run_123", "test", "state1", "First state", 25)
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(25));
    }

    #[tokio::test]
    async fn test_notification_sender_send_state_complete() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_state_complete("run_123", "test", "state1", Some("state2"), 50)
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(50));
    }

    #[tokio::test]
    async fn test_notification_sender_send_flow_complete() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_flow_complete("run_123", "test", "completed", "done")
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, Some(100));
    }

    #[tokio::test]
    async fn test_notification_sender_send_flow_error() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_flow_error("run_123", "test", "failed", "error_state", "Test error")
            .unwrap();

        let notification = rx.recv().await.unwrap();
        assert_eq!(notification.token, "run_123");
        assert_eq!(notification.progress, None);
        assert!(notification.message.contains("failed"));
    }

    #[tokio::test]
    async fn test_notification_sender_channel_closed() {
        let (tx, rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        // Drop the receiver to close the channel
        drop(rx);

        let result = sender.send_flow_start("run_123", "test", json!({}), "start");
        assert!(result.is_err());

        match result {
            Err(SendError::ChannelClosed(_)) => {}
            _ => panic!("Expected ChannelClosed error"),
        }
    }

    #[tokio::test]
    async fn test_notification_sender_multiple_sends() {
        let (tx, mut rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        sender
            .send_flow_start("run_123", "test", json!({}), "start")
            .unwrap();
        sender
            .send_state_start("run_123", "test", "state1", "State 1", 25)
            .unwrap();
        sender
            .send_state_complete("run_123", "test", "state1", Some("state2"), 50)
            .unwrap();
        sender
            .send_flow_complete("run_123", "test", "completed", "done")
            .unwrap();

        let notif1 = rx.recv().await.unwrap();
        assert_eq!(notif1.progress, Some(0));

        let notif2 = rx.recv().await.unwrap();
        assert_eq!(notif2.progress, Some(25));

        let notif3 = rx.recv().await.unwrap();
        assert_eq!(notif3.progress, Some(50));

        let notif4 = rx.recv().await.unwrap();
        assert_eq!(notif4.progress, Some(100));
    }

    #[test]
    fn test_notification_sender_clone() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let sender = NotificationSender::new(tx);

        let sender_clone = sender.clone();

        sender
            .send_flow_start("run_123", "test", json!({}), "start")
            .unwrap();
        sender_clone
            .send_flow_start("run_456", "test2", json!({}), "start")
            .unwrap();
    }

    #[test]
    fn test_send_error_display() {
        let error = SendError::ChannelClosed("test error".to_string());
        let display_str = format!("{}", error);
        assert!(display_str.contains("Notification channel closed"));
        assert!(display_str.contains("test error"));
    }
}
