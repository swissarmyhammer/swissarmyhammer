//! Coverage tests for claude-agent crate
//!
//! Tests targeting previously uncovered code in:
//! - mcp.rs (MCP integration)
//! - terminal_manager.rs (terminal management)
//! - tools.rs (tool execution)
//! - agent.rs (core agent loop)
//! - mcp_error_handling.rs (MCP error handling)
//! - config.rs (configuration types)
//! - session.rs (session management)
//! - plan.rs (plan types)
//! - conversation_manager.rs (conversation types)
//! - editor_state.rs (editor state management)
//! - error.rs (error types)
//! - tool_types.rs (tool type conversions)
//! - tool_classification.rs (tool classification)

use claude_agent::config;
use claude_agent::error::*;
use claude_agent::mcp::*;
use claude_agent::plan::*;
use claude_agent::terminal_manager::{
    self, ExitStatus, GracefulShutdownTimeout, TerminalCreateParams, TerminalCreateResponse,
    TerminalManager, TerminalOutputParams, TerminalOutputResponse, TerminalReleaseParams,
    TerminalSession, TerminalState, TimeoutConfig, DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS,
};
use claude_agent::tool_types::*;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

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
    let mut manager = McpServerManager::new();
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
        name: "format".to_string(),
        description: None,
        required: false,
    };
    assert_eq!(arg.name, "format");
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
                name: "format".to_string(),
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

    let err = McpError::ServerError(json!({"code": -32000, "message": "fail"}));
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
        -32600
    );
    assert_eq!(McpError::RequestTimeout.to_json_rpc_code(), -32000);
    assert_eq!(McpError::ConnectionClosed.to_json_rpc_code(), -32000);
    assert_eq!(McpError::ProcessCrashed.to_json_rpc_code(), -32000);
    assert_eq!(McpError::ServerError(json!({})).to_json_rpc_code(), -32000);
    assert_eq!(McpError::StdinNotAvailable.to_json_rpc_code(), -32603);
    assert_eq!(McpError::StdoutNotAvailable.to_json_rpc_code(), -32603);
    assert_eq!(McpError::StderrNotAvailable.to_json_rpc_code(), -32603);
    assert_eq!(McpError::MissingResult.to_json_rpc_code(), -32603);
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
    assert_eq!(err.to_json_rpc_code(), -32600);
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
        -32700
    );
}

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
    assert_eq!(err.to_string(), "Server error: server crashed");
    assert_eq!(err.to_json_rpc_code(), -32603);
}

#[test]
fn test_agent_error_io_display() {
    let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "not found");
    let err: AgentError = io_err.into();
    assert!(err.to_string().contains("not found"));
    assert_eq!(err.to_json_rpc_code(), -32603);
}

#[test]
fn test_agent_error_mcp_conversion() {
    let mcp_err = McpError::RequestTimeout;
    let agent_err: AgentError = mcp_err.into();
    assert!(agent_err.to_string().contains("timeout"));
    assert_eq!(agent_err.to_json_rpc_code(), -32000);
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
// TerminalManager tests
// ============================================================================

#[test]
fn test_terminal_state_variants() {
    assert_eq!(TerminalState::Created, TerminalState::Created);
    assert_eq!(TerminalState::Running, TerminalState::Running);
    assert_eq!(TerminalState::Finished, TerminalState::Finished);
    assert_eq!(TerminalState::Killed, TerminalState::Killed);
    assert_eq!(TerminalState::Released, TerminalState::Released);
    assert_ne!(TerminalState::Created, TerminalState::Running);
}

#[test]
fn test_terminal_state_debug() {
    let state = TerminalState::Created;
    let debug_str = format!("{:?}", state);
    assert_eq!(debug_str, "Created");
}

#[test]
fn test_graceful_shutdown_timeout_new() {
    let timeout = GracefulShutdownTimeout::new(Duration::from_secs(10));
    assert_eq!(timeout.as_duration(), Duration::from_secs(10));
}

#[test]
fn test_graceful_shutdown_timeout_default() {
    let timeout = GracefulShutdownTimeout::default();
    assert_eq!(
        timeout.as_duration(),
        Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS)
    );
}

#[test]
fn test_graceful_shutdown_timeout_serialization() {
    let timeout = GracefulShutdownTimeout::new(Duration::from_secs(30));
    let json = serde_json::to_string(&timeout).unwrap();
    let deserialized: GracefulShutdownTimeout = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.as_duration(), Duration::from_secs(30));
}

#[test]
fn test_graceful_shutdown_timeout_clone() {
    let timeout = GracefulShutdownTimeout::new(Duration::from_millis(500));
    let cloned = timeout;
    assert_eq!(timeout.as_duration(), cloned.as_duration());
}

#[test]
fn test_timeout_config_default() {
    let config = TimeoutConfig::default();
    assert_eq!(
        config.graceful_shutdown_timeout.as_duration(),
        Duration::from_secs(DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS)
    );
}

#[test]
fn test_terminal_manager_default() {
    let manager = TerminalManager::default();
    let terminals = futures::executor::block_on(async {
        let t = manager.terminals.read().await;
        t.len()
    });
    assert_eq!(terminals, 0);
}

