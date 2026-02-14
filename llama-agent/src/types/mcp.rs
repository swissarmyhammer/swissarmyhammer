//! MCP (Model Context Protocol) server configuration types.
//!
//! This module provides configuration types for integrating with MCP servers,
//! supporting both in-process and HTTP-based server deployments.

use rmcp::transport::StreamableHttpServerConfig;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::types::errors::MCPError;

/// Configuration for in-process MCP servers spawned as child processes.
///
/// In-process servers provide the lowest latency communication through stdin/stdout
/// and are ideal for local development and simple deployments where tight integration
/// is desired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessServerConfig {
    /// Human-readable identifier for this MCP server instance.
    ///
    /// Used in logging, error messages, and tool attribution. Should be unique
    /// within your application to distinguish between different server instances.
    ///
    /// # Examples
    /// - `"filesystem"` - for file system operations
    /// - `"calculator"` - for mathematical computations
    /// - `"git-tools"` - for git repository operations
    pub name: String,

    /// Executable command to launch the MCP server process.
    ///
    /// Can be an absolute path, relative path, or command name that exists in PATH.
    /// The command is executed directly without shell interpretation for security.
    ///
    /// # Path Resolution
    /// - Absolute paths: `/usr/bin/python3`, `C:\Python\python.exe`
    /// - Relative paths: `./mcp-server`, `../bin/server`
    /// - PATH commands: `python`, `node`, `java`
    ///
    /// # Platform Considerations
    /// - Use forward slashes `/` for cross-platform compatibility
    /// - Consider using `python3` explicitly on Unix systems
    /// - Windows users may need `.exe` extensions for some commands
    ///
    /// # Examples
    /// ```rust
    /// // Using PATH command
    /// command: "python".to_string()
    ///
    /// // Using absolute path
    /// command: "/usr/local/bin/node".to_string()
    ///
    /// // Using relative path
    /// command: "./bin/mcp-server".to_string()
    /// ```
    pub command: String,

    /// Command-line arguments passed to the MCP server process.
    ///
    /// Arguments are passed directly to the process without shell interpretation.
    /// Each argument should be a separate string element in the vector.
    ///
    /// # Argument Parsing
    /// - No shell expansion (wildcards, variables, etc.)
    /// - No string concatenation or splitting
    /// - Each vector element becomes one process argument
    ///
    /// # Common Patterns
    /// ```rust
    /// // Python module execution
    /// args: vec!["-m".to_string(), "mcp_server.filesystem".to_string()]
    ///
    /// // Configuration file
    /// args: vec!["--config".to_string(), "server.json".to_string()]
    ///
    /// // Multiple flags and values
    /// args: vec![
    ///     "--port".to_string(), "8080".to_string(),
    ///     "--log-level".to_string(), "info".to_string(),
    ///     "--enable-cors".to_string()
    /// ]
    ///
    /// // No arguments
    /// args: vec![]
    /// ```
    ///
    /// # Security Note
    /// Arguments are not processed by the shell, preventing injection attacks
    /// but also meaning shell features like variable expansion won't work.
    pub args: Vec<String>,

    /// Process operation timeout in seconds.
    ///
    /// Controls how long to wait for server responses before timing out.
    /// Set to `None` to disable timeouts (not recommended for production).
    /// Applies to individual operations, not the overall process lifetime.
    ///
    /// # Timeout Behavior
    /// - Covers server startup, tool calls, and shutdown operations
    /// - Does not affect the server process lifetime
    /// - Failed operations due to timeout will trigger error handling
    /// - Server process may be restarted if timeouts indicate failure
    ///
    /// # Recommended Values
    /// - **Development**: 60-120 seconds for debugging and slow operations
    /// - **Production**: 30-60 seconds for responsive user experience
    /// - **Heavy computation**: 180+ seconds for complex tools
    /// - **Quick operations**: 10-15 seconds for simple tools
    ///
    /// # Examples
    /// ```rust
    /// // Standard production timeout
    /// timeout_secs: Some(30)
    ///
    /// // Extended timeout for heavy operations
    /// timeout_secs: Some(120)
    ///
    /// // Quick timeout for simple tools
    /// timeout_secs: Some(15)
    ///
    /// // No timeout (use with caution)
    /// timeout_secs: None
    /// ```
    pub timeout_secs: Option<u64>,
}

