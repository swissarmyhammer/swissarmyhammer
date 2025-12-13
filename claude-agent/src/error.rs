//! Error types for the Claude Agent

use serde_json::Value;
use thiserror::Error;

/// JSON-RPC 2.0 error structure following ACP specification
#[derive(Debug, Clone)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    pub data: Option<Value>,
}

/// Trait for converting errors to JSON-RPC format
///
/// All error types in the system should implement this trait to ensure
/// consistent error handling and reporting across the ACP protocol boundary.
pub trait ToJsonRpcError: std::fmt::Display {
    /// Convert error to JSON-RPC error code
    fn to_json_rpc_code(&self) -> i32;

    /// Convert error to structured error data (optional)
    fn to_error_data(&self) -> Option<Value> {
        None
    }

    /// Convert error to complete JSON-RPC error structure
    fn to_json_rpc_error(&self) -> JsonRpcError {
        JsonRpcError {
            code: self.to_json_rpc_code(),
            message: self.to_string(),
            data: self.to_error_data(),
        }
    }
}

/// MCP-specific error types for better error handling and debugging
#[derive(Error, Debug)]
pub enum McpError {
    /// Failed to spawn an MCP server process
    ///
    /// Occurs when the system cannot execute the MCP server command,
    /// typically due to missing executable, insufficient permissions,
    /// or invalid command configuration.
    #[error("Failed to spawn MCP server process '{0}': {1}")]
    ProcessSpawnFailed(String, #[source] std::io::Error),

    /// MCP server stdin stream is not available for writing
    ///
    /// Occurs when attempting to send messages to an MCP server
    /// whose stdin pipe has been closed or is not accessible.
    #[error("MCP server stdin not available")]
    StdinNotAvailable,

    /// MCP server stdout stream is not available for reading
    ///
    /// Occurs when attempting to read responses from an MCP server
    /// whose stdout pipe has been closed or is not accessible.
    #[error("MCP server stdout not available")]
    StdoutNotAvailable,

    /// MCP server stderr stream is not available for reading
    ///
    /// Occurs when attempting to read error messages from an MCP server
    /// whose stderr pipe has been closed or is not accessible.
    #[error("MCP server stderr not available")]
    StderrNotAvailable,

    /// MCP server returned an error response
    ///
    /// Occurs when the MCP server processes a request successfully
    /// but returns an error result according to the MCP protocol.
    #[error("MCP server error: {0}")]
    ServerError(serde_json::Value),

    /// MCP protocol violation or malformed message
    ///
    /// Occurs when messages don't conform to the MCP protocol specification,
    /// such as missing required fields or invalid message structure.
    #[error("MCP protocol error: {0}")]
    ProtocolError(String),

    /// MCP server connection closed unexpectedly
    ///
    /// Occurs when the MCP server process terminates or closes its
    /// communication channels while still expected to be active.
    #[error("MCP connection closed unexpectedly")]
    ConnectionClosed,

    /// MCP response message missing required result field
    ///
    /// Occurs when an MCP server response is received but lacks
    /// the expected result field for successful operations.
    #[error("MCP response missing result field")]
    MissingResult,

    /// MCP server initialization handshake failed
    ///
    /// Occurs during the initial MCP protocol handshake when the server
    /// fails to respond correctly to initialization requests.
    #[error("MCP server initialization failed: {0}")]
    InitializationFailed(String),

    /// Failed to retrieve tools list from MCP server
    ///
    /// Occurs when the MCP server fails to respond to a tools/list
    /// request or returns an invalid tools list response.
    #[error("MCP server tools list request failed: {0}")]
    ToolsListFailed(String),

    /// MCP server configuration is invalid
    ///
    /// Occurs when MCP server configuration contains invalid values,
    /// missing required fields, or unsupported transport types.
    #[error("Invalid MCP configuration: {0}")]
    InvalidConfiguration(String),

    /// JSON message serialization or deserialization failed
    ///
    /// Occurs when converting MCP messages to/from JSON format fails,
    /// typically due to malformed JSON or incompatible data structures.
    #[error("MCP message serialization failed: {0}")]
    SerializationFailed(#[from] serde_json::Error),

    /// Input/output operation failed
    ///
    /// Occurs when reading from or writing to MCP server pipes fails
    /// due to system-level I/O errors.
    #[error("MCP I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// MCP request timed out waiting for response
    ///
    /// Occurs when an MCP server doesn't respond within the configured
    /// timeout period for request-response operations.
    #[error("MCP request timeout")]
    RequestTimeout,

    /// MCP server process terminated unexpectedly
    ///
    /// Occurs when the MCP server process crashes or exits with
    /// a non-zero status code during normal operation.
    #[error("MCP server process crashed")]
    ProcessCrashed,
}

impl ToJsonRpcError for McpError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            McpError::ProtocolError(_) => -32600,       // Invalid Request
            McpError::SerializationFailed(_) => -32700, // Parse error
            McpError::ServerError(_) => -32000,         // Server error
            McpError::RequestTimeout => -32000,         // Server error
            McpError::ConnectionClosed => -32000,       // Server error
            McpError::ProcessCrashed => -32000,         // Server error
            _ => -32603,                                // Internal error (default)
        }
    }
}

