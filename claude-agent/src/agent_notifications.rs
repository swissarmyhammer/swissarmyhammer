//! Notification sender for streaming session updates

use agent_client_protocol::SessionNotification;
use tokio::sync::broadcast;

/// Notification sender for streaming updates
///
/// Manages the broadcasting of session update notifications to multiple receivers.
/// This allows the agent to send real-time updates about session state changes,
/// streaming content, and tool execution results to interested subscribers.
#[derive(Debug, Clone)]
pub struct NotificationSender {
    /// The broadcast sender for distributing notifications
    sender: broadcast::Sender<SessionNotification>,
}

impl NotificationSender {
    /// Create a new notification sender with receiver
    ///
    /// Returns a tuple containing the sender and a receiver that can be used
    /// to listen for session update notifications. The receiver can be cloned
    /// to create multiple subscribers.
    ///
    /// # Parameters
    ///
    /// * `buffer_size` - The size of the broadcast channel buffer for notifications
    ///
    /// # Returns
    ///
    /// A tuple of (NotificationSender, Receiver) where the receiver can be used
    /// to subscribe to session update notifications.
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<SessionNotification>) {
        let (sender, receiver) = broadcast::channel(buffer_size);
        (Self { sender }, receiver)
    }

    /// Send a session update notification
    ///
    /// Broadcasts a session update notification to all subscribers. This is used
    /// to notify clients of real-time changes in session state, streaming content,
    /// or tool execution results.
    ///
    /// # Arguments
    ///
    /// * `notification` - The session notification to broadcast
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` if the notification was sent successfully, or an error
    /// if the broadcast channel has no receivers or encounters other issues.
    pub async fn send_update(&self, notification: SessionNotification) -> crate::Result<()> {
        self.sender
            .send(notification)
            .map_err(|_| crate::AgentError::Protocol("Failed to send notification".to_string()))?;
        Ok(())
    }

    /// Get a clone of the underlying broadcast sender
    pub fn sender(&self) -> broadcast::Sender<SessionNotification> {
        self.sender.clone()
    }
}
