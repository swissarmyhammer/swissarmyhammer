use crate::filter::ToolFilter;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::{ErrorData as McpError, RoleServer, ServerHandler};
use std::sync::Arc;

/// Type alias for cached peer connection
type CachedPeerConnection = Option<(rmcp::Peer<rmcp::RoleClient>, tokio::task::JoinHandle<()>)>;

/// Filtering proxy that wraps an upstream MCP server via HTTP.
///
/// This proxy implements ServerHandler and forwards all requests to an upstream
/// MCP server via HTTP, except for list_tools() which filters the results based on
/// allow/deny regex patterns.
///
/// Note: Only tool discovery (list_tools) is filtered. Tool execution (call_tool)
/// is forwarded without validation, relying on the LLM not attempting to call
/// tools it cannot see.
#[derive(Clone)]
pub struct FilteringMcpProxy {
    /// URL of upstream MCP server (e.g., "http://127.0.0.1:8080/mcp")
    upstream_url: String,
    /// Tool filter with allow/deny patterns
    tool_filter: ToolFilter,
    /// Cached peer connection (with interior mutability)
    cached_peer: Arc<tokio::sync::Mutex<CachedPeerConnection>>,
}

impl FilteringMcpProxy {
    pub fn new(upstream_url: String, tool_filter: ToolFilter) -> Self {
        tracing::info!("Created FilteringMcpProxy for upstream: {}", upstream_url);
        Self {
            upstream_url,
            tool_filter,
            cached_peer: Arc::new(tokio::sync::Mutex::new(None)),
        }
    }

    /// Get a connected Peer to the upstream MCP server (with caching)
    async fn get_peer(&self) -> Result<rmcp::Peer<rmcp::RoleClient>, McpError> {
        let mut cache = self.cached_peer.lock().await;

        // Return cached peer if available
        if let Some((peer, _handle)) = cache.as_ref() {
            return Ok(peer.clone());
        }

        // Create new connection
        use rmcp::service::serve_client;
        use rmcp::transport::StreamableHttpClientTransport;

        // Create transport to upstream
        let transport = StreamableHttpClientTransport::from_uri(self.upstream_url.clone());

        // Create client info
        let client_info = InitializeRequestParam {
            protocol_version: ProtocolVersion::default(),
            capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "filtering-proxy".into(),
                version: env!("CARGO_PKG_VERSION").into(),
                title: Some("Filtering Proxy".into()),
                website_url: None,
                icons: None,
            },
        };

        // Connect and initialize
        let running = serve_client(client_info, transport).await.map_err(|e| {
            McpError::internal_error(format!("Failed to connect to upstream: {}", e), None)
        })?;

        let peer = running.peer().clone();
        let handle = tokio::spawn(async move {
            let _ = running.waiting().await;
        });

        // Cache the peer and handle
        *cache = Some((peer.clone(), handle));

        Ok(peer)
    }

    /// Create proxy implementation metadata
    fn proxy_implementation() -> Implementation {
        Implementation {
            name: "swissarmyhammer-filtering-proxy".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            title: Some("SwissArmyHammer Filtering Proxy".into()),
            website_url: None,
            icons: None,
        }
    }

    /// Create proxy capabilities
    fn proxy_capabilities() -> ServerCapabilities {
        ServerCapabilities {
            prompts: Some(PromptsCapability {
                list_changed: Some(false),
            }),
            tools: Some(ToolsCapability {
                list_changed: Some(false),
            }),
            ..Default::default()
        }
    }

    /// Map upstream service errors to MCP errors
    fn map_upstream_error(operation: &str, e: rmcp::service::ServiceError) -> McpError {
        McpError::internal_error(format!("{} failed: {}", operation, e), None)
    }
}

