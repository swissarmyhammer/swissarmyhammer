//! Tests for ACP error propagation
//!
//! This test suite verifies that errors propagate correctly through all layers
//! of the ACP implementation:
//! 1. Low-level errors (IO, serialization, etc.)
//! 2. Domain-specific errors (Filesystem, Terminal, Session, etc.)
//! 3. JSON-RPC protocol errors sent to clients
//!
//! Each test verifies that:
//! - Error codes are correct per JSON-RPC 2.0 spec
//! - Error messages are informative
//! - Error data contains relevant details
//! - Errors convert properly between layers

mod acp_error_propagation_tests {
    use llama_agent::acp::error::{
        ConfigError, PermissionError, ServerError, SessionError, TerminalError,
    };
    use llama_agent::acp::translation::{ToJsonRpcError, TranslationError};

    /// Test that TerminalError variants convert to correct JSON-RPC error codes
    #[test]
    fn test_terminal_error_to_json_rpc_codes() {
        // NotFound should map to invalid params (-32602)
        assert_eq!(
            TerminalError::NotFound("term-123".to_string()).to_json_rpc_code(),
            -32602
        );

        // CreationFailed should map to internal error (-32603)
        assert_eq!(
            TerminalError::CreationFailed("spawn failed".to_string()).to_json_rpc_code(),
            -32603
        );

        // ExecutionFailed should map to internal error (-32603)
        assert_eq!(
            TerminalError::ExecutionFailed("command not found".to_string()).to_json_rpc_code(),
            -32603
        );

        // InvalidState should map to invalid params (-32602)
        assert_eq!(
            TerminalError::InvalidState("already exited".to_string()).to_json_rpc_code(),
            -32602
        );

        // OutputTruncated should map to server error (-32000)
        assert_eq!(TerminalError::OutputTruncated.to_json_rpc_code(), -32000);

        // KillFailed should map to internal error (-32603)
        assert_eq!(
            TerminalError::KillFailed("process not found".to_string()).to_json_rpc_code(),
            -32603
        );

        // ReleaseFailed should map to internal error (-32603)
        assert_eq!(
            TerminalError::ReleaseFailed("cleanup failed".to_string()).to_json_rpc_code(),
            -32603
        );

        // IO error should map to internal error (-32603)
        let io_error = std::io::Error::other("io failed");
        assert_eq!(TerminalError::Io(io_error).to_json_rpc_code(), -32603);

        // AlreadyExited should map to invalid params (-32602)
        assert_eq!(TerminalError::AlreadyExited(1).to_json_rpc_code(), -32602);
    }

