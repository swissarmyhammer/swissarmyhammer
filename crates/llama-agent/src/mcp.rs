//! Clean MCP implementation using pure rmcp SDK
//!
//! This module provides a minimal, clean MCP client and server implementation
//! using the rmcp SDK without custom protocol implementations.

use crate::types::errors::MCPError;
use crate::types::tools::ToolDefinition;
use async_trait::async_trait;
use rmcp::{
    model::*,
    transport::{
        common::client_side_sse::ExponentialBackoff, stdio,
        streamable_http_client::StreamableHttpClientTransportConfig, StreamableHttpClientTransport,
    },
    ClientHandler, RoleClient, ServiceExt,
};
use serde_json::Value;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::timeout;

/// Default timeout for MCP tool calls (10 minutes).
///
/// This needs to be long enough for shell commands that may run for extended periods
/// (e.g., `cargo build`, `cargo test`, long-running scripts).
const DEFAULT_MCP_TIMEOUT_SECS: u64 = 600;

/// Maximum number of automatic SSE reconnect attempts for the streamable-HTTP
/// transport before the transport stops retrying.
///
/// rmcp's default streamable-HTTP retry policy is an [`ExponentialBackoff`] with
/// `max_times: None` — i.e. it reconnects the standalone `GET /mcp` SSE stream
/// *forever*. When the server tears down a session (for example after the
/// streamable-HTTP serve loop closes), every reconnect attempt comes back
/// `404 Not Found`, and with an unbounded policy the client loops on the dead
/// session indefinitely (observed backoff: 3s, 2s, 4s, 8s, 16s, 32s, 64s…),
/// which presents to the user as a panel that is "stuck forever".
///
/// Bounding the policy lets the reconnect loop give up after a finite window so
/// the transport stops hammering a session the server already deleted. With the
/// default 1s base duration, six attempts span roughly 1s + 2s + 4s + 8s + 16s +
/// 32s ≈ 63s of reconnect effort before the standalone stream task ends.
const MAX_SSE_RECONNECT_ATTEMPTS: usize = 6;

/// Build the rmcp streamable-HTTP transport configuration for `url`.
///
/// This mirrors [`StreamableHttpClientTransport::from_uri`] (default reqwest
/// client, transparent re-initialization on a 404'd session) but replaces the
/// **unbounded** default reconnect policy with a bounded [`ExponentialBackoff`]
/// capped at [`MAX_SSE_RECONNECT_ATTEMPTS`]. Without this cap the transport
/// reconnects a dead session's SSE stream forever; with it, the reconnect loop
/// terminates so a torn-down session can no longer wedge the client.
///
/// `reinit_on_expired_session` is left enabled (the rmcp default) so a *transient*
/// session expiry still self-heals via a single re-initialization handshake; only
/// genuinely dead sessions, where re-init also fails, fall through to the bounded
/// reconnect path.
///
/// # Arguments
///
/// * `url` — the streamable-HTTP MCP endpoint (e.g. `http://127.0.0.1:8080/mcp`).
fn streamable_http_transport_config(url: &str) -> StreamableHttpClientTransportConfig {
    // `ExponentialBackoff` is `#[non_exhaustive]`, so it can only be built from
    // its `Default` and then have its public fields adjusted.
    let mut retry_policy = ExponentialBackoff::default();
    retry_policy.max_times = Some(MAX_SSE_RECONNECT_ATTEMPTS);

    let mut config = StreamableHttpClientTransportConfig::with_uri(url);
    config.retry_config = Arc::new(retry_policy);
    config
}

/// Convert an rmcp [`Tool`] returned from `tools/list` into a llama-agent
/// [`ToolDefinition`].
///
/// The MCP `Tool` carries a JSON Schema (`input_schema`) and an optional
/// `description`; both are required for a chat-template-friendly
/// rendering of tools (e.g. the Qwen3 `# Tools` block, which serialises
/// each `ToolDefinition` through `qwen3_tool_envelope`). Earlier
/// implementations of `list_tools` discarded both — this conversion
/// preserves them so the model sees the real parameter contract instead
/// of a placeholder.
///
/// # Arguments
///
/// * `tool` — the rmcp `Tool` value returned by `tools/list`.
/// * `server_name` — fallback server name to record on the
///   [`ToolDefinition`]; the MCP protocol does not attach the server
///   name to individual tools, so callers pass it from the connection
///   context (e.g. the rmcp `peer_info()` server name, or a static
///   label like `"mcp"`).
fn tool_to_definition(tool: Tool, server_name: &str) -> ToolDefinition {
    let parameters = serde_json::Value::Object((*tool.input_schema).clone());
    let description = tool
        .description
        .map(|cow| cow.to_string())
        .unwrap_or_else(|| format!("Tool: {}", tool.name));

    ToolDefinition {
        name: tool.name.to_string(),
        description,
        parameters,
        server_name: server_name.to_string(),
    }
}

