//! MCP-specific validation logic for server configurations and security

use super::{ValidationError, ValidationResult, Validator};
use crate::types::{MCPError, MCPServerConfig, Session};

/// Validates MCP server configurations for security and correctness
///
/// The `MCPValidator` performs comprehensive validation of MCP (Model Context Protocol)
/// server configurations, focusing on security, correctness, and operational safety.
/// It validates both in-process and HTTP server configurations with appropriate
/// security checks for each type.
///
/// # Security Features
///
/// - **Command Injection Prevention**: Blocks potentially dangerous shell operators
/// - **Localhost Protection**: Prevents HTTP connections to localhost for security
/// - **Path Traversal Prevention**: Validates server names for path traversal attacks
/// - **URL Format Validation**: Ensures HTTP URLs use proper protocols
///
/// # Validation Scope
///
/// ## InProcess Servers
/// - Server name validation (non-empty, safe characters)
/// - Command validation (non-empty, no dangerous operators)
/// - Security checks for command injection attempts
///
/// ## HTTP Servers  
/// - Server name validation (non-empty, safe characters)
/// - URL format validation (proper protocol)
/// - Localhost restriction for security
/// - Empty URL detection
///
/// # Usage
///
/// ```rust
/// use crate::validation::{MCPValidator, Validator};
/// use crate::types::{MCPServerConfig, Session};
///
/// let validator = MCPValidator::new();
/// let result = validator.validate(&session, &server_config);
/// match result {
///     Ok(()) => println!("MCP server configuration is valid and secure"),
///     Err(e) => println!("Validation failed: {}", e),
/// }
/// ```
///
/// # Thread Safety
///
/// This validator is stateless and can be safely used across multiple threads.
pub struct MCPValidator;

impl MCPValidator {
    /// Creates a new MCP validator
    ///
    /// # Returns
    ///
    /// A new `MCPValidator` instance ready for validation
    pub fn new() -> Self {
        Self
    }

    /// Helper method to extract the server name from an MCP server configuration
    ///
    /// # Arguments
    ///
    /// * `config` - The MCP server configuration to extract the name from
    ///
    /// # Returns
    ///
    /// The server name as a string slice
    fn get_name(config: &MCPServerConfig) -> &str {
        match config {
            MCPServerConfig::InProcess(config) => &config.name,
            MCPServerConfig::Http(config) => &config.name,
        }
    }
}

impl Default for MCPValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl Validator<MCPServerConfig> for MCPValidator {
    type Error = ValidationError;

    fn validate(&self, _context: &Session, target: &MCPServerConfig) -> ValidationResult {
        let name = Self::get_name(target);

        // Validate server name
        if name.is_empty() {
            return Err(ValidationError::invalid_state(
                "MCP server name cannot be empty",
            ));
        }

        // Validate server name doesn't contain invalid characters
        if name.contains("..") || name.contains("/") {
            return Err(ValidationError::security_violation(
                "MCP server name contains invalid characters",
            ));
        }

        // Validate configuration based on type
        match target {
            MCPServerConfig::InProcess(config) => {
                if config.command.is_empty() {
                    return Err(ValidationError::invalid_state(
                        "InProcess MCP server command cannot be empty",
                    ));
                }

                // Basic security check for command injection
                if config.command.contains("&&")
                    || config.command.contains("||")
                    || config.command.contains(";")
                {
                    return Err(ValidationError::security_violation(
                        "MCP server command contains potentially dangerous operators",
                    ));
                }
            }
            MCPServerConfig::Http(config) => {
                if config.url.is_empty() {
                    return Err(ValidationError::invalid_state(
                        "HTTP MCP server URL cannot be empty",
                    ));
                }

                // Validate URL format
                if !config.url.starts_with("http://") && !config.url.starts_with("https://") {
                    return Err(ValidationError::invalid_state(
                        "HTTP MCP server URL must start with http:// or https://",
                    ));
                }

                // Security check: prevent localhost access unless explicitly allowed
                if config.url.contains("localhost")
                    || config.url.contains("127.0.0.1")
                    || config.url.contains("::1")
                {
                    return Err(ValidationError::security_violation(
                        "MCP server URL cannot point to localhost for security reasons",
                    ));
                }
            }
        }

        Ok(())
    }
}

