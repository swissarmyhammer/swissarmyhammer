//! Coverage tests: AgentError and JSON-RPC error conversions.

use super::{JSON_RPC_INTERNAL_ERROR, JSON_RPC_INVALID_REQUEST, JSON_RPC_SERVER_ERROR};
use claude_agent::error::*;

// ============================================================================
// AgentError tests (additional coverage)
// ============================================================================

#[test]
fn test_agent_error_serialization() {
    let err = AgentError::Protocol("test protocol".to_string());
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("test protocol"));

    let err = AgentError::ToolExecution("tool failed".to_string());
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("tool failed"));
}

#[test]
fn test_agent_error_server_error_display() {
    let err = AgentError::ServerError("server crashed".to_string());
    assert_eq!(err.to_string(), "server error: server crashed");
    assert_eq!(err.to_json_rpc_code(), JSON_RPC_INTERNAL_ERROR);
}

#[test]
fn test_agent_error_io_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: AgentError = io_err.into();
    assert!(err.to_string().contains("not found"));
    assert_eq!(err.to_json_rpc_code(), JSON_RPC_INTERNAL_ERROR);
}

#[test]
fn test_agent_error_mcp_conversion() {
    let mcp_err = McpError::RequestTimeout;
    let agent_err: AgentError = mcp_err.into();
    assert!(agent_err.to_string().contains("timeout"));
    assert_eq!(agent_err.to_json_rpc_code(), JSON_RPC_SERVER_ERROR);
}

#[test]
fn test_agent_error_to_acp_error_all_variants() {
    // Process variant
    let err = AgentError::Process("process failed".to_string());
    let acp_err: agent_client_protocol::Error = err.into();
    assert!(acp_err.message.contains("process failed"));

    // Config variant
    let err = AgentError::Config("bad config".to_string());
    let acp_err: agent_client_protocol::Error = err.into();
    assert!(acp_err.message.contains("bad config"));

    // PermissionDenied variant
    let err = AgentError::PermissionDenied("no access".to_string());
    let acp_err: agent_client_protocol::Error = err.into();
    assert!(acp_err.message.contains("no access"));
}

// ============================================================================
// JsonRpcError and ToJsonRpcError trait tests (additional coverage)
// ============================================================================

#[test]
fn test_to_json_rpc_error_carries_code_message_and_data() {
    let err = AgentError::Protocol("missing field".to_string());
    let rpc = ToJsonRpcError::to_json_rpc_error(&err);
    assert_eq!(
        (rpc.code, rpc.message, rpc.data.is_none()),
        (JSON_RPC_INVALID_REQUEST, err.to_string(), true)
    );
}

#[test]
fn test_to_json_rpc_error_default_data() {
    // Test that default to_error_data returns None
    let err = AgentError::Process("test".to_string());
    assert!(err.to_error_data().is_none());
}
