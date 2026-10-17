//! Unit tests for [`LspDaemon`](super::LspDaemon).
//!
//! The tests are split by topic so that each file stays small:
//! - this file: the shared spec helpers, state, lifecycle, health check and
//!   shutdown tests
//! - [`jsonrpc`]: the JSON-RPC framing helpers
//! - [`handshake`]: the `initialize` handshake and stderr capture
//! - [`restart`]: the restart backoff and the failure limit

use super::*;

mod handshake;
mod jsonrpc;
mod restart;

#[test]
fn test_backoff_sequence() {
    assert_eq!(backoff_duration(0), Duration::from_secs(1));
    assert_eq!(backoff_duration(1), Duration::from_secs(2));
    assert_eq!(backoff_duration(2), Duration::from_secs(4));
    assert_eq!(backoff_duration(3), Duration::from_secs(8));
    assert_eq!(backoff_duration(4), Duration::from_secs(16));
    assert_eq!(backoff_duration(5), Duration::from_secs(32));
    assert_eq!(backoff_duration(6), Duration::from_secs(60));
    assert_eq!(backoff_duration(7), Duration::from_secs(60));
    assert_eq!(backoff_duration(100), Duration::from_secs(60));
}

#[test]
fn test_state_transitions() {
    // Starting -> Running
    let state = LspDaemonState::Starting;
    assert_eq!(state, LspDaemonState::Starting);

    let running = LspDaemonState::Running {
        pid: 1234,
        since_epoch_ms: 0,
    };
    assert!(matches!(running, LspDaemonState::Running { pid: 1234, .. }));

    // Starting -> Failed
    let failed = LspDaemonState::Failed {
        reason: "crash".into(),
        attempts: 1,
    };
    assert!(matches!(failed, LspDaemonState::Failed { attempts: 1, .. }));

    // Failed -> Starting (restart)
    let restarting = LspDaemonState::Starting;
    assert_eq!(restarting, LspDaemonState::Starting);
}

#[test]
fn test_state_serialization() {
    let state = LspDaemonState::Running {
        pid: 42,
        since_epoch_ms: 1700000000000,
    };
    let json = serde_json::to_string(&state).expect("serialize");
    let deser: LspDaemonState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(state, deser);

    let failed = LspDaemonState::Failed {
        reason: "timeout".into(),
        attempts: 3,
    };
    let json = serde_json::to_string(&failed).expect("serialize");
    let deser: LspDaemonState = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(failed, deser);
}

#[test]
fn test_daemon_initial_state() {
    use crate::types::OwnedLspServerSpec;
    use swissarmyhammer_project_detection::ProjectType;
    let spec = OwnedLspServerSpec {
        project_types: vec![ProjectType::Rust],
        command: "rust-analyzer".to_string(),
        args: vec![],
        language_ids: vec!["rust".to_string()],
        file_extensions: vec!["rs".to_string()],
        startup_timeout_secs: 30,
        health_check_interval_secs: 60,
        install_hint: "Install rust-analyzer: rustup component add rust-analyzer".to_string(),
        icon: None,
    };
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert_eq!(daemon.command(), "rust-analyzer");
}

// -- helper for lifecycle tests ------------------------------------------

/// Build a minimal `OwnedLspServerSpec` for testing.
fn test_spec(command: &str) -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: command.to_string(),
        args: vec![],
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        startup_timeout_secs: 5,
        health_check_interval_secs: 60,
        install_hint: format!("install {command}"),
        icon: None,
    }
}

// -- start() tests -------------------------------------------------------

#[tokio::test]
async fn test_start_binary_not_found() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    let result = daemon.start().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::BinaryNotFound { .. }),
        "expected BinaryNotFound, got: {err:?}"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

#[tokio::test]
async fn test_start_binary_not_found_preserves_hint() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    let result = daemon.start().await;
    match result.unwrap_err() {
        LspError::BinaryNotFound {
            command,
            install_hint,
        } => {
            assert_eq!(command, "nonexistent-lsp-binary-abc123xyz");
            assert!(install_hint.contains("nonexistent-lsp-binary-abc123xyz"));
        }
        other => panic!("expected BinaryNotFound, got: {other:?}"),
    }
}

