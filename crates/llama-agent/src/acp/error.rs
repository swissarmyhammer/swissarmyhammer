//! Error types for ACP operations
//!
//! This module provides comprehensive error handling for ACP protocol operations.
//! All error types implement `thiserror::Error` for ergonomic error handling and
//! the `ToJsonRpcError` trait for proper JSON-RPC 2.0 error reporting.

use thiserror::Error;

use super::translation::ToJsonRpcError;

/// Errors that can occur during terminal operations
#[derive(Debug, Error)]
pub enum TerminalError {
    /// Terminal not found
    #[error("Terminal not found: {0}")]
    NotFound(String),

    /// Failed to create terminal process
    #[error("Failed to create terminal: {0}")]
    CreationFailed(String),

    /// Failed to execute terminal command
    #[error("Failed to execute command: {0}")]
    ExecutionFailed(String),

    /// Terminal is in an invalid state for the requested operation
    #[error("Invalid terminal state: {0}")]
    InvalidState(String),

    /// Terminal output buffer overflow
    #[error("Terminal output truncated: buffer size exceeded")]
    OutputTruncated,

    /// Failed to kill terminal process
    #[error("Failed to kill terminal: {0}")]
    KillFailed(String),

    /// Failed to release terminal resources
    #[error("Failed to release terminal: {0}")]
    ReleaseFailed(String),

    /// IO error during terminal operations
    #[error("Terminal IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Process already exited
    #[error("Process already exited with code {0}")]
    AlreadyExited(i32),

    /// Client capability not supported
    #[error("Client does not support terminal capability")]
    CapabilityNotSupported,
}

impl ToJsonRpcError for TerminalError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            TerminalError::NotFound(_) => -32602,        // Invalid params
            TerminalError::CreationFailed(_) => -32603,  // Internal error
            TerminalError::ExecutionFailed(_) => -32603, // Internal error
            TerminalError::InvalidState(_) => -32602,    // Invalid params
            TerminalError::OutputTruncated => -32000,    // Server error
            TerminalError::KillFailed(_) => -32603,      // Internal error
            TerminalError::ReleaseFailed(_) => -32603,   // Internal error
            TerminalError::Io(_) => -32603,              // Internal error
            TerminalError::AlreadyExited(_) => -32602,   // Invalid params
            TerminalError::CapabilityNotSupported => -32600, // Invalid Request
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            TerminalError::NotFound(id) => Some(json!({
                "error": "terminal_not_found",
                "terminalId": id,
                "suggestion": "Verify the terminal ID is correct and the terminal hasn't been released"
            })),
            TerminalError::CreationFailed(msg) => Some(json!({
                "error": "terminal_creation_failed",
                "details": msg,
                "suggestion": "Check command syntax and system resources"
            })),
            TerminalError::ExecutionFailed(msg) => Some(json!({
                "error": "terminal_execution_failed",
                "details": msg,
                "suggestion": "Verify command is valid and accessible"
            })),
            TerminalError::InvalidState(msg) => Some(json!({
                "error": "invalid_terminal_state",
                "details": msg,
                "suggestion": "Check terminal state before performing this operation"
            })),
            TerminalError::OutputTruncated => Some(json!({
                "error": "terminal_output_truncated",
                "suggestion": "Output buffer size exceeded. Consider increasing buffer size or reading output more frequently"
            })),
            TerminalError::KillFailed(msg) => Some(json!({
                "error": "terminal_kill_failed",
                "details": msg,
                "suggestion": "Process may have already exited or may require elevated privileges"
            })),
            TerminalError::ReleaseFailed(msg) => Some(json!({
                "error": "terminal_release_failed",
                "details": msg,
                "suggestion": "Check if terminal resources are still accessible"
            })),
            TerminalError::Io(e) => Some(json!({
                "error": "terminal_io_error",
                "details": e.to_string(),
                "suggestion": "Check system I/O resources and permissions"
            })),
            TerminalError::AlreadyExited(code) => Some(json!({
                "error": "process_already_exited",
                "exitCode": code,
                "suggestion": "Process has already exited. Check exit code for details"
            })),
            TerminalError::CapabilityNotSupported => Some(json!({
                "error": "capability_not_supported",
                "capability": "terminal",
                "suggestion": "Client must declare terminal capability during initialization"
            })),
        }
    }
}

