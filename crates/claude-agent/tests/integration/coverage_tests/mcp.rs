//! Coverage tests: MCP server manager, MCP types and McpError.

use super::{
    JSON_RPC_INTERNAL_ERROR, JSON_RPC_INVALID_REQUEST, JSON_RPC_PARSE_ERROR, JSON_RPC_SERVER_ERROR,
};
use claude_agent::error::*;
use claude_agent::mcp::*;
use serde_json::json;

/// Name of the optional prompt argument in the argument tests.
const FORMAT_ARGUMENT_NAME: &str = "format";

// ============================================================================
// McpServerManager tests
// ============================================================================

#[test]
fn test_mcp_server_manager_new() {
    let manager = McpServerManager::new();
    // Manager starts with no connections
    let tools = futures::executor::block_on(manager.list_available_tools());
    assert!(tools.is_empty());
}

#[tokio::test]
async fn test_mcp_server_manager_list_available_tools_empty() {
    let manager = McpServerManager::new();
    let tools = manager.list_available_tools().await;
    assert!(tools.is_empty());
}

#[tokio::test]
async fn test_mcp_server_manager_list_available_prompts_empty() {
    let manager = McpServerManager::new();
    let prompts = manager.list_available_prompts().await;
    assert!(prompts.is_empty());
}

#[tokio::test]
async fn test_mcp_server_manager_connect_servers_empty() {
    let manager = McpServerManager::new();
    let result = manager.connect_servers(vec![]).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_mcp_server_manager_shutdown_empty() {
    let manager = McpServerManager::new();
    let result = manager.shutdown().await;
    assert!(result.is_ok());
}

// ============================================================================
// MCP types tests
// ============================================================================

#[test]
fn test_mcp_prompt_argument_creation() {
    let arg = McpPromptArgument {
        name: "file_path".to_string(),
        description: Some("Path to the file".to_string()),
        required: true,
    };
    assert_eq!(arg.name, "file_path");
    assert_eq!(arg.description, Some("Path to the file".to_string()));
    assert!(arg.required);
}

#[test]
fn test_mcp_prompt_argument_optional() {
    let arg = McpPromptArgument {
        name: FORMAT_ARGUMENT_NAME.to_string(),
        description: None,
        required: false,
    };
    assert_eq!(arg.name, FORMAT_ARGUMENT_NAME);
    assert!(arg.description.is_none());
    assert!(!arg.required);
}

#[test]
fn test_mcp_prompt_creation() {
    let prompt = McpPrompt {
        name: "test_prompt".to_string(),
        description: Some("A test prompt".to_string()),
        arguments: vec![
            McpPromptArgument {
                name: "input".to_string(),
                description: Some("Input text".to_string()),
                required: true,
            },
            McpPromptArgument {
                name: FORMAT_ARGUMENT_NAME.to_string(),
                description: None,
                required: false,
            },
        ],
    };
    assert_eq!(prompt.name, "test_prompt");
    assert_eq!(prompt.arguments.len(), 2);
    assert!(prompt.arguments[0].required);
    assert!(!prompt.arguments[1].required);
}

#[test]
fn test_mcp_prompt_no_arguments() {
    let prompt = McpPrompt {
        name: "simple".to_string(),
        description: None,
        arguments: vec![],
    };
    assert_eq!(prompt.name, "simple");
    assert!(prompt.arguments.is_empty());
}

// ============================================================================
// McpError tests
// ============================================================================

#[test]
fn test_mcp_error_display() {
    let err = McpError::StdinNotAvailable;
    assert_eq!(err.to_string(), "MCP server stdin not available");

    let err = McpError::StdoutNotAvailable;
    assert_eq!(err.to_string(), "MCP server stdout not available");

    let err = McpError::StderrNotAvailable;
    assert_eq!(err.to_string(), "MCP server stderr not available");

    let err = McpError::ConnectionClosed;
    assert_eq!(err.to_string(), "MCP connection closed unexpectedly");

    let err = McpError::MissingResult;
    assert_eq!(err.to_string(), "MCP response missing result field");

    let err = McpError::RequestTimeout;
    assert_eq!(err.to_string(), "MCP request timeout");

    let err = McpError::ProcessCrashed;
    assert_eq!(err.to_string(), "MCP server process crashed");

    let err = McpError::ProtocolError("bad message".to_string());
    assert!(err.to_string().contains("bad message"));

    let err = McpError::ServerError(json!({"code": JSON_RPC_SERVER_ERROR, "message": "fail"}));
    assert!(err.to_string().contains("MCP server error"));

    let err = McpError::InitializationFailed("handshake failed".to_string());
    assert!(err.to_string().contains("handshake failed"));

    let err = McpError::ToolsListFailed("timeout".to_string());
    assert!(err.to_string().contains("timeout"));

    let err = McpError::InvalidConfiguration("bad config".to_string());
    assert!(err.to_string().contains("bad config"));
}

#[test]
fn test_mcp_error_json_rpc_codes() {
    use claude_agent::error::ToJsonRpcError;

    assert_eq!(
        McpError::ProtocolError("test".to_string()).to_json_rpc_code(),
        JSON_RPC_INVALID_REQUEST
    );
    assert_eq!(
        McpError::RequestTimeout.to_json_rpc_code(),
        JSON_RPC_SERVER_ERROR
    );
    assert_eq!(
        McpError::ConnectionClosed.to_json_rpc_code(),
        JSON_RPC_SERVER_ERROR
    );
    assert_eq!(
        McpError::ProcessCrashed.to_json_rpc_code(),
        JSON_RPC_SERVER_ERROR
    );
    assert_eq!(
        McpError::ServerError(json!({})).to_json_rpc_code(),
        JSON_RPC_SERVER_ERROR
    );
    assert_eq!(
        McpError::StdinNotAvailable.to_json_rpc_code(),
        JSON_RPC_INTERNAL_ERROR
    );
    assert_eq!(
        McpError::StdoutNotAvailable.to_json_rpc_code(),
        JSON_RPC_INTERNAL_ERROR
    );
    assert_eq!(
        McpError::StderrNotAvailable.to_json_rpc_code(),
        JSON_RPC_INTERNAL_ERROR
    );
    assert_eq!(
        McpError::MissingResult.to_json_rpc_code(),
        JSON_RPC_INTERNAL_ERROR
    );
}

#[test]
fn test_mcp_error_serialization() {
    let err = McpError::ProtocolError("test error".to_string());
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("test error"));

    let err = McpError::StdinNotAvailable;
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("stdin not available"));
}

#[test]
fn test_mcp_error_to_json_rpc_code_protocol() {
    use claude_agent::error::ToJsonRpcError;

    let err = McpError::ProtocolError("invalid msg".to_string());
    assert_eq!(err.to_json_rpc_code(), JSON_RPC_INVALID_REQUEST);
}

#[test]
fn test_mcp_error_io_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, "pipe broken");
    let mcp_err: McpError = io_err.into();
    assert!(mcp_err.to_string().contains("pipe broken"));
}

#[test]
fn test_mcp_error_serde_json_conversion() {
    let serde_err = serde_json::from_str::<serde_json::Value>("not json").unwrap_err();
    let mcp_err: McpError = serde_err.into();
    assert!(mcp_err.to_string().contains("MCP message serialization"));
    assert_eq!(
        <McpError as ToJsonRpcError>::to_json_rpc_code(&mcp_err),
        JSON_RPC_PARSE_ERROR
    );
}