    /// Test that TerminalError generates structured error data
    #[test]
    fn test_terminal_error_data_structure() {
        let error = TerminalError::NotFound("term-123".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "terminal_not_found");
        assert_eq!(data["terminalId"], "term-123");
        assert!(data["suggestion"].is_string());

        let error = TerminalError::AlreadyExited(42);
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "process_already_exited");
        assert_eq!(data["exitCode"], 42);
    }

    /// Test that SessionError variants convert to correct JSON-RPC error codes
    #[test]
    fn test_session_error_to_json_rpc_codes() {
        // InvalidMode should map to invalid params (-32602)
        assert_eq!(
            SessionError::InvalidMode("bad-mode".to_string()).to_json_rpc_code(),
            -32602
        );

        // NotFound should map to invalid params (-32602)
        assert_eq!(
            SessionError::NotFound("session-123".to_string()).to_json_rpc_code(),
            -32602
        );

        // AlreadyExists should map to invalid params (-32602)
        assert_eq!(
            SessionError::AlreadyExists("session-123".to_string()).to_json_rpc_code(),
            -32602
        );

        // InvalidState should map to invalid params (-32602)
        assert_eq!(
            SessionError::InvalidState("not ready".to_string()).to_json_rpc_code(),
            -32602
        );

        // LimitExceeded should map to server error (-32000)
        assert_eq!(SessionError::LimitExceeded(100).to_json_rpc_code(), -32000);

        // Expired should map to invalid params (-32602)
        assert_eq!(
            SessionError::Expired("session-123".to_string()).to_json_rpc_code(),
            -32602
        );

        // SerializationFailed should map to internal error (-32603)
        assert_eq!(
            SessionError::SerializationFailed("json error".to_string()).to_json_rpc_code(),
            -32603
        );

        // DeserializationFailed should map to internal error (-32603)
        assert_eq!(
            SessionError::DeserializationFailed("corrupt data".to_string()).to_json_rpc_code(),
            -32603
        );
    }

    /// Test that SessionError generates structured error data
    #[test]
    fn test_session_error_data_structure() {
        let error = SessionError::InvalidMode("bad-mode".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "invalid_session_mode");
        assert_eq!(data["mode"], "bad-mode");

        let error = SessionError::LimitExceeded(100);
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "session_limit_exceeded");
        assert_eq!(data["maxSessions"], 100);
    }

    /// Test that PermissionError variants convert to correct JSON-RPC error codes
    #[test]
    fn test_permission_error_to_json_rpc_codes() {
        // Denied should map to invalid request (-32600)
        assert_eq!(
            PermissionError::Denied("fs_write".to_string()).to_json_rpc_code(),
            -32600
        );

        // NotFound should map to invalid params (-32602)
        assert_eq!(
            PermissionError::NotFound("perm-123".to_string()).to_json_rpc_code(),
            -32602
        );

        // InvalidPolicy should map to invalid params (-32602)
        assert_eq!(
            PermissionError::InvalidPolicy("malformed".to_string()).to_json_rpc_code(),
            -32602
        );

        // AlreadyGranted should map to invalid params (-32602)
        assert_eq!(
            PermissionError::AlreadyGranted("fs_read".to_string()).to_json_rpc_code(),
            -32602
        );

        // StorageFailed should map to internal error (-32603)
        assert_eq!(
            PermissionError::StorageFailed("db error".to_string()).to_json_rpc_code(),
            -32603
        );

        // UserCancelled should map to invalid request (-32600)
        assert_eq!(
            PermissionError::UserCancelled("fs_write".to_string()).to_json_rpc_code(),
            -32600
        );
    }

    /// Test that PermissionError generates structured error data
    #[test]
    fn test_permission_error_data_structure() {
        let error = PermissionError::Denied("fs_write".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "permission_denied");
        assert_eq!(data["operation"], "fs_write");

        let error = PermissionError::UserCancelled("terminal_create".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "user_cancelled_permission");
        assert_eq!(data["operation"], "terminal_create");
    }

    /// Test that ServerError variants convert to correct JSON-RPC error codes
    #[test]
    fn test_server_error_to_json_rpc_codes() {
        // InitializationFailed should map to internal error (-32603)
        assert_eq!(
            ServerError::InitializationFailed("startup failed".to_string()).to_json_rpc_code(),
            -32603
        );

        // ConfigError should map to internal error (-32603)
        assert_eq!(
            ServerError::ConfigError("bad config".to_string()).to_json_rpc_code(),
            -32603
        );

        // BindFailed should map to internal error (-32603)
        assert_eq!(
            ServerError::BindFailed("port in use".to_string()).to_json_rpc_code(),
            -32603
        );

        // TransportError should map to internal error (-32603)
        assert_eq!(
            ServerError::TransportError("connection lost".to_string()).to_json_rpc_code(),
            -32603
        );

        // VersionMismatch should map to invalid request (-32600)
        assert_eq!(
            ServerError::VersionMismatch {
                client: "1.0".to_string(),
                server: "2.0".to_string(),
            }
            .to_json_rpc_code(),
            -32600
        );

        // InvalidRequest should map to invalid request (-32600)
        assert_eq!(
            ServerError::InvalidRequest("malformed".to_string()).to_json_rpc_code(),
            -32600
        );

        // MethodNotFound should map to method not found (-32601)
        assert_eq!(
            ServerError::MethodNotFound("unknown_method".to_string()).to_json_rpc_code(),
            -32601
        );

        // CapabilityNotSupported should map to invalid request (-32600)
        assert_eq!(
            ServerError::CapabilityNotSupported("streaming".to_string()).to_json_rpc_code(),
            -32600
        );

        // ShuttingDown should map to server error (-32000)
        assert_eq!(ServerError::ShuttingDown.to_json_rpc_code(), -32000);

        // Internal should map to internal error (-32603)
        assert_eq!(
            ServerError::Internal("unexpected".to_string()).to_json_rpc_code(),
            -32603
        );
    }

    /// Test that ServerError generates structured error data
    #[test]
    fn test_server_error_data_structure() {
        let error = ServerError::VersionMismatch {
            client: "1.0".to_string(),
            server: "2.0".to_string(),
        };
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "protocol_version_mismatch");
        assert_eq!(data["clientVersion"], "1.0");
        assert_eq!(data["serverVersion"], "2.0");

        let error = ServerError::MethodNotFound("unknown".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "method_not_found");
        assert_eq!(data["method"], "unknown");
    }

    /// Test that ConfigError variants convert to correct JSON-RPC error codes
    #[test]
    fn test_config_error_to_json_rpc_codes() {
        // InvalidValue should map to invalid params (-32602)
        assert_eq!(
            ConfigError::InvalidValue {
                key: "max_tokens".to_string(),
                reason: "must be positive".to_string(),
            }
            .to_json_rpc_code(),
            -32602
        );

        // MissingRequired should map to invalid params (-32602)
        assert_eq!(
            ConfigError::MissingRequired("api_key".to_string()).to_json_rpc_code(),
            -32602
        );

        // FileNotFound should map to internal error (-32603)
        assert_eq!(
            ConfigError::FileNotFound("/path/to/config".to_string()).to_json_rpc_code(),
            -32603
        );

        // ParseFailed should map to internal error (-32603)
        assert_eq!(
            ConfigError::ParseFailed("invalid yaml".to_string()).to_json_rpc_code(),
            -32603
        );

        // ValidationFailed should map to invalid params (-32602)
        assert_eq!(
            ConfigError::ValidationFailed("constraint violation".to_string()).to_json_rpc_code(),
            -32602
        );
    }

    /// Test that ConfigError generates structured error data
    #[test]
    fn test_config_error_data_structure() {
        let error = ConfigError::InvalidValue {
            key: "max_tokens".to_string(),
            reason: "must be positive".to_string(),
        };
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "invalid_config_value");
        assert_eq!(data["configKey"], "max_tokens");
        assert_eq!(data["reason"], "must be positive");

        let error = ConfigError::MissingRequired("api_key".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["error"], "missing_required_config");
        assert_eq!(data["configKey"], "api_key");
    }

    /// Test that TranslationError Display implementation works correctly
    #[test]
    fn test_translation_error_display() {
        let error = TranslationError::UnsupportedContent("images".to_string());
        assert_eq!(error.to_string(), "Unsupported content type: images");

        let error = TranslationError::InvalidFormat("malformed json".to_string());
        assert_eq!(error.to_string(), "Invalid content format: malformed json");

        let error = TranslationError::InvalidSessionId("not-a-ulid".to_string());
        assert_eq!(error.to_string(), "Invalid session ID format: not-a-ulid");
    }

    /// Test that IO errors propagate correctly through TerminalError
    #[test]
    fn test_io_error_propagation_through_terminal() {
        let io_error = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
        let terminal_error = TerminalError::Io(io_error);

        // Verify error code mapping
        assert_eq!(terminal_error.to_json_rpc_code(), -32603);

        // Verify error data contains IO error details
        let data = terminal_error
            .to_error_data()
            .expect("Should have error data");
        assert_eq!(data["error"], "terminal_io_error");
        assert!(data["details"].as_str().unwrap().contains("pipe broken"));
    }

    /// Test error display formatting for all error types
    #[test]
    fn test_error_display_formatting() {
        // TerminalError
        let error = TerminalError::NotFound("term-123".to_string());
        assert_eq!(error.to_string(), "Terminal not found: term-123");

        // SessionError
        let error = SessionError::LimitExceeded(50);
        assert_eq!(
            error.to_string(),
            "Session limit exceeded: maximum 50 sessions allowed"
        );

        // PermissionError
        let error = PermissionError::Denied("fs_write".to_string());
        assert_eq!(error.to_string(), "Permission denied: fs_write");

        // ServerError
        let error = ServerError::MethodNotFound("unknown".to_string());
        assert_eq!(error.to_string(), "Method not found: unknown");

        // ConfigError
        let error = ConfigError::MissingRequired("api_key".to_string());
        assert_eq!(error.to_string(), "Missing required configuration: api_key");
    }

    /// Test that all error types have proper suggestions in error data
    #[test]
    fn test_error_data_has_suggestions() {
        // Terminal errors should have suggestions
        let error = TerminalError::NotFound("term-123".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert!(data["suggestion"].is_string());
        assert!(!data["suggestion"].as_str().unwrap().is_empty());

        // Session errors should have suggestions
        let error = SessionError::LimitExceeded(100);
        let data = error.to_error_data().expect("Should have error data");
        assert!(data["suggestion"].is_string());
        assert!(!data["suggestion"].as_str().unwrap().is_empty());

        // Permission errors should have suggestions
        let error = PermissionError::Denied("operation".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert!(data["suggestion"].is_string());
        assert!(!data["suggestion"].as_str().unwrap().is_empty());

        // Server errors should have suggestions
        let error = ServerError::MethodNotFound("test".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert!(data["suggestion"].is_string());
        assert!(!data["suggestion"].as_str().unwrap().is_empty());

        // Config errors should have suggestions
        let error = ConfigError::MissingRequired("key".to_string());
        let data = error.to_error_data().expect("Should have error data");
        assert!(data["suggestion"].is_string());
        assert!(!data["suggestion"].as_str().unwrap().is_empty());
    }

    /// Test that error data is serializable to JSON
    #[test]
    fn test_error_data_json_serialization() {
        // Create an error with structured data
        let error = ServerError::VersionMismatch {
            client: "1.0".to_string(),
            server: "2.0".to_string(),
        };

        let data = error.to_error_data().expect("Should have error data");

        // Verify we can serialize to JSON
        let json_str = serde_json::to_string(&data).expect("Should serialize to JSON");
        assert!(json_str.contains("clientVersion"));
        assert!(json_str.contains("serverVersion"));

        // Verify we can deserialize back
        let parsed: serde_json::Value =
            serde_json::from_str(&json_str).expect("Should deserialize from JSON");
        assert_eq!(parsed["clientVersion"], "1.0");
        assert_eq!(parsed["serverVersion"], "2.0");
    }

    /// Test error code consistency across all error types
    #[test]
    fn test_error_code_consistency_across_types() {
        // All "not found" errors should use invalid params (-32602)
        assert_eq!(
            TerminalError::NotFound("x".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            SessionError::NotFound("x".to_string()).to_json_rpc_code(),
            -32602
        );
        assert_eq!(
            PermissionError::NotFound("x".to_string()).to_json_rpc_code(),
            -32602
        );

        // All "internal" errors should use internal error (-32603)
        assert_eq!(
            TerminalError::CreationFailed("x".to_string()).to_json_rpc_code(),
            -32603
        );
        assert_eq!(
            SessionError::SerializationFailed("x".to_string()).to_json_rpc_code(),
            -32603
        );
        assert_eq!(
            PermissionError::StorageFailed("x".to_string()).to_json_rpc_code(),
            -32603
        );
        assert_eq!(
            ServerError::Internal("x".to_string()).to_json_rpc_code(),
            -32603
        );

        // Method not found should always be -32601
        assert_eq!(
            ServerError::MethodNotFound("x".to_string()).to_json_rpc_code(),
            -32601
        );
    }

    /// Test that error propagation through layers maintains information
    #[test]
    fn test_error_information_preservation() {
        // Create a detailed error
        let terminal_id = "term-abc123";
        let error = TerminalError::NotFound(terminal_id.to_string());

        // Verify the terminal ID is preserved in error data
        let data = error.to_error_data().expect("Should have error data");
        assert_eq!(data["terminalId"], terminal_id);

        // Verify the error type is identifiable
        assert_eq!(data["error"], "terminal_not_found");

        // Verify helpful suggestion is provided
        assert!(data["suggestion"].as_str().unwrap().contains("terminal ID"));
    }
}