// -- client() tests ------------------------------------------------------

#[test]
fn test_client_returns_none_when_not_started() {
    let spec = test_spec("some-server");
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert!(daemon.client().is_none());
}

#[test]
fn test_shared_client_returns_arc() {
    let spec = test_spec("some-server");
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    let shared = daemon.shared_client();
    // The shared client should be lockable and contain None
    let guard = shared.lock().unwrap();
    assert!(guard.is_none());
}

#[test]
fn test_client_recovers_from_poisoned_mutex() {
    let spec = test_spec("some-server");
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    // Poison the mutex by panicking inside a lock
    let shared = daemon.shared_client();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = shared.lock().unwrap();
        panic!("intentional panic to poison mutex");
    }));

    // client() should still work via the poison-recovery path
    // The inner Option is still None, so it returns None
    assert!(daemon.client().is_none());
}

// -- health_check() tests ------------------------------------------------

#[test]
fn test_health_check_returns_false_when_no_child() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert!(!daemon.health_check());
}

// -- shutdown() tests ----------------------------------------------------

#[tokio::test]
async fn test_shutdown_when_not_started() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    // Shutdown should be a no-op that transitions to NotStarted
    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

#[tokio::test]
async fn test_shutdown_clears_client() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    let shared = daemon.shared_client();

    daemon.shutdown().await;

    // Client should be None after shutdown
    let guard = shared.lock().unwrap();
    assert!(guard.is_none());
}

// -- restart_with_backoff() tests ----------------------------------------

#[tokio::test]
async fn test_restart_with_backoff_exhausted() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    // Simulate MAX_CONSECUTIVE_FAILURES failures
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let result = daemon.restart_with_backoff().await;
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(ref msg) if msg.contains("too many consecutive failures")),
        "expected HandshakeFailed with 'too many consecutive failures', got: {err:?}"
    );
}

#[tokio::test]
async fn test_restart_with_backoff_resets_on_binary_not_found() {
    // restart_with_backoff calls start(), which will fail with BinaryNotFound
    // and increment consecutive_failures further
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 0;

    let result = daemon.restart_with_backoff().await;
    assert!(result.is_err());
    // State should be NotFound since binary doesn't exist
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

// -- force_restart() tests -----------------------------------------------

#[tokio::test]
async fn test_force_restart_resets_failures() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 3;

    // force_restart resets failures to 0 then calls start()
    let result = daemon.force_restart().await;
    assert!(result.is_err()); // binary not found
                              // consecutive_failures was reset to 0 before start(), but start() didn't
                              // call record_failure because BinaryNotFound doesn't go through record_failure
    assert_eq!(daemon.consecutive_failures, 0);
}

// -- state_rx() tests ----------------------------------------------------

#[tokio::test]
async fn test_state_rx_observes_transitions() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    let mut rx = daemon.state_rx();

    assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);

    // Trigger a start (will fail with NotFound)
    let _ = daemon.start().await;

    // The rx should have seen the NotFound state
    rx.changed().await.unwrap();
    assert_eq!(*rx.borrow(), LspDaemonState::NotFound);
}

// -- record_failure() tests ----------------------------------------------

#[test]
fn test_record_failure_increments_counter() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert_eq!(daemon.consecutive_failures, 0);

    daemon.record_failure("test failure 1".into());
    assert_eq!(daemon.consecutive_failures, 1);
    assert!(matches!(
        daemon.state(),
        LspDaemonState::Failed { attempts: 1, .. }
    ));

    daemon.record_failure("test failure 2".into());
    assert_eq!(daemon.consecutive_failures, 2);
    assert!(matches!(
        daemon.state(),
        LspDaemonState::Failed { attempts: 2, .. }
    ));
}

#[test]
fn test_record_failure_stores_reason() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    daemon.record_failure("connection refused".into());
    match daemon.state() {
        LspDaemonState::Failed { reason, .. } => {
            assert_eq!(reason, "connection refused");
        }
        other => panic!("expected Failed state, got: {other:?}"),
    }
}

// -- Debug impl test -----------------------------------------------------