impl ServerHandler for FilteringMcpProxy {
    /// Forward initialize request to upstream server via HTTP.
    async fn initialize(
        &self,
        _request: InitializeRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<InitializeResult, McpError> {
        tracing::debug!(
            "FilteringMcpProxy: Initializing proxy (upstream: {})",
            self.upstream_url
        );

        // Return proxy's own initialization - we don't need to initialize upstream
        // The upstream server is already running and initialized
        Ok(InitializeResult {
            protocol_version: ProtocolVersion::default(),
            capabilities: Self::proxy_capabilities(),
            instructions: Some("Filtering proxy for MCP tool access control".into()),
            server_info: Self::proxy_implementation(),
        })
    }

    /// Forward list_prompts request to upstream server via rmcp peer.
    async fn list_prompts(
        &self,
        request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListPromptsResult, McpError> {
        tracing::debug!(
            "FilteringMcpProxy: Forwarding list_prompts to {}",
            self.upstream_url
        );

        let peer = self.get_peer().await?;
        peer.list_prompts(request)
            .await
            .map_err(|e| Self::map_upstream_error("list_prompts", e))
    }

    /// Forward get_prompt request to upstream server via rmcp peer.
    async fn get_prompt(
        &self,
        request: GetPromptRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<GetPromptResult, McpError> {
        tracing::debug!(
            prompt_name = %request.name,
            "FilteringMcpProxy: Forwarding get_prompt to {}",
            self.upstream_url
        );

        let peer = self.get_peer().await?;
        peer.get_prompt(request)
            .await
            .map_err(|e| Self::map_upstream_error("get_prompt", e))
    }

    /// Filter list_tools to only return allowed tools.
    ///
    /// This is where tool filtering happens - tools are filtered during discovery
    /// based on the allow/deny regex patterns configured for this proxy.
    async fn list_tools(
        &self,
        _request: Option<PaginatedRequestParam>,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<ListToolsResult, McpError> {
        tracing::warn!(
            "🔍 PROXY list_tools called - filtering from {}",
            self.upstream_url
        );

        // Get all tools from upstream server via rmcp peer
        let peer = self.get_peer().await?;
        let result = peer
            .list_tools(None)
            .await
            .map_err(|e| Self::map_upstream_error("list_tools", e))?;

        // Filter tools based on allow/deny patterns
        let total_tools = result.tools.len();
        let filtered_tools: Vec<Tool> = result
            .tools
            .into_iter()
            .filter(|tool| {
                let allowed = self.tool_filter.is_allowed(&tool.name);
                tracing::debug!(
                    tool_name = %tool.name,
                    allowed = allowed,
                    "FilteringMcpProxy: Tool filter evaluation"
                );
                allowed
            })
            .collect();

        let filtered_count = filtered_tools.len();
        tracing::info!(
            total_tools = total_tools,
            filtered_tools = filtered_count,
            removed_tools = total_tools - filtered_count,
            "FilteringMcpProxy: Filtered tool list"
        );

        Ok(ListToolsResult {
            tools: filtered_tools,
            next_cursor: result.next_cursor,
        })
    }

    /// Forward call_tool request to upstream server via HTTP without validation.
    ///
    /// Note: We do NOT validate tool names here. Tool filtering happens at
    /// discovery time (list_tools). The assumption is that the LLM will not
    /// attempt to call tools it cannot see in the list.
    async fn call_tool(
        &self,
        request: CallToolRequestParam,
        _context: RequestContext<RoleServer>,
    ) -> std::result::Result<CallToolResult, McpError> {
        tracing::info!(
            "call_tool '{}' - forwarding to {}",
            request.name,
            self.upstream_url
        );

        let peer = self.get_peer().await?;
        peer.call_tool(request)
            .await
            .map_err(|e| Self::map_upstream_error("call_tool", e))
    }

    /// Return server info for the proxy itself
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            protocol_version: ProtocolVersion::default(),
            capabilities: Self::proxy_capabilities(),
            server_info: Self::proxy_implementation(),
            instructions: Some("Filtering proxy for MCP tool access control".into()),
        }
    }
}
