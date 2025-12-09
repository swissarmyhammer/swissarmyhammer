//! Configuration types for the Claude Agent

use crate::constants::sizes;
use serde::{Deserialize, Serialize};

/// Default value for max_prompt_length
fn default_max_prompt_length() -> usize {
    sizes::messages::MAX_PROMPT_LENGTH
}

/// Default value for notification_buffer_size
fn default_notification_buffer_size() -> usize {
    sizes::buffers::NOTIFICATION_BUFFER_LARGE
}

/// Default value for cancellation_buffer_size
fn default_cancellation_buffer_size() -> usize {
    sizes::buffers::CANCELLATION_BUFFER
}

/// Default value for max_tokens_per_turn (100k tokens)
fn default_max_tokens_per_turn() -> u64 {
    sizes::messages::MAX_TOKENS_PER_TURN as u64
}

/// Default value for max_turn_requests (50 requests)
fn default_max_turn_requests() -> u64 {
    50
}

/// Main configuration structure for the Claude Agent
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AgentConfig {
    pub claude: ClaudeConfig,
    pub server: ServerConfig,
    pub security: SecurityConfig,
    pub mcp_servers: Vec<McpServerConfig>,
    /// Maximum allowed prompt length in characters (default: 100,000)
    #[serde(default = "default_max_prompt_length")]
    pub max_prompt_length: usize,
    /// Buffer size for notification broadcast channel (default: 1,000)
    #[serde(default = "default_notification_buffer_size")]
    pub notification_buffer_size: usize,
    /// Buffer size for cancellation broadcast channel (default: 100)
    #[serde(default = "default_cancellation_buffer_size")]
    pub cancellation_buffer_size: usize,
    /// Maximum tokens allowed per turn (default: 100,000) - triggers MaxTokens stop reason
    #[serde(default = "default_max_tokens_per_turn")]
    pub max_tokens_per_turn: u64,
    /// Maximum language model requests per turn (default: 50) - triggers MaxTurnRequests stop reason
    #[serde(default = "default_max_turn_requests")]
    pub max_turn_requests: u64,
}

/// Configuration for Claude SDK integration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ClaudeConfig {
    pub model: String,
    pub stream_format: StreamFormat,
}

/// Server configuration options  
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ServerConfig {
    pub port: Option<u16>,
    pub log_level: String,
}

/// Security configuration options
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SecurityConfig {
    pub allowed_file_patterns: Vec<String>,
    pub forbidden_paths: Vec<String>,
    pub require_permission_for: Vec<String>,
}

/// Environment variable for MCP server
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct EnvVariable {
    pub name: String,
    pub value: String,
}

/// HTTP header for MCP server
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct HttpHeader {
    pub name: String,
    pub value: String,
}

/// Stdio transport configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StdioTransport {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVariable>,
    /// Optional working directory for the MCP server process
    pub cwd: Option<String>,
}

/// HTTP transport configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpTransport {
    #[serde(rename = "type")]
    pub transport_type: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
}

/// SSE transport configuration
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SseTransport {
    #[serde(rename = "type")]
    pub transport_type: String,
    pub name: String,
    pub url: String,
    #[serde(default)]
    pub headers: Vec<HttpHeader>,
}

/// Configuration for MCP server connections supporting all ACP transport types
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum McpServerConfig {
    /// Stdio transport (mandatory - all agents must support)
    Stdio(StdioTransport),
    /// HTTP transport (optional - only if mcpCapabilities.http: true)
    Http(HttpTransport),
    /// SSE transport (optional - deprecated but spec-compliant)
    Sse(SseTransport),
}

impl McpServerConfig {
    /// Get the name of this MCP server configuration
    pub fn name(&self) -> &str {
        match self {
            McpServerConfig::Stdio(config) => &config.name,
            McpServerConfig::Http(config) => &config.name,
            McpServerConfig::Sse(config) => &config.name,
        }
    }

    /// Get the transport type as a string
    pub fn transport_type(&self) -> &str {
        match self {
            McpServerConfig::Stdio(_) => "stdio",
            McpServerConfig::Http(_) => "http",
            McpServerConfig::Sse(_) => "sse",
        }
    }

    /// Validate this transport configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        match self {
            McpServerConfig::Stdio(config) => config.validate(),
            McpServerConfig::Http(config) => config.validate(),
            McpServerConfig::Sse(config) => config.validate(),
        }
    }
}

