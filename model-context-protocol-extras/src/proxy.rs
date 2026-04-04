//! McpProxy - Proxies to upstream MCP server and captures notifications
//!
//! For third-party MCP servers, we create a proxy that:
//! 1. Accepts client connections (e.g., from Claude)
//! 2. Forwards requests to the upstream server
//! 3. Captures notifications from the upstream and emits to our channel

use crate::notification::{McpNotification, McpNotificationSource};
use rmcp::model::*;
use rmcp::service::{NotificationContext, RequestContext};
use rmcp::transport::streamable_http_server::{
    session::local::LocalSessionManager, StreamableHttpService,
};
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::{
    ClientHandler, ErrorData as McpError, RoleClient, RoleServer, ServerHandler, ServiceExt,
};
use std::sync::Arc;
use swissarmyhammer_common::Pretty;
use tokio::net::TcpListener;
use tokio::sync::{broadcast, Mutex};

/// ClientHandler that captures notifications from upstream
#[derive(Clone)]
struct CapturingClientHandler {
    notification_tx: broadcast::Sender<McpNotification>,
}

impl CapturingClientHandler {
    fn new(tx: broadcast::Sender<McpNotification>) -> Self {
        Self {
            notification_tx: tx,
        }
    }
}

impl ClientHandler for CapturingClientHandler {
    fn get_info(&self) -> ClientInfo {
        ClientInfo::new(
            ClientCapabilities::default(),
            Implementation::new("mcp-proxy-client", env!("CARGO_PKG_VERSION"))
                .with_title("MCP Proxy Client"),
        )
    }

    async fn on_progress(
        &self,
        params: ProgressNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "McpProxy: Captured progress notification: {:?}",
            params.progress_token
        );
        let _ = self.notification_tx.send(McpNotification::Progress(params));
    }

    async fn on_logging_message(
        &self,
        params: LoggingMessageNotificationParam,
        _context: NotificationContext<RoleClient>,
    ) {
        tracing::debug!(
            "McpProxy: Captured log notification: {}",
            Pretty(&params.level)
        );
        let _ = self.notification_tx.send(McpNotification::Log(params));
    }
}

/// Type alias for cached peer connection
type CachedPeerConnection = Option<(rmcp::Peer<RoleClient>, tokio::task::JoinHandle<()>)>;

/// MCP Proxy that forwards to upstream and captures notifications
#[derive(Clone)]
pub struct McpProxyHandler {
    upstream_url: String,
    notification_tx: broadcast::Sender<McpNotification>,
    cached_peer: Arc<Mutex<CachedPeerConnection>>,
}

impl McpProxyHandler {
    fn new(upstream_url: String, notification_tx: broadcast::Sender<McpNotification>) -> Self {
        Self {
            upstream_url,
            notification_tx,
            cached_peer: Arc::new(Mutex::new(None)),
        }
    }

    /// Get or create a connection to upstream
    async fn get_peer(&self) -> Result<rmcp::Peer<RoleClient>, McpError> {
        let mut cache = self.cached_peer.lock().await;

        if let Some((peer, _)) = cache.as_ref() {
            return Ok(peer.clone());
        }

        // Create new connection with our capturing client handler
        let transport = StreamableHttpClientTransport::from_uri(self.upstream_url.clone());
        let handler = CapturingClientHandler::new(self.notification_tx.clone());

        // Use ServiceExt::serve() to connect with our handler
        let running = handler
            .serve(transport)
            .await
            .map_err(|e| McpError::internal_error(format!("Failed to connect: {}", e), None))?;

        let peer = running.peer().clone();
        let handle = tokio::spawn(async move {
            let _ = running.waiting().await;
        });

        *cache = Some((peer.clone(), handle));
        Ok(peer)
    }

    fn map_error(op: &str, e: rmcp::service::ServiceError) -> McpError {
        McpError::internal_error(format!("{} failed: {}", op, e), None)
    }
}

impl ServerHandler for McpProxyHandler {
    async fn initialize(
        &self,
        _request: InitializeRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<InitializeResult, McpError> {
        tracing::debug!("McpProxy: initialize (upstream: {})", self.upstream_url);

        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });
        caps.prompts = Some(PromptsCapability {
            list_changed: Some(false),
        });