/// Simple client handler for rmcp operations
#[derive(Clone, Debug)]
pub struct SimpleClientHandler;

impl ClientHandler for SimpleClientHandler {
    // Default implementations for all required methods
}

/// Unified MCP client that works with all rmcp transports
pub struct UnifiedMCPClient {
    service: rmcp::service::RunningService<
        RoleClient,
        crate::mcp_client_handler::NotifyingClientHandler,
    >,
    default_timeout: Duration,
    /// Handler for setting session context
    handler: Arc<crate::mcp_client_handler::NotifyingClientHandler>,
}

// Note: Now using real rmcp services

impl UnifiedMCPClient {
    /// Create a test client without connection (for testing only)
    pub async fn with_no_connection() -> Result<Self, MCPError> {
        // Use a minimal transport that doesn't require a server for testing
        Err(MCPError::Protocol(
            "No connection client for testing only".to_string(),
        ))
    }

    /// Create a new client with spawned process using rmcp child process support
    pub async fn with_spawned_process(
        command: &str,
        args: &[String],
        timeout_secs: Option<u64>,
    ) -> Result<Self, MCPError> {
        // Create dummy handler for non-ACP usage
        let (dummy_tx, _) = tokio::sync::broadcast::channel(1);
        let handler = Arc::new(crate::mcp_client_handler::NotifyingClientHandler::new(
            dummy_tx,
        ));

        // Spawn process manually and use tuple transport
        let mut child = tokio::process::Command::new(command)
            .args(args)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
            .map_err(|e| {
                MCPError::Protocol(format!("Failed to spawn MCP server '{}': {}", command, e))
            })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            MCPError::Protocol("Failed to get stdin from spawned process".to_string())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            MCPError::Protocol("Failed to get stdout from spawned process".to_string())
        })?;

        // Use rmcp tuple transport pattern (stdout, stdin)
        let transport = (stdout, stdin);

        let service = (*handler).clone().serve(transport).await.map_err(|e| {
            MCPError::Protocol(format!("Failed to create child process client: {:?}", e))
        })?;

        let default_timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_MCP_TIMEOUT_SECS));

        Ok(Self {
            service,
            default_timeout,
            handler,
        })
    }

    /// Create a new client with stdio transport (for existing connections)
    pub async fn with_stdio(timeout_secs: Option<u64>) -> Result<Self, MCPError> {
        // Create dummy handler for non-ACP usage
        let (dummy_tx, _) = tokio::sync::broadcast::channel(1);
        let handler = Arc::new(crate::mcp_client_handler::NotifyingClientHandler::new(
            dummy_tx,
        ));

        let transport = stdio();

        let service = (*handler)
            .clone()
            .serve(transport)
            .await
            .map_err(|e| MCPError::Protocol(format!("Failed to create MCP client: {:?}", e)))?;

        let default_timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_MCP_TIMEOUT_SECS));

        Ok(Self {
            service,
            default_timeout,
            handler,
        })
    }

    /// Create a new client over an in-process [`tokio::io::DuplexStream`].
    ///
    /// This is the client half of the always-on **Agent-tools mount**: the tier
    /// above (which sees both `swissarmyhammer-tools` and `llama-agent`) builds
    /// the agent-tools MCP server, serves it on the *server* half of a
    /// `tokio::io::duplex` pair, and hands the *client* half here. The
    /// connection is purely in-memory — no subprocess, no socket, no port — yet
    /// goes through the standard rmcp `initialize` / `tools/list` handshake like
    /// any other MCP server.
    ///
    /// `client_half` is a valid `RoleClient` transport on its own (rmcp's
    /// blanket `IntoTransport` impl splits it internally); it is served exactly
    /// like the stdio transport in [`with_stdio`](Self::with_stdio). The server
    /// half must already be serving (spawn its serve future before, or
    /// concurrently with, this call) or the handshake will hang.
    ///
    /// # Errors
    ///
    /// Returns [`MCPError::Protocol`] if the rmcp handshake over the duplex
    /// fails.
    pub async fn with_duplex(
        client_half: tokio::io::DuplexStream,
        timeout_secs: Option<u64>,
    ) -> Result<Self, MCPError> {
        // Dummy handler: the mount is not an ACP-provided server, so it does not
        // forward elicitations to a connected ACP client.
        let (dummy_tx, _) = tokio::sync::broadcast::channel(1);
        let handler = Arc::new(crate::mcp_client_handler::NotifyingClientHandler::new(
            dummy_tx,
        ));

        let service = (*handler).clone().serve(client_half).await.map_err(|e| {
            MCPError::Protocol(format!("Failed to create duplex MCP client: {:?}", e))
        })?;

        let default_timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_MCP_TIMEOUT_SECS));

        Ok(Self {
            service,
            default_timeout,
            handler,
        })
    }

    /// Create a new client with SSE transport (deprecated - use with_streamable_http instead)
    #[deprecated(
        note = "SSE transport was removed in rmcp 0.11.0, use with_streamable_http instead"
    )]
    pub async fn with_sse(url: &str, timeout_secs: Option<u64>) -> Result<Self, MCPError> {
        // SSE transport was removed in rmcp 0.11.0
        // Fall back to StreamableHttp which supports SSE-like streaming
        Self::with_streamable_http(url, timeout_secs).await
    }

    /// Create a new client with streamable HTTP transport
    pub async fn with_streamable_http(
        url: &str,
        timeout_secs: Option<u64>,
    ) -> Result<Self, MCPError> {
        // Create dummy handler for non-ACP usage
        let (dummy_tx, _) = tokio::sync::broadcast::channel(1);
        let handler = Arc::new(crate::mcp_client_handler::NotifyingClientHandler::new(
            dummy_tx,
        ));

        Self::with_streamable_http_and_handler(url, timeout_secs, handler).await
    }

    /// Create a new client with streamable HTTP transport and custom handler
    pub async fn with_streamable_http_and_handler(
        url: &str,
        timeout_secs: Option<u64>,
        handler: Arc<crate::mcp_client_handler::NotifyingClientHandler>,
    ) -> Result<Self, MCPError> {
        let transport =
            StreamableHttpClientTransport::from_config(streamable_http_transport_config(url));

        let service = (*handler).clone().serve(transport).await.map_err(|e| {
            MCPError::Protocol(format!("Failed to create HTTP MCP client: {:?}", e))
        })?;

        let default_timeout = Duration::from_secs(timeout_secs.unwrap_or(DEFAULT_MCP_TIMEOUT_SECS));

        Ok(Self {
            service,
            default_timeout,
            handler,
        })
    }

    /// Set session context for MCP notification forwarding
    pub async fn set_session(&self, session_id: agent_client_protocol::schema::SessionId) {
        self.handler.set_session(session_id).await;
    }

    /// Clear session context after tool calls
    pub async fn clear_session(&self) {
        self.handler.clear_session().await;
    }

    /// List available tools
    pub async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
        let result = timeout(self.default_timeout, self.service.list_tools(None))
            .await
            .map_err(|_| MCPError::Timeout("list_tools timed out".to_string()))?
            .map_err(|e| MCPError::Protocol(format!("list_tools failed: {:?}", e)))?;

        Ok(result
            .tools
            .into_iter()
            .map(|tool| tool.name.to_string())
            .collect())
    }

    /// List available tools with full schema metadata.
    ///
    /// Unlike [`list_tools`], which returns only tool names, this method
    /// preserves the full `description` and `input_schema` (JSON Schema)
    /// returned by the MCP server's `tools/list` response and surfaces
    /// each tool as a [`ToolDefinition`] ready to be dropped into a
    /// [`crate::types::sessions::Session::available_tools`] vector.
    ///
    /// The JSON Schema is what `format_tools_for_qwen3` (and the legacy
    /// default tool-rendering path) use to render the `# Tools` block in
    /// the system prompt. Without it the model sees a placeholder schema
    /// like `{}` and cannot infer parameter names — which silently
    /// degrades tool calling for any agent that fetched tools through
    /// the MCP path instead of injecting them manually.
    ///
    /// # Errors
    ///
    /// Returns [`MCPError::Timeout`] if the MCP server does not respond
    /// within the configured `default_timeout`, or [`MCPError::Protocol`]
    /// if the server returns a transport-level failure.
    pub async fn list_tools_with_schemas(&self) -> Result<Vec<ToolDefinition>, MCPError> {
        let result = timeout(self.default_timeout, self.service.list_tools(None))
            .await
            .map_err(|_| MCPError::Timeout("list_tools timed out".to_string()))?
            .map_err(|e| MCPError::Protocol(format!("list_tools failed: {:?}", e)))?;

        let server_name = self
            .service
            .peer_info()
            .map(|info| info.server_info.name.to_string())
            .unwrap_or_else(|| "mcp".to_string());

        Ok(result
            .tools
            .into_iter()
            .map(|tool| tool_to_definition(tool, &server_name))
            .collect())
    }

    /// Call a tool with arguments
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<String, MCPError> {
        let mut params = CallToolRequestParams::new(name.to_string());
        if let Some(args) = arguments.as_object().cloned() {
            params = params.with_arguments(args);
        }

        let result = timeout(self.default_timeout, self.service.call_tool(params))
            .await
            .map_err(|_| MCPError::Timeout(format!("call_tool '{}' timed out", name)))?
            .map_err(|e| {
                MCPError::ToolCallFailed(format!("call_tool '{}' failed: {:?}", name, e))
            })?;

        // Extract text content from the result
        if let Some(content) = result.content.first() {
            match &**content {
                RawContent::Text(text_content) => Ok(text_content.text.clone()),
                RawContent::Image(_) => Ok("Image content (not displayed)".to_string()),
                RawContent::Resource(_) => Ok("Resource content".to_string()),
                _ => Ok("Unknown content type".to_string()),
            }
        } else {
            Ok("No result content".to_string())
        }
    }

    /// List available prompts
    pub async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError> {
        let result = timeout(self.default_timeout, self.service.list_prompts(None))
            .await
            .map_err(|_| MCPError::Timeout("list_prompts timed out".to_string()))?
            .map_err(|e| MCPError::Protocol(format!("list_prompts failed: {:?}", e)))?;

        Ok(result.prompts)
    }

    /// Get a prompt with arguments
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<Vec<String>, MCPError> {
        let mut params = GetPromptRequestParams::new(name.to_string());
        if let Some(map) = arguments {
            let mut json_map = serde_json::Map::new();
            for (k, v) in map {
                json_map.insert(k, v);
            }
            params = params.with_arguments(json_map);
        }

        let result = timeout(self.default_timeout, self.service.get_prompt(params))
            .await
            .map_err(|_| MCPError::Timeout(format!("get_prompt '{}' timed out", name)))?
            .map_err(|e| MCPError::Protocol(format!("get_prompt '{}' failed: {:?}", name, e)))?;

        // Extract message content
        let messages: Vec<String> = result
            .messages
            .into_iter()
            .map(|msg| match &msg.content {
                PromptMessageContent::Text { text } => text.clone(),
                PromptMessageContent::Image { .. } => "Image content".to_string(),
                PromptMessageContent::Resource { .. } => "Resource content".to_string(),
                PromptMessageContent::ResourceLink { .. } => "Resource link".to_string(),
            })
            .collect();

        Ok(messages)
    }

    /// Get server information
    pub async fn get_server_info(&self) -> Result<String, MCPError> {
        let server_info = self.service.peer_info();
        if let Some(info) = server_info {
            Ok(format!(
                "Connected to MCP server: {}",
                info.server_info.name
            ))
        } else {
            Ok("Connected to MCP server (no server info available)".to_string())
        }
    }

    /// Health check
    pub async fn health_check(&self) -> Result<(), MCPError> {
        // Simple health check by listing tools
        self.list_tools().await?;
        Ok(())
    }

    /// Cancel and shutdown the client
    pub async fn cancel(self) -> Result<(), MCPError> {
        let _ = self
            .service
            .cancel()
            .await
            .map_err(|e| MCPError::Protocol(format!("Failed to cancel client: {:?}", e)))?;
        Ok(())
    }

    /// Shutdown all servers (for compatibility)
    pub async fn shutdown_all(&self) -> Result<(), MCPError> {
        self.health_check().await
    }

    /// Add server (for compatibility with agent.rs)
    pub async fn add_server(&self, _config: crate::types::MCPServerConfig) -> Result<(), MCPError> {
        // Real client doesn't dynamically add servers - they're created with specific transports
        Ok(())
    }

    /// Discover tools (alias for list_tools)
    pub async fn discover_tools(&self) -> Result<Vec<String>, MCPError> {
        self.list_tools().await
    }
}