impl StdioTransport {
    /// Validate stdio transport configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.name.is_empty() {
            return Err(crate::error::AgentError::Config(
                "MCP server name cannot be empty".to_string(),
            ));
        }
        if self.command.is_empty() {
            return Err(crate::error::AgentError::Config(format!(
                "MCP server '{}' command cannot be empty",
                self.name
            )));
        }

        // Validate working directory if provided
        if let Some(cwd) = &self.cwd {
            if cwd.is_empty() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' working directory cannot be empty",
                    self.name
                )));
            }

            let cwd_path = std::path::Path::new(cwd);
            if !cwd_path.exists() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' working directory does not exist: {}",
                    self.name, cwd
                )));
            }

            if !cwd_path.is_dir() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' working directory is not a directory: {}",
                    self.name, cwd
                )));
            }
        }

        // Validate environment variables
        for env_var in &self.env {
            if env_var.name.is_empty() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' environment variable name cannot be empty",
                    self.name
                )));
            }
        }

        Ok(())
    }
}

impl HttpTransport {
    /// Validate HTTP transport configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.name.is_empty() {
            return Err(crate::error::AgentError::Config(
                "MCP server name cannot be empty".to_string(),
            ));
        }
        if self.url.is_empty() {
            return Err(crate::error::AgentError::Config(format!(
                "MCP server '{}' URL cannot be empty",
                self.name
            )));
        }

        // Validate URL format
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(crate::error::AgentError::Config(format!(
                "MCP server '{}' URL must start with http:// or https://",
                self.name
            )));
        }

        // Validate HTTP headers
        for header in &self.headers {
            if header.name.is_empty() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' HTTP header name cannot be empty",
                    self.name
                )));
            }
        }

        Ok(())
    }
}

impl SseTransport {
    /// Validate SSE transport configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        if self.name.is_empty() {
            return Err(crate::error::AgentError::Config(
                "MCP server name cannot be empty".to_string(),
            ));
        }
        if self.url.is_empty() {
            return Err(crate::error::AgentError::Config(format!(
                "MCP server '{}' URL cannot be empty",
                self.name
            )));
        }

        // Validate URL format
        if !self.url.starts_with("http://") && !self.url.starts_with("https://") {
            return Err(crate::error::AgentError::Config(format!(
                "MCP server '{}' URL must start with http:// or https://",
                self.name
            )));
        }

        // Validate HTTP headers
        for header in &self.headers {
            if header.name.is_empty() {
                return Err(crate::error::AgentError::Config(format!(
                    "MCP server '{}' HTTP header name cannot be empty",
                    self.name
                )));
            }
        }

        Ok(())
    }
}

/// MCP protocol configuration settings
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct McpProtocolConfig {
    /// MCP protocol version (default: "2024-11-05")
    #[serde(default = "default_mcp_protocol_version")]
    pub version: String,
    /// Connection timeout in seconds (default: 30)
    #[serde(default = "default_mcp_timeout")]
    pub timeout_seconds: u64,
    /// Maximum retries for initialization (default: 3)
    #[serde(default = "default_mcp_max_retries")]
    pub max_retries: u32,
}

fn default_mcp_protocol_version() -> String {
    "2024-11-05".to_string()
}

fn default_mcp_timeout() -> u64 {
    30
}

fn default_mcp_max_retries() -> u32 {
    3
}

impl Default for McpProtocolConfig {
    fn default() -> Self {
        Self {
            version: default_mcp_protocol_version(),
            timeout_seconds: default_mcp_timeout(),
            max_retries: default_mcp_max_retries(),
        }
    }
}

/// Stream format options for Claude responses
#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum StreamFormat {
    StreamJson,
    Standard,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            claude: ClaudeConfig {
                model: "claude-sonnet-4-20250514".to_string(),
                stream_format: StreamFormat::StreamJson,
            },
            server: ServerConfig {
                port: None,
                log_level: "info".to_string(),
            },
            security: SecurityConfig {
                allowed_file_patterns: vec![
                    "**/*.rs".to_string(),
                    "**/*.md".to_string(),
                    "**/*.toml".to_string(),
                ],
                forbidden_paths: vec!["/etc".to_string(), "/usr".to_string(), "/bin".to_string()],
                require_permission_for: vec!["fs_write".to_string(), "terminal_create".to_string()],
            },
            mcp_servers: vec![],
            max_prompt_length: default_max_prompt_length(),
            notification_buffer_size: default_notification_buffer_size(),
            cancellation_buffer_size: default_cancellation_buffer_size(),
            max_tokens_per_turn: default_max_tokens_per_turn(),
            max_turn_requests: default_max_turn_requests(),
        }
    }
}