/// Errors that can occur during session operations
#[derive(Debug, Error)]
pub enum SessionError {
    /// Invalid session mode identifier
    #[error("Invalid session mode: {0}")]
    InvalidMode(String),

    /// Session not found
    #[error("Session not found: {0}")]
    NotFound(String),

    /// Session already exists
    #[error("Session already exists: {0}")]
    AlreadyExists(String),

    /// Invalid session state for operation
    #[error("Invalid session state: {0}")]
    InvalidState(String),

    /// Session limit exceeded
    #[error("Session limit exceeded: maximum {0} sessions allowed")]
    LimitExceeded(usize),

    /// Session expired
    #[error("Session expired: {0}")]
    Expired(String),

    /// Failed to serialize session data
    #[error("Failed to serialize session: {0}")]
    SerializationFailed(String),

    /// Failed to deserialize session data
    #[error("Failed to deserialize session: {0}")]
    DeserializationFailed(String),
}

impl ToJsonRpcError for SessionError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            SessionError::InvalidMode(_) => -32602,   // Invalid params
            SessionError::NotFound(_) => -32602,      // Invalid params
            SessionError::AlreadyExists(_) => -32602, // Invalid params
            SessionError::InvalidState(_) => -32602,  // Invalid params
            SessionError::LimitExceeded(_) => -32000, // Server error
            SessionError::Expired(_) => -32602,       // Invalid params
            SessionError::SerializationFailed(_) => -32603, // Internal error
            SessionError::DeserializationFailed(_) => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            SessionError::InvalidMode(mode) => Some(json!({
                "error": "invalid_session_mode",
                "mode": mode,
                "suggestion": "Use a valid mode: 'code', 'plan', 'test', or a custom mode string"
            })),
            SessionError::NotFound(id) => Some(json!({
                "error": "session_not_found",
                "sessionId": id,
                "suggestion": "Verify the session ID is correct and the session hasn't expired"
            })),
            SessionError::AlreadyExists(id) => Some(json!({
                "error": "session_already_exists",
                "sessionId": id,
                "suggestion": "Use a different session ID or retrieve the existing session"
            })),
            SessionError::InvalidState(msg) => Some(json!({
                "error": "invalid_session_state",
                "details": msg,
                "suggestion": "Check that the session is in the correct state for this operation"
            })),
            SessionError::LimitExceeded(max) => Some(json!({
                "error": "session_limit_exceeded",
                "maxSessions": max,
                "suggestion": "Close unused sessions or increase session limits in configuration"
            })),
            SessionError::Expired(id) => Some(json!({
                "error": "session_expired",
                "sessionId": id,
                "suggestion": "Create a new session or increase session timeout"
            })),
            SessionError::SerializationFailed(msg) => Some(json!({
                "error": "session_serialization_failed",
                "details": msg,
                "suggestion": "Check session data integrity"
            })),
            SessionError::DeserializationFailed(msg) => Some(json!({
                "error": "session_deserialization_failed",
                "details": msg,
                "suggestion": "Session data may be corrupted or from an incompatible version"
            })),
        }
    }
}

/// Errors that can occur during permission operations
#[derive(Debug, Error)]
pub enum PermissionError {
    /// Permission denied for operation
    #[error("Permission denied: {0}")]
    Denied(String),

    /// Permission not found
    #[error("Permission not found: {0}")]
    NotFound(String),

    /// Invalid permission policy
    #[error("Invalid permission policy: {0}")]
    InvalidPolicy(String),

    /// Permission already granted
    #[error("Permission already granted: {0}")]
    AlreadyGranted(String),

    /// Failed to store permission
    #[error("Failed to store permission: {0}")]
    StorageFailed(String),

    /// User cancelled permission request
    #[error("User cancelled permission request for: {0}")]
    UserCancelled(String),
}

