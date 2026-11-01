//! Coverage tests: MCP server manager, MCP types and McpError.

use super::{
    JSON_RPC_INTERNAL_ERROR, JSON_RPC_INVALID_REQUEST, JSON_RPC_PARSE_ERROR, JSON_RPC_SERVER_ERROR,
};
use claude_agent::error::*;
use claude_agent::mcp::*;
use serde_json::json;

/// Name of the optional prompt argument in the argument tests.
const FORMAT_ARGUMENT_NAME: &str = "format";

/// Prompt argument name that the argument test uses.
const FILE_PATH_ARGUMENT_NAME: &str = "file_path";

/// Prompt argument description that the argument test uses.
const FILE_PATH_DESCRIPTION: &str = "Path to the file";

/// Prompt name that the prompt tests use.
const TEST_PROMPT_NAME: &str = "test_prompt";

/// Prompt name that the simple prompt test uses.
const SIMPLE_PROMPT_NAME: &str = "simple";

/// Message of the protocol error.
const BAD_MESSAGE_TEXT: &str = "bad message";

/// Message of the initialization error.
const HANDSHAKE_FAILED_TEXT: &str = "handshake failed";

/// Message of the tools list error.
const TIMEOUT_TEXT: &str = "timeout";

/// Message of the configuration error.
const BAD_CONFIG_TEXT: &str = "bad config";

/// Message of the serialized error.
const TEST_ERROR_TEXT: &str = "test error";

/// Message of the I/O error.
const PIPE_BROKEN_TEXT: &str = "pipe broken";

/// Character count of a long error message.
const LONG_MESSAGE_CHARS: usize = 1000;

/// Debug text of a required argument.
const REQUIRED_TRUE_DEBUG: &str = "required: true";

/// Debug text of an optional argument.
const REQUIRED_FALSE_DEBUG: &str = "required: false";

/// Debug text of a missing description.
const DESCRIPTION_NONE_DEBUG: &str = "description: None";

/// Debug text that starts each prompt argument.
const PROMPT_ARGUMENT_DEBUG: &str = "McpPromptArgument {";

/// Debug text of an empty argument list.
const EMPTY_ARGUMENTS_DEBUG: &str = "arguments: []";

/// Render the debug text of a clone of the value.
fn debug_text<T: Clone + std::fmt::Debug>(value: &T) -> String {
    format!("{:?}", value.clone())
}

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
        name: FILE_PATH_ARGUMENT_NAME.to_string(),
        description: Some(FILE_PATH_DESCRIPTION.to_string()),
        required: true,
    };
    let debug = debug_text(&arg);
    assert!(debug.contains(FILE_PATH_ARGUMENT_NAME));
    assert!(debug.contains(FILE_PATH_DESCRIPTION));
    assert!(debug.contains(REQUIRED_TRUE_DEBUG));
}

#[test]
fn test_mcp_prompt_argument_optional() {
    let arg = McpPromptArgument {
        name: FORMAT_ARGUMENT_NAME.to_string(),
        description: None,
        required: false,
    };
    let debug = debug_text(&arg);
    assert!(debug.contains(FORMAT_ARGUMENT_NAME));
    assert!(debug.contains(DESCRIPTION_NONE_DEBUG));
    assert!(debug.contains(REQUIRED_FALSE_DEBUG));
}

#[test]
fn test_mcp_prompt_creation() {
    let prompt = McpPrompt {
        name: TEST_PROMPT_NAME.to_string(),
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
    let debug = debug_text(&prompt);
    assert!(debug.contains(TEST_PROMPT_NAME));
    assert_eq!(debug.matches(PROMPT_ARGUMENT_DEBUG).count(), 2);
    assert!(debug.contains(REQUIRED_TRUE_DEBUG));
    assert!(debug.contains(REQUIRED_FALSE_DEBUG));
}

#[test]
fn test_mcp_prompt_no_arguments() {
    let prompt = McpPrompt {
        name: SIMPLE_PROMPT_NAME.to_string(),
        description: None,
        arguments: vec![],
    };
    let debug = debug_text(&prompt);
    assert!(debug.contains(SIMPLE_PROMPT_NAME));
    assert!(debug.contains(EMPTY_ARGUMENTS_DEBUG));
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

    let err = McpError::ProtocolError(BAD_MESSAGE_TEXT.to_string());
    assert!(err.to_string().contains(BAD_MESSAGE_TEXT));

    let err = McpError::ServerError(json!({"code": JSON_RPC_SERVER_ERROR, "message": "fail"}));
    assert!(err.to_string().contains("MCP server error"));

    let err = McpError::InitializationFailed(HANDSHAKE_FAILED_TEXT.to_string());
    assert!(err.to_string().contains(HANDSHAKE_FAILED_TEXT));

    let err = McpError::ToolsListFailed(TIMEOUT_TEXT.to_string());
    assert!(err.to_string().contains(TIMEOUT_TEXT));

    let err = McpError::InvalidConfiguration(BAD_CONFIG_TEXT.to_string());
    assert!(err.to_string().contains(BAD_CONFIG_TEXT));
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
    let err = McpError::ProtocolError(TEST_ERROR_TEXT.to_string());
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains(TEST_ERROR_TEXT));

    let err = McpError::StdinNotAvailable;
    let json = serde_json::to_string(&err).unwrap();
    assert!(json.contains("stdin not available"));
}

#[test]
fn test_mcp_error_to_json_rpc_code_protocol() {
    use claude_agent::error::ToJsonRpcError;

    // The code depends on the error kind, never on the message text.
    for message in ["", "invalid msg", &"x".repeat(LONG_MESSAGE_CHARS)] {
        let err = McpError::ProtocolError(message.to_string());
        assert_eq!(err.to_json_rpc_code(), JSON_RPC_INVALID_REQUEST);
    }
}

#[test]
fn test_mcp_error_io_conversion() {
    let io_err = std::io::Error::new(std::io::ErrorKind::BrokenPipe, PIPE_BROKEN_TEXT);
    let mcp_err: McpError = io_err.into();
    assert!(mcp_err.to_string().contains(PIPE_BROKEN_TEXT));
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
