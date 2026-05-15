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

    #[test]
    fn test_capture_log_notification() {
        let (capture, mut rx) = NotificationCapture::new();

        let params = LoggingMessageNotificationParam {
            level: LoggingLevel::Info,
            logger: Some("test-logger".to_string()),
            data: serde_json::json!("log message"),
        };

        capture.capture_log(&params);

        match rx.try_recv() {
            Ok(McpNotification::Log(l)) => {
                assert_eq!(l.logger.as_deref(), Some("test-logger"));
                assert_eq!(l.data, serde_json::json!("log message"));
            }
            _ => panic!("Expected log notification"),
        }
    }

    #[test]
    fn test_notification_capture_subscribe_creates_new_receiver() {
        let (capture, _initial_rx) = NotificationCapture::new();

        // subscribe() creates a new receiver independent of the initial one
        let mut sub_rx = capture.subscribe();

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("sub-test".into())),
            progress: 75.0,
            total: None,
            message: None,
        };

        capture.capture_progress(&params);

        match sub_rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 75.0);
            }
            _ => panic!("Expected progress notification on subscribed receiver"),
        }
    }

    #[test]
    fn test_notification_capture_multiple_subscribers() {
        let (capture, mut rx1) = NotificationCapture::new();
        let mut rx2 = capture.subscribe();
        let mut rx3 = capture.subscribe();

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("multi".into())),
            progress: 10.0,
            total: Some(50.0),
            message: Some("multi-sub".to_string()),
        };

        capture.capture_progress(&params);

        // All three receivers should get the same notification
        for (i, rx) in [&mut rx1, &mut rx2, &mut rx3].iter_mut().enumerate() {
            match rx.try_recv() {
                Ok(McpNotification::Progress(p)) => {
                    assert_eq!(p.progress, 10.0, "receiver {} got wrong progress", i);
                }
                _ => panic!("receiver {} did not get progress notification", i),
            }
        }
    }

    #[test]
    fn test_notification_capture_default() {
        let capture = NotificationCapture::default();

        // default() should create a working capture (no initial receiver)
        let mut rx = capture.subscribe();

        let params = LoggingMessageNotificationParam {
            level: LoggingLevel::Warning,
            logger: None,
            data: serde_json::json!("default test"),
        };

        capture.capture_log(&params);

        match rx.try_recv() {
            Ok(McpNotification::Log(l)) => {
                assert_eq!(l.data, serde_json::json!("default test"));
            }
            _ => panic!("Expected log notification from default capture"),
        }
    }

    #[test]
    fn test_notification_capture_no_receivers_does_not_panic() {
        // If all receivers are dropped, sending should silently succeed (not panic)
        let (capture, rx) = NotificationCapture::new();
        drop(rx);

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("dropped".into())),
            progress: 1.0,
            total: None,
            message: None,
        };

        // This should not panic even though there are no receivers
        capture.capture_progress(&params);

        let log_params = LoggingMessageNotificationParam {
            level: LoggingLevel::Error,
            logger: None,
            data: serde_json::json!("dropped"),
        };
        capture.capture_log(&log_params);
    }

    #[test]
    fn test_notification_capture_clone() {
        let (capture, _rx) = NotificationCapture::new();
        let cloned = capture.clone();

        // Both the original and clone should send to the same channel
        let mut rx = cloned.subscribe();

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("clone-test".into())),
            progress: 42.0,
            total: None,
            message: None,
        };

        // Send from original
        capture.capture_progress(&params);

        match rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 42.0);
            }
            _ => panic!("Expected notification from original capture on cloned subscriber"),
        }
    }

    #[test_log::test(tokio::test)]
    async fn test_start_notifying_server_binds_and_provides_url() {
        // Create a minimal handler that implements NotifyingServerHandler
        #[derive(Clone)]
        struct TestHandler {
            _capture: Option<NotificationCapture>,
        }

        impl NotifyingServerHandler for TestHandler {
            fn set_notification_capture(&mut self, capture: NotificationCapture) {
                self._capture = Some(capture);
            }
        }

        #[async_trait::async_trait]
        impl ServerHandler for TestHandler {
            fn get_info(&self) -> ServerInfo {
                ServerInfo::new(ServerCapabilities::default())
                    .with_server_info(Implementation::new("test", "0.1.0"))
            }
        }

        let handler = TestHandler { _capture: None };
        let server = start_notifying_server(handler)
            .await
            .expect("server should start");

        // URL should be a valid localhost HTTP URL
        assert!(
            server.url().starts_with("http://127.0.0.1:"),
            "URL should be localhost: {}",
            server.url()
        );
        assert!(
            server.url().ends_with("/mcp"),
            "URL should end with /mcp: {}",
            server.url()
        );
    }

    #[test_log::test(tokio::test)]
    async fn test_notifying_server_notification_source_trait() {
        #[derive(Clone)]
        struct TestHandler2 {
            _capture: Option<NotificationCapture>,
        }

        impl NotifyingServerHandler for TestHandler2 {
            fn set_notification_capture(&mut self, capture: NotificationCapture) {
                self._capture = Some(capture);
            }
        }

        #[async_trait::async_trait]
        impl ServerHandler for TestHandler2 {
            fn get_info(&self) -> ServerInfo {
                ServerInfo::new(ServerCapabilities::default())
                    .with_server_info(Implementation::new("test2", "0.1.0"))
            }
        }

        let handler = TestHandler2 { _capture: None };
        let server = start_notifying_server(handler)
            .await
            .expect("server should start");

        // McpNotificationSource::url() should match
        let source: &dyn McpNotificationSource = &server;
        assert_eq!(source.url(), server.url());

        // subscribe() should return a working receiver
        let _rx = source.subscribe();
    }
}