impl ToJsonRpcError for PermissionError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            PermissionError::Denied(_) => -32600,         // Invalid Request
            PermissionError::NotFound(_) => -32602,       // Invalid params
            PermissionError::InvalidPolicy(_) => -32602,  // Invalid params
            PermissionError::AlreadyGranted(_) => -32602, // Invalid params
            PermissionError::StorageFailed(_) => -32603,  // Internal error
            PermissionError::UserCancelled(_) => -32600,  // Invalid Request
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            PermissionError::Denied(operation) => Some(json!({
                "error": "permission_denied",
                "operation": operation,
                "suggestion": "Request permission or check policy configuration"
            })),
            PermissionError::NotFound(permission) => Some(json!({
                "error": "permission_not_found",
                "permission": permission,
                "suggestion": "Verify the permission identifier is correct"
            })),
            PermissionError::InvalidPolicy(msg) => Some(json!({
                "error": "invalid_permission_policy",
                "details": msg,
                "suggestion": "Check permission policy configuration syntax"
            })),
            PermissionError::AlreadyGranted(permission) => Some(json!({
                "error": "permission_already_granted",
                "permission": permission,
                "suggestion": "Permission is already active for this session"
            })),
            PermissionError::StorageFailed(msg) => Some(json!({
                "error": "permission_storage_failed",
                "details": msg,
                "suggestion": "Check storage backend is accessible and has sufficient space"
            })),
            PermissionError::UserCancelled(operation) => Some(json!({
                "error": "user_cancelled_permission",
                "operation": operation,
                "suggestion": "User declined to grant permission for this operation"
            })),
        }
    }
}

/// Errors that can occur during ACP server operations
#[derive(Debug, Error)]
pub enum ServerError {
    /// Failed to initialize server
    #[error("Failed to initialize server: {0}")]
    InitializationFailed(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    ConfigError(String),

    /// Failed to bind to address
    #[error("Failed to bind to address: {0}")]
    BindFailed(String),

    /// Transport error
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Protocol version mismatch
    #[error("Protocol version mismatch: client {client}, server {server}")]
    VersionMismatch { client: String, server: String },

    /// Invalid JSON-RPC request
    #[error("Invalid JSON-RPC request: {0}")]
    InvalidRequest(String),

    /// Method not found
    #[error("Method not found: {0}")]
    MethodNotFound(String),

    /// Capability not supported
    #[error("Capability not supported: {0}")]
    CapabilityNotSupported(String),

    /// Server shutdown in progress
    #[error("Server is shutting down")]
    ShuttingDown,

    /// Internal server error
    #[error("Internal server error: {0}")]
    Internal(String),
}

impl ToJsonRpcError for ServerError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            ServerError::InitializationFailed(_) => -32603, // Internal error
            ServerError::ConfigError(_) => -32603,          // Internal error
            ServerError::BindFailed(_) => -32603,           // Internal error
            ServerError::TransportError(_) => -32603,       // Internal error
            ServerError::VersionMismatch { .. } => -32600,  // Invalid Request
            ServerError::InvalidRequest(_) => -32600,       // Invalid Request
            ServerError::MethodNotFound(_) => -32601,       // Method not found
            ServerError::CapabilityNotSupported(_) => -32600, // Invalid Request
            ServerError::ShuttingDown => -32000,            // Server error
            ServerError::Internal(_) => -32603,             // Internal error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            ServerError::InitializationFailed(msg) => Some(json!({
                "error": "server_initialization_failed",
                "details": msg,
                "suggestion": "Check server configuration and dependencies"
            })),
            ServerError::ConfigError(msg) => Some(json!({
                "error": "configuration_error",
                "details": msg,
                "suggestion": "Review and correct the server configuration"
            })),
            ServerError::BindFailed(msg) => Some(json!({
                "error": "bind_failed",
                "details": msg,
                "suggestion": "Check if port is already in use or if you have sufficient permissions"
            })),
            ServerError::TransportError(msg) => Some(json!({
                "error": "transport_error",
                "details": msg,
                "suggestion": "Check network connectivity and transport configuration"
            })),
            ServerError::VersionMismatch { client, server } => Some(json!({
                "error": "protocol_version_mismatch",
                "clientVersion": client,
                "serverVersion": server,
                "suggestion": "Update client or server to compatible versions"
            })),
            ServerError::InvalidRequest(msg) => Some(json!({
                "error": "invalid_jsonrpc_request",
                "details": msg,
                "suggestion": "Check JSON-RPC request format and required fields"
            })),
            ServerError::MethodNotFound(method) => Some(json!({
                "error": "method_not_found",
                "method": method,
                "suggestion": "Verify the method name is correct and supported by this server"
            })),
            ServerError::CapabilityNotSupported(capability) => Some(json!({
                "error": "capability_not_supported",
                "capability": capability,
                "suggestion": "This server does not support the requested capability"
            })),
            ServerError::ShuttingDown => Some(json!({
                "error": "server_shutting_down",
                "suggestion": "Server is in the process of shutting down. Please retry with a new connection"
            })),
            ServerError::Internal(msg) => Some(json!({
                "error": "internal_server_error",
                "details": msg,
                "suggestion": "An unexpected error occurred. Check server logs for details"
            })),
        }
    }
}

