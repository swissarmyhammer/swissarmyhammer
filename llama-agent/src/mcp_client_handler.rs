//! MCP ClientHandler that converts MCP notifications to ACP SessionNotifications

use agent_client_protocol::{SessionId, SessionNotification, SessionUpdate};
use rmcp::{
    model::{LoggingMessageNotificationParam, ProgressNotificationParam},
    service::{NotificationContext, RoleClient},
    ClientHandler,
};
use std::sync::Arc;
use tokio::sync::{broadcast, Mutex};

/// ClientHandler that forwards MCP notifications as ACP SessionNotifications
#[derive(Clone)]
pub struct NotifyingClientHandler {
    /// Broadcaster for ACP notifications
    notification_tx: broadcast::Sender<SessionNotification>,
    /// Current session context (set before tool calls)
    current_session: Arc<Mutex<Option<SessionId>>>,
}

impl NotifyingClientHandler {
    /// Create a new NotifyingClientHandler
    pub fn new(notification_tx: broadcast::Sender<SessionNotification>) -> Self {
        Self {
            notification_tx,
            current_session: Arc::new(Mutex::new(None)),
        }
    }

    /// Set the current session context before making MCP tool calls
    pub async fn set_session(&self, session_id: SessionId) {
        *self.current_session.lock().await = Some(session_id);
    }

    /// Clear the session context after tool calls complete
    pub async fn clear_session(&self) {
        *self.current_session.lock().await = None;
    }

    /// Broadcast an ACP notification if we have a session context
    fn broadcast_acp_notification(&self, session_id: SessionId, update: SessionUpdate) {
        let notification = SessionNotification::new(session_id, update);

        match self.notification_tx.send(notification) {
            Ok(count) => {
                tracing::debug!("Forwarded MCP notification as ACP to {} subscribers", count);
            }
            Err(e) => {
                tracing::warn!("Failed to forward MCP notification: {}", e);
            }
        }
    }
}

impl ClientHandler for NotifyingClientHandler {
    fn get_info(&self) -> rmcp::model::ClientInfo {
        rmcp::model::ClientInfo {
            protocol_version: Default::default(),
            capabilities: rmcp::model::ClientCapabilities::default(),
            client_info: rmcp::model::Implementation {
                name: "llama_agent_notifying_client".to_string(),
                title: Some("Llama Agent Notifying MCP Client".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                website_url: None,
                icons: None,
            },
        }
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "MCP progress notification: token={:?}, progress={}/{:?}, message={:?}",
            params.progress_token,
            params.progress,
            params.total,
            params.message
        );

        // Get current session if available
        if let Some(session_id) = self.current_session.lock().await.clone() {
            // For now, log as AgentMessageChunk with progress info
            // In the future, we could add a custom Progress update type to ACP
            if let Some(message) = params.message {
                let text = format!(
                    "[Progress: {:.0}%] {}",
                    (params.progress / params.total.unwrap_or(100.0)) * 100.0,
                    message
                );
                let text_content = agent_client_protocol::TextContent::new(text);
                let update =
                    SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                        agent_client_protocol::ContentBlock::Text(text_content),
                    ));
                self.broadcast_acp_notification(session_id, update);
            }
        } else {
            tracing::warn!("Received MCP progress notification but no session context available");
        }
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "MCP logging notification: level={:?}, logger={:?}, data={:?}",
            params.level,
            params.logger,
            params.data
        );

        // Get current session if available
        if let Some(session_id) = self.current_session.lock().await.clone() {
            // Convert logging message to AgentMessageChunk
            let message = if let Some(msg) = params.data.get("message") {
                msg.as_str()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| params.data.to_string())
            } else {
                params.data.to_string()
            };

            // Only forward Info and higher level messages to avoid spam
            if matches!(
                params.level,
                rmcp::model::LoggingLevel::Info
                    | rmcp::model::LoggingLevel::Warning
                    | rmcp::model::LoggingLevel::Error
                    | rmcp::model::LoggingLevel::Critical
            ) {
                let text = format!("[MCP] {}", message);
                let text_content = agent_client_protocol::TextContent::new(text);
                let update =
                    SessionUpdate::AgentMessageChunk(agent_client_protocol::ContentChunk::new(
                        agent_client_protocol::ContentBlock::Text(text_content),
                    ));
                self.broadcast_acp_notification(session_id, update);
            }
        } else {
            tracing::warn!("Received MCP logging notification but no session context available");
        }
    }
}