/// Health status for compatibility
#[derive(Debug, Clone, PartialEq)]
pub enum HealthStatus {
    Healthy,
    Unhealthy(String),
}

/// Retry configuration for compatibility
#[derive(Debug, Clone)]
pub struct RetryConfig {
    pub max_retries: u32,
    pub initial_delay: Duration,
    pub max_delay: Duration,
}

impl Default for RetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            initial_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(5),
        }
    }
}

/// Simplified MCP client trait for compatibility
#[async_trait]
pub trait MCPClient: Send + Sync {
    async fn list_tools(&self) -> Result<Vec<String>, MCPError>;

    /// List tools with full JSON Schema metadata.
    ///
    /// The default implementation calls [`list_tools`] and constructs
    /// degraded [`ToolDefinition`]s with a placeholder description and
    /// an empty parameter schema. Implementations that have access to
    /// the underlying rmcp `Tool` (e.g. [`UnifiedMCPClient`]) override
    /// this to preserve the real schema, which the chat-template
    /// rendering path needs to produce a faithful `# Tools` block.
    async fn list_tools_with_schemas(&self) -> Result<Vec<ToolDefinition>, MCPError> {
        let names = self.list_tools().await?;
        Ok(names
            .into_iter()
            .map(|name| ToolDefinition {
                name: name.clone(),
                description: format!("Tool: {}", name),
                parameters: serde_json::Value::Object(serde_json::Map::new()),
                server_name: "mcp".to_string(),
            })
            .collect())
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<String, MCPError>;
    async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError>;
    async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<Vec<String>, MCPError>;
    async fn health_check(&self) -> Result<(), MCPError>;
    async fn shutdown_all(&self) -> Result<(), MCPError>;

    /// Set session context for MCP notification forwarding
    async fn set_session(&self, session_id: agent_client_protocol::schema::SessionId);

    /// Clear session context after tool calls
    async fn clear_session(&self);
}

#[async_trait]
impl MCPClient for UnifiedMCPClient {
    async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
        self.list_tools().await
    }