#[test]
fn test_terminal_manager_new() {
    let manager = TerminalManager::new();
    let terminals = futures::executor::block_on(async {
        let t = manager.terminals.read().await;
        t.len()
    });
    assert_eq!(terminals, 0);
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_with_caps() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let result = manager.create_terminal(None).await;
    assert!(result.is_ok());
    let terminal_id = result.unwrap();
    assert!(terminal_id.starts_with("term_"));
    let terminals = manager.terminals.read().await;
    assert_eq!(terminals.len(), 1);
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_with_working_dir() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let result = manager.create_terminal(Some("/tmp".to_string())).await;
    assert!(result.is_ok());
    let terminal_id = result.unwrap();
    let terminals = manager.terminals.read().await;
    let session = terminals.get(&terminal_id).unwrap();
    assert_eq!(session.working_dir, std::path::PathBuf::from("/tmp"));
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_without_capability() {
    let manager = TerminalManager::new();
    // Without setting capabilities, create_terminal should fail
    let result = manager.create_terminal(None).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("capabilities"));
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_with_capability() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let result = manager.create_terminal(None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_with_capability_disabled() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::ClientCapabilities::new()
        .terminal(false)
        .fs(agent_client_protocol::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let result = manager.create_terminal(None).await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("terminal capability"));
}

#[tokio::test]
async fn test_terminal_manager_execute_command_no_capability() {
    let manager = TerminalManager::new();
    let result = manager.execute_command("term_123", "echo hello").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_terminal_manager_prepare_environment_empty() {
    let manager = TerminalManager::new();
    let env = manager.prepare_environment(vec![]).unwrap();
    // Should contain at least system env vars
    assert!(!env.is_empty());
}

#[tokio::test]
async fn test_terminal_manager_prepare_environment_with_vars() {
    let manager = TerminalManager::new();
    let env = manager
        .prepare_environment(vec![
            terminal_manager::EnvVariable {
                name: "TEST_VAR".to_string(),
                value: "test_value".to_string(),
            },
            terminal_manager::EnvVariable {
                name: "ANOTHER_VAR".to_string(),
                value: "another_value".to_string(),
            },
        ])
        .unwrap();
    assert_eq!(env.get("TEST_VAR").unwrap(), "test_value");
    assert_eq!(env.get("ANOTHER_VAR").unwrap(), "another_value");
}

#[tokio::test]
async fn test_terminal_manager_prepare_environment_empty_name() {
    let manager = TerminalManager::new();
    let result = manager.prepare_environment(vec![terminal_manager::EnvVariable {
        name: "".to_string(),
        value: "value".to_string(),
    }]);
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("empty"));
}

#[tokio::test]
async fn test_terminal_manager_prepare_environment_override() {
    let manager = TerminalManager::new();
    // Override PATH with a custom value
    let env = manager
        .prepare_environment(vec![terminal_manager::EnvVariable {
            name: "PATH".to_string(),
            value: "/custom/path".to_string(),
        }])
        .unwrap();
    assert_eq!(env.get("PATH").unwrap(), "/custom/path");
}

#[tokio::test]
async fn test_terminal_manager_change_directory_not_found() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let terminal_id = manager.create_terminal(None).await.unwrap();
    let result = manager
        .change_directory(&terminal_id, "/nonexistent/path/does/not/exist")
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_terminal_manager_change_directory_terminal_not_found() {
    let manager = TerminalManager::new();
    let result = manager
        .change_directory("nonexistent_terminal", "/tmp")
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_terminal_manager_cleanup_session_terminals_empty() {
    let manager = TerminalManager::new();
    let count = manager
        .cleanup_session_terminals("nonexistent")
        .await
        .unwrap();
    assert_eq!(count, 0);
}

// ============================================================================
// TerminalSession tests (output buffering, UTF-8 boundary detection)
// ============================================================================

/// Helper to create a minimal terminal session for testing
fn create_test_terminal_session() -> TerminalSession {
    TerminalSession {
        process: None,
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
        command: None,
        args: Vec::new(),
        session_id: None,
        output_byte_limit: 100,
        output_buffer: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        buffer_truncated: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        exit_status: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        state: std::sync::Arc::new(tokio::sync::RwLock::new(TerminalState::Created)),
        output_task: None,
        timeout_config: TimeoutConfig::default(),
    }
}

#[tokio::test]
async fn test_terminal_session_add_output_basic() {
    let session = create_test_terminal_session();
    session.add_output(b"hello world").await;
    let output = session.get_output_string().await;
    assert_eq!(output, "hello world");
    assert!(!session.is_output_truncated().await);
}

#[tokio::test]
async fn test_terminal_session_add_output_truncation() {
    let session = create_test_terminal_session();
    // Session has 100 byte limit, write more than that
    let data = vec![b'A'; 150];
    session.add_output(&data).await;
    let size = session.get_buffer_size().await;
    assert!(size <= 100);
    assert!(session.is_output_truncated().await);
}

#[tokio::test]
async fn test_terminal_session_add_output_multiple() {
    let session = create_test_terminal_session();
    session.add_output(b"hello ").await;
    session.add_output(b"world").await;
    let output = session.get_output_string().await;
    assert_eq!(output, "hello world");
}

#[tokio::test]
async fn test_terminal_session_clear_output() {
    let session = create_test_terminal_session();
    session.add_output(b"some data").await;
    assert!(session.get_buffer_size().await > 0);
    session.clear_output().await;
    assert_eq!(session.get_buffer_size().await, 0);
    assert!(!session.is_output_truncated().await);
}

#[tokio::test]
async fn test_terminal_session_exit_status() {
    let session = create_test_terminal_session();
    assert!(session.get_exit_status().await.is_none());

    let status = ExitStatus {
        exit_code: Some(0),
        signal: None,
    };
    session.set_exit_status(status).await;

    let result = session.get_exit_status().await;
    assert!(result.is_some());
    assert_eq!(result.unwrap().exit_code, Some(0));
}

#[tokio::test]
async fn test_terminal_session_exit_status_with_signal() {
    let session = create_test_terminal_session();
    let status = ExitStatus {
        exit_code: None,
        signal: Some("SIGTERM".to_string()),
    };
    session.set_exit_status(status).await;

    let result = session.get_exit_status().await;
    assert!(result.is_some());
    let es = result.unwrap();
    assert!(es.exit_code.is_none());
    assert_eq!(es.signal, Some("SIGTERM".to_string()));
}

#[tokio::test]
async fn test_terminal_session_state_lifecycle() {
    let session = create_test_terminal_session();
    assert_eq!(session.get_state().await, TerminalState::Created);
    assert!(!session.is_released().await);
    assert!(!session.is_finished().await);
    assert!(session.validate_not_released().await.is_ok());
}

#[tokio::test]
async fn test_terminal_session_released_state() {
    let session = create_test_terminal_session();
    *session.state.write().await = TerminalState::Released;
    assert!(session.is_released().await);
    assert!(session.validate_not_released().await.is_err());
}

#[tokio::test]
async fn test_terminal_session_finished_state() {
    let session = create_test_terminal_session();
    *session.state.write().await = TerminalState::Finished;
    assert!(session.is_finished().await);
    assert!(!session.is_released().await);
}

#[tokio::test]
async fn test_terminal_session_utf8_truncation_preserves_boundaries() {
    // Test that truncation respects UTF-8 character boundaries by using
    // a session with a small byte limit and multibyte characters
    let session = TerminalSession {
        process: None,
        working_dir: PathBuf::from("/tmp"),
        environment: HashMap::new(),
        command: None,
        args: Vec::new(),
        session_id: None,
        output_byte_limit: 10, // Small limit to trigger truncation
        output_buffer: std::sync::Arc::new(tokio::sync::RwLock::new(Vec::new())),
        buffer_truncated: std::sync::Arc::new(tokio::sync::RwLock::new(false)),
        exit_status: std::sync::Arc::new(tokio::sync::RwLock::new(None)),
        state: std::sync::Arc::new(tokio::sync::RwLock::new(TerminalState::Created)),
        output_task: None,
        timeout_config: TimeoutConfig::default(),
    };

    // Write multibyte UTF-8 data that exceeds the limit
    // "héllo" = [0x68, 0xC3, 0xA9, 0x6C, 0x6C, 0x6F] = 6 bytes
    session.add_output("héllo".as_bytes()).await;
    session.add_output("wörld".as_bytes()).await; // 6 bytes, total > 10

    // The output should still be valid UTF-8 after truncation
    let output = session.get_output_string().await;
    assert!(!output.is_empty());
    // Verify the string is valid UTF-8 (get_output_string uses from_utf8_lossy)
    assert!(!output.contains('\u{FFFD}')); // No replacement characters
}

#[tokio::test]
async fn test_terminal_session_wait_for_exit_released() {
    let session = create_test_terminal_session();
    *session.state.write().await = TerminalState::Released;
    let result = session.wait_for_exit().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("released"));
}

#[tokio::test]
async fn test_terminal_session_wait_for_exit_already_finished() {
    let session = create_test_terminal_session();
    let status = ExitStatus {
        exit_code: Some(42),
        signal: None,
    };
    session.set_exit_status(status).await;
    let result = session.wait_for_exit().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().exit_code, Some(42));
}