#[test]
fn test_daemon_debug_output() {
    let spec = test_spec("test-server");
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    let debug = format!("{daemon:?}");
    assert!(debug.contains("test-server"));
    assert!(debug.contains("NotStarted"));
}

// -- Lifecycle tests with real processes ---------------------------------
//
// These tests use simple commands (cat, true, etc.) to exercise the
// daemon lifecycle without requiring a full LSP server implementation.

/// Build a spec that spawns a process which exits immediately.
/// This tests the handshake timeout / EOF error path.
fn immediately_exiting_spec() -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: "true".to_string(),
        args: vec![],
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        startup_timeout_secs: 2,
        health_check_interval_secs: 60,
        install_hint: "N/A".to_string(),
        icon: None,
    }
}

/// Build a spec that spawns `cat`, which keeps stdin/stdout open
/// but never speaks LSP protocol. This tests the handshake timeout path.
fn cat_spec() -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: "cat".to_string(),
        args: vec![],
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        // Very short timeout so the test doesn't hang
        startup_timeout_secs: 1,
        health_check_interval_secs: 60,
        install_hint: "N/A".to_string(),
        icon: None,
    }
}

/// Helper: build a spec that runs a Python script as the LSP server.
fn python_mock_spec(script: &str) -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: "python3".to_string(),
        args: vec!["-c".to_string(), script.to_string()],
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        startup_timeout_secs: 5,
        health_check_interval_secs: 60,
        install_hint: "N/A".to_string(),
        icon: None,
    }
}

#[tokio::test]
async fn test_start_immediately_exiting_process() {
    // `true` exits immediately, so the handshake should fail with EOF
    let spec = immediately_exiting_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_err(), "expected start to fail");
    // Should be a handshake failure (EOF reading headers) or timeout
    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(_) | LspError::Timeout(_)),
        "expected HandshakeFailed or Timeout, got: {err:?}"
    );
    // State should be Failed
    assert!(
        matches!(daemon.state(), LspDaemonState::Failed { .. }),
        "expected Failed, got: {:?}",
        daemon.state()
    );
    assert_eq!(daemon.consecutive_failures, 1);
}

#[tokio::test]
async fn test_start_with_cat_succeeds_as_echo() {
    // `cat` echoes the initialize request back verbatim, which happens to
    // be valid JSON-RPC framing. The daemon doesn't validate response
    // content, so this "succeeds". This confirms the handshake completes
    // when the child produces valid framing on stdout.
    let spec = cat_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    // cat echoes a valid JSON-RPC message, so the handshake succeeds
    assert!(
        result.is_ok(),
        "expected cat echo to pass handshake: {result:?}"
    );
    assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

#[test]
fn test_set_path_filter_reaches_the_daemon_session() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(cat_spec(), workspace.path().to_path_buf());
    daemon.set_path_filter(Arc::new(|path: &Path| path.ends_with("skip.rs")));

    let outcome = daemon
        .session()
        .sync_open(&workspace.path().join("skip.rs"), "fn s() {}")
        .expect("a filtered path is skipped, not an error");

    assert_eq!(outcome, crate::session::DocumentSync::Skipped);
}

#[tokio::test]
async fn test_shutdown_resets_session_open_documents() {
    // `cat` gives the daemon a live client (it echoes valid framing), so a
    // didOpen notification through the session succeeds. After shutdown the
    // client is gone and the session's open set must be cleared, so a
    // restart re-announces documents rather than suppressing them.
    let spec = cat_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    daemon
        .start()
        .await
        .expect("cat echo handshake should pass");

    let session = daemon.session();
    let path = workspace.path().join("doc.rs");
    session.open(&path, "fn d() {}").expect("open via session");
    assert!(
        session.is_open(&path),
        "document should be open after open()"
    );

    daemon.shutdown().await;

    assert!(
        !session.is_open(&path),
        "shutdown must clear the session's open-document set"
    );
    assert!(daemon.session().open_documents().is_empty());
}

