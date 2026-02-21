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
        ClientInfo {
            meta: None,
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "mcp-proxy-client".to_string(),
                title: Some("MCP Proxy Client".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
                description: None,
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

        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                prompts: Some(PromptsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            instructions: Some("MCP proxy with notification capture".into()),
            server_info: Implementation {
                name: "mcp-notification-proxy".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some("MCP Notification Capturing Proxy".into()),
                description: None,
                website_url: None,
                icons: None,
            },
        })
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
        tracing::info!("McpProxy: call_tool '{}'", request.name);
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
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: ServerCapabilities {
                tools: Some(ToolsCapability {
                    list_changed: Some(false),
                }),
                prompts: Some(PromptsCapability {
                    list_changed: Some(false),
                }),
                ..Default::default()
            },
            server_info: Implementation {
                name: "mcp-notification-proxy".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some("MCP Notification Capturing Proxy".into()),
                description: None,
                website_url: None,
                icons: None,
            },
            instructions: Some("MCP proxy with notification capture".into()),
        }
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
}
