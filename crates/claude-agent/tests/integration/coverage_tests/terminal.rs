//! Coverage tests: TerminalManager, TerminalSession and terminal type serialization.

use claude_agent::terminal_manager::{
    self, ExitStatus, GracefulShutdownTimeout, TerminalCreateParams, TerminalCreateResponse,
    TerminalManager, TerminalOutputParams, TerminalOutputResponse, TerminalReleaseParams,
    TerminalSession, TerminalState, TimeoutConfig, DEFAULT_GRACEFUL_SHUTDOWN_TIMEOUT_SECS,
};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

/// Output limit of the session that `create_test_terminal_session` builds.
const TEST_OUTPUT_BYTE_LIMIT: u64 = 100;

/// Exit code that the wait-for-exit test sets and expects back.
const TEST_EXIT_CODE: i32 = 42;

/// Output limit that the create-params round trip sets and expects back.
const CREATE_PARAMS_OUTPUT_BYTE_LIMIT: u64 = 1024;

/// Session id used by the output and release params tests.
const TEST_SESSION_ID: &str = "sess123";

/// Terminal id used by the output and release params tests.
const TEST_TERMINAL_ID: &str = "term_456";

/// Terminal id used by the create response test.
const TEST_CREATED_TERMINAL_ID: &str = "term_01ARZ3NDEKTSV4RRFFQ69G5FAV";

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
    let caps = agent_client_protocol::schema::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::schema::FileSystemCapabilities::new());
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
    let caps = agent_client_protocol::schema::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::schema::FileSystemCapabilities::new());
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
    let caps = agent_client_protocol::schema::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::schema::FileSystemCapabilities::new());
    manager.set_client_capabilities(caps).await;
    let result = manager.create_terminal(None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn test_terminal_manager_create_terminal_with_capability_disabled() {
    let manager = TerminalManager::new();
    let caps = agent_client_protocol::schema::ClientCapabilities::new()
        .terminal(false)
        .fs(agent_client_protocol::schema::FileSystemCapabilities::new());
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
    let caps = agent_client_protocol::schema::ClientCapabilities::new()
        .terminal(true)
        .fs(agent_client_protocol::schema::FileSystemCapabilities::new());
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
        output_byte_limit: TEST_OUTPUT_BYTE_LIMIT,
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
    // Write more than the session limit
    let data = vec![b'A'; (TEST_OUTPUT_BYTE_LIMIT + TEST_OUTPUT_BYTE_LIMIT / 2) as usize];
    session.add_output(&data).await;
    let size = session.get_buffer_size().await;
    assert!(size as u64 <= TEST_OUTPUT_BYTE_LIMIT);
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
        exit_code: Some(TEST_EXIT_CODE),
        signal: None,
    };
    session.set_exit_status(status).await;
    let result = session.wait_for_exit().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap().exit_code, Some(TEST_EXIT_CODE));
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
        output_byte_limit: Some(CREATE_PARAMS_OUTPUT_BYTE_LIMIT),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("echo"));
    assert!(json.contains("outputByteLimit"));

    let deserialized: TerminalCreateParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, "01ARZ3NDEKTSV4RRFFQ69G5FAV");
    assert_eq!(deserialized.command, "echo");
    assert_eq!(
        deserialized.output_byte_limit,
        Some(CREATE_PARAMS_OUTPUT_BYTE_LIMIT)
    );
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
        session_id: TEST_SESSION_ID.to_string(),
        terminal_id: TEST_TERMINAL_ID.to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("terminalId"));

    let deserialized: TerminalOutputParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, TEST_SESSION_ID);
    assert_eq!(deserialized.terminal_id, TEST_TERMINAL_ID);
}

#[test]
fn test_terminal_release_params_serialization() {
    let params = TerminalReleaseParams {
        session_id: TEST_SESSION_ID.to_string(),
        terminal_id: TEST_TERMINAL_ID.to_string(),
    };
    let json = serde_json::to_string(&params).unwrap();
    assert!(json.contains("sessionId"));
    assert!(json.contains("terminalId"));

    let deserialized: TerminalReleaseParams = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.session_id, TEST_SESSION_ID);
    assert_eq!(deserialized.terminal_id, TEST_TERMINAL_ID);
}

#[test]
fn test_terminal_create_response_serialization() {
    let response = TerminalCreateResponse {
        terminal_id: TEST_CREATED_TERMINAL_ID.to_string(),
    };
    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("terminalId"));
    assert!(json.contains(TEST_CREATED_TERMINAL_ID));

    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"terminalId": TEST_CREATED_TERMINAL_ID})
    );
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

    let wire: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(
        wire,
        serde_json::json!({"exitCode": 1, "signal": "SIGKILL"})
    );
}