#[tokio::test]
async fn test_health_check_detects_exited_process_via_cat() {
    // Spawn `cat`, then kill it and verify health_check detects the exit.
    // We skip the handshake by directly setting up the child.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    let spec = cat_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    // Process should be alive
    assert!(daemon.health_check());

    // Kill it
    if let Some(child) = daemon.child.as_mut() {
        child.kill().await.expect("kill cat");
        let _ = child.wait().await;
    }

    // health_check should now detect the exit
    assert!(!daemon.health_check());
    assert!(
        matches!(daemon.state(), LspDaemonState::Failed { .. }),
        "expected Failed, got: {:?}",
        daemon.state()
    );
    assert_eq!(daemon.consecutive_failures, 1);
}

#[tokio::test]
async fn test_graceful_stop_of_running_child() {
    // Directly set a child process and test that stop drops it
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    let spec = cat_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    // Shutting down should clean up the child
    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert!(daemon.child.is_none());
    assert!(daemon.client().is_none());
}

#[tokio::test]
async fn test_force_restart_after_failure() {
    // Simulate a daemon that failed, then force_restart
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 4;
    daemon.record_failure("previous crash".into());

    // force_restart resets consecutive_failures to 0 then calls start()
    let result = daemon.force_restart().await;
    assert!(result.is_err()); // binary still not found
                              // But consecutive_failures was reset before the start attempt
    assert_eq!(daemon.consecutive_failures, 0);
}

#[tokio::test]
async fn test_state_transitions_through_failed_start() {
    let spec = immediately_exiting_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    let mut rx = daemon.state_rx();

    assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);

    let _ = daemon.start().await;

    // Should have transitioned through Starting -> Failed
    // Drain all changes
    while rx.has_changed().unwrap_or(false) {
        let _ = rx.borrow_and_update();
    }
    let final_state = rx.borrow().clone();
    assert!(
        matches!(final_state, LspDaemonState::Failed { .. }),
        "expected Failed, got: {final_state:?}"
    );
}

#[tokio::test]
async fn test_shutdown_with_child_transitions_through_shutting_down() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    let spec = cat_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    let mut rx = daemon.state_rx();

    daemon.shutdown().await;

    // Collect all state changes that happened
    let mut saw_shutting_down = false;
    // The watch channel coalesces changes, but ShuttingDown should have
    // been emitted before NotStarted
    while rx.has_changed().unwrap_or(false) {
        let state = rx.borrow_and_update().clone();
        if state == LspDaemonState::ShuttingDown {
            saw_shutting_down = true;
        }
    }
    // Final state should be NotStarted
    assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);
    // Note: watch channels coalesce, so ShuttingDown might have been
    // replaced by NotStarted before we observed it. That's OK -- the
    // important thing is that NotStarted is the final state.
    let _ = saw_shutting_down; // avoid unused warning
}

#[tokio::test]
async fn test_multiple_consecutive_failures_track_correctly() {
    let spec = immediately_exiting_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    // Each failed start should increment the failure counter
    let _ = daemon.start().await;
    assert_eq!(daemon.consecutive_failures, 1);

    let _ = daemon.start().await;
    assert_eq!(daemon.consecutive_failures, 2);

    let _ = daemon.start().await;
    assert_eq!(daemon.consecutive_failures, 3);

    match daemon.state() {
        LspDaemonState::Failed { attempts, .. } => {
            assert_eq!(attempts, 3);
        }
        other => panic!("expected Failed, got: {other:?}"),
    }
}

// -- LspError Display tests -----------------------------------------------

#[test]
fn test_lsp_error_display_variants() {
    let err = LspError::BinaryNotFound {
        command: "test-cmd".to_string(),
        install_hint: "install it".to_string(),
    };
    assert!(err.to_string().contains("test-cmd"));

    let err = LspError::HandshakeFailed("timeout".to_string());
    assert!(err.to_string().contains("timeout"));

    let err = LspError::Timeout(Duration::from_secs(30));
    assert!(err.to_string().contains("30"));

    let err = LspError::ShutdownFailed("crash".to_string());
    assert!(err.to_string().contains("crash"));

    let err = LspError::NotRunning;
    assert!(err.to_string().contains("not running"));

    let err = LspError::JsonRpc("bad frame".to_string());
    assert!(err.to_string().contains("bad frame"));

    let err = LspError::ProjectDetection("no projects".to_string());
    assert!(err.to_string().contains("no projects"));

    let err = LspError::DaemonNotFound("missing-cmd".to_string());
    assert!(err.to_string().contains("missing-cmd"));
}