    async fn list_tools_with_schemas(&self) -> Result<Vec<ToolDefinition>, MCPError> {
        self.list_tools_with_schemas().await
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<String, MCPError> {
        self.call_tool(name, arguments).await
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError> {
        self.list_prompts().await
    }

    async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<Vec<String>, MCPError> {
        self.get_prompt(name, arguments).await
    }

    async fn health_check(&self) -> Result<(), MCPError> {
        self.health_check().await
    }

    async fn shutdown_all(&self) -> Result<(), MCPError> {
        self.shutdown_all().await
    }

    async fn set_session(&self, session_id: agent_client_protocol::schema::SessionId) {
        self.set_session(session_id).await
    }

    async fn clear_session(&self) {
        self.clear_session().await
    }
}

// Note: MCPServer trait removed as unused in clean implementation

/// Always-on source of a llama-agent's intrinsic Agent tools.
///
/// A llama-agent's Agent tools (files, web, skill, subagent, shell) are not
/// "servers it connects to" — they are intrinsic to *being* a llama-agent. This
/// trait is the required construction input that supplies them: every
/// [`AcpServer`](crate::acp::AcpServer) is built with one, and every session it
/// creates mounts a fresh connection from it, regardless of how many (or how
/// few) external MCP servers the `session/new` request lists. An empty external
/// server list therefore still yields a fully-tooled agent.
///
/// llama-agent depends only on rmcp/tokio here: [`connect`](Self::connect)
/// returns an opaque [`MCPClient`] and the trait never names the concrete tool
/// crate. The implementation lives in the tier above (which legally depends on
/// both `swissarmyhammer-tools` and `llama-agent`); it serves the agent-tools
/// MCP server over the *server* half of a `tokio::io::duplex` pair and returns a
/// client built from the *client* half via [`UnifiedMCPClient::with_duplex`].
/// Keeping the trait rmcp-only preserves the acyclic graph — `llama-agent`
/// never gains a runtime dependency on the tools crate.
#[async_trait]
pub trait AgentToolsMount: Send + Sync {
    /// Open a fresh in-process connection to the agent-tools server.
    ///
    /// Called once per session. The returned client is stored alongside the
    /// session's external MCP clients and is cancelled when the session is torn
    /// down, so the implementation should bundle any server-side serve handle
    /// with the returned client to tie the server task's lifetime to it.
    ///
    /// # Errors
    ///
    /// Returns an [`MCPError`] if the in-process MCP handshake fails.
    async fn connect(&self) -> Result<Arc<dyn MCPClient>, MCPError>;
}

/// Duplex buffer size for in-process mounts (32 KiB).
///
/// Tool-call payloads (e.g. large file reads or grep output) can be sizeable;
/// a 32 KiB buffer keeps backpressure rare. Backpressure is never a deadlock as
/// long as both the server serve task and the client run concurrently.
const MOUNT_DUPLEX_BUFFER: usize = 32 * 1024;

/// A [`MCPClient`] that bundles an in-process server's serve handle with the
/// duplex client connected to it.
///
/// Serving a [`RoleServer`] over a `tokio::io::duplex` pair produces a
/// [`RunningService<RoleServer>`]; if that handle (or its task) is dropped, the
/// transport closes and the client's `tools/list` / `call_tool` fail. Holding
/// it here ties the server task's lifetime to the client's, so the in-process
/// connection stays alive exactly as long as the session that owns this client.
///
/// All [`MCPClient`] methods delegate to the inner [`UnifiedMCPClient`]; the
/// server handle is inert storage whose only job is to stay alive.
struct MountedClient<S>
where
    S: rmcp::ServerHandler,
{
    client: UnifiedMCPClient,
    /// Server-side serve handle kept alive for the connection's lifetime.
    _server: rmcp::service::RunningService<rmcp::RoleServer, S>,
}

#[async_trait]
impl<S> MCPClient for MountedClient<S>
where
    S: rmcp::ServerHandler + 'static,
{
    async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
        self.client.list_tools().await
    }

    async fn list_tools_with_schemas(&self) -> Result<Vec<ToolDefinition>, MCPError> {
        self.client.list_tools_with_schemas().await
    }

    async fn call_tool(&self, name: &str, arguments: Value) -> Result<String, MCPError> {
        self.client.call_tool(name, arguments).await
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError> {
        self.client.list_prompts().await
    }

    async fn get_prompt(
        &self,
        name: &str,
        arguments: Option<HashMap<String, Value>>,
    ) -> Result<Vec<String>, MCPError> {
        self.client.get_prompt(name, arguments).await
    }

    async fn health_check(&self) -> Result<(), MCPError> {
        self.client.health_check().await
    }

    async fn shutdown_all(&self) -> Result<(), MCPError> {
        self.client.shutdown_all().await
    }

    async fn set_session(&self, session_id: agent_client_protocol::schema::SessionId) {
        self.client.set_session(session_id).await
    }

    async fn clear_session(&self) {
        self.client.clear_session().await
    }
}

/// Generic [`AgentToolsMount`] that serves any rmcp [`ServerHandler`] in-process.
///
/// This is the one place the duplex serve/connect dance lives: it serves a
/// cloned `handler` on the server half of a `tokio::io::duplex` pair and
/// connects a [`UnifiedMCPClient`] to the client half on each
/// [`connect`](AgentToolsMount::connect). Any crate that can produce an rmcp
/// `ServerHandler` — `swissarmyhammer-tools` with its agent-tools `McpServer`,
/// or tests with an `EchoService` — mounts it through this type without
/// reimplementing the transport plumbing, and `llama-agent` stays rmcp-only.
///
/// `S` must be `Clone` so a fresh handler instance backs each session's
/// connection.
pub struct InProcessMount<S>
where
    S: rmcp::ServerHandler + Clone,
{
    handler: S,
}

impl<S> InProcessMount<S>
where
    S: rmcp::ServerHandler + Clone,
{
    /// Create a mount that serves clones of `handler` in-process.
    pub fn new(handler: S) -> Self {
        Self { handler }
    }
}

#[async_trait]
impl<S> AgentToolsMount for InProcessMount<S>
where
    S: rmcp::ServerHandler + Clone + 'static,
{
    async fn connect(&self) -> Result<Arc<dyn MCPClient>, MCPError> {
        let (server_half, client_half) = tokio::io::duplex(MOUNT_DUPLEX_BUFFER);

        // Both `serve_server` (server side) and `with_duplex` (client side)
        // block on the rmcp `initialize` round-trip: the server waits for the
        // client's request before returning. They must therefore make progress
        // concurrently — `tokio::join!` drives both halves of the handshake at
        // once. Awaiting either alone first would deadlock.
        let (server_res, client_res) = tokio::join!(
            rmcp::serve_server(self.handler.clone(), server_half),
            UnifiedMCPClient::with_duplex(client_half, None),
        );

        let server = server_res.map_err(|e| {
            MCPError::Protocol(format!(
                "Failed to serve in-process Agent-tools server: {:?}",
                e
            ))
        })?;
        let client = client_res?;

        Ok(Arc::new(MountedClient {
            client,
            _server: server,
        }))
    }
}

/// Client builder for creating unified MCP clients with different transports
pub struct MCPClientBuilder {
    servers: Vec<ServerConfig>,
}

#[derive(Debug, Clone)]
pub struct ServerConfig {
    pub name: String,
    pub connection: ServerConnectionConfig,
}

#[derive(Debug, Clone)]
pub enum ServerConnectionConfig {
    Stdio,
    Sse {
        url: String,
    },
    StreamableHttp {
        url: String,
    },
    Http {
        url: String,
        timeout_secs: Option<u64>,
    },
}

impl MCPClientBuilder {
    pub fn new() -> Self {
        Self {
            servers: Vec::new(),
        }
    }

    pub fn add_stdio_server(mut self, name: String) -> Self {
        self.servers.push(ServerConfig {
            name,
            connection: ServerConnectionConfig::Stdio,
        });
        self
    }

    pub fn add_sse_server(mut self, name: String, url: String) -> Self {
        self.servers.push(ServerConfig {
            name,
            connection: ServerConnectionConfig::Sse { url },
        });
        self
    }

    pub fn add_streamable_server(mut self, name: String, url: String) -> Self {
        self.servers.push(ServerConfig {
            name,
            connection: ServerConnectionConfig::StreamableHttp { url },
        });
        self
    }

    pub fn add_process_server(
        mut self,
        name: String,
        _command: String,
        _args: Vec<String>,
    ) -> Self {
        // Simplified implementation - actual command/args unused in current client builder
        self.servers.push(ServerConfig {
            name,
            connection: ServerConnectionConfig::Stdio,
        });
        self
    }

    pub fn add_server(mut self, name: String, config: ServerConnectionConfig) -> Self {
        self.servers.push(ServerConfig {
            name,
            connection: config,
        });
        self
    }

    pub fn add_http_server(mut self, name: String, url: String) -> Self {
        self.servers.push(ServerConfig {
            name,
            connection: ServerConnectionConfig::Http {
                url,
                timeout_secs: None,
            },
        });
        self
    }

    pub async fn build(self) -> Result<Vec<(String, UnifiedMCPClient)>, MCPError> {
        let mut clients = Vec::new();

        for server in self.servers {
            let client = match server.connection {
                ServerConnectionConfig::Stdio => UnifiedMCPClient::with_stdio(None).await?,
                ServerConnectionConfig::Sse { url } => {
                    UnifiedMCPClient::with_streamable_http(&url, None).await?
                }
                ServerConnectionConfig::StreamableHttp { url } => {
                    UnifiedMCPClient::with_streamable_http(&url, None).await?
                }
                ServerConnectionConfig::Http { url, timeout_secs } => {
                    UnifiedMCPClient::with_streamable_http(&url, timeout_secs).await?
                }
            };

            clients.push((server.name, client));
        }

        Ok(clients)
    }
}

impl Default for MCPClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// No-op MCP client for when no MCP servers are configured
#[derive(Debug, Clone)]
pub struct NoOpMCPClient;

impl NoOpMCPClient {
    pub fn new() -> Self {
        Self
    }

    pub async fn add_server(&self, _config: crate::types::MCPServerConfig) -> Result<(), MCPError> {
        Ok(())
    }
}

#[async_trait]
impl MCPClient for NoOpMCPClient {
    async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
        Ok(vec![])
    }

    async fn call_tool(&self, _name: &str, _arguments: Value) -> Result<String, MCPError> {
        Err(MCPError::ServerNotFound(
            "No MCP servers configured".to_string(),
        ))
    }

    async fn list_prompts(&self) -> Result<Vec<Prompt>, MCPError> {
        Ok(vec![])
    }

    async fn get_prompt(
        &self,
        _name: &str,
        _arguments: Option<HashMap<String, Value>>,
    ) -> Result<Vec<String>, MCPError> {
        Err(MCPError::ServerNotFound(
            "No MCP servers configured".to_string(),
        ))
    }

    async fn health_check(&self) -> Result<(), MCPError> {
        Ok(())
    }

    async fn shutdown_all(&self) -> Result<(), MCPError> {
        Ok(())
    }

    async fn set_session(&self, _session_id: agent_client_protocol::schema::SessionId) {
        // No-op
    }

    async fn clear_session(&self) {
        // No-op
    }
}

impl Default for NoOpMCPClient {
    fn default() -> Self {
        Self::new()
    }
}

/// Client error type for compatibility
#[derive(Debug, thiserror::Error)]
pub enum MCPClientError {
    #[error("Protocol error: {0}")]
    Protocol(String),
    #[error("Connection error: {0}")]
    Connection(String),
    #[error("Timeout: {0}")]
    Timeout(String),
}

impl From<MCPError> for MCPClientError {
    fn from(error: MCPError) -> Self {
        match error {
            MCPError::Protocol(msg) => MCPClientError::Protocol(msg),
            MCPError::Timeout(msg) => MCPClientError::Timeout(msg),
            _ => MCPClientError::Connection(error.to_string()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_client_builder() {
        let builder = MCPClientBuilder::new()
            .add_sse_server(
                "test_sse".to_string(),
                "http://localhost:8000/sse".to_string(),
            )
            .add_streamable_server(
                "test_http".to_string(),
                "http://localhost:8001/mcp".to_string(),
            );

        // In actual tests, this would connect to running servers
        // For unit tests, we just verify the builder works
        assert_eq!(builder.servers.len(), 2);
        assert_eq!(builder.servers[0].name, "test_sse");
        assert_eq!(builder.servers[1].name, "test_http");
    }

    #[tokio::test]
    async fn test_health_status() {
        let healthy = HealthStatus::Healthy;
        let unhealthy = HealthStatus::Unhealthy("Connection failed".to_string());

        assert_eq!(healthy, HealthStatus::Healthy);
        match unhealthy {
            HealthStatus::Unhealthy(msg) => assert_eq!(msg, "Connection failed"),
            _ => panic!("Expected unhealthy status"),
        }
    }

    /// The streamable-HTTP transport must use a *bounded* reconnect policy so a
    /// torn-down session (repeated `404 Not Found`) can no longer be retried
    /// forever. rmcp's default policy is unbounded, which is the bug this card
    /// fixes; this test pins the bound in place.
    #[test]
    fn streamable_http_config_uses_bounded_reconnect_policy() {
        let config = streamable_http_transport_config("http://127.0.0.1:8080/mcp");

        // Early attempts still back off and reconnect...
        assert!(
            config.retry_config.retry(0).is_some(),
            "first reconnect attempt should be allowed"
        );
        assert!(
            config
                .retry_config
                .retry(MAX_SSE_RECONNECT_ATTEMPTS - 1)
                .is_some(),
            "attempts below the cap should still reconnect"
        );

        // ...but once the cap is reached the policy gives up instead of looping
        // on the dead session forever.
        assert!(
            config
                .retry_config
                .retry(MAX_SSE_RECONNECT_ATTEMPTS)
                .is_none(),
            "reconnect policy must stop retrying at the configured cap"
        );
        assert!(
            config
                .retry_config
                .retry(MAX_SSE_RECONNECT_ATTEMPTS + 10)
                .is_none(),
            "reconnect policy must stay terminal past the cap"
        );
    }
}
