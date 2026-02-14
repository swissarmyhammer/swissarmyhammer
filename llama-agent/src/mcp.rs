//! Clean MCP implementation using pure rmcp SDK
//!
//! This module provides a minimal, clean MCP client and server implementation
//! using the rmcp SDK without custom protocol implementations.

use crate::types::errors::MCPError;
use async_trait::async_trait;
use rmcp::{
    model::*,
    transport::{stdio, StreamableHttpClientTransport},
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
        let transport = StreamableHttpClientTransport::from_uri(url);

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
    pub async fn set_session(&self, session_id: agent_client_protocol::SessionId) {
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

    /// Call a tool with arguments
    pub async fn call_tool(&self, name: &str, arguments: Value) -> Result<String, MCPError> {
        let params = CallToolRequestParams {
            name: name.to_string().into(),
            arguments: arguments.as_object().cloned(),
            meta: None,
            task: None,
        };

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
        let params = GetPromptRequestParams {
            name: name.to_string(),
            arguments: arguments.map(|map| {
                let mut json_map = serde_json::Map::new();
                for (k, v) in map {
                    json_map.insert(k, v);
                }
                json_map
            }),
            meta: None,
        };

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
    async fn set_session(&self, session_id: agent_client_protocol::SessionId);

    /// Clear session context after tool calls
    async fn clear_session(&self);
}

#[async_trait]
impl MCPClient for UnifiedMCPClient {
    async fn list_tools(&self) -> Result<Vec<String>, MCPError> {
        self.list_tools().await
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

    async fn set_session(&self, session_id: agent_client_protocol::SessionId) {
        self.set_session(session_id).await
    }

    async fn clear_session(&self) {
        self.clear_session().await
    }
}

// Note: MCPServer trait removed as unused in clean implementation

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

    async fn set_session(&self, _session_id: agent_client_protocol::SessionId) {
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
}