impl McpError {
    /// Convert MCP error to JSON-RPC error code (deprecated, use trait method)
    #[deprecated(note = "Use ToJsonRpcError trait method instead")]
    pub fn to_json_rpc_error(&self) -> i32 {
        self.to_json_rpc_code()
    }
}

/// Main error type for the Claude Agent
#[derive(Error, Debug)]
pub enum AgentError {
    #[error("Claude process error: {0}")]
    Process(String),

    #[error("MCP error: {0}")]
    Mcp(#[from] McpError),

    #[error("Path validation error: {0}")]
    PathValidation(#[from] crate::path_validator::PathValidationError),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Session error: {0}")]
    Session(String),

    #[error("Tool execution error: {0}")]
    ToolExecution(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Server error: {0}")]
    ServerError(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Invalid request: {0}")]
    InvalidRequest(String),

    #[error("Method not found: {0}")]
    MethodNotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl ToJsonRpcError for AgentError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            AgentError::Process(_) => -32000, // Server error
            AgentError::Mcp(mcp_error) => mcp_error.to_json_rpc_code(),
            AgentError::PathValidation(_) => -32602, // Invalid params
            AgentError::Protocol(_) => -32600,       // Invalid Request
            AgentError::MethodNotFound(_) => -32601, // Method not found
            AgentError::InvalidRequest(_) => -32602, // Invalid params
            AgentError::Internal(_) => -32603,       // Internal error
            AgentError::PermissionDenied(_) => -32000, // Server error
            AgentError::ToolExecution(_) => -32000,  // Server error
            AgentError::Session(_) => -32000,        // Server error
            AgentError::Config(_) => -32000,         // Server error
            _ => -32603,                             // Internal error (default)
        }
    }
}

impl AgentError {
    /// Convert agent error to JSON-RPC error code (deprecated, use trait method)
    #[deprecated(note = "Use ToJsonRpcError trait method instead")]
    pub fn to_json_rpc_error(&self) -> i32 {
        self.to_json_rpc_code()
    }
}

/// Convenience type alias for Results using AgentError
pub type Result<T> = std::result::Result<T, AgentError>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::io;

    #[test]
    fn test_to_json_rpc_error_trait() {
        // Test AgentError trait implementation
        let error = AgentError::Protocol("test protocol error".to_string());
        let json_rpc = <AgentError as ToJsonRpcError>::to_json_rpc_error(&error);
        assert_eq!(json_rpc.code, -32600);
        assert_eq!(json_rpc.message, "Protocol error: test protocol error");
        assert!(json_rpc.data.is_none());

        // Test McpError trait implementation
        let mcp_error = McpError::ProtocolError("bad protocol".to_string());
        let json_rpc = <McpError as ToJsonRpcError>::to_json_rpc_error(&mcp_error);
        assert_eq!(json_rpc.code, -32600);
        assert!(json_rpc.message.contains("bad protocol"));
    }

    #[test]
    fn test_trait_json_rpc_error_codes() {
        // Test all error code mappings using trait
        assert_eq!(
            AgentError::Protocol("test".to_string()).to_json_rpc_code(),
            -32600
        );
        assert_eq!(
            AgentError::MethodNotFound("test".to_string()).to_json_rpc_code(),
            -32601
        );
        assert_eq!(
            AgentError::InvalidRequest("test".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            AgentError::Internal("test".to_string()).to_json_rpc_code(),
            -32603
        );

        // Test MCP error codes
        assert_eq!(
            McpError::ProtocolError("test".to_string()).to_json_rpc_code(),
            -32600
        );
        assert_eq!(McpError::RequestTimeout.to_json_rpc_code(), -32000);
    }

    #[test]
    fn test_error_display() {
        let err = AgentError::Process("process failed".to_string());
        assert_eq!(err.to_string(), "Claude process error: process failed");

        let err = AgentError::Protocol("test protocol error".to_string());
        assert_eq!(err.to_string(), "Protocol error: test protocol error");

        let err = AgentError::Session("session timeout".to_string());
        assert_eq!(err.to_string(), "Session error: session timeout");

        let err = AgentError::ToolExecution("tool failed".to_string());
        assert_eq!(err.to_string(), "Tool execution error: tool failed");

        let err = AgentError::Config("invalid config".to_string());
        assert_eq!(err.to_string(), "Configuration error: invalid config");

        let err = AgentError::PermissionDenied("access denied".to_string());
        assert_eq!(err.to_string(), "Permission denied: access denied");

        let err = AgentError::InvalidRequest("bad request".to_string());
        assert_eq!(err.to_string(), "Invalid request: bad request");

        let err = AgentError::MethodNotFound("unknown method".to_string());
        assert_eq!(err.to_string(), "Method not found: unknown method");

        let err = AgentError::Internal("internal error".to_string());
        assert_eq!(err.to_string(), "Internal error: internal error");

        // Test PathValidation error
        let path_err =
            crate::path_validator::PathValidationError::NotAbsolute("relative/path".to_string());
        let err = AgentError::PathValidation(path_err);
        assert_eq!(
            err.to_string(),
            "Path validation error: Path is not absolute: relative/path"
        );
    }

    #[test]
    fn test_json_rpc_error_codes() {
        let err = AgentError::Process("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32000);

        let err = AgentError::Protocol("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32600);

        let err = AgentError::MethodNotFound("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32601);

        let err = AgentError::InvalidRequest("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32602);

        let err = AgentError::Internal("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32603);

        let err = AgentError::PermissionDenied("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32000);

        let err = AgentError::ToolExecution("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32000);

        let err = AgentError::Session("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32000);

        let err = AgentError::Config("test".to_string());
        assert_eq!(err.to_json_rpc_code(), -32000);

        // Test PathValidation error code
        let path_err = crate::path_validator::PathValidationError::NotAbsolute("test".to_string());
        let err = AgentError::PathValidation(path_err);
        assert_eq!(err.to_json_rpc_code(), -32602);
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let agent_error: AgentError = io_error.into();

        match agent_error {
            AgentError::Io(_) => {} // Expected
            _ => panic!("Expected IoError variant"),
        }
    }

    #[test]
    fn test_serde_error_conversion() {
        let json = "{invalid json";
        let serde_error = serde_json::from_str::<serde_json::Value>(json).unwrap_err();
        let agent_error: AgentError = serde_error.into();

        match agent_error {
            AgentError::Serialization(_) => {} // Expected
            _ => panic!("Expected Serialization variant"),
        }
    }

    #[test]
    fn test_path_validation_error_conversion() {
        let path_err = crate::path_validator::PathValidationError::EmptyPath;
        let agent_error: AgentError = path_err.into();

        match agent_error {
            AgentError::PathValidation(_) => {} // Expected
            _ => panic!("Expected PathValidation variant"),
        }
    }

    #[test]
    fn test_result_type_alias() {
        let success: Result<i32> = Ok(42);
        let failure: Result<i32> = Err(AgentError::Protocol("test".to_string()));

        assert!(success.is_ok());
        assert!(failure.is_err());

        // Test successful result
        if let Ok(value) = success {
            assert_eq!(value, 42);
        }

        // Test error result
        if let Err(error) = failure {
            assert!(matches!(error, AgentError::Protocol(_)));
        }
    }
}
