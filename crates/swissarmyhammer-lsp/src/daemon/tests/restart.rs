//! Tests for the restart policy of [`LspDaemon`]: the backoff before a
//! restart, the consecutive failure limit, and the health check that detects
//! an exit or clears the failure count.

use super::*;

/// The failure count of the restart tests that start below the limit and
/// must show that a health check adds one failure.
const PRIOR_FAILURES: u32 = 2;

/// The failure count that the healthy-uptime tests start from.
const FAILURES_BEFORE_HEALTHY_UPTIME: u32 = 3;

// -- health_check + restart_with_backoff edge case tests ------------------

#[tokio::test]
async fn test_health_check_detects_naturally_exiting_process() {
    // `cat` exits by itself when its stdin closes. Verify health_check
    // detects the exit and transitions to Failed.
    let mut daemon = daemon_with_child(spawn_echo());

    // Initially alive: `cat` waits for input until its stdin closes.
    assert!(daemon.health_check());

    // Close stdin, so `cat` exits naturally, and wait for the exit.
    drop(
        daemon
            .child
            .as_mut()
            .expect("the daemon holds a child")
            .stdin
            .take(),
    );
    wait_for_child_exit(&mut daemon).await;

    // health_check should detect the exit
    assert!(!daemon.health_check());
    match daemon.state() {
        LspDaemonState::Failed { reason, .. } => assert!(
            reason.contains("process exited"),
            "expected 'process exited' in reason, got: {reason}"
        ),
        other => panic!("expected Failed after natural exit, got: {other:?}"),
    }
}

#[tokio::test]
async fn test_health_check_clears_client_on_exit() {
    // When a process exits, health_check should clear the shared client.
    let mut daemon = daemon_with_child(spawn_exiting());
    let _client = install_cat_client(&daemon);
    assert!(daemon.client().is_some(), "the client is installed");

    wait_for_child_exit(&mut daemon).await;

    // health_check should detect exit and clear client
    assert!(!daemon.health_check());
    assert!(daemon.child.is_none());
    assert!(daemon.client().is_none());
}

#[tokio::test]
async fn test_restart_with_backoff_sleeps_before_restart() {
    // Verify that restart_with_backoff introduces a delay before
    // attempting restart. We use a nonexistent binary so start() fails
    // fast, and measure wall time to confirm the backoff delay.
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = 0; // first failure: 1s backoff

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    let backoff = backoff_duration(0);
    assert!(
        elapsed >= backoff,
        "expected at least the {backoff:?} backoff, got: {elapsed:?}"
    );
}

#[tokio::test]
async fn test_restart_with_backoff_gives_up_at_max_failures() {
    // When consecutive_failures >= MAX_CONSECUTIVE_FAILURES, restart
    // should return an error immediately without sleeping.
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert_handshake_failed(result, RESTART_LIMIT_TEXT);
    // Should return immediately, not sleep
    assert!(
        elapsed < Duration::from_millis(100),
        "expected immediate return, got: {elapsed:?}"
    );
}

/// Start `daemon`, wait until its server exits, and run one health check
/// that sees the exit.
async fn start_and_wait_for_crash(daemon: &mut LspDaemon) {
    daemon
        .start()
        .await
        .expect("the fake server answers initialize");
    wait_for_child_exit(daemon).await;
    assert!(!daemon.health_check(), "the health check must see the exit");
}

/// A server that crashes after each good `initialize` must not restart
/// with no limit: a good start does not clear the failure count, so the
/// count reaches the limit and the restart stops.
#[tokio::test]
async fn test_crash_after_each_good_initialize_reaches_the_failure_limit() {
    // The fake server answers `initialize`, reads `initialized`, and then
    // exits: a server that crashes after each good start.
    let (_workspace, mut daemon) = daemon_in_tempdir(handshake_spec(INITIALIZE_REQUEST_ID, ""));

    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        start_and_wait_for_crash(&mut daemon).await;
    }

    assert_eq!(daemon.consecutive_failures, MAX_CONSECUTIVE_FAILURES);
    let result = daemon.restart_with_backoff().await;
    assert_handshake_failed(result, RESTART_LIMIT_TEXT);
    assert!(daemon.child.is_none(), "no server may start at the limit");
}

/// The error at the restart limit is logged one time, not at each call
/// of the health loop.
#[tokio::test]
#[tracing_test::traced_test]
async fn test_restart_at_the_limit_logs_the_error_one_time() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let _ = daemon.restart_with_backoff().await;
    let _ = daemon.restart_with_backoff().await;

    logs_assert(|lines: &[&str]| {
        let count = lines
            .iter()
            .filter(|line| line.contains("ERROR") && line.contains(RESTART_LIMIT_TEXT))
            .count();
        match count {
            1 => Ok(()),
            n => Err(format!("expected one error log at the limit, got {n}")),
        }
    });
}

/// A server that stays up for the minimum healthy uptime has recovered:
/// the health check then clears the failure count.
#[tokio::test]
async fn test_health_check_clears_failures_after_the_minimum_healthy_uptime() {
    let (_workspace, mut daemon) = daemon_in_tempdir(cat_spec());
    daemon.set_min_healthy_uptime(Duration::ZERO);
    daemon
        .start()
        .await
        .expect("cat echo handshake should pass");
    daemon.consecutive_failures = FAILURES_BEFORE_HEALTHY_UPTIME;

    assert!(daemon.health_check(), "cat is still running");

    assert_eq!(daemon.consecutive_failures, 0);
    daemon.shutdown().await;
}