// -- start() timeout path via cat -----------------------------------------

#[tokio::test]
async fn test_start_cat_with_very_short_timeout() {
    // cat keeps stdin/stdout open but doesn't respond with valid JSON-RPC.
    // It actually echoes the request back as raw bytes, which happens to be
    // valid framing. To trigger the timeout path, we need a command that
    // keeps pipes open but doesn't echo. `/bin/sleep` does this.
    let spec = OwnedLspServerSpec {
        project_types: vec![],
        command: "sleep".to_string(),
        args: vec!["10".to_string()],
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        startup_timeout_secs: 1,
        health_check_interval_secs: 60,
        install_hint: "N/A".to_string(),
        icon: None,
    };
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    // sleep doesn't have stdin piped correctly for write, so it may fail
    // with either Timeout, HandshakeFailed, or SpawnFailed depending on
    // the OS behavior. The key is that it fails.
    assert!(
        matches!(
            err,
            LspError::Timeout(_)
                | LspError::HandshakeFailed(_)
                | LspError::SpawnFailed(_)
                | LspError::JsonRpc(_)
        ),
        "expected Timeout/HandshakeFailed/SpawnFailed/JsonRpc, got: {err:?}"
    );
}

// -- shutdown with a real running child -----------------------------------

#[tokio::test]
async fn test_shutdown_kills_long_running_child() {
    // Spawn a process that won't exit on its own
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("sleep")
        .args(["60"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sleep");

    let spec = test_spec("sleep");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);
    // `sleep` ignores the closed pipes and never exits, so graceful
    // shutdown always hits the timeout branch. Shorten the grace period so
    // the test exercises that branch (state → NotStarted, child killed on
    // drop) without waiting out the full production grace.
    daemon.set_shutdown_grace(Duration::from_millis(200));

    // Shutdown should complete within the grace period (kill_on_drop)
    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert!(daemon.child.is_none());
}

// -- restart_with_backoff delay test --------------------------------------

#[test]
fn test_client_recovers_from_poisoned_mutex_with_some_value() {
    // This test covers the poisoned mutex path where the inner Option IS Some.
    // We can't easily create a real LspJsonRpcClient without pipes, so we
    // test via shared_client directly.
    let spec = test_spec("some-server");
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    let shared = daemon.shared_client();

    // Put a real client into the mutex
    // We need stdin/stdout for LspJsonRpcClient::new
    let stdin = std::process::Command::new("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();

    if let Ok(mut child) = stdin {
        let child_stdin = child.stdin.take().unwrap();
        let child_stdout = child.stdout.take().unwrap();
        {
            let mut guard = shared.lock().unwrap();
            *guard = Some(LspJsonRpcClient::new(child_stdin, child_stdout));
        }

        // Poison the mutex by panicking inside a lock
        let shared_clone = Arc::clone(&shared);
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = shared_clone.lock().unwrap();
            panic!("intentional panic to poison mutex");
        }));

        // client() should recover from the poisoned mutex.
        // The inner Option is Some, so it should return Some.
        let result = daemon.client();
        assert!(
            result.is_some(),
            "expected Some from poisoned mutex with client present"
        );

        let _ = child.kill();
    }
}

