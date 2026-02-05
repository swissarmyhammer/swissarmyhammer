//! Notification sender for streaming session updates

use std::collections::HashMap;
use std::sync::RwLock;

use agent_client_protocol::SessionNotification;
use tokio::sync::broadcast;

/// Default buffer size for per-session notification channels.
const DEFAULT_SESSION_BUFFER_SIZE: usize = 256;

/// Notification sender that maintains per-session broadcast channels.
///
/// Each session gets its own broadcast channel so concurrent sessions
/// don't interfere with each other. Subscribers receive only notifications
/// for the session they subscribed to.
#[derive(Debug)]
pub struct NotificationSender {
    /// Per-session broadcast senders
    sessions: RwLock<HashMap<String, broadcast::Sender<SessionNotification>>>,
    /// Global sender for backward compatibility (catches notifications before session is created)
    global_sender: broadcast::Sender<SessionNotification>,
}

impl Clone for NotificationSender {
    fn clone(&self) -> Self {
        let sessions = self.sessions.read().unwrap();
        Self {
            sessions: RwLock::new(sessions.clone()),
            global_sender: self.global_sender.clone(),
        }
    }
}

impl NotificationSender {
    /// Create a new notification sender.
    ///
    /// Returns a tuple containing the sender and a global receiver for
    /// backward compatibility.
    pub fn new(buffer_size: usize) -> (Self, broadcast::Receiver<SessionNotification>) {
        let (global_sender, receiver) = broadcast::channel(buffer_size);
        (
            Self {
                sessions: RwLock::new(HashMap::new()),
                global_sender,
            },
            receiver,
        )
    }

    /// Register a session, creating its dedicated broadcast channel.
    pub fn register_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        if !sessions.contains_key(session_id) {
            let (tx, _) = broadcast::channel(DEFAULT_SESSION_BUFFER_SIZE);
            sessions.insert(session_id.to_string(), tx);
            tracing::debug!("Registered notification channel for session {}", session_id);
        }
    }

    /// Unregister a session, removing its broadcast channel.
    pub fn unregister_session(&self, session_id: &str) {
        let mut sessions = self.sessions.write().unwrap();
        sessions.remove(session_id);
    }

    /// Subscribe to notifications for a specific session.
    ///
    /// Returns a receiver that only gets notifications for this session.
    /// The session must be registered first via `register_session`.
    /// Falls back to the global channel if the session isn't registered.
    pub fn subscribe_session(&self, session_id: &str) -> broadcast::Receiver<SessionNotification> {
        let sessions = self.sessions.read().unwrap();
        if let Some(tx) = sessions.get(session_id) {
            tx.subscribe()
        } else {
            tracing::warn!(
                "subscribe_session called for unregistered session {}, using global channel",
                session_id
            );
            self.global_sender.subscribe()
        }
    }

    /// Send a session update notification.
    ///
    /// Routes to the session-specific channel if registered, otherwise
    /// falls back to the global channel.
    pub async fn send_update(&self, notification: SessionNotification) -> crate::Result<()> {
        let session_id: String = notification.session_id.0.to_string();

        // Send to session-specific channel if it exists
        let sent_to_session = {
            let sessions = self.sessions.read().unwrap();
            if let Some(tx) = sessions.get(&session_id) {
                let _ = tx.send(notification.clone());
                true
            } else {
                false
            }
        };

        // Always also send to global channel for backward compatibility
        let _ = self.global_sender.send(notification);

        if !sent_to_session {
            tracing::trace!(
                "No session-specific channel for {}, sent to global only",
                session_id
            );
        }

        Ok(())
    }

    /// Get a clone of the global broadcast sender (for backward compatibility).
    pub fn sender(&self) -> broadcast::Sender<SessionNotification> {
        self.global_sender.clone()
    }

    /// Get the per-session sender for a specific session.
    pub fn session_sender(&self, session_id: &str) -> Option<broadcast::Sender<SessionNotification>> {
        let sessions = self.sessions.read().unwrap();
        sessions.get(session_id).cloned()
    }
}