#[tokio::test]
async fn test_terminal_session_wait_for_exit_no_process() {
    let session = create_test_terminal_session();
    // No exit status and no process
    let result = session.wait_for_exit().await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No process"));
}

// ============================================================================
// Terminal type serialization tests
// ============================================================================

#[test]
fn test_terminal_create_params_serialization() {
    let params = TerminalCreateParams {
        session_id: "01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
        command: "echo".to_string(),
        args: Some(vec!["hello".to_string()]),
        env: Some(vec![terminal_manager::EnvVariable {
            name: "FOO".to_string(),
            value: "bar".to_string(),
        }]),
        cwd: Some("/tmp".to_string()),
        output_byte_limit: Some(1024),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("echo"));
    assert!(json.contains("outputByteLimit"));

    let deserialized: TerminalCreateParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert_eq!(deserialized.command, "echo");
    assert_eq!(deserialized.output_byte_limit, Some(1024));
}

#[test]
fn test_terminal_create_params_minimal() {
    let json = r#"{"sessionId":"01ARZ3NDEKTSV4RRFFQ69G5FAV","command":"bash"}"#;
    let params: TerminalCreateParams = serde_json::from_str(json).unwrap();
    assert_eq!(params.command, "bash");
    assert!(params.args.is_none());
    assert!(params.env.is_none());
    assert!(params.cwd.is_none());
    assert!(params.output_byte_limit.is_none());
}

#[test]
fn test_terminal_output_params_serialization() {
    let params = TerminalOutputParams {
        session_id: "sess123".to_string(),
        terminal_id: "term_456".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("terminalId"));
}

#[test]
fn test_terminal_release_params_serialization() {
    let params = TerminalReleaseParams {
        session_id: "sess123".to_string(),
        terminal_id: "term_456".to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("terminalId"));
}

#[test]
fn test_terminal_create_response_serialization() {
    let response = TerminalCreateResponse {
        terminal_id: "term_01ARZ3NDEKTSV4RRFFQ69G5FAV".to_string(),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("terminalId"));
    assert!(json.contains("term_01ARZ3NDEKTSV4RRFFQ69G5FAV"));
}

#[test]
fn test_terminal_output_response_serialization() {
    let response = TerminalOutputResponse {
        output: "hello world\n".to_string(),
        truncated: false,
        exit_status: Some(ExitStatus {
            exit_code: Some(0),
            signal: None,
        }),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("hello world"));
    assert!(json.contains("exitStatus"));
    assert!(json.contains("exitCode"));
}

#[test]
fn test_terminal_output_response_no_exit_status() {
    let response = TerminalOutputResponse {
        output: "running...".to_string(),
        truncated: false,
        exit_status: None,
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("running..."));
    // exit_status should be skipped when None
    assert!(!json.contains("exitStatus"));
}

#[test]
fn test_exit_status_serialization() {
    let status = ExitStatus {
        exit_code: Some(1),
        signal: Some("SIGKILL".to_string()),
    };
    let json = serde_json::to_string(&status).unwrap();
    assert!(json.contains("exitCode"));
    assert!(json.contains("SIGKILL"));
}

// ============================================================================
// Config tests
// ============================================================================

#[test]
fn test_mcp_server_config_stdio_name() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: "test_server".to_string(),
        command: "test".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    });
    assert_eq!(config.name(), "test_server");
    assert_eq!(config.transport_type(), "stdio");
}

#[test]
fn test_mcp_server_config_http_name() {
    let config = config::McpServerConfig::Http(config::HttpTransport {
        transport_type: "http".to_string(),
        name: "http_server".to_string(),
        url: "http://localhost:8080".to_string(),
        headers: vec![],
    });
    assert_eq!(config.name(), "http_server");
    assert_eq!(config.transport_type(), "http");
}

#[test]
fn test_mcp_server_config_sse_name() {
    let config = config::McpServerConfig::Sse(config::SseTransport {
        transport_type: "sse".to_string(),
        name: "sse_server".to_string(),
        url: "http://localhost:9090/events".to_string(),
        headers: vec![],
    });
    assert_eq!(config.name(), "sse_server");
    assert_eq!(config.transport_type(), "sse");
}