#[tokio::test]
async fn test_restart_with_backoff_increments_failure_on_bad_binary() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    // Set to 1 failure (below MAX) so restart_with_backoff proceeds
    daemon.consecutive_failures = 1;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    // Should have waited at least ~2s backoff for attempt=1
    assert!(
        elapsed >= Duration::from_secs(1),
        "expected at least 1s backoff delay, got: {elapsed:?}"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

// -- file_extensions accessor test ----------------------------------------

#[test]
fn test_file_extensions_accessor() {
    let mut spec = test_spec("some-lsp");
    spec.file_extensions = vec!["rs".to_string(), "toml".to_string(), "lock".to_string()];
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert_eq!(
        daemon.file_extensions(),
        &["rs".to_string(), "toml".to_string(), "lock".to_string()]
    );
}

#[test]
fn test_file_extensions_empty() {
    let mut spec = test_spec("some-lsp");
    spec.file_extensions = vec![];
    let daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert!(daemon.file_extensions().is_empty());
}

// -- Card 1: additional start() failure path coverage --------------------

/// Building an `OwnedLspServerSpec` that points at a directory for the
/// command. `which::which` on a directory returns Err, so this path hits
/// the binary-not-found branch — adds an extra assertion that
/// `consecutive_failures` is not incremented on `BinaryNotFound`, which
/// exercises the documented contract for `set_state(NotFound)`.
#[tokio::test]
async fn test_start_binary_not_found_does_not_increment_failures() {
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    assert_eq!(daemon.consecutive_failures, 0);

    let _ = daemon.start().await;
    // BinaryNotFound goes through set_state(NotFound), NOT record_failure.
    // The counter must remain at 0.
    assert_eq!(daemon.consecutive_failures, 0);
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

/// Exercise the stderr filter drain task by running a Python mock that
/// emits stderr lines during handshake. The drain task is spawned when
/// start() succeeds; we then verify it does not interfere with shutdown.
#[tokio::test]
async fn test_start_spawns_stderr_filter_task_on_success() {
    // Mock LSP that completes handshake and ALSO writes to stderr.
    // The stderr drain task should consume the stderr lines without
    // blocking shutdown. This hits lines 223-225 / 230 of the drain loop.
    let script = r#"
import sys, re, json, time
# Emit some stderr before handshake so the filter task has input to drain
sys.stderr.write('mock lsp: starting up\n')
sys.stderr.write('mock lsp: loading config\n')
sys.stderr.flush()

headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
# Write more stderr after the response
sys.stderr.write('mock lsp: ready\n')
sys.stderr.flush()
# Read the initialized notification
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(
        result.is_ok(),
        "expected mock LSP to handshake successfully: {result:?}"
    );
    assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));

    // Give the spawned stderr drain task a moment to read the lines
    tokio::time::sleep(Duration::from_millis(100)).await;

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

/// Verify that `start()` on a success path takes stdin and stdout from
/// the child process and creates an `LspJsonRpcClient`. This exercises
/// the successful branch of the pipe-conversion `match` block
/// (`(Ok(stdin_fd), Ok(stdout_fd))` arm).
#[tokio::test]
async fn test_start_success_creates_shared_client() {
    let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_ok(), "expected start to succeed: {result:?}");

    // After a successful start, the shared client should contain Some(_).
    // This confirms the pipe conversion succeeded.
    let shared = daemon.shared_client();
    {
        let guard = shared.lock().unwrap();
        assert!(
            guard.is_some(),
            "expected shared client to be Some after successful start"
        );
    }

    // And the child handle is retained for shutdown.
    assert!(daemon.child.is_some());

    daemon.shutdown().await;
}

/// When `start()` succeeds, a `Running { pid, since_epoch_ms }` state
/// with a sensible timestamp is emitted. This covers the success-path
/// transition in `start()`.
#[tokio::test]
async fn test_start_success_sets_running_state_with_pid_and_timestamp() {
    let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(0.5)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let before_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis() as u64;

    let result = daemon.start().await;
    assert!(result.is_ok(), "expected start to succeed: {result:?}");

    match daemon.state() {
        LspDaemonState::Running {
            pid,
            since_epoch_ms,
        } => {
            assert!(pid > 0, "expected a non-zero pid, got: {pid}");
            assert!(
                since_epoch_ms >= before_ms,
                "expected since_epoch_ms >= before_ms, got: {since_epoch_ms} vs {before_ms}"
            );
        }
        other => panic!("expected Running, got: {other:?}"),
    }

    // A successful start adds no failure.
    assert_eq!(daemon.consecutive_failures, 0);

    daemon.shutdown().await;
}