/// Errors that can occur during configuration operations
#[derive(Debug, Error)]
pub enum ConfigError {
    /// Invalid configuration value
    #[error("Invalid configuration value for {key}: {reason}")]
    InvalidValue { key: String, reason: String },

    /// Missing required configuration
    #[error("Missing required configuration: {0}")]
    MissingRequired(String),

    /// Configuration file not found
    #[error("Configuration file not found: {0}")]
    FileNotFound(String),

    /// Failed to parse configuration
    #[error("Failed to parse configuration: {0}")]
    ParseFailed(String),

    /// Configuration validation failed
    #[error("Configuration validation failed: {0}")]
    ValidationFailed(String),

    /// Failed to read configuration file
    #[error("Failed to read configuration file: {0}")]
    FileReadError(String),

    /// Failed to write configuration file
    #[error("Failed to write configuration file: {0}")]
    FileWriteError(String),

    /// Failed to parse configuration from YAML/JSON
    #[error("Failed to parse configuration: {0}")]
    ParseError(String),

    /// Failed to serialize configuration to YAML/JSON
    #[error("Failed to serialize configuration: {0}")]
    SerializationError(String),
}

impl ToJsonRpcError for ConfigError {
    fn to_json_rpc_code(&self) -> i32 {
        match self {
            ConfigError::InvalidValue { .. } => -32602, // Invalid params
            ConfigError::MissingRequired(_) => -32602,  // Invalid params
            ConfigError::FileNotFound(_) => -32603,     // Internal error
            ConfigError::ParseFailed(_) => -32603,      // Internal error
            ConfigError::ValidationFailed(_) => -32602, // Invalid params
            ConfigError::FileReadError(_) => -32603,    // Internal error
            ConfigError::FileWriteError(_) => -32603,   // Internal error
            ConfigError::ParseError(_) => -32603,       // Internal error
            ConfigError::SerializationError(_) => -32603, // Internal error
        }
    }