#[test]
fn test_stdio_transport_validate_success() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec!["hello".to_string()],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_stdio_transport_validate_empty_name() {
    let config = config::StdioTransport {
        name: "".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stdio_transport_validate_empty_command() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_stdio_transport_validate_empty_env_name() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![config::EnvVariable {
            name: "".to_string(),
            value: "val".to_string(),
        }],
        cwd: None,
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_success() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "http://localhost:8080".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_http_transport_validate_https() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "https://api.example.com".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_http_transport_validate_empty_name() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "".to_string(),
        url: "http://localhost".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_empty_url() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_bad_url() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "ftp://invalid".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_http_transport_validate_empty_header_name() {
    let config = config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "http://localhost".to_string(),
        headers: vec![config::HttpHeader {
            name: "".to_string(),
            value: "val".to_string(),
        }],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_success() {
    let config = config::SseTransport {
        transport_type: "sse".to_string(),
        name: "test".to_string(),
        url: "http://localhost/events".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_ok());
}

#[test]
fn test_sse_transport_validate_empty_name() {
    let config = config::SseTransport {
        transport_type: "sse".to_string(),
        name: "".to_string(),
        url: "http://localhost/events".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_empty_url() {
    let config = config::SseTransport {
        transport_type: "sse".to_string(),
        name: "test".to_string(),
        url: "".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_sse_transport_validate_bad_url() {
    let config = config::SseTransport {
        transport_type: "sse".to_string(),
        name: "test".to_string(),
        url: "ftp://invalid".to_string(),
        headers: vec![],
    };
    assert!(config.validate().is_err());
}

#[test]
fn test_mcp_server_config_validate_delegates_to_transport() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    });
    assert!(config.validate().is_ok());

    let config = config::McpServerConfig::Http(config::HttpTransport {
        transport_type: "http".to_string(),
        name: "test".to_string(),
        url: "http://localhost".to_string(),
        headers: vec![],
    });
    assert!(config.validate().is_ok());

    let config = config::McpServerConfig::Sse(config::SseTransport {
        transport_type: "sse".to_string(),
        name: "test".to_string(),
        url: "http://localhost/events".to_string(),
        headers: vec![],
    });
    assert!(config.validate().is_ok());
}

#[test]
fn test_config_serialization_roundtrip() {
    let config = config::McpServerConfig::Stdio(config::StdioTransport {
        name: "test_server".to_string(),
        command: "node".to_string(),
        args: vec!["server.js".to_string()],
        env: vec![config::EnvVariable {
            name: "PORT".to_string(),
            value: "3000".to_string(),
        }],
        cwd: Some("/app".to_string()),
    });
    let json = serde_json::to_string(&config).unwrap();
    let _deserialized: config::McpServerConfig = serde_json::from_str(&json).unwrap();
}

#[test]
fn test_env_variable_eq() {
    let a = claude_agent::config::EnvVariable {
        name: "KEY".to_string(),
        value: "VAL".to_string(),
    };
    let b = claude_agent::config::EnvVariable {
        name: "KEY".to_string(),
        value: "VAL".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_http_header_eq() {
    let a = config::HttpHeader {
        name: "Auth".to_string(),
        value: "Bearer xyz".to_string(),
    };
    let b = config::HttpHeader {
        name: "Auth".to_string(),
        value: "Bearer xyz".to_string(),
    };
    assert_eq!(a, b);
}

#[test]
fn test_claude_agent_mode_default() {
    let mode = config::ClaudeAgentMode::default();
    assert_eq!(mode, config::ClaudeAgentMode::Normal);
}

#[test]
fn test_claude_agent_mode_record() {
    let mode = config::ClaudeAgentMode::Record {
        output_path: PathBuf::from("/tmp/recording.json"),
    };
    assert!(matches!(mode, config::ClaudeAgentMode::Record { .. }));
}

#[test]
fn test_claude_agent_mode_playback() {
    let mode = config::ClaudeAgentMode::Playback {
        input_path: PathBuf::from("/tmp/recording.json"),
    };
    assert!(matches!(mode, config::ClaudeAgentMode::Playback { .. }));
}

#[test]
fn test_stdio_transport_validate_working_directory_none() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: None,
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_ok());
}

#[test]
fn test_stdio_transport_validate_working_directory_empty() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_err());
}

#[test]
fn test_stdio_transport_validate_working_directory_relative() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("relative/path".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_err());
}

#[test]
fn test_stdio_transport_validate_working_directory_valid() {
    let config = config::StdioTransport {
        name: "test".to_string(),
        command: "echo".to_string(),
        args: vec![],
        env: vec![],
        cwd: Some("/tmp".to_string()),
    };
    let security = config::SecurityConfig {
        allowed_file_patterns: vec![],
        forbidden_paths: vec![],
        require_permission_for: vec![],
    };
    assert!(config.validate_working_directory(&security).is_ok());
}

// ============================================================================
// Plan tests
// ============================================================================

#[test]
fn test_plan_entry_status_to_acp() {
    assert_eq!(
        PlanEntryStatus::Pending.to_acp_status(),
        agent_client_protocol::PlanEntryStatus::Pending
    );
    assert_eq!(
        PlanEntryStatus::InProgress.to_acp_status(),
        agent_client_protocol::PlanEntryStatus::InProgress
    );
    assert_eq!(
        PlanEntryStatus::Completed.to_acp_status(),
        agent_client_protocol::PlanEntryStatus::Completed
    );
    // Failed and Cancelled map to Completed
    assert_eq!(
        PlanEntryStatus::Failed.to_acp_status(),
        agent_client_protocol::PlanEntryStatus::Completed
    );
    assert_eq!(
        PlanEntryStatus::Cancelled.to_acp_status(),
        agent_client_protocol::PlanEntryStatus::Completed
    );
}

#[test]
fn test_priority_to_acp() {
    assert_eq!(
        Priority::High.to_acp_priority(),
        agent_client_protocol::PlanEntryPriority::High
    );
    assert_eq!(
        Priority::Medium.to_acp_priority(),
        agent_client_protocol::PlanEntryPriority::Medium
    );
    assert_eq!(
        Priority::Low.to_acp_priority(),
        agent_client_protocol::PlanEntryPriority::Low
    );
}

#[test]
fn test_priority_ordering() {
    assert!(Priority::High < Priority::Medium);
    assert!(Priority::Medium < Priority::Low);
    assert!(Priority::High < Priority::Low);
}

#[test]
fn test_plan_entry_new() {
    let entry = PlanEntry::new("Do something".to_string(), Priority::High);
    assert_eq!(entry.content, "Do something");
    assert_eq!(entry.priority, Priority::High);
    assert_eq!(entry.status, PlanEntryStatus::Pending);
    assert!(entry.notes.is_none());
    assert!(entry.created_at.is_some());
    assert!(entry.updated_at.is_some());
    assert!(!entry.id.is_empty());
}

#[test]
fn test_plan_entry_update_status() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    assert_eq!(entry.status, PlanEntryStatus::Pending);
    assert!(!entry.is_in_progress());
    assert!(!entry.is_complete());

    entry.update_status(PlanEntryStatus::InProgress);
    assert!(entry.is_in_progress());
    assert!(!entry.is_complete());

    entry.update_status(PlanEntryStatus::Completed);
    assert!(!entry.is_in_progress());
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_failed_is_complete() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Low);
    entry.update_status(PlanEntryStatus::Failed);
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_cancelled_is_complete() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Low);
    entry.update_status(PlanEntryStatus::Cancelled);
    assert!(entry.is_complete());
}

#[test]
fn test_plan_entry_set_notes() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    entry.set_notes("Important note".to_string());
    assert_eq!(entry.notes, Some("Important note".to_string()));
}

#[test]
fn test_plan_entry_same_status_no_update() {
    let mut entry = PlanEntry::new("task".to_string(), Priority::Medium);
    let initial_updated = entry.updated_at;
    // Updating with same status should not change updated_at
    entry.update_status(PlanEntryStatus::Pending);
    assert_eq!(entry.updated_at, initial_updated);
}

#[test]
fn test_plan_entry_to_acp_entry() {
    let entry = PlanEntry::new("Test task".to_string(), Priority::High);
    let acp = entry.to_acp_entry();
    assert_eq!(acp.content, "Test task");
}

#[test]
fn test_plan_entry_to_acp_entry_with_notes() {
    let mut entry = PlanEntry::new("Test task".to_string(), Priority::High);
    entry.set_notes("some notes".to_string());
    let acp = entry.to_acp_entry();
    assert!(acp.meta.is_some());
    let meta = acp.meta.unwrap();
    assert!(meta.contains_key("notes"));
    assert!(meta.contains_key("id"));
}

#[test]
fn test_agent_plan_new() {
    let plan = AgentPlan::new();
    assert!(plan.entries.is_empty());
    assert!(plan.metadata.is_none());
    assert!(!plan.id.is_empty());
}

#[test]
fn test_agent_plan_add_entry() {
    let mut plan = AgentPlan::new();
    plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
    plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Medium));
    assert_eq!(plan.entries.len(), 2);
}

#[test]
fn test_agent_plan_get_entry() {
    let mut plan = AgentPlan::new();
    let entry = PlanEntry::new("Test".to_string(), Priority::High);
    let entry_id = entry.id.clone();
    plan.add_entry(entry);

    let found = plan.get_entry(&entry_id);
    assert!(found.is_some());
    assert_eq!(found.unwrap().content, "Test");

    assert!(plan.get_entry("nonexistent").is_none());
}