impl AgentConfig {
    /// Validate the configuration
    pub fn validate(&self) -> crate::error::Result<()> {
        // Validate model name is not empty
        if self.claude.model.is_empty() {
            return Err(crate::error::AgentError::Config(
                "Claude model cannot be empty".to_string(),
            ));
        }

        // Validate log level
        if !["error", "warn", "info", "debug", "trace"].contains(&self.server.log_level.as_str()) {
            return Err(crate::error::AgentError::Config(format!(
                "Invalid log level: {}",
                self.server.log_level
            )));
        }

        // Validate MCP server configurations
        for server in &self.mcp_servers {
            server.validate()?;
        }

        Ok(())
    }

    /// Load configuration from JSON string
    pub fn from_json(json: &str) -> crate::error::Result<Self> {
        let config: AgentConfig = serde_json::from_str(json)?;
        config.validate()?;
        Ok(config)
    }

    /// Serialize configuration to JSON string
    pub fn to_json(&self) -> crate::error::Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }
}

impl SecurityConfig {
    /// Convert SecurityConfig to ToolPermissions for tool call handler
    pub fn to_tool_permissions(&self) -> crate::tools::ToolPermissions {
        crate::tools::ToolPermissions {
            require_permission_for: self.require_permission_for.clone(),
            auto_approved: vec![], // Can be extended later if needed
            forbidden_paths: self.forbidden_paths.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = AgentConfig::default();

        assert_eq!(config.claude.model, "claude-sonnet-4-20250514");
        assert!(matches!(
            config.claude.stream_format,
            StreamFormat::StreamJson
        ));
        assert_eq!(config.server.port, None);
        assert_eq!(config.server.log_level, "info");
        assert_eq!(config.security.allowed_file_patterns.len(), 3);
        assert_eq!(config.security.forbidden_paths.len(), 3);
        assert_eq!(config.security.require_permission_for.len(), 2);
        assert_eq!(config.mcp_servers.len(), 0);
    }

    #[test]
    fn test_config_validation_success() {
        let config = AgentConfig::default();
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_config_validation_empty_model() {
        let mut config = AgentConfig::default();
        config.claude.model = String::new();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("model cannot be empty"));
    }

    #[test]
    fn test_config_validation_invalid_log_level() {
        let mut config = AgentConfig::default();
        config.server.log_level = "invalid".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid log level"));
    }

    #[test]
    fn test_config_validation_empty_mcp_server_name() {
        let mut config = AgentConfig::default();
        config
            .mcp_servers
            .push(McpServerConfig::Stdio(StdioTransport {
                name: String::new(),
                command: "test".to_string(),
                args: vec![],
                env: vec![],
                cwd: None,
            }));

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("name cannot be empty"));
    }

