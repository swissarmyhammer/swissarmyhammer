//! Tests for terminal operation rate limiting
//!
//! This test module verifies that rate limiting is properly enforced
//! for terminal operations to prevent denial of service attacks.
//!
//! These tests spawn real processes and must run serially to avoid
//! flaky behavior from parallel process spawning.

use claude_agent::terminal_manager::{
    TerminalCreateParams, TerminalManager, TerminalOutputParams, TerminalReleaseParams,
};
use serial_test::serial;
use std::time::Duration;
use swissarmyhammer_common::rate_limiter::{RateLimiter, RateLimiterConfig};

async fn create_test_session_manager() -> claude_agent::session::SessionManager {
    claude_agent::session::SessionManager::new()
}

fn create_client_capabilities_with_terminal() -> agent_client_protocol::ClientCapabilities {
    agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(true)
}

fn create_client_capabilities_without_terminal() -> agent_client_protocol::ClientCapabilities {
    agent_client_protocol::ClientCapabilities::new()
        .fs(agent_client_protocol::FileSystemCapability::new()
            .read_text_file(true)
            .write_text_file(true))
        .terminal(false)
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_terminal_create() {
    // Create terminal manager with strict rate limits
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 3,
        global_limit: 10,
        expensive_operation_limit: 5,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    let session_manager = create_test_session_manager().await;

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_id = session_manager.create_session(cwd, None).unwrap();
    let session_id_str = session_id.to_string();

    // First 3 creates should succeed
    for i in 0..3 {
        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "/bin/echo".to_string(),
            args: Some(vec![format!("test{}", i)]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let result = manager
            .create_terminal_with_command(&session_manager, params)
            .await;
        assert!(
            result.is_ok(),
            "Terminal creation {} should succeed, got: {:?}",
            i,
            result.err()
        );
    }

    // 4th create should fail due to rate limit
    let params = TerminalCreateParams {
        session_id: session_id_str.clone(),
        command: "/bin/echo".to_string(),
        args: Some(vec!["test4".to_string()]),
        env: None,
        cwd: None,
        output_byte_limit: None,
    };

    let result = manager
        .create_terminal_with_command(&session_manager, params)
        .await;
    assert!(
        result.is_err(),
        "4th terminal creation should be rate limited"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("rate limit") || err_msg.contains("Rate limit"),
        "Error should mention rate limiting, got: {}",
        err_msg
    );
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_terminal_execute() {
    // Create terminal manager with strict rate limits for execute
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 5, // execute has cost 2, so 2 executions allowed
        global_limit: 20,
        expensive_operation_limit: 10,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    // Create a terminal
    let terminal_id = manager.create_terminal(None).await.unwrap();

    // First 2 executions should succeed (cost 2 each = 4 tokens)
    for i in 0..2 {
        let result = manager
            .execute_command(&terminal_id, &format!("/bin/echo test{}", i))
            .await;
        assert!(
            result.is_ok(),
            "Execution {} should succeed, got: {:?}",
            i,
            result.err()
        );
    }

    // 3rd execution should fail (would need 2 more tokens, only 1 remaining)
    let result = manager
        .execute_command(&terminal_id, "/bin/echo test3")
        .await;
    assert!(
        result.is_err(),
        "3rd terminal execution should be rate limited"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("rate limit") || err_msg.contains("Rate limit"),
        "Error should mention rate limiting, got: {}",
        err_msg
    );
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_different_sessions() {
    // Create terminal manager with per-client rate limits
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 2,
        global_limit: 10,
        expensive_operation_limit: 5,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    let session_manager = create_test_session_manager().await;

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));

    // Create two different sessions
    let session_id1 = session_manager.create_session(cwd.clone(), None).unwrap();
    let session_id1_str = session_id1.to_string();
    let session_id2 = session_manager.create_session(cwd, None).unwrap();
    let session_id2_str = session_id2.to_string();

    // Session 1: use up its rate limit
    for i in 0..2 {
        let params = TerminalCreateParams {
            session_id: session_id1_str.clone(),
            command: "/bin/echo".to_string(),
            args: Some(vec![format!("session1-{}", i)]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let result = manager
            .create_terminal_with_command(&session_manager, params)
            .await;
        assert!(result.is_ok(), "Session 1 create {} should succeed", i);
    }

    // Session 1: 3rd create should fail
    let params = TerminalCreateParams {
        session_id: session_id1_str.clone(),
        command: "/bin/echo".to_string(),
        args: Some(vec!["session1-3".to_string()]),
        env: None,
        cwd: None,
        output_byte_limit: None,
    };

    let result = manager
        .create_terminal_with_command(&session_manager, params)
        .await;
    assert!(
        result.is_err(),
        "Session 1 should be rate limited after 2 creates"
    );

    // Session 2: should still be able to create terminals
    let params = TerminalCreateParams {
        session_id: session_id2_str.clone(),
        command: "/bin/echo".to_string(),
        args: Some(vec!["session2-1".to_string()]),
        env: None,
        cwd: None,
        output_byte_limit: None,
    };

    let result = manager
        .create_terminal_with_command(&session_manager, params)
        .await;
    assert!(
        result.is_ok(),
        "Session 2 should not be rate limited, got: {:?}",
        result.err()
    );
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_get_output_allowed() {
    // Create terminal manager with reasonable rate limits
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 100, // High limit for reads
        global_limit: 1000,
        expensive_operation_limit: 500,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    let session_manager = create_test_session_manager().await;

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_id = session_manager.create_session(cwd, None).unwrap();
    let session_id_str = session_id.to_string();

    // Create a terminal
    let params = TerminalCreateParams {
        session_id: session_id_str.clone(),
        command: "/bin/echo".to_string(),
        args: Some(vec!["test".to_string()]),
        env: None,
        cwd: None,
        output_byte_limit: None,
    };

    let terminal_id = manager
        .create_terminal_with_command(&session_manager, params)
        .await
        .unwrap();

    // Multiple get_output calls should succeed with high rate limit
    for _ in 0..10 {
        let params = TerminalOutputParams {
            session_id: session_id_str.clone(),
            terminal_id: terminal_id.clone(),
        };

        let result = manager.get_output(&session_manager, params).await;
        assert!(result.is_ok(), "get_output should succeed");
    }
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_kill_terminal() {
    // Create terminal manager with strict rate limits
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 2,
        global_limit: 10,
        expensive_operation_limit: 5,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    let session_manager = create_test_session_manager().await;

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_id = session_manager.create_session(cwd, None).unwrap();
    let session_id_str = session_id.to_string();

    // Create two terminals
    let mut terminal_ids = Vec::new();
    for i in 0..2 {
        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "/bin/sleep".to_string(),
            args: Some(vec!["30".to_string()]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();
        terminal_ids.push(terminal_id);

        // Start the process
        let mut terminals = manager.terminals.write().await;
        let session = terminals.get_mut(&terminal_ids[i]).unwrap();

        let mut cmd = tokio::process::Command::new("/bin/sleep");
        cmd.arg("30")
            .current_dir(&session.working_dir)
            .envs(&session.environment)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());

        let child = cmd.spawn().unwrap();
        session.process = Some(std::sync::Arc::new(tokio::sync::RwLock::new(child)));
        *session.state.write().await = claude_agent::terminal_manager::TerminalState::Running;
    }

    // Note: We've already used 2 tokens for creates, so we have 0 left for kills
    // Kill operations should be rate limited

    let kill_params = TerminalOutputParams {
        session_id: session_id_str.clone(),
        terminal_id: terminal_ids[0].clone(),
    };

    let result = manager.kill_terminal(&session_manager, kill_params).await;
    assert!(
        result.is_err(),
        "Kill should be rate limited after using up tokens for creates"
    );
}

#[tokio::test]
#[serial]
async fn test_rate_limiting_release_terminal() {
    // Create terminal manager with strict rate limits
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 2,
        global_limit: 10,
        expensive_operation_limit: 5,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set client capabilities with terminal support
    manager
        .set_client_capabilities(create_client_capabilities_with_terminal())
        .await;

    let session_manager = create_test_session_manager().await;

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
        .canonicalize()
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_id = session_manager.create_session(cwd, None).unwrap();
    let session_id_str = session_id.to_string();

    // Create two terminals (uses 2 tokens)
    let mut terminal_ids = Vec::new();
    for i in 0..2 {
        let params = TerminalCreateParams {
            session_id: session_id_str.clone(),
            command: "/bin/echo".to_string(),
            args: Some(vec![format!("test{}", i)]),
            env: None,
            cwd: None,
            output_byte_limit: None,
        };

        let terminal_id = manager
            .create_terminal_with_command(&session_manager, params)
            .await
            .unwrap();
        terminal_ids.push(terminal_id);
    }

    // Release should be rate limited (no tokens left)
    let release_params = TerminalReleaseParams {
        session_id: session_id_str.clone(),
        terminal_id: terminal_ids[0].clone(),
    };

    let result = manager
        .release_terminal(&session_manager, release_params)
        .await;
    assert!(
        result.is_err(),
        "Release should be rate limited after using up tokens"
    );
}

#[tokio::test]
#[serial]
async fn test_capability_enforcement_on_execute() {
    let rate_limiter = RateLimiter::with_config(RateLimiterConfig {
        per_client_limit: 100,
        global_limit: 1000,
        expensive_operation_limit: 500,
        window_duration: Duration::from_secs(60),
    });
    let manager = TerminalManager::with_rate_limiter(rate_limiter);

    // Set capabilities without terminal support
    manager
        .set_client_capabilities(create_client_capabilities_without_terminal())
        .await;

    // Create a terminal ID (this would normally be created with capability check)
    let terminal_id = format!("term_{}", ulid::Ulid::new());

    // Attempt to execute command - should fail due to missing capability
    let result = manager
        .execute_command(&terminal_id, "/bin/echo test")
        .await;
    assert!(
        result.is_err(),
        "Execute should fail without terminal capability"
    );
    match result {
        Err(claude_agent::AgentError::Protocol(msg)) => {
            assert!(
                msg.contains("terminal capability") || msg.contains("terminal operations"),
                "Error should mention terminal capability requirement, got: {}",
                msg
            );
        }
        Err(other) => panic!(
            "Expected Protocol error for capability violation, got: {:?}",
            other
        ),
        Ok(_) => panic!("Operation should have failed"),
    }
}