#[test]
fn test_agent_plan_update_entry_status() {
    let mut plan = AgentPlan::new();
    let entry = PlanEntry::new("Test".to_string(), Priority::High);
    let entry_id = entry.id.clone();
    plan.add_entry(entry);

    let updated = plan.update_entry_status(&entry_id, PlanEntryStatus::Completed);
    assert!(updated);
    assert_eq!(
        plan.get_entry(&entry_id).unwrap().status,
        PlanEntryStatus::Completed
    );

    let not_found = plan.update_entry_status("nonexistent", PlanEntryStatus::Failed);
    assert!(!not_found);
}

#[test]
fn test_agent_plan_is_complete() {
    let mut plan = AgentPlan::new();
    assert!(!plan.is_complete()); // Empty plan is NOT complete (requires entries)

    let entry = PlanEntry::new("task".to_string(), Priority::High);
    let id = entry.id.clone();
    plan.add_entry(entry);
    assert!(!plan.is_complete());

    plan.update_entry_status(&id, PlanEntryStatus::Completed);
    assert!(plan.is_complete());
}

#[test]
fn test_agent_plan_to_acp_plan() {
    let mut plan = AgentPlan::new();
    plan.add_entry(PlanEntry::new("Step 1".to_string(), Priority::High));
    plan.add_entry(PlanEntry::new("Step 2".to_string(), Priority::Low));

    let acp_plan = plan.to_acp_plan();
    assert_eq!(acp_plan.entries.len(), 2);
}

#[test]
fn test_plan_entry_status_serialization() {
    let status = PlanEntryStatus::InProgress;
    let json = serde_json::to_string(&status).unwrap();
    assert_eq!(json, "\"in_progress\"");

    let deserialized: PlanEntryStatus = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, PlanEntryStatus::InProgress);
}

#[test]
fn test_priority_serialization() {
    let p = Priority::High;
    let json = serde_json::to_string(&p).unwrap();
    assert_eq!(json, "\"high\"");

    let deserialized: Priority = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, Priority::High);
}

// ============================================================================
// Conversation manager type tests
// ============================================================================

#[test]
fn test_token_usage_default() {
    let usage = claude_agent::conversation_manager::TokenUsage::default();
    assert_eq!(usage.input_tokens, 0);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.total(), 0);
}

#[test]
fn test_token_usage_estimate() {
    let usage =
        claude_agent::conversation_manager::TokenUsage::estimate_from_text("hello world", "hi");
    // 11 chars / 4 = 2, 2 chars / 4 = 0
    assert_eq!(usage.input_tokens, 2);
    assert_eq!(usage.output_tokens, 0);
    assert_eq!(usage.total(), 2);
}

#[test]
fn test_token_usage_estimate_longer() {
    let input = "a".repeat(100);
    let output = "b".repeat(200);
    let usage = claude_agent::conversation_manager::TokenUsage::estimate_from_text(&input, &output);
    assert_eq!(usage.input_tokens, 25);
    assert_eq!(usage.output_tokens, 50);
    assert_eq!(usage.total(), 75);
}

#[test]
fn test_tool_execution_status_eq() {
    use claude_agent::conversation_manager::ToolExecutionStatus;
    assert_eq!(ToolExecutionStatus::Success, ToolExecutionStatus::Success);
    assert_eq!(ToolExecutionStatus::Error, ToolExecutionStatus::Error);
    assert_ne!(ToolExecutionStatus::Success, ToolExecutionStatus::Error);
}

#[test]
fn test_tool_call_request_serialization() {
    use claude_agent::conversation_manager::ToolCallRequest;
    let req = ToolCallRequest {
        id: "call_123".to_string(),
        name: "fs_read".to_string(),
        arguments: json!({"path": "/tmp/test.txt"}),
    };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("call_123"));
    assert!(json.contains("fs_read"));
    let deserialized: ToolCallRequest = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.id, "call_123");
}

// ============================================================================
// Session tests
// ============================================================================

#[test]
fn test_session_id_new() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    assert_eq!(s.len(), 26); // ULID length
}

#[test]
fn test_session_id_parse_valid() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    let parsed = claude_agent::session::SessionId::parse(&s).unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_session_id_parse_empty() {
    let result = claude_agent::session::SessionId::parse("");
    assert!(result.is_err());
}

#[test]
fn test_session_id_parse_invalid() {
    let result = claude_agent::session::SessionId::parse("not-a-ulid");
    assert!(result.is_err());
}

#[test]
fn test_session_id_default() {
    let id = claude_agent::session::SessionId::default();
    assert!(!id.to_string().is_empty());
}

#[test]
fn test_session_id_from_str() {
    let id = claude_agent::session::SessionId::new();
    let s = id.to_string();
    let parsed: claude_agent::session::SessionId = s.parse().unwrap();
    assert_eq!(id, parsed);
}

#[test]
fn test_session_id_serialization() {
    let id = claude_agent::session::SessionId::new();
    let json = serde_json::to_string(&id).unwrap();
    let deserialized: claude_agent::session::SessionId = serde_json::from_str(&json).unwrap();
    assert_eq!(id, deserialized);
}

#[test]
fn test_session_id_to_uuid_string() {
    let id = claude_agent::session::SessionId::new();
    let uuid = id.to_uuid_string();
    // UUID format: 8-4-4-4-12
    assert_eq!(uuid.len(), 36);
    assert_eq!(uuid.chars().filter(|c| *c == '-').count(), 4);
}

#[test]
fn test_session_id_ulid_string() {
    let id = claude_agent::session::SessionId::new();
    let ulid = id.ulid_string();
    assert_eq!(ulid.len(), 26);
    assert_eq!(ulid, id.to_string());
}

#[test]
fn test_session_id_as_ulid() {
    let id = claude_agent::session::SessionId::new();
    let ulid = id.as_ulid();
    assert_eq!(ulid.to_string(), id.to_string());
}

#[test]
fn test_session_id_from_ulid() {
    let ulid = ulid::Ulid::new();
    let id: claude_agent::session::SessionId = ulid.into();
    assert_eq!(id.as_ulid(), ulid);
}

#[test]
fn test_session_new() {
    let id = claude_agent::session::SessionId::new();
    let session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"));
    assert_eq!(session.id, id);
    assert_eq!(session.cwd, PathBuf::from("/tmp"));
    assert!(session.context.is_empty());
    assert_eq!(session.turn_request_count, 0);
    assert_eq!(session.turn_token_count, 0);
    assert!(session.current_mode.is_none());
}

#[test]
fn test_session_add_message() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"));
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::User,
        "hello".to_string(),
    );
    session.add_message(msg);
    assert_eq!(session.context.len(), 1);
}

#[test]
fn test_session_turn_counters() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"));

    assert_eq!(session.get_turn_request_count(), 0);
    assert_eq!(session.get_turn_token_count(), 0);

    let count = session.increment_turn_requests();
    assert_eq!(count, 1);
    assert_eq!(session.get_turn_request_count(), 1);

    let count = session.increment_turn_requests();
    assert_eq!(count, 2);

    let tokens = session.add_turn_tokens(100);
    assert_eq!(tokens, 100);
    assert_eq!(session.get_turn_token_count(), 100);

    let tokens = session.add_turn_tokens(50);
    assert_eq!(tokens, 150);

    session.reset_turn_counters();
    assert_eq!(session.get_turn_request_count(), 0);
    assert_eq!(session.get_turn_token_count(), 0);
}