/// Configuration for HTTP-based MCP servers accessed via remote endpoints.
///
/// This configuration defines how to connect to and communicate with an MCP server
/// running over HTTP using Server-Sent Events (SSE) for streaming transport. HTTP servers
/// provide better isolation, independent scaling, and cross-language compatibility compared
/// to in-process servers.
///
/// # Network Requirements
///
/// - Server must support Server-Sent Events (SSE) for streaming responses
/// - HTTPS is recommended for production deployments
/// - Server should handle connection management and graceful disconnections
///
/// # Performance Considerations
///
/// - Network latency affects tool call response times
/// - Keep-alive settings help maintain connection stability
/// - Stateful mode reduces overhead for multi-turn conversations
///
/// # Examples
///
/// ## Basic HTTP Server Configuration
/// ```rust
/// use llama_agent::HttpServerConfig;
///
/// let config = HttpServerConfig {
///     name: "web-search".to_string(),
///     url: "https://api.example.com/mcp/sse".to_string(),
///     timeout_secs: Some(30),
///     sse_keep_alive_secs: Some(30),
///     stateful_mode: true,
/// };
/// ```
///
/// ## Local Development Server
/// ```rust
/// use llama_agent::HttpServerConfig;
///
/// let config = HttpServerConfig {
///     name: "local-dev".to_string(),
///     url: "http://localhost:8080/sse".to_string(),
///     timeout_secs: Some(60),
///     sse_keep_alive_secs: None, // Disable for local dev
///     stateful_mode: false, // Stateless for testing
/// };
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpServerConfig {
    /// Human-readable identifier for this MCP server instance.
    ///
    /// Used in logging, error messages, and tool attribution. Should be unique
    /// within your application to distinguish between different server instances.
    ///
    /// # Examples
    /// - `"filesystem"` - for file system operations
    /// - `"web-search"` - for web search capabilities
    /// - `"database-prod"` - for production database access
    pub name: String,

    /// HTTP URL endpoint where the MCP server accepts SSE connections.
    ///
    /// Must be a valid HTTP or HTTPS URL. The server should implement the MCP
    /// protocol over Server-Sent Events at this endpoint.
    ///
    /// # Supported Schemes
    /// - `http://` - for development and internal networks
    /// - `https://` - recommended for production and external servers
    ///
    /// # Validation
    /// - URL format must be valid according to RFC 3986
    /// - Must use HTTP or HTTPS scheme (other schemes rejected)
    /// - Host and path components are required
    ///
    /// # Examples
    /// - `"https://mcp-server.example.com/sse"`
    /// - `"http://localhost:8080/mcp/stream"`
    pub url: String,

    /// Request timeout in seconds for HTTP operations.
    ///
    /// Controls how long to wait for server responses before timing out.
    /// Set to `None` to disable timeouts (not recommended for production).
    ///
    /// # Recommended Values
    /// - **Development**: 60-120 seconds for debugging
    /// - **Production**: 30-60 seconds for responsiveness
    /// - **Long-running tools**: 180+ seconds if tools perform heavy computation
    ///
    /// # Examples
    /// ```rust
    /// // Standard production timeout
    /// timeout_secs: Some(30)
    ///
    /// // Extended timeout for heavy operations
    /// timeout_secs: Some(120)
    ///
    /// // No timeout (use with caution)
    /// timeout_secs: None
    /// ```
    pub timeout_secs: Option<u64>,

    /// Server-Sent Events keep-alive interval in seconds.
    ///
    /// Sends periodic keep-alive messages to maintain the HTTP connection and
    /// detect network issues early. Set to `None` to disable keep-alive pings.
    ///
    /// # Network Considerations
    /// - Helps prevent connection timeouts through firewalls and proxies
    /// - Lower values provide faster failure detection but increase bandwidth
    /// - Higher values reduce overhead but slower failure detection
    ///
    /// # Recommended Values
    /// - **Production**: 30-60 seconds
    /// - **Unreliable networks**: 15-30 seconds
    /// - **Development/testing**: `None` (disabled)
    ///
    /// # Examples
    /// ```rust
    /// // Standard keep-alive for production
    /// sse_keep_alive_secs: Some(30)
    ///
    /// // Aggressive keep-alive for unreliable networks
    /// sse_keep_alive_secs: Some(15)
    ///
    /// // Disabled for local development
    /// sse_keep_alive_secs: None
    /// ```
    pub sse_keep_alive_secs: Option<u64>,

    /// Enable stateful session management on the server.
    ///
    /// When `true`, the server maintains conversation state between requests,
    /// allowing tools to access previous context and maintain continuity.
    /// When `false`, each request is independent with no shared state.
    ///
    /// # Stateful Mode (`true`)
    /// - **Pros**: Context continuity, reduced data transfer, conversation memory
    /// - **Cons**: Server-side resource usage, session management complexity
    /// - **Use cases**: Multi-turn conversations, tools that build on previous results
    ///
    /// # Stateless Mode (`false`)
    /// - **Pros**: Simpler server implementation, better scalability, no session cleanup
    /// - **Cons**: No context continuity, higher data transfer for context
    /// - **Use cases**: Single-shot tool calls, stateless microservices
    ///
    /// # Examples
    /// ```rust
    /// // Enable for conversational agents
    /// stateful_mode: true
    ///
    /// // Disable for simple tool servers
    /// stateful_mode: false
    /// ```
    pub stateful_mode: bool,
}