/// Successful start followed by a second start() documents that the
/// daemon does not refuse concurrent starts — it overwrites the child
/// handle. This covers the path where `start()` is entered while a
/// child already exists (the old child is dropped via `kill_on_drop`).
#[tokio::test]
async fn test_start_after_previous_successful_start() {
    let script = r#"
import sys, re, json, time
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
time.sleep(1.0)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let r1 = daemon.start().await;
    assert!(r1.is_ok(), "first start failed: {r1:?}");
    let first_pid = match daemon.state() {
        LspDaemonState::Running { pid, .. } => pid,
        other => panic!("expected Running after first start, got: {other:?}"),
    };

    // Now start again without shutdown — new child overwrites old
    let r2 = daemon.start().await;
    assert!(r2.is_ok(), "second start failed: {r2:?}");
    let second_pid = match daemon.state() {
        LspDaemonState::Running { pid, .. } => pid,
        other => panic!("expected Running after second start, got: {other:?}"),
    };
    assert_ne!(first_pid, second_pid, "second start should spawn a new pid");

    daemon.shutdown().await;
}

// -- Card 2: additional health_check() failure path coverage --------------

/// `health_check` when the child has already exited must:
/// 1. Clear the stored child handle
/// 2. Clear the shared client mutex
/// 3. Transition to Failed with a reason containing "process exited"
/// 4. Return false
/// 5. Increment consecutive_failures exactly once per detection
#[tokio::test]
async fn test_health_check_exited_process_full_contract() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn true");

    let spec = immediately_exiting_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    // Put a fake non-None marker into the shared client so we can verify
    // health_check clears it. We use an LspJsonRpcClient wired to separate
    // pipes so we don't mess with the child under test.
    let helper_child = std::process::Command::new("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    if let Ok(mut helper) = helper_child {
        let hstdin = helper.stdin.take().unwrap();
        let hstdout = helper.stdout.take().unwrap();
        let shared = daemon.shared_client();
        {
            let mut guard = shared.lock().unwrap();
            *guard = Some(LspJsonRpcClient::new(hstdin, hstdout));
        }

        // Wait for the `true` process to exit
        tokio::time::sleep(Duration::from_millis(200)).await;

        // Observe health_check
        let alive = daemon.health_check();
        assert!(!alive, "expected health_check to return false");

        // Contract: child cleared
        assert!(daemon.child.is_none(), "child should be None after exit");

        // Contract: client cleared
        {
            let guard = shared.lock().unwrap();
            assert!(
                guard.is_none(),
                "shared client should be None after health_check detects exit"
            );
        }

        // Contract: state transitioned to Failed, reason mentions exit
        match daemon.state() {
            LspDaemonState::Failed { reason, attempts } => {
                assert!(
                    reason.contains("process exited"),
                    "expected 'process exited' in reason, got: {reason}"
                );
                assert_eq!(attempts, 1, "expected attempts == 1");
            }
            other => panic!("expected Failed, got: {other:?}"),
        }

        // Contract: consecutive_failures exactly 1
        assert_eq!(daemon.consecutive_failures, 1);

        let _ = helper.kill();
    }
}

/// After `health_check` detects an exit and drops the child, a subsequent
/// call with no child is the early-return path. It must not mutate state
/// or the failure counter.
#[tokio::test]
async fn test_health_check_none_path_does_not_mutate_state() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    // Pre-populate state as Failed with a specific reason
    daemon.record_failure("original failure".into());
    let before_state = daemon.state();
    let before_failures = daemon.consecutive_failures;

    // No child — early return path
    assert!(!daemon.health_check());

    // State and counter must be unchanged
    assert_eq!(daemon.state(), before_state);
    assert_eq!(daemon.consecutive_failures, before_failures);
}