#[test]
fn test_session_available_commands() {
    let id = claude_agent::session::SessionId::new();
    let mut session = claude_agent::session::Session::new(id, PathBuf::from("/tmp"));

    let cmd1 = agent_client_protocol::AvailableCommand::new("test", "A test command");

    // Initially empty
    assert!(!session.has_available_commands_changed(&[]));
    assert!(session.has_available_commands_changed(std::slice::from_ref(&cmd1)));

    session.update_available_commands(vec![cmd1.clone()]);
    assert!(!session.has_available_commands_changed(&[cmd1]));
}

#[test]
fn test_message_from_update() {
    use agent_client_protocol::{ContentBlock, ContentChunk, SessionUpdate, TextContent};

    let text = TextContent::new("hello".to_string());
    let block = ContentBlock::Text(text);
    let chunk = ContentChunk::new(block);
    let update = SessionUpdate::AgentMessageChunk(chunk);
    let msg = claude_agent::session::Message::from_update(update);
    assert!(msg.timestamp <= SystemTime::now());
}

#[test]
fn test_message_new_user() {
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::User,
        "hello".to_string(),
    );
    assert!(msg.timestamp <= SystemTime::now());
}

#[test]
fn test_message_new_assistant() {
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::Assistant,
        "response".to_string(),
    );
    assert!(msg.timestamp <= SystemTime::now());
}

#[test]
fn test_message_new_system() {
    let msg = claude_agent::session::Message::new(
        claude_agent::session::MessageRole::System,
        "system msg".to_string(),
    );
    assert!(msg.timestamp <= SystemTime::now());
}

// ============================================================================
// SessionManager tests
// ============================================================================

#[test]
fn test_session_manager_new_isolated() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = claude_agent::session::SessionManager::new()
        .with_storage_path(Some(tmp.path().to_path_buf()));
    assert!(mgr.list_sessions().unwrap().is_empty());
}

#[test]
fn test_session_manager_create_and_get_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = claude_agent::session::SessionManager::new()
        .with_storage_path(Some(tmp.path().to_path_buf()));
    let id = mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    let session = mgr.get_session(&id).unwrap();
    assert!(session.is_some());
    let session = session.unwrap();
    assert_eq!(session.id, id);
}

#[test]
fn test_session_manager_list_sessions() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = claude_agent::session::SessionManager::new()
        .with_storage_path(Some(tmp.path().to_path_buf()));
    mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    let sessions = mgr.list_sessions().unwrap();
    assert_eq!(sessions.len(), 2);
}

#[test]
fn test_session_manager_remove_session() {
    let tmp = tempfile::tempdir().unwrap();
    let mgr = claude_agent::session::SessionManager::new()
        .with_storage_path(Some(tmp.path().to_path_buf()));
    let id = mgr.create_session(PathBuf::from("/tmp"), None).unwrap();
    assert!(mgr.get_session(&id).unwrap().is_some());
    let _ = mgr.remove_session(&id);
    assert!(mgr.get_session(&id).unwrap().is_none());
}

// ============================================================================
// ToolType tests (additional coverage)
// ============================================================================

#[test]
fn test_tool_kind_serialization_all_variants() {
    let kinds = vec![
        ToolKind::Read,
        ToolKind::Edit,
        ToolKind::Delete,
        ToolKind::Move,
        ToolKind::Search,
        ToolKind::Execute,
        ToolKind::Think,
        ToolKind::Fetch,
        ToolKind::Other,
    ];
    for kind in kinds {
        let json = serde_json::to_string(&kind).unwrap();
        let deserialized: ToolKind = serde_json::from_str(&json).unwrap();
        assert_eq!(kind, deserialized);
    }
}

#[test]
fn test_tool_kind_to_acp_kind_all_variants() {
    assert!(matches!(
        ToolKind::Read.to_acp_kind(),
        agent_client_protocol::ToolKind::Read
    ));
    assert!(matches!(
        ToolKind::Edit.to_acp_kind(),
        agent_client_protocol::ToolKind::Edit
    ));
    assert!(matches!(
        ToolKind::Delete.to_acp_kind(),
        agent_client_protocol::ToolKind::Delete
    ));
    assert!(matches!(
        ToolKind::Move.to_acp_kind(),
        agent_client_protocol::ToolKind::Move
    ));
    assert!(matches!(
        ToolKind::Search.to_acp_kind(),
        agent_client_protocol::ToolKind::Search
    ));
    assert!(matches!(
        ToolKind::Execute.to_acp_kind(),
        agent_client_protocol::ToolKind::Execute
    ));
    assert!(matches!(
        ToolKind::Think.to_acp_kind(),
        agent_client_protocol::ToolKind::Think
    ));
    assert!(matches!(
        ToolKind::Fetch.to_acp_kind(),
        agent_client_protocol::ToolKind::Fetch
    ));
    assert!(matches!(
        ToolKind::Other.to_acp_kind(),
        agent_client_protocol::ToolKind::Other
    ));
}

#[test]
fn test_tool_call_status_serialization() {
    let statuses = vec![
        ToolCallStatus::Pending,
        ToolCallStatus::InProgress,
        ToolCallStatus::Completed,
        ToolCallStatus::Failed,
        ToolCallStatus::Cancelled,
    ];
    for status in statuses {
        let json = serde_json::to_string(&status).unwrap();
        let deserialized: ToolCallStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, deserialized);
    }
}

#[test]
fn test_tool_call_status_to_acp_all_variants() {
    assert!(matches!(
        ToolCallStatus::Pending.to_acp_status(),
        agent_client_protocol::ToolCallStatus::Pending
    ));
    assert!(matches!(
        ToolCallStatus::InProgress.to_acp_status(),
        agent_client_protocol::ToolCallStatus::InProgress
    ));
    assert!(matches!(
        ToolCallStatus::Completed.to_acp_status(),
        agent_client_protocol::ToolCallStatus::Completed
    ));
    assert!(matches!(
        ToolCallStatus::Failed.to_acp_status(),
        agent_client_protocol::ToolCallStatus::Failed
    ));
    // Cancelled maps to Failed
    assert!(matches!(
        ToolCallStatus::Cancelled.to_acp_status(),
        agent_client_protocol::ToolCallStatus::Failed
    ));
}

#[test]
fn test_tool_call_report_new() {
    let report = ToolCallReport::new(
        "tc_1".to_string(),
        "Reading file".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    assert_eq!(report.tool_call_id, "tc_1");
    assert_eq!(report.title, "Reading file");
    assert_eq!(report.kind, ToolKind::Read);
    assert_eq!(report.status, ToolCallStatus::Pending);
    assert!(report.content.is_empty());
    assert!(report.locations.is_empty());
    assert!(report.raw_input.is_none());
    assert!(report.raw_output.is_none());
}

#[test]
fn test_tool_call_report_update_status() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.update_status(ToolCallStatus::InProgress);
    assert_eq!(report.status, ToolCallStatus::InProgress);
    report.update_status(ToolCallStatus::Completed);
    assert_eq!(report.status, ToolCallStatus::Completed);
}