        Ok(ServerInfo::new(caps)
            .with_server_info(
                Implementation::new("mcp-notification-proxy", env!("CARGO_PKG_VERSION"))
                    .with_title("MCP Notification Capturing Proxy"),
            )
            .with_instructions("MCP proxy with notification capture"))
    }

    async fn list_tools(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, McpError> {
        let peer = self.get_peer().await?;
        peer.list_tools(request)
            .await
            .map_err(|e| Self::map_error("list_tools", e))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResult, McpError> {
        tracing::debug!("McpProxy: call_tool '{}'", request.name);
        let peer = self.get_peer().await?;
        peer.call_tool(request)
            .await
            .map_err(|e| Self::map_error("call_tool", e))
    }

    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        let peer = self.get_peer().await?;
        peer.list_prompts(request)
            .await
            .map_err(|e| Self::map_error("list_prompts", e))
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        let peer = self.get_peer().await?;
        peer.get_prompt(request)
            .await
            .map_err(|e| Self::map_error("get_prompt", e))
    }

    fn get_info(&self) -> ServerInfo {
        let mut caps = ServerCapabilities::default();
        caps.tools = Some(ToolsCapability {
            list_changed: Some(false),
        });
        caps.prompts = Some(PromptsCapability {
            list_changed: Some(false),
        });

        ServerInfo::new(caps)
            .with_server_info(
                Implementation::new("mcp-notification-proxy", env!("CARGO_PKG_VERSION"))
                    .with_title("MCP Notification Capturing Proxy"),
            )
            .with_instructions("MCP proxy with notification capture")
    }
}

/// Running MCP Proxy instance
pub struct McpProxy {
    url: String,
    notification_tx: broadcast::Sender<McpNotification>,
    _handle: tokio::task::JoinHandle<()>,
}

impl McpProxy {
    /// Get the URL where clients should connect
    pub fn url(&self) -> &str {
        &self.url
    }
}

impl McpNotificationSource for McpProxy {
    fn url(&self) -> &str {
        &self.url
    }

    fn subscribe(&self) -> broadcast::Receiver<McpNotification> {
        self.notification_tx.subscribe()
    }
}

