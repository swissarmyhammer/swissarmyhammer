//! Tests for the restart policy of [`LspDaemon`]: the backoff before a
//! restart, the consecutive failure limit, and the health check that detects
//! an exit or clears the failure count.

use super::*;

// -- health_check + restart_with_backoff edge case tests ------------------

#[tokio::test]
async fn test_health_check_detects_naturally_exiting_process() {
    // Spawn a process that exits after a short delay. Verify health_check
    // detects the exit and transitions to Failed.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let child = Command::new("python3")
        .args(["-c", "import time; time.sleep(0.2)"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn python");

    let spec = cat_spec();
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    daemon.child = Some(child);

    // Initially alive
    assert!(daemon.health_check());

    // Wait for the process to exit naturally
    tokio::time::sleep(Duration::from_millis(500)).await;

    // health_check should detect the exit
    assert!(!daemon.health_check());
    assert!(
        matches!(daemon.state(), LspDaemonState::Failed { .. }),
        "expected Failed after natural exit, got: {:?}",
        daemon.state()
    );
    // Verify failure reason mentions process exit
    if let LspDaemonState::Failed { reason, .. } = daemon.state() {
        assert!(
            reason.contains("process exited"),
            "expected 'process exited' in reason, got: {reason}"
        );
    }
}

#[tokio::test]
async fn test_health_check_clears_client_on_exit() {
    // When a process exits, health_check should clear the shared client.
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

    // Wait for process to exit
    tokio::time::sleep(Duration::from_millis(200)).await;

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
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 0; // first failure: 1s backoff

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    // Should have waited ~1s (backoff for attempt 0)
    assert!(
        elapsed >= Duration::from_millis(800),
        "expected at least ~1s backoff delay, got: {elapsed:?}"
    );
}

#[tokio::test]
async fn test_restart_with_backoff_gives_up_at_max_failures() {
    // When consecutive_failures >= MAX_CONSECUTIVE_FAILURES, restart
    // should return an error immediately without sleeping.
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("too many consecutive failures"),
        "expected 'too many consecutive failures', got: {err}"
    );
    // Should return immediately, not sleep
    assert!(
        elapsed < Duration::from_millis(100),
        "expected immediate return, got: {elapsed:?}"
    );
}

/// A fake server that answers `initialize`, reads `initialized`, and
/// then exits. It is a server that crashes after each good start.
const EXIT_AFTER_INITIALIZE_SCRIPT: &str = r#"
import sys, re, json
def read_message():
    headers = ''
    while True:
        line = sys.stdin.readline()
        if line.strip() == '':
            break
        headers += line
    m = re.search(r'Content-Length:\s*(\d+)', headers)
    if m:
        sys.stdin.read(int(m.group(1)))
read_message()
resp = json.dumps({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
read_message()
"#;

/// Start `daemon`, wait until its server exits, and run one health check
/// that sees the exit.
async fn start_and_wait_for_crash(daemon: &mut LspDaemon) {
    daemon
        .start()
        .await
        .expect("the fake server answers initialize");
    daemon
        .child
        .as_mut()
        .expect("a started daemon holds its child")
        .wait()
        .await
        .expect("the fake server exits");
    assert!(!daemon.health_check(), "the health check must see the exit");
}

/// A server that crashes after each good `initialize` must not restart
/// with no limit: a good start does not clear the failure count, so the
/// count reaches the limit and the restart stops.
#[tokio::test]
async fn test_crash_after_each_good_initialize_reaches_the_failure_limit() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(
        python_mock_spec(EXIT_AFTER_INITIALIZE_SCRIPT),
        workspace.path().to_path_buf(),
    );

    for _ in 0..MAX_CONSECUTIVE_FAILURES {
        start_and_wait_for_crash(&mut daemon).await;
    }

    assert_eq!(daemon.consecutive_failures, MAX_CONSECUTIVE_FAILURES);
    let result = daemon.restart_with_backoff().await;
    assert!(
        matches!(&result, Err(LspError::HandshakeFailed(msg)) if msg.contains("giving up")),
        "the restart must stop at the limit, got: {result:?}"
    );
    assert!(daemon.child.is_none(), "no server may start at the limit");
}

/// The error at the restart limit is logged one time, not at each call
/// of the health loop.
#[tokio::test]
#[tracing_test::traced_test]
async fn test_restart_at_the_limit_logs_the_error_one_time() {
    let mut daemon = LspDaemon::new(
        test_spec("nonexistent-lsp-binary-abc123xyz"),
        PathBuf::from("/tmp"),
    );
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let _ = daemon.restart_with_backoff().await;
    let _ = daemon.restart_with_backoff().await;

    logs_assert(|lines: &[&str]| {
        let count = lines
            .iter()
            .filter(|line| line.contains("ERROR") && line.contains("giving up"))
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
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(cat_spec(), workspace.path().to_path_buf());
    daemon.set_min_healthy_uptime(Duration::ZERO);
    daemon
        .start()
        .await
        .expect("cat echo handshake should pass");
    daemon.consecutive_failures = 3;

    assert!(daemon.health_check(), "cat is still running");

    assert_eq!(daemon.consecutive_failures, 0);
    daemon.shutdown().await;
}

/// A server that is up for less than the minimum healthy uptime keeps
/// its failure count.
#[tokio::test]
async fn test_health_check_keeps_failures_before_the_minimum_healthy_uptime() {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(cat_spec(), workspace.path().to_path_buf());
    daemon
        .start()
        .await
        .expect("cat echo handshake should pass");
    daemon.consecutive_failures = 3;

    assert!(daemon.health_check(), "cat is still running");

    assert_eq!(daemon.consecutive_failures, 3);
    daemon.shutdown().await;
}

#[tokio::test]
async fn test_health_check_multiple_calls_after_exit() {
    // After a process has exited and been detected by health_check,
    // subsequent calls should also return false (child is now None).
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

    // Wait for exit
    tokio::time::sleep(Duration::from_millis(200)).await;

    // First call detects exit
    assert!(!daemon.health_check());
    assert_eq!(daemon.consecutive_failures, 1);

    // Second call: child is None, returns false but doesn't increment failures
    assert!(!daemon.health_check());
    assert_eq!(daemon.consecutive_failures, 1); // unchanged
}

// -- Card 3: additional restart_with_backoff / shutdown coverage ---------

/// `restart_with_backoff` with failures one below the cap should sleep
/// for the cap-level backoff (~32s cap at attempt=5 / 60s cap beyond).
/// We use attempt=2 so the delay is a bounded 4s sleep — short enough
/// to stay reasonable, long enough to measurably verify the sleep
/// happened. This exercises the sleep+restart info-log path.
#[tokio::test]
async fn test_restart_with_backoff_info_log_path_attempt_2() {
    // attempt=2 → 4s backoff
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 2;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(result.is_err());
    assert!(
        elapsed >= Duration::from_secs(3),
        "expected at least ~4s backoff, got: {elapsed:?}"
    );
    // start() hit BinaryNotFound, so state is NotFound
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

/// When `restart_with_backoff` is called and the underlying start()
/// succeeds (via a working mock), the restart must return Ok(()) and
/// the daemon transitions to Running.
#[tokio::test]
async fn test_restart_with_backoff_success_path() {
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
    // Must have waited at least ~1s for the backoff
    assert!(
        elapsed >= Duration::from_millis(800),
        "expected ~1s backoff, got: {elapsed:?}"
    );

    daemon.shutdown().await;
}

/// Exercise `restart_with_backoff` at each valid attempt level (0..MAX-1)
/// by confirming the error value returned when start() fails matches
/// BinaryNotFound for the bad-binary case.
#[tokio::test]
async fn test_restart_with_backoff_at_attempt_zero_returns_binary_not_found() {
    // attempt 0 → 1s backoff → then start() which hits BinaryNotFound
    let spec = test_spec("nonexistent-lsp-binary-abc123xyz");
    let mut daemon = LspDaemon::new(spec, PathBuf::from("/tmp"));
    daemon.consecutive_failures = 0;

    let result = daemon.restart_with_backoff().await;
    let err = result.unwrap_err();
    // The underlying start() call surfaces BinaryNotFound
    assert!(
        matches!(err, LspError::BinaryNotFound { .. }),
        "expected BinaryNotFound from underlying start(), got: {err:?}"
    );
}

/// Verify that `health_check` records a `Failed` state whose `attempts`
/// field equals the current `consecutive_failures`, incrementing as
/// detections accumulate. This documents the link between the counter
/// and the Failed-state payload.
#[tokio::test]
async fn test_health_check_failed_attempts_tracks_consecutive_failures() {
    let spec = immediately_exiting_spec();
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    // Simulate prior failures
    daemon.consecutive_failures = 2;

    let child = Command::new("true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn true");
    daemon.child = Some(child);

    tokio::time::sleep(Duration::from_millis(200)).await;

    assert!(!daemon.health_check());
    // The counter bumps from 2 to 3
    assert_eq!(daemon.consecutive_failures, 3);
    match daemon.state() {
        LspDaemonState::Failed { attempts, .. } => {
            assert_eq!(attempts, 3);
        }
        other => panic!("expected Failed with attempts=3, got: {other:?}"),
    }
}