    #[test]
    fn test_config_validation_empty_mcp_server_command() {
        let mut config = AgentConfig::default();
        config
            .mcp_servers
            .push(McpServerConfig::Stdio(StdioTransport {
                name: "test".to_string(),
                command: String::new(),
                args: vec![],
                env: vec![],
                cwd: None,
            }));

        let result = config.validate();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("command cannot be empty"));
    }

    #[test]
    fn test_json_serialization() {
        let config = AgentConfig::default();
        let json = config.to_json().unwrap();

        // Should be valid JSON
        assert!(serde_json::from_str::<serde_json::Value>(&json).is_ok());

        // Should contain expected fields
        assert!(json.contains("claude"));
        assert!(json.contains("server"));
        assert!(json.contains("security"));
        assert!(json.contains("mcp_servers"));
    }

    #[test]
    fn test_json_deserialization() {
        let json = r#"{
            "claude": {
                "model": "test-model",
                "stream_format": "Standard"
            },
            "server": {
                "port": 8080,
                "log_level": "debug"
            },
            "security": {
                "allowed_file_patterns": ["**/*.txt"],
                "forbidden_paths": ["/tmp"],
                "require_permission_for": ["test"]
            },
            "mcp_servers": [
                {
                    "name": "test-server",
                    "command": "test-command",
                    "args": ["--test"],
                    "env": [
                        {
                            "name": "TEST_VAR",
                            "value": "test_value"
                        }
                    ]
                }
            ]
        }"#;

        let config = AgentConfig::from_json(json).unwrap();

        assert_eq!(config.claude.model, "test-model");
        assert!(matches!(
            config.claude.stream_format,
            StreamFormat::Standard
        ));
        assert_eq!(config.server.port, Some(8080));
        assert_eq!(config.server.log_level, "debug");
        assert_eq!(config.security.allowed_file_patterns, vec!["**/*.txt"]);
        assert_eq!(config.security.forbidden_paths, vec!["/tmp"]);
        assert_eq!(config.security.require_permission_for, vec!["test"]);
        assert_eq!(config.mcp_servers.len(), 1);
        match &config.mcp_servers[0] {
            McpServerConfig::Stdio(stdio_config) => {
                assert_eq!(stdio_config.name, "test-server");
                assert_eq!(stdio_config.command, "test-command");
                assert_eq!(stdio_config.args, vec!["--test"]);
                assert_eq!(stdio_config.env.len(), 1);
                assert_eq!(stdio_config.env[0].name, "TEST_VAR");
                assert_eq!(stdio_config.env[0].value, "test_value");
            }
            _ => panic!("Expected stdio transport configuration"),
        }
        assert_eq!(config.max_prompt_length, sizes::messages::MAX_PROMPT_LENGTH);
    }

    #[test]
    fn test_round_trip_serialization() {
        let original = AgentConfig::default();
        let json = original.to_json().unwrap();
        let deserialized = AgentConfig::from_json(&json).unwrap();

        // Should be equivalent after round trip
        assert_eq!(original.claude.model, deserialized.claude.model);
        assert_eq!(original.server.port, deserialized.server.port);
        assert_eq!(original.server.log_level, deserialized.server.log_level);
        assert_eq!(
            original.security.allowed_file_patterns,
            deserialized.security.allowed_file_patterns
        );
        assert_eq!(
            original.security.forbidden_paths,
            deserialized.security.forbidden_paths
        );
        assert_eq!(
            original.security.require_permission_for,
            deserialized.security.require_permission_for
        );
        assert_eq!(original.mcp_servers.len(), deserialized.mcp_servers.len());
    }

    #[test]
    fn test_stdio_transport_validation() {
        let stdio = StdioTransport {
            name: "test-stdio".to_string(),
            command: "/path/to/server".to_string(),
            args: vec!["--stdio".to_string()],
            env: vec![EnvVariable {
                name: "API_KEY".to_string(),
                value: "secret123".to_string(),
            }],
            cwd: None,
        };
        assert!(stdio.validate().is_ok());

        // Test empty name
        let invalid_stdio = StdioTransport {
            name: String::new(),
            command: "/path/to/server".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        };
        assert!(invalid_stdio.validate().is_err());

        // Test empty command
        let invalid_stdio = StdioTransport {
            name: "test".to_string(),
            command: String::new(),
            args: vec![],
            env: vec![],
            cwd: None,
        };
        assert!(invalid_stdio.validate().is_err());

        // Test empty env var name
        let invalid_stdio = StdioTransport {
            name: "test".to_string(),
            command: "/path/to/server".to_string(),
            args: vec![],
            env: vec![EnvVariable {
                name: String::new(),
                value: "value".to_string(),
            }],
            cwd: None,
        };
        assert!(invalid_stdio.validate().is_err());

        // Test working directory validation
        let stdio_with_cwd = StdioTransport {
            name: "test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec![],
            env: vec![],
            cwd: Some(".".to_string()), // Current directory should exist
        };
        assert!(stdio_with_cwd.validate().is_ok());

        // Test invalid working directory
        let stdio_invalid_cwd = StdioTransport {
            name: "test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec![],
            env: vec![],
            cwd: Some("/nonexistent/directory/path".to_string()),
        };
        assert!(stdio_invalid_cwd.validate().is_err());

        // Test empty working directory
        let stdio_empty_cwd = StdioTransport {
            name: "test".to_string(),
            command: "/bin/echo".to_string(),
            args: vec![],
            env: vec![],
            cwd: Some(String::new()),
        };
        assert!(stdio_empty_cwd.validate().is_err());
    }

    #[test]
    fn test_http_transport_validation() {
        let http = HttpTransport {
            transport_type: "http".to_string(),
            name: "test-http".to_string(),
            url: "https://api.example.com/mcp".to_string(),
            headers: vec![HttpHeader {
                name: "Authorization".to_string(),
                value: "Bearer token123".to_string(),
            }],
        };
        assert!(http.validate().is_ok());

        // Test empty name
        let invalid_http = HttpTransport {
            transport_type: "http".to_string(),
            name: String::new(),
            url: "https://example.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_http.validate().is_err());

        // Test empty URL
        let invalid_http = HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: String::new(),
            headers: vec![],
        };
        assert!(invalid_http.validate().is_err());

        // Test invalid URL format
        let invalid_http = HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "ftp://example.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_http.validate().is_err());

        // Test empty header name
        let invalid_http = HttpTransport {
            transport_type: "http".to_string(),
            name: "test".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![HttpHeader {
                name: String::new(),
                value: "value".to_string(),
            }],
        };
        assert!(invalid_http.validate().is_err());
    }

    #[test]
    fn test_sse_transport_validation() {
        let sse = SseTransport {
            transport_type: "sse".to_string(),
            name: "test-sse".to_string(),
            url: "https://events.example.com/mcp".to_string(),
            headers: vec![HttpHeader {
                name: "X-API-Key".to_string(),
                value: "apikey456".to_string(),
            }],
        };
        assert!(sse.validate().is_ok());

        // Test similar validations as HTTP transport
        let invalid_sse = SseTransport {
            transport_type: "sse".to_string(),
            name: String::new(),
            url: "https://example.com".to_string(),
            headers: vec![],
        };
        assert!(invalid_sse.validate().is_err());
    }

    #[test]
    fn test_mcp_server_config_methods() {
        let stdio_config = McpServerConfig::Stdio(StdioTransport {
            name: "stdio-server".to_string(),
            command: "/path/to/server".to_string(),
            args: vec![],
            env: vec![],
            cwd: None,
        });

        let http_config = McpServerConfig::Http(HttpTransport {
            transport_type: "http".to_string(),
            name: "http-server".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        let sse_config = McpServerConfig::Sse(SseTransport {
            transport_type: "sse".to_string(),
            name: "sse-server".to_string(),
            url: "https://example.com".to_string(),
            headers: vec![],
        });

        assert_eq!(stdio_config.name(), "stdio-server");
        assert_eq!(stdio_config.transport_type(), "stdio");

        assert_eq!(http_config.name(), "http-server");
        assert_eq!(http_config.transport_type(), "http");

        assert_eq!(sse_config.name(), "sse-server");
        assert_eq!(sse_config.transport_type(), "sse");
    }

    #[test]
    fn test_transport_json_serialization() {
        // Test HTTP transport JSON
        let http_json = r#"{
            "type": "http",
            "name": "api-server",
            "url": "https://api.example.com/mcp",
            "headers": [
                {"name": "Authorization", "value": "Bearer token123"},
                {"name": "Content-Type", "value": "application/json"}
            ]
        }"#;

        let parsed: HttpTransport = serde_json::from_str(http_json).unwrap();
        assert_eq!(parsed.transport_type, "http");
        assert_eq!(parsed.name, "api-server");
        assert_eq!(parsed.url, "https://api.example.com/mcp");
        assert_eq!(parsed.headers.len(), 2);
        assert_eq!(parsed.headers[0].name, "Authorization");
        assert_eq!(parsed.headers[0].value, "Bearer token123");

        // Test SSE transport JSON
        let sse_json = r#"{
            "type": "sse",
            "name": "event-stream",
            "url": "https://events.example.com/mcp",
            "headers": [
                {"name": "X-API-Key", "value": "apikey456"}
            ]
        }"#;

        let parsed: SseTransport = serde_json::from_str(sse_json).unwrap();
        assert_eq!(parsed.transport_type, "sse");
        assert_eq!(parsed.name, "event-stream");
        assert_eq!(parsed.url, "https://events.example.com/mcp");
        assert_eq!(parsed.headers.len(), 1);
        assert_eq!(parsed.headers[0].name, "X-API-Key");
        assert_eq!(parsed.headers[0].value, "apikey456");
    }
}