/// A server that is up for less than the minimum healthy uptime keeps
/// its failure count.
#[tokio::test]
async fn test_health_check_keeps_failures_before_the_minimum_healthy_uptime() {
    let (_workspace, mut daemon) = daemon_in_tempdir(cat_spec());
    daemon
        .start()
        .await
        .expect("cat echo handshake should pass");
    daemon.consecutive_failures = FAILURES_BEFORE_HEALTHY_UPTIME;

    assert!(daemon.health_check(), "cat is still running");

    assert_eq!(daemon.consecutive_failures, FAILURES_BEFORE_HEALTHY_UPTIME);
    daemon.shutdown().await;
}

#[tokio::test]
async fn test_health_check_multiple_calls_after_exit() {
    // After a process has exited and been detected by health_check,
    // subsequent calls should also return false (child is now None).
    let mut daemon = daemon_with_child(spawn_exiting());
    wait_for_child_exit(&mut daemon).await;

    // First call detects exit
    assert!(!daemon.health_check());
    assert_eq!(daemon.consecutive_failures, 1);

    // Second call: child is None, returns false but doesn't increment failures
    assert!(!daemon.health_check());
    assert_eq!(daemon.consecutive_failures, 1); // unchanged
}

// -- Card 3: additional restart_with_backoff / shutdown coverage ---------

/// `restart_with_backoff` with [`PRIOR_FAILURES`] failures waits the
/// backoff for that attempt, logs the restart at info level with the
/// attempt number and the delay, and then calls `start()`.
#[tokio::test]
#[tracing_test::traced_test]
async fn test_restart_with_backoff_info_log_path_attempt_2() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = PRIOR_FAILURES;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    let backoff = backoff_duration(PRIOR_FAILURES);
    assert!(
        elapsed >= backoff,
        "expected at least the {backoff:?} backoff, got: {elapsed:?}"
    );
    // start() hit BinaryNotFound, so state is NotFound
    assert_eq!(daemon.state(), LspDaemonState::NotFound);

    let attempt = format!("attempt={}", PRIOR_FAILURES + 1);
    let delay = format!("delay_secs={}", backoff.as_secs());
    logs_assert(|lines: &[&str]| {
        let found = lines.iter().any(|line| {
            line.contains("INFO")
                && line.contains("Restarting LSP server after backoff")
                && line.contains(&attempt)
                && line.contains(&delay)
        });
        if found {
            Ok(())
        } else {
            Err(format!("expected an info log with {attempt} and {delay}"))
        }
    });
}

/// When `restart_with_backoff` is called and the underlying start()
/// succeeds (via a working mock), the restart must return Ok(()) and
/// the daemon transitions to Running.
#[tokio::test]
async fn test_restart_with_backoff_success_path() {
    let (_workspace, mut daemon) = daemon_in_tempdir(serving_handshake_spec());
    // attempt 0: 1s backoff before start
    daemon.consecutive_failures = 0;

    let start_time = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start_time.elapsed();

    assert!(
        result.is_ok(),
        "expected restart_with_backoff to succeed: {result:?}"
    );
    assert!(matches!(daemon.state(), LspDaemonState::Running { .. }));
    // A successful start adds no failure
    assert_eq!(daemon.consecutive_failures, 0);
    // Must have waited at least the backoff
    let backoff = backoff_duration(0);
    assert!(
        elapsed >= backoff,
        "expected at least the {backoff:?} backoff, got: {elapsed:?}"
    );

    daemon.shutdown().await;
}

/// Exercise `restart_with_backoff` at attempt 0 by confirming the error
/// value returned when start() fails matches BinaryNotFound for the
/// bad-binary case.
#[tokio::test]
async fn test_restart_with_backoff_at_attempt_zero_returns_binary_not_found() {
    // attempt 0 → 1s backoff → then start() which hits BinaryNotFound
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = 0;

    let result = daemon.restart_with_backoff().await;
    // The underlying start() call surfaces BinaryNotFound
    assert!(
        matches!(result, Err(LspError::BinaryNotFound { .. })),
        "expected BinaryNotFound from underlying start(), got: {result:?}"
    );
}

/// Verify that `health_check` records a `Failed` state whose `attempts`
/// field equals the current `consecutive_failures`, incrementing as
/// detections accumulate. This documents the link between the counter
/// and the Failed-state payload.
#[tokio::test]
async fn test_health_check_failed_attempts_tracks_consecutive_failures() {
    let mut daemon = daemon_with_child(spawn_exiting());

    // Simulate prior failures
    daemon.consecutive_failures = PRIOR_FAILURES;
    wait_for_child_exit(&mut daemon).await;

    assert!(!daemon.health_check());
    // The health check adds one failure
    let expected = PRIOR_FAILURES + 1;
    assert_eq!(daemon.consecutive_failures, expected);
    match daemon.state() {
        LspDaemonState::Failed { attempts, .. } => {
            assert_eq!(attempts, expected);
        }
        other => panic!("expected Failed with attempts={expected}, got: {other:?}"),
    }
}