#[test]
fn test_tool_call_report_add_location() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.add_location(ToolCallLocation {
        path: "/tmp/test.txt".to_string(),
        line: Some(10),
    });
    assert_eq!(report.locations.len(), 1);
}

#[test]
fn test_tool_call_report_set_raw_io() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.set_raw_input(json!({"path": "/tmp/test.txt"}));
    report.set_raw_output(json!("file contents here"));
    assert!(report.raw_input.is_some());
    assert!(report.raw_output.is_some());
}

#[test]
fn test_tool_call_report_mark_state_sent() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.mark_state_sent();
    // After marking state, partial updates should be possible
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
}

#[test]
fn test_tool_call_report_to_acp_tool_call() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Reading file".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.set_raw_input(json!({"path": "/test"}));
    report.add_location(ToolCallLocation {
        path: "/test".to_string(),
        line: None,
    });

    let acp_call = report.to_acp_tool_call();
    assert_eq!(acp_call.tool_call_id.to_string(), "tc_1");
    assert_eq!(acp_call.title, "Reading file");
}

#[test]
fn test_tool_call_report_to_acp_update_first() {
    let report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    // First update (no previous state) - should include all fields
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
}

#[test]
fn test_tool_call_report_partial_update_after_state_sent() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.mark_state_sent();

    // Change status
    report.update_status(ToolCallStatus::InProgress);
    let update = report.to_acp_tool_call_update();
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
}

#[test]
fn test_tool_call_report_update_with_context() {
    let mut report = ToolCallReport::new(
        "tc_1".to_string(),
        "Test".to_string(),
        ToolKind::Read,
        "Read".to_string(),
    );
    report.mark_state_sent();
    report.update_status(ToolCallStatus::Completed);
    // With context includes content/locations even if unchanged
    let update = report.to_acp_tool_call_update_with_context(true);
    assert_eq!(update.tool_call_id.to_string(), "tc_1");
}

#[test]
fn test_tool_call_location_to_acp_with_line() {
    let loc = ToolCallLocation {
        path: "/test/file.rs".to_string(),
        line: Some(42),
    };
    let acp = loc.to_acp_location();
    assert_eq!(acp.line, Some(42));
}

#[test]
fn test_tool_call_location_to_acp_without_line() {
    let loc = ToolCallLocation {
        path: "/test/file.rs".to_string(),
        line: None,
    };
    let acp = loc.to_acp_location();
    assert!(acp.line.is_none());
}

#[test]
fn test_tool_call_content_diff_to_acp() {
    let content = ToolCallContent::Diff {
        path: "/test/file.rs".to_string(),
        old_text: Some("old".to_string()),
        new_text: "new".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::ToolCallContent::Diff(_)
    ));
}

#[test]
fn test_tool_call_content_diff_no_old_text() {
    let content = ToolCallContent::Diff {
        path: "/test/new_file.rs".to_string(),
        old_text: None,
        new_text: "new content".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::ToolCallContent::Diff(_)
    ));
}

#[test]
fn test_tool_call_content_terminal_to_acp() {
    let content = ToolCallContent::Terminal {
        terminal_id: "term_123".to_string(),
    };
    let acp = content.to_acp_content();
    assert!(matches!(
        acp,
        agent_client_protocol::ToolCallContent::Terminal(_)
    ));
}

// ============================================================================
// Tool classification tests
// ============================================================================

#[test]
fn test_tool_kind_classify_read_operations() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("fs_read_text_file", &args),
        ToolKind::Read
    );
    assert_eq!(ToolKind::classify_tool("fs_read", &args), ToolKind::Read);
    assert_eq!(ToolKind::classify_tool("read_file", &args), ToolKind::Read);
}

#[test]
fn test_tool_kind_classify_edit_operations() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("fs_write_text_file", &args),
        ToolKind::Edit
    );
    assert_eq!(ToolKind::classify_tool("fs_write", &args), ToolKind::Edit);
    assert_eq!(ToolKind::classify_tool("edit_file", &args), ToolKind::Edit);
}

#[test]
fn test_tool_kind_classify_delete() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("fs_delete", &args),
        ToolKind::Delete
    );
    assert_eq!(
        ToolKind::classify_tool("delete_file", &args),
        ToolKind::Delete
    );
}

#[test]
fn test_tool_kind_classify_move() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("fs_move", &args), ToolKind::Move);
    assert_eq!(
        ToolKind::classify_tool("rename_file", &args),
        ToolKind::Move
    );
}

#[test]
fn test_tool_kind_classify_search() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("search", &args), ToolKind::Search);
    assert_eq!(ToolKind::classify_tool("grep", &args), ToolKind::Search);
    assert_eq!(ToolKind::classify_tool("find", &args), ToolKind::Search);
}

#[test]
fn test_tool_kind_classify_execute() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("terminal_create", &args),
        ToolKind::Execute
    );
    assert_eq!(ToolKind::classify_tool("execute", &args), ToolKind::Execute);
    assert_eq!(ToolKind::classify_tool("run", &args), ToolKind::Execute);
}

#[test]
fn test_tool_kind_classify_fetch() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("fetch", &args), ToolKind::Fetch);
    assert_eq!(ToolKind::classify_tool("curl", &args), ToolKind::Fetch);
}

#[test]
fn test_tool_kind_classify_think() {
    let args = json!({});
    assert_eq!(ToolKind::classify_tool("think", &args), ToolKind::Think);
    assert_eq!(ToolKind::classify_tool("plan", &args), ToolKind::Think);
}

#[test]
fn test_tool_kind_classify_unknown() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("unknown_tool", &args),
        ToolKind::Other
    );
}

#[test]
fn test_tool_kind_classify_mcp_tools() {
    let args = json!({});
    assert_eq!(
        ToolKind::classify_tool("mcp__server__read_file", &args),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__write_data", &args),
        ToolKind::Edit
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__delete_item", &args),
        ToolKind::Delete
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__search_code", &args),
        ToolKind::Search
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__execute_cmd", &args),
        ToolKind::Execute
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__fetch_data", &args),
        ToolKind::Fetch
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__server__misc_op", &args),
        ToolKind::Other
    );
}

#[test]
fn test_tool_kind_classify_by_op() {
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "read file"})),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "write file"})),
        ToolKind::Edit
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "edit file"})),
        ToolKind::Edit
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "glob files"})),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "grep files"})),
        ToolKind::Search
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "search code"})),
        ToolKind::Search
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "query ast"})),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("files", &json!({"op": "unknown op"})),
        ToolKind::Other
    );
}

#[test]
fn test_tool_kind_classify_mcp_files_by_op() {
    assert_eq!(
        ToolKind::classify_tool("mcp__sah__files", &json!({"op": "read file"})),
        ToolKind::Read
    );
    assert_eq!(
        ToolKind::classify_tool("mcp__sah__treesitter", &json!({"op": "query ast"})),
        ToolKind::Read
    );
}

// ============================================================================
// Tool title generation tests
// ============================================================================

