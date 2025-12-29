//! NotifyingServer - Wraps a ServerHandler and captures outgoing notifications
//!
//! For our own MCP servers, we need to capture notifications at the source.
//! This module provides infrastructure to do that.

use crate::notification::{McpNotification, McpNotificationSource};
use rmcp::model::*;
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use rmcp::ServerHandler;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::broadcast;

/// A notification sender that handlers can use to emit captured notifications
#[derive(Clone)]
pub struct NotificationCapture {
    sender: broadcast::Sender<McpNotification>,
}

impl NotificationCapture {
    /// Create a new notification capture channel
    pub fn new() -> (Self, broadcast::Receiver<McpNotification>) {
        let (sender, receiver) = broadcast::channel(256);
        (Self { sender }, receiver)
    }

    /// Capture a progress notification
    pub fn capture_progress(&self, params: &ProgressNotificationParam) {
        let _ = self.sender.send(McpNotification::Progress(params.clone()));
    }

    /// Capture a logging notification
    pub fn capture_log(&self, params: &LoggingMessageNotificationParam) {
        let _ = self.sender.send(McpNotification::Log(params.clone()));
    }

    /// Get a subscriber to the notification stream
    pub fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        self.sender.subscribe()
    }
}

impl Default for NotificationCapture {
    fn default() -> Self {
        Self::new().0
    }
}

/// Wrapper that holds a ServerHandler and provides notification capture
pub struct NotifyingServer<H> {
    url: String,
    capture: NotificationCapture,
    _handle: tokio::task::JoinHandle<()>,
    _phantom: std::marker::PhantomData<H>,
}

impl<H> NotifyingServer<H> {
    /// Get the URL where clients should connect
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl<H: Send + Sync> McpNotificationSource for NotifyingServer<H> {
    fn url(&self) -> &str {
        &self.url
    }

    fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        self.capture.subscribe()
    }
}

/// Trait for ServerHandlers that support notification capture
///
/// Implement this trait for your ServerHandler to enable notification capture.
/// The handler receives a `NotificationCapture` which it should use to
/// emit notifications in addition to sending them via `context.peer.send_notification()`.
pub trait NotifyingServerHandler: ServerHandler + Clone + Send + Sync + 'static {
    /// Set the notification capture channel
    fn set_notification_capture(&mut self, capture: NotificationCapture);
}

/// Start an MCP server with notification capture
///
/// # Arguments
/// * `handler` - The ServerHandler implementation (must implement NotifyingServerHandler)
///
/// # Returns
/// A NotifyingServer that provides the URL and notification subscription
pub async fn start_notifying_server<H>(
    mut handler: H,
) -> Result<NotifyingServer<H>, Box<dyn std::error::Error + Send + Sync>>
where
    H: NotifyingServerHandler,
{
    let (capture, _initial_rx) = NotificationCapture::new();
    handler.set_notification_capture(capture.clone());

    let handler = Arc::new(handler);
    let handler_for_service = handler.clone();

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}/mcp", addr);

    tracing::info!("NotifyingServer starting on {}", url);

    let http_service = StreamableHttpService::new(
        move || Ok((*handler_for_service).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", http_service);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("NotifyingServer error: {}", e);
        }
    });

    // Small delay to ensure server is ready
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    Ok(NotifyingServer {
        url,
        capture,
        _handle: handle,
        _phantom: std::marker::PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_notification_capture() {
        let (capture, mut rx) = NotificationCapture::new();

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("test".into())),
            progress: 50.0,
            total: Some(100.0),
            message: Some("Test progress".to_string()),
        };

        capture.capture_progress(&params);

        // Check we can receive
        match rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 50.0);
            }
            _ => panic!("Expected progress notification"),
        }
    }
}