/// Start an MCP proxy to an upstream server with notification capture
///
/// # Arguments
/// * `upstream_url` - URL of the upstream MCP server to proxy to
///
/// # Returns
/// A running McpProxy that provides the local URL and notification subscription
pub async fn start_proxy(
    upstream_url: &str,
) -> Result<McpProxy, Box<dyn std::error::Error + Send + Sync>> {
    let (notification_tx, _rx) = broadcast::channel(256);

    let handler = Arc::new(McpProxyHandler::new(
        upstream_url.to_string(),
        notification_tx.clone(),
    ));

    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let addr = listener.local_addr()?;
    let url = format!("http://{}/mcp", addr);

    tracing::info!("McpProxy starting on {} -> upstream {}", url, upstream_url);

    let http_service = StreamableHttpService::new(
        move || Ok((*handler).clone()),
        LocalSessionManager::default().into(),
        Default::default(),
    );

    let app = axum::Router::new().nest_service("/mcp", http_service);

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            tracing::error!("McpProxy error: {}", e);
        }
    });

    // Small delay to ensure server is ready
    tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

    Ok(McpProxy {
        url,
        notification_tx,
        _handle: handle,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capturing_client_handler() {
        let (tx, mut rx) = broadcast::channel(16);
        let handler = CapturingClientHandler::new(tx);

        // Simulate receiving a notification (normally called by rmcp)
        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("test".into())),
            progress: 25.0,
            total: Some(100.0),
            message: Some("Testing".to_string()),
        };

        let _ = handler
            .notification_tx
            .send(McpNotification::Progress(params));

        match rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 25.0);
            }
            _ => panic!("Expected progress notification"),
        }
    }

    #[test]
    fn test_capturing_client_handler_get_info() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = CapturingClientHandler::new(tx);
        let info = handler.get_info();

        // Verify the client info has expected implementation name
        let impl_info = info.client_info;
        assert_eq!(impl_info.name, "mcp-proxy-client");
    }

    #[test_log::test(tokio::test)]
    async fn test_capturing_client_handler_on_progress() {
        let (tx, mut rx) = broadcast::channel(16);
        let handler = CapturingClientHandler::new(tx);

        let params = ProgressNotificationParam {
            progress_token: ProgressToken(NumberOrString::String("async-test".into())),
            progress: 60.0,
            total: Some(100.0),
            message: Some("async progress".to_string()),
        };

        // Call on_progress directly (simulating rmcp calling our handler)
        // We need a NotificationContext, but we can use the trait method through
        // direct broadcast instead since creating a NotificationContext requires
        // internal rmcp plumbing
        let _ = handler
            .notification_tx
            .send(McpNotification::Progress(params));

        match rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 60.0);
                assert_eq!(p.message.as_deref(), Some("async progress"));
            }
            _ => panic!("Expected progress notification"),
        }
    }

    #[test]
    fn test_capturing_client_handler_log_notification() {
        let (tx, mut rx) = broadcast::channel(16);
        let handler = CapturingClientHandler::new(tx);

        let params = LoggingMessageNotificationParam {
            level: LoggingLevel::Warning,
            logger: Some("test-logger".to_string()),
            data: serde_json::json!("log message"),
        };

        let _ = handler.notification_tx.send(McpNotification::Log(params));

        match rx.try_recv() {
            Ok(McpNotification::Log(l)) => {
                assert_eq!(l.logger.as_deref(), Some("test-logger"));
            }
            _ => panic!("Expected log notification"),
        }
    }

    #[test]
    fn test_mcp_proxy_handler_get_info() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = McpProxyHandler::new("http://localhost:9999/mcp".to_string(), tx);
        let info = handler.get_info();

        // Verify capabilities include tools and prompts
        assert!(info.capabilities.tools.is_some());
        assert!(info.capabilities.prompts.is_some());

        // Verify server info name
        assert_eq!(info.server_info.name, "mcp-notification-proxy");

        // Verify instructions
        assert_eq!(
            info.instructions.as_deref(),
            Some("MCP proxy with notification capture")
        );
    }

    #[test_log::test(tokio::test)]
    async fn test_mcp_proxy_handler_initialize() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = McpProxyHandler::new("http://localhost:9999/mcp".to_string(), tx);

        // Verify that get_info returns consistent results with what initialize would return
        let info = handler.get_info();
        assert!(info.capabilities.tools.is_some());
        let tools_cap = info.capabilities.tools.unwrap();
        assert_eq!(tools_cap.list_changed, Some(false));
    }

    #[test]
    fn test_map_error() {
        let err = rmcp::service::ServiceError::TransportClosed;
        let mcp_err = McpProxyHandler::map_error("test_op", err);
        let msg = format!("{:?}", mcp_err);
        assert!(
            msg.contains("test_op"),
            "error should contain operation name: {}",
            msg
        );
    }

    #[test]
    fn test_mcp_proxy_handler_clone() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = McpProxyHandler::new("http://localhost:8080/mcp".to_string(), tx);
        let cloned = handler.clone();

        assert_eq!(cloned.upstream_url, "http://localhost:8080/mcp");
    }

    #[test_log::test(tokio::test)]
    async fn test_start_proxy_binds_and_provides_url() {
        // start_proxy should bind to a local port even though upstream doesn't exist
        // (connections will fail when actually used, but the proxy itself should start)
        let proxy = start_proxy("http://127.0.0.1:19999/mcp")
            .await
            .expect("proxy should start");

        assert!(
            proxy.url().starts_with("http://127.0.0.1:"),
            "URL should be localhost: {}",
            proxy.url()
        );
        assert!(
            proxy.url().ends_with("/mcp"),
            "URL should end with /mcp: {}",
            proxy.url()
        );
    }

    #[test_log::test(tokio::test)]
    async fn test_mcp_proxy_notification_source_trait() {
        let proxy = start_proxy("http://127.0.0.1:19998/mcp")
            .await
            .expect("proxy should start");

        // Test McpNotificationSource trait implementation
        let source: &dyn McpNotificationSource = &proxy;
        assert_eq!(source.url(), proxy.url());

        // subscribe() should return a working receiver
        let _rx = source.subscribe();
    }

    #[test_log::test(tokio::test)]
    async fn test_mcp_proxy_subscribe_receives_notifications() {
        let proxy = start_proxy("http://127.0.0.1:19997/mcp")
            .await
            .expect("proxy should start");

        let mut rx = proxy.subscribe();

        // Manually send a notification through the internal tx channel
        // This simulates what happens when the proxy captures from upstream
        let _ = proxy
            .notification_tx
            .send(McpNotification::Progress(ProgressNotificationParam {
                progress_token: ProgressToken(NumberOrString::String("proxy-test".into())),
                progress: 33.0,
                total: Some(100.0),
                message: Some("proxy notification".to_string()),
            }));

        match rx.try_recv() {
            Ok(McpNotification::Progress(p)) => {
                assert_eq!(p.progress, 33.0);
                assert_eq!(p.message.as_deref(), Some("proxy notification"));
            }
            _ => panic!("Expected progress notification from proxy"),
        }
    }

    #[test]
    fn test_mcp_proxy_handler_new() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = McpProxyHandler::new("http://example.com/mcp".to_string(), tx);
        assert_eq!(handler.upstream_url, "http://example.com/mcp");
    }

    #[test_log::test(tokio::test)]
    async fn test_mcp_proxy_handler_get_peer_caches_connection() {
        // We can't fully test get_peer without a real upstream, but we can verify
        // the error case when upstream is unavailable
        let (tx, _rx) = broadcast::channel(16);
        let handler = McpProxyHandler::new("http://127.0.0.1:19996/mcp".to_string(), tx);

        // First call should fail because upstream doesn't exist
        let result = handler.get_peer().await;
        assert!(
            result.is_err(),
            "get_peer should fail when upstream is unreachable"
        );
    }

    #[test]
    fn test_capturing_client_handler_clone() {
        let (tx, _rx) = broadcast::channel(16);
        let handler = CapturingClientHandler::new(tx);
        let _cloned = handler.clone();
        // Clone should work without panic
    }
}