impl HttpServerConfig {
    /// Convert to rmcp's StreamableHttpServerConfig for direct integration with rmcp transport.
    ///
    /// This method extracts the relevant HTTP transport configuration fields
    /// and creates the corresponding rmcp configuration object, enabling users
    /// to work with rmcp types directly when needed for advanced scenarios.
    pub fn to_streamable_config(&self) -> StreamableHttpServerConfig {
        StreamableHttpServerConfig {
            sse_keep_alive: self.sse_keep_alive_secs.map(Duration::from_secs),
            stateful_mode: self.stateful_mode,
            cancellation_token: Default::default(),
            sse_retry: None,
        }
    }

    /// Create HttpServerConfig from rmcp's StreamableHttpServerConfig.
    ///
    /// This method allows users who are working with rmcp types directly
    /// to convert back to our ergonomic configuration format, combining
    /// rmcp transport settings with our additional connection parameters.
    pub fn from_streamable_config(
        name: String,
        url: String,
        timeout_secs: Option<u64>,
        config: &StreamableHttpServerConfig,
    ) -> Self {
        Self {
            name,
            url,
            timeout_secs,
            sse_keep_alive_secs: config.sse_keep_alive.map(|d| d.as_secs()),
            stateful_mode: config.stateful_mode,
        }
    }

    /// Validate the HTTP server configuration for correctness and security.
    pub fn validate(&self) -> Result<(), MCPError> {
        // Validate name
        if self.name.is_empty() {
            return Err(MCPError::Protocol(
                "HTTP server name cannot be empty".to_string(),
            ));
        }

        // Check for invalid characters in name (alphanumeric, hyphens, underscores only)
        if self
            .name
            .chars()
            .any(|c| !c.is_alphanumeric() && !"-_".contains(c))
        {
            return Err(MCPError::Protocol(
                "HTTP server name contains invalid characters. Use only alphanumeric characters, hyphens, and underscores".to_string(),
            ));
        }

        // Validate URL
        if self.url.is_empty() {
            return Err(MCPError::HttpUrlInvalid(
                "HTTP server URL cannot be empty".to_string(),
            ));
        }

        // Parse URL to ensure it's valid
        match url::Url::parse(&self.url) {
            Ok(parsed_url) => {
                // Check scheme
                let scheme = parsed_url.scheme();
                if scheme != "http" && scheme != "https" {
                    return Err(MCPError::HttpUrlInvalid(format!(
                        "HTTP server URL must use http:// or https:// scheme, got: {}",
                        scheme
                    )));
                }

                // Check if host is present
                if parsed_url.host().is_none() {
                    return Err(MCPError::HttpUrlInvalid(
                        "HTTP server URL must include a host".to_string(),
                    ));
                }

                // Warn about localhost/127.0.0.1 in production (but don't fail)
                if let Some(host) = parsed_url.host_str() {
                    if host == "localhost" || host == "127.0.0.1" || host == "::1" {
                        tracing::warn!("HTTP server configured with localhost address: {}. Ensure this is intended for your deployment environment", host);
                    }
                }
            }
            Err(e) => {
                return Err(MCPError::HttpUrlInvalid(format!(
                    "Invalid HTTP server URL: {}",
                    e
                )));
            }
        }

        Ok(())
    }
}