    fn to_error_data(&self) -> Option<serde_json::Value> {
        use serde_json::json;

        match self {
            ConfigError::InvalidValue { key, reason } => Some(json!({
                "error": "invalid_config_value",
                "configKey": key,
                "reason": reason,
                "suggestion": "Check configuration documentation for valid values"
            })),
            ConfigError::MissingRequired(key) => Some(json!({
                "error": "missing_required_config",
                "configKey": key,
                "suggestion": "Provide a value for this required configuration option"
            })),
            ConfigError::FileNotFound(path) => Some(json!({
                "error": "config_file_not_found",
                "path": path,
                "suggestion": "Ensure configuration file exists at the specified path"
            })),
            ConfigError::ParseFailed(msg) => Some(json!({
                "error": "config_parse_failed",
                "details": msg,
                "suggestion": "Check configuration file syntax (YAML/JSON)"
            })),
            ConfigError::ValidationFailed(msg) => Some(json!({
                "error": "config_validation_failed",
                "details": msg,
                "suggestion": "Review configuration values against requirements"
            })),
            ConfigError::FileReadError(msg) => Some(json!({
                "error": "config_file_read_error",
                "details": msg,
                "suggestion": "Check file permissions and path"
            })),
            ConfigError::FileWriteError(msg) => Some(json!({
                "error": "config_file_write_error",
                "details": msg,
                "suggestion": "Check file permissions and available disk space"
            })),
            ConfigError::ParseError(msg) => Some(json!({
                "error": "config_parse_error",
                "details": msg,
                "suggestion": "Check configuration file syntax (YAML/JSON)"
            })),
            ConfigError::SerializationError(msg) => Some(json!({
                "error": "config_serialization_error",
                "details": msg,
                "suggestion": "Check configuration data validity"
            })),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_error_codes() {
        assert_eq!(
            TerminalError::NotFound("test".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            TerminalError::CreationFailed("test".to_string()).to_json_rpc_code(),
            -32603
        );
        assert_eq!(
            TerminalError::InvalidState("test".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            TerminalError::CapabilityNotSupported.to_json_rpc_code(),
            -32600
        );
    }

    #[test]
    fn test_terminal_error_data() {
        let error = TerminalError::NotFound("term-123".to_string());
        let data = error.to_error_data().expect("Should have error data");

        assert_eq!(data["error"], "terminal_not_found");
        assert_eq!(data["terminalId"], "term-123");
        assert!(data["suggestion"].is_string());

        let error = TerminalError::CapabilityNotSupported;
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "capability_not_supported");
        assert_eq!(data["capability"], "terminal");
    }

    #[test]
    fn test_session_error_codes() {
        assert_eq!(
            SessionError::InvalidMode("bad".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(SessionError::LimitExceeded(10).to_json_rpc_code(), -32000);
    }

    #[test]
    fn test_session_error_data() {
        let error = SessionError::LimitExceeded(100);
        let data = error.to_error_data().expect("Should have error data");

        assert_eq!(data["error"], "session_limit_exceeded");
        assert_eq!(data["maxSessions"], 100);
    }

    #[test]
    fn test_permission_error_codes() {
        assert_eq!(
            PermissionError::Denied("test".to_string()).to_json_rpc_code(),
            -32600
        );
        assert_eq!(
            PermissionError::StorageFailed("test".to_string()).to_json_rpc_code(),
            -32603
        );
    }

    #[test]
    fn test_permission_error_data() {
        let error = PermissionError::UserCancelled("fs_write".to_string());
        let data = error.to_error_data().expect("Should have error data");

        assert_eq!(data["error"], "user_cancelled_permission");
        assert_eq!(data["operation"], "fs_write");
    }

    #[test]
    fn test_server_error_codes() {
        assert_eq!(
            ServerError::MethodNotFound("test".to_string()).to_json_rpc_code(),
            -32601
        );
        assert_eq!(
            ServerError::InvalidRequest("test".to_string()).to_json_rpc_code(),
            -32600
        );
        assert_eq!(
            ServerError::Internal("test".to_string()).to_json_rpc_code(),
            -32603
        );
    }

    #[test]
    fn test_server_error_version_mismatch() {
        let error = ServerError::VersionMismatch {
            client: "1.0".to_string(),
            server: "2.0".to_string(),
        };
        let data = error.to_error_data().expect("Should have error data");

        assert_eq!(data["error"], "protocol_version_mismatch");
        assert_eq!(data["clientVersion"], "1.0");
        assert_eq!(data["serverVersion"], "2.0");
    }

    #[test]
    fn test_config_error_codes() {
        assert_eq!(
            ConfigError::InvalidValue {
                key: "test".to_string(),
                reason: "bad".to_string()
            }
            .to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            ConfigError::ParseFailed("test".to_string()).to_json_rpc_code(),
            -32603
        );
    }

    #[test]
    fn test_config_error_data() {
        let error = ConfigError::InvalidValue {
            key: "max_tokens".to_string(),
            reason: "must be positive".to_string(),
        };
        let data = error.to_error_data().expect("Should have error data");

        assert_eq!(data["error"], "invalid_config_value");
        assert_eq!(data["configKey"], "max_tokens");
        assert_eq!(data["reason"], "must be positive");
    }

    #[test]
    fn test_error_display() {
        let error = TerminalError::NotFound("term-123".to_string());
        assert_eq!(error.to_string(), "Terminal not found: term-123");

        let error = SessionError::LimitExceeded(50);
        assert_eq!(
            error.to_string(),
            "Session limit exceeded: maximum 50 sessions allowed"
        );

        let error = PermissionError::Denied("operation".to_string());
        assert_eq!(error.to_string(), "Permission denied: operation");
    }
}