#[test]
fn test_generate_title_fs_read() {
    let title =
        ToolCallReport::generate_title("fs_read_text_file", &json!({"path": "/home/user/test.rs"}));
    assert_eq!(title, "Reading test.rs");
}

#[test]
fn test_generate_title_fs_read_no_path() {
    let title = ToolCallReport::generate_title("fs_read_text_file", &json!({}));
    assert_eq!(title, "Reading file");
}

#[test]
fn test_generate_title_fs_write() {
    let title =
        ToolCallReport::generate_title("fs_write_text_file", &json!({"path": "/tmp/output.txt"}));
    assert_eq!(title, "Writing to output.txt");
}

#[test]
fn test_generate_title_terminal_create() {
    let title =
        ToolCallReport::generate_title("terminal_create", &json!({"command": "cargo test"}));
    assert_eq!(title, "Running cargo test");
}

#[test]
fn test_generate_title_terminal_create_no_command() {
    let title = ToolCallReport::generate_title("terminal_create", &json!({}));
    assert_eq!(title, "Creating terminal session");
}

#[test]
fn test_generate_title_search() {
    let title = ToolCallReport::generate_title("search", &json!({"pattern": "TODO"}));
    assert_eq!(title, "Searching for 'TODO'");
}

#[test]
fn test_generate_title_mcp_tool() {
    let title = ToolCallReport::generate_title("mcp__server__read_data", &json!({}));
    // Should clean up and capitalize
    assert!(title.starts_with('S'));
}

#[test]
fn test_generate_title_unknown_tool() {
    let title = ToolCallReport::generate_title("custom_tool_name", &json!({}));
    assert_eq!(title, "Custom tool name");
}

#[test]
fn test_generate_title_empty_tool_name() {
    let title = ToolCallReport::generate_title("", &json!({}));
    assert_eq!(title, "Unknown tool");
}

// ============================================================================
// File location extraction tests (additional coverage)
// ============================================================================

#[test]
fn test_extract_file_locations_glob_patterns() {
    let args = json!({
        "patterns": ["src/**/*.rs", "tests/**/*.rs"]
    });
    let locations = ToolCallReport::extract_file_locations("glob", &args);
    assert_eq!(locations.len(), 2);
}

#[test]
fn test_extract_file_locations_string_argument() {
    let args = json!("/home/user/file.txt");
    let locations = ToolCallReport::extract_file_locations("read", &args);
    assert_eq!(locations.len(), 1);
}

#[test]
fn test_extract_file_locations_string_not_path() {
    let args = json!("just a string without path chars");
    let locations = ToolCallReport::extract_file_locations("read", &args);
    assert!(locations.is_empty());
}

#[test]
fn test_extract_file_locations_with_glob_wildcards() {
    // Test glob patterns are recognized as file paths via extract_file_locations
    let args = json!({"path": "*.rs"});
    let locations = ToolCallReport::extract_file_locations("glob", &args);
    assert_eq!(locations.len(), 1);
    assert!(locations[0].path.contains("*.rs"));
}

#[test]
fn test_extract_file_locations_with_bracket_glob() {
    let args = json!({"path": "test[0-9].txt"});
    let locations = ToolCallReport::extract_file_locations("glob", &args);
    assert_eq!(locations.len(), 1);
}

// ============================================================================
// EditorState tests (additional coverage)
// ============================================================================

#[tokio::test]
async fn test_editor_state_manager_default() {
    let manager = claude_agent::editor_state::EditorStateManager::default();
    assert_eq!(manager.cache_size().await, 0);
}

#[tokio::test]
async fn test_editor_state_manager_with_cache_duration() {
    let manager = claude_agent::editor_state::EditorStateManager::with_cache_duration(
        Duration::from_secs(60),
    );
    assert_eq!(manager.cache_size().await, 0);
}

#[tokio::test]
async fn test_editor_state_update_buffers_with_unavailable() {
    let manager = claude_agent::editor_state::EditorStateManager::new();
    let path1 = PathBuf::from("/test/file1.rs");

    // First cache a buffer
    let buffer = claude_agent::editor_state::EditorBuffer {
        path: path1.clone(),
        content: "content".to_string(),
        modified: true,
        last_modified: SystemTime::now(),
        encoding: "UTF-8".to_string(),
    };
    manager.cache_buffer(path1.clone(), buffer).await;
    assert_eq!(manager.cache_size().await, 1);

    // Then mark it as unavailable via update
    let response = claude_agent::editor_state::EditorBufferResponse {
        buffers: HashMap::new(),
        unavailable_paths: vec![path1],
    };
    manager.update_buffers_from_response(response).await;
    assert_eq!(manager.cache_size().await, 0);
}

// ============================================================================
// CollectedResponse and CreateAgentConfig tests
// ============================================================================

#[test]
fn test_collected_response_debug() {
    let resp = claude_agent::CollectedResponse {
        content: "hello".to_string(),
        stop_reason: agent_client_protocol::StopReason::EndTurn,
    };
    let debug = format!("{:?}", resp);
    assert!(debug.contains("hello"));
}

#[test]
fn test_create_agent_config_builder() {
    let config = claude_agent::CreateAgentConfig::builder()
        .ephemeral(true)
        .build();
    assert!(config.ephemeral);
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_create_agent_config_builder_default() {
    let config = claude_agent::CreateAgentConfig::builder().build();
    assert!(!config.ephemeral);
    assert!(config.mcp_servers.is_empty());
}

#[test]
fn test_prompt_request_creation() {
    // Verify we can construct ACP PromptRequest objects
    let session_id = agent_client_protocol::SessionId::new("test-session-id");
    let request = agent_client_protocol::PromptRequest::new(
        session_id,
        vec![agent_client_protocol::ContentBlock::Text(
            agent_client_protocol::TextContent::new("test prompt".to_string()),
        )],
    );
    assert_eq!(request.session_id.to_string(), "test-session-id");
}

// ============================================================================
// JsonRpcError and ToJsonRpcError trait tests (additional coverage)
// ============================================================================

#[test]
fn test_json_rpc_error_struct() {
    let err = JsonRpcError {
        code: -32600,
        message: "Invalid Request".to_string(),
        data: Some(json!({"detail": "missing field"})),
    };
    assert_eq!(err.code, -32600);
    assert_eq!(err.message, "Invalid Request");
    assert!(err.data.is_some());
}

#[test]
fn test_to_json_rpc_error_default_data() {
    // Test that default to_error_data returns None
    let err = AgentError::Process("test".to_string());
    assert!(err.to_error_data().is_none());
}

// ============================================================================
// Agent version constant test
// ============================================================================

#[test]
fn test_version_constant() {
    assert!(!claude_agent::VERSION.is_empty());
}

// ============================================================================
// SessionIdError tests
// ============================================================================

#[test]
fn test_session_id_error_empty_display() {
    let err = claude_agent::session::SessionId::parse("").unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[test]
fn test_session_id_error_invalid_display() {
    let err = claude_agent::session::SessionId::parse("INVALID!!!").unwrap_err();
    assert!(err.to_string().contains("Invalid ULID"));
}