/// Configuration for MCP (Model Context Protocol) servers.
///
/// MCP servers provide tools, resources, and prompts that agents can use during conversations.
/// This enum supports two deployment modes: in-process servers (spawned as child processes)
/// and HTTP servers (remote endpoints accessed via Server-Sent Events).
///
/// # Migration from Previous Versions
///
/// Previous versions used a struct-based configuration. To migrate:
///
/// ```rust
/// // Old struct-based approach (no longer supported)
/// // let config = MCPServerConfig {
/// //     name: "my-server".to_string(),
/// //     command: "python".to_string(),
/// //     args: vec!["-m".to_string(), "server".to_string()],
/// //     timeout_secs: Some(30),
/// // };
///
/// // New enum-based approach
/// use llama_agent::{MCPServerConfig, ProcessServerConfig};
///
/// let config = MCPServerConfig::InProcess(ProcessServerConfig {
///     name: "my-server".to_string(),
///     command: "python".to_string(),
///     args: vec!["-m".to_string(), "server".to_string()],
///     timeout_secs: Some(30),
/// });
/// ```
///
/// # Server Types
///
/// ## InProcess Servers
/// Best for local development, simple deployments, and when you need tight integration:
/// - Lower latency (no network overhead)
/// - Automatic lifecycle management
/// - Direct stdio communication
/// - Suitable for trusted code execution
///
/// ## HTTP Servers
/// Best for production deployments, microservices, and distributed systems:
/// - Network-based communication via Server-Sent Events
/// - Independent scaling and deployment
/// - Better isolation and security
/// - Cross-language server implementations
///
/// # Examples
///
/// ## Local Python MCP Server
/// ```rust
/// use llama_agent::{MCPServerConfig, ProcessServerConfig};
///
/// let config = MCPServerConfig::InProcess(ProcessServerConfig {
///     name: "filesystem".to_string(),
///     command: "python".to_string(),
///     args: vec!["-m".to_string(), "mcp_server.filesystem".to_string()],
///     timeout_secs: Some(30),
/// });
/// ```
///
/// ## Remote HTTP MCP Server
/// ```rust
/// use llama_agent::{MCPServerConfig, HttpServerConfig};
///
/// let config = MCPServerConfig::Http(HttpServerConfig {
///     name: "web-search".to_string(),
///     url: "https://mcp-server.example.com/sse".to_string(),
///     timeout_secs: Some(60),
///     sse_keep_alive_secs: Some(30),
///     stateful_mode: true,
/// });
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MCPServerConfig {
    /// In-process MCP server spawned as a child process.
    ///
    /// The server process is managed by the agent framework, with communication
    /// happening via stdin/stdout. Best for local development and simple deployments.
    InProcess(ProcessServerConfig),

    /// HTTP-based MCP server accessed via remote endpoint.
    ///
    /// The server runs independently and is accessed via HTTP with Server-Sent Events
    /// for streaming communication. Best for production deployments and microservices.
    Http(HttpServerConfig),
}

impl MCPServerConfig {
    pub fn validate(&self) -> Result<(), MCPError> {
        match self {
            MCPServerConfig::InProcess(config) => {
                if config.name.is_empty() {
                    return Err(MCPError::Protocol(
                        "MCP server name cannot be empty".to_string(),
                    ));
                }

                if config.command.is_empty() {
                    return Err(MCPError::Protocol(
                        "MCP server command cannot be empty".to_string(),
                    ));
                }

                // Check for invalid characters in name
                if config
                    .name
                    .chars()
                    .any(|c| !c.is_alphanumeric() && !"-_".contains(c))
                {
                    return Err(MCPError::Protocol(
                        "MCP server name contains invalid characters".to_string(),
                    ));
                }

                Ok(())
            }
            MCPServerConfig::Http(config) => config.validate(),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            MCPServerConfig::InProcess(config) => &config.name,
            MCPServerConfig::Http(config) => &config.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_server_config_allows_zero_timeout() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            timeout_secs: Some(0),
            sse_keep_alive_secs: Some(30),
            stateful_mode: true,
        };

        // This should pass after removing validation - zero timeout means no timeout
        match config.validate() {
            Ok(()) => println!("✓ Test passed as expected"),
            Err(e) => panic!("Test failed with error: {:?}", e),
        }
    }

    #[test]
    fn test_http_server_config_allows_large_timeout() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            timeout_secs: Some(3600), // 1 hour - should be allowed
            sse_keep_alive_secs: Some(30),
            stateful_mode: true,
        };

        // This should pass after removing validation - long timeouts are valid
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_http_server_config_allows_zero_keep_alive() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(0),
            stateful_mode: true,
        };

        // This should pass after removing validation - zero keep-alive means disable
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_http_server_config_allows_large_keep_alive() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(600), // 10 minutes - should be allowed
            stateful_mode: true,
        };

        // This should pass after removing validation - long keep-alive is valid
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_http_server_config_allows_very_short_keep_alive_without_warning() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "http://localhost:8080".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(5), // Very short - should not warn after removal
            stateful_mode: true,
        };

        // This should pass after removing validation - users know their needs
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_http_server_config_valid_basic() {
        let config = HttpServerConfig {
            name: "test".to_string(),
            url: "https://example.com/mcp".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(60),
            stateful_mode: true,
        };

        // This should always pass - valid normal configuration
        assert!(config.validate().is_ok());
    }
}