/// Convert MCPError to ValidationError for compatibility
impl From<MCPError> for ValidationError {
    fn from(err: MCPError) -> Self {
        match err {
            MCPError::ServerNotFound(msg) => {
                ValidationError::invalid_state(format!("MCP server not found: {}", msg))
            }
            MCPError::Protocol(msg) => {
                ValidationError::schema_validation(format!("MCP protocol error: {}", msg))
            }
            MCPError::Connection(msg) => {
                ValidationError::invalid_state(format!("MCP connection error: {}", msg))
            }
            MCPError::ToolCallFailed(msg) => {
                ValidationError::invalid_state(format!("MCP tool call failed: {}", msg))
            }
            MCPError::HttpUrlInvalid(msg) => {
                ValidationError::invalid_state(format!("Invalid MCP HTTP URL: {}", msg))
            }
            MCPError::HttpTimeout(msg) => {
                ValidationError::invalid_state(format!("MCP HTTP timeout: {}", msg))
            }
            MCPError::HttpConnection(msg) => {
                ValidationError::invalid_state(format!("MCP HTTP connection error: {}", msg))
            }
            MCPError::Timeout(msg) => {
                ValidationError::invalid_state(format!("MCP operation timed out: {}", msg))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{HttpServerConfig, Message, MessageRole, ProcessServerConfig, SessionId};
    use std::time::SystemTime;

    fn create_test_session() -> Session {
        Session {
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Test message".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: vec![],
            available_tools: vec![],
            available_prompts: vec![],
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,
            template_token_count: None,
            #[cfg(feature = "acp")]
            todos: Vec::new(),
            #[cfg(feature = "acp")]
            available_commands: Vec::new(),
            current_mode: None,
            #[cfg(feature = "acp")]
            client_capabilities: None,
        }
    }

    #[test]
    fn test_mcp_validator_valid_inprocess_config() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::InProcess(ProcessServerConfig {
            name: "test-server".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            timeout_secs: Some(30),
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_validator_valid_http_config() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::Http(HttpServerConfig {
            name: "test-server".to_string(),
            url: "https://api.example.com/mcp".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(60),
            stateful_mode: false,
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_ok());
    }

    #[test]
    fn test_mcp_validator_empty_name() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::InProcess(ProcessServerConfig {
            name: "".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            timeout_secs: Some(30),
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::InvalidState(_)));
        assert!(err.to_string().contains("name cannot be empty"));
    }

    #[test]
    fn test_mcp_validator_invalid_name_characters() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::InProcess(ProcessServerConfig {
            name: "../malicious".to_string(),
            command: "node".to_string(),
            args: vec!["server.js".to_string()],
            timeout_secs: Some(30),
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::SecurityViolation(_)));
        assert!(err.to_string().contains("invalid characters"));
    }

    #[test]
    fn test_mcp_validator_empty_command() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::InProcess(ProcessServerConfig {
            name: "test-server".to_string(),
            command: "".to_string(),
            args: vec![],
            timeout_secs: Some(30),
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::InvalidState(_)));
        assert!(err.to_string().contains("command cannot be empty"));
    }

    #[test]
    fn test_mcp_validator_dangerous_command() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::InProcess(ProcessServerConfig {
            name: "test-server".to_string(),
            command: "node server.js && rm -rf /".to_string(),
            args: vec![],
            timeout_secs: Some(30),
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::SecurityViolation(_)));
        assert!(err.to_string().contains("dangerous operators"));
    }

    #[test]
    fn test_mcp_validator_invalid_http_url() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::Http(HttpServerConfig {
            name: "test-server".to_string(),
            url: "ftp://example.com".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(60),
            stateful_mode: false,
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::InvalidState(_)));
        assert!(err.to_string().contains("must start with http"));
    }

    #[test]
    fn test_mcp_validator_localhost_security() {
        let validator = MCPValidator::new();
        let session = create_test_session();
        let config = MCPServerConfig::Http(HttpServerConfig {
            name: "test-server".to_string(),
            url: "http://localhost:8080/mcp".to_string(),
            timeout_secs: Some(30),
            sse_keep_alive_secs: Some(60),
            stateful_mode: false,
        });

        let result = validator.validate(&session, &config);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ValidationError::SecurityViolation(_)));
        assert!(err.to_string().contains("cannot point to localhost"));
    }

    #[test]
    fn test_mcp_error_conversion() {
        let mcp_error = MCPError::Protocol("invalid message format".to_string());
        let validation_error: ValidationError = mcp_error.into();

        assert!(matches!(
            validation_error,
            ValidationError::SchemaValidation(_)
        ));
        assert!(validation_error.to_string().contains("protocol error"));
        assert!(validation_error
            .to_string()
            .contains("invalid message format"));
    }
}