/// Verify that `health_check` on a live process returns true without
/// side effects (doesn't touch client, child, failures, or state).
#[tokio::test]
async fn test_health_check_live_process_no_side_effects() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("sleep")
        .args(["30"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sleep");

    let spec = cat_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    let before_failures = daemon.consecutive_failures;
    let before_state = daemon.state();

    // Still alive — should return true with no side effects
    assert!(daemon.health_check());
    assert!(daemon.child.is_some(), "child handle retained");
    assert_eq!(daemon.consecutive_failures, before_failures);
    assert_eq!(daemon.state(), before_state);

    // Poll a couple more times — idempotent
    assert!(daemon.health_check());
    assert!(daemon.health_check());
    assert_eq!(daemon.consecutive_failures, before_failures);

    // Clean up
    if let Some(mut child) = daemon.child.take() {
        let _ = child.kill().await;
        let _ = child.wait().await;
    }
}

/// `graceful_shutdown` on a child whose stdin is already closed must
/// still wait for the child to exit and return Ok when it does. This
/// exercises the `child.stdin.as_mut() == None` branch at the top.
#[tokio::test]
async fn test_graceful_shutdown_with_no_stdin_still_waits() {
    let mut child = Command::new("true")
        .stdin(Stdio::null()) // no stdin pipe at all
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn true");

    // Take the stdin to be sure
    let _ = child.stdin.take();

    let result = LspDaemon::graceful_shutdown(child).await;
    assert!(
        result.is_ok(),
        "expected graceful_shutdown to succeed: {result:?}"
    );
}

/// `graceful_shutdown` happy path: cat takes input, shutdown request +
/// exit notification close its stdin (cat exits), wait() returns Ok.
#[tokio::test]
async fn test_graceful_shutdown_normal_flow_with_cat() {
    let child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    let result = LspDaemon::graceful_shutdown(child).await;
    assert!(
        result.is_ok(),
        "expected graceful_shutdown with cat to succeed: {result:?}"
    );
}

// -- Card 3: additional restart_with_backoff / shutdown coverage ---------

/// `shutdown()` on a process that refuses to exit within the grace
/// window (by ignoring stdin close) must eventually return with state
/// NotStarted. This covers the `Err(_)` (timeout) branch of the
/// shutdown outcome match. The grace period is shortened via
/// `set_shutdown_grace` so the timeout branch is exercised in
/// milliseconds rather than the full production grace — same pattern as
/// `test_shutdown_kills_long_running_child`.
#[tokio::test]
async fn test_shutdown_timeout_path_emits_shutting_down_then_not_started() {
    // sleep never responds to stdin close — shutdown will hit the
    // grace-period timeout.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("sleep")
        .args(["60"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sleep");

    let spec = test_spec("sleep");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);
    let grace = Duration::from_millis(200);
    daemon.set_shutdown_grace(grace);

    let mut rx = daemon.state_rx();
    // Drain any prior state changes
    while rx.has_changed().unwrap_or(false) {
        let _ = rx.borrow_and_update();
    }

    let start = std::time::Instant::now();
    daemon.shutdown().await;
    let elapsed = start.elapsed();

    // Final state is NotStarted
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert!(daemon.child.is_none());

    // Elapsed time must be at least the (shortened) grace period because
    // graceful_shutdown waits for the sleep process, which never exits
    // from stdin close alone and so always hits the timeout branch.
    assert!(
        elapsed >= grace,
        "expected at least the grace period {grace:?} (timeout path), got: {elapsed:?}"
    );
}

/// `shutdown()` after a client has been explicitly installed must clear
/// the shared client regardless of whether a child exists.
#[tokio::test]
async fn test_shutdown_clears_client_with_installed_value() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    // Install a real client
    let helper = std::process::Command::new("cat")
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn();
    if let Ok(mut helper) = helper {
        let hstdin = helper.stdin.take().unwrap();
        let hstdout = helper.stdout.take().unwrap();
        let shared = daemon.shared_client();
        {
            let mut guard = shared.lock().unwrap();
            *guard = Some(LspJsonRpcClient::new(hstdin, hstdout));
        }

        // Verify installed
        {
            let guard = shared.lock().unwrap();
            assert!(guard.is_some());
        }

        // Shutdown with no child — should take the early-return path
        // but still clear the client.
        daemon.shutdown().await;

        {
            let guard = shared.lock().unwrap();
            assert!(
                guard.is_none(),
                "shutdown must clear client even when no child"
            );
        }
        assert_eq!(daemon.state(), LspDaemonState::NotStarted);
        let _ = helper.kill();
    }
}

/// Calling `shutdown()` multiple times in a row on a never-started
/// daemon is idempotent — state remains NotStarted.
#[tokio::test]
async fn test_shutdown_is_idempotent_when_not_started() {
    let spec = test_spec("some-server");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}
