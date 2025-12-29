//! Test utilities for ToolCallHandler
//!
//! This module provides helper functions for creating and working with
//! ToolCallHandler in tests.

use agent_client_protocol::{SessionId, SessionNotification};
use claude_agent::agent::NotificationSender;
use claude_agent::tools::ToolCallHandler;
use tokio::sync::broadcast;

use super::fixtures;

/// Create a test handler with notification sender and receiver
pub async fn create_handler_with_notifications(
) -> (ToolCallHandler, broadcast::Receiver<SessionNotification>) {
    let permissions = fixtures::tool_permissions();
    let session_manager = fixtures::session_manager();
    let permission_engine = fixtures::permission_engine();

    let mut handler = ToolCallHandler::new(permissions, session_manager, permission_engine);
    let (sender, receiver) = NotificationSender::new(32);
    handler.set_notification_sender(sender);

    (handler, receiver)
}

/// Create a test handler with custom permissions and notifications
pub async fn create_handler_with_custom_permissions(
    permissions: claude_agent::tools::ToolPermissions,
) -> (ToolCallHandler, broadcast::Receiver<SessionNotification>) {
    let session_manager = fixtures::session_manager();
    let permission_engine = fixtures::permission_engine();

    let mut handler = ToolCallHandler::new(permissions, session_manager, permission_engine);
    let (sender, receiver) = NotificationSender::new(32);
    handler.set_notification_sender(sender);

    (handler, receiver)
}

/// Create a test handler without notification sender
pub fn create_handler_without_notifications() -> ToolCallHandler {
    let permissions = fixtures::tool_permissions();
    let session_manager = fixtures::session_manager();
    let permission_engine = fixtures::permission_engine();

    ToolCallHandler::new(permissions, session_manager, permission_engine)
}

/// Try to consume a notification without blocking
/// Returns None if no notification is available
pub fn try_consume_notification(
    receiver: &mut broadcast::Receiver<SessionNotification>,
) -> Option<SessionNotification> {
    receiver.try_recv().ok()
}

/// Consume all pending notifications and return them
pub fn consume_all_notifications(
    receiver: &mut broadcast::Receiver<SessionNotification>,
) -> Vec<SessionNotification> {
    let mut notifications = Vec::new();
    while let Ok(notification) = receiver.try_recv() {
        notifications.push(notification);
    }
    notifications
}

/// Create a test session ID with a given identifier
pub fn test_session_id(id: &str) -> SessionId {
    SessionId::new(id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_create_handler_with_notifications() {
        let (_handler, _receiver) = create_handler_with_notifications().await;

        // Handler creation succeeded if we got here
    }

    #[tokio::test]
    async fn test_create_handler_with_custom_permissions() {
        let custom_perms = claude_agent::tools::ToolPermissions {
            require_permission_for: vec!["sensitive_tool".to_string()],
            auto_approved: vec!["safe_tool".to_string()],
            forbidden_paths: vec!["/etc".to_string()],
        };

        let (_handler, _receiver) = create_handler_with_custom_permissions(custom_perms).await;

        // Handler creation succeeded if we got here
    }

    #[test]
    fn test_create_handler_without_notifications() {
        let _handler = create_handler_without_notifications();

        // Handler creation succeeded if we got here
    }

    #[test]
    fn test_session_id_creation() {
        let id = test_session_id("test_123");
        assert_eq!(id.0.as_ref(), "test_123");
    }

    #[test]
    fn test_consume_all_notifications_empty() {
        let (_sender, mut receiver) = NotificationSender::new(32);
        let notifications = consume_all_notifications(&mut receiver);
        assert!(notifications.is_empty());
    }

    #[test]
    fn test_try_consume_notification_empty() {
        let (_sender, mut receiver) = NotificationSender::new(32);
        let notification = try_consume_notification(&mut receiver);
        assert!(notification.is_none());
    }
}
