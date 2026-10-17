//! Unit tests for [`LspDaemon`](super::LspDaemon).
//!
//! The tests are split by topic so that each file stays small:
//! - this file: the shared test helpers, state, lifecycle, health check and
//!   shutdown tests
//! - [`jsonrpc`]: the JSON-RPC framing helpers
//! - [`handshake`]: the `initialize` handshake and stderr capture
//! - [`restart`]: the restart backoff and the failure limit

use super::*;

mod handshake;
mod jsonrpc;
mod restart;

// -- shared test constants ------------------------------------------------

/// The health check interval of each test spec. No test runs the health
/// loop, so the value only has to be valid.
const TEST_HEALTH_CHECK_INTERVAL_SECS: u64 = 60;

/// The startup timeout of a spec whose server answers the handshake.
const DEFAULT_TEST_STARTUP_TIMEOUT_SECS: u64 = 5;

/// The startup timeout of a spec whose test wants a fast handshake result.
const SHORT_STARTUP_TIMEOUT_SECS: u64 = 1;

/// A command that is guaranteed not to exist on `PATH`.
const MISSING_BINARY: &str = "nonexistent-lsp-binary-abc123xyz";

/// A command name for a daemon that the test never starts.
const IDLE_SERVER: &str = "some-server";

/// The workspace root of a daemon whose test does not use the workspace.
const UNUSED_WORKSPACE_ROOT: &str = "/tmp";

/// A program that echoes stdin to stdout and exits at stdin EOF.
const ECHO_PROGRAM: &str = "cat";

/// A program that exits at once.
const EXITING_PROGRAM: &str = "true";

/// A program that keeps its pipes open and never writes (with
/// [`LONG_SLEEP_SECS`]).
const SILENT_PROGRAM: &str = "sleep";

/// The interpreter of the mock LSP server scripts.
const PYTHON_PROGRAM: &str = "python3";

/// The `sleep` argument for a child that must outlive the test.
const LONG_SLEEP_SECS: &str = "60";

/// The shutdown grace of a test that drives the shutdown timeout branch.
const SHORT_SHUTDOWN_GRACE: Duration = Duration::from_millis(200);

/// The longest time a test waits for a signal that must come. The wait is a
/// fail-safe: a broken path fails the test instead of hanging it.
const SIGNAL_TIMEOUT: Duration = Duration::from_secs(10);

/// The JSON-RPC id that the daemon gives to its `initialize` request.
const INITIALIZE_REQUEST_ID: u64 = 1;

/// The Python statement that keeps a mock server alive until the daemon
/// closes its stdin (on shutdown or when a new start drops the client).
const SERVE_UNTIL_STDIN_EOF: &str = "wait_for_stdin_eof()";

/// The text of the handshake error when the server closes stdout before it
/// answers `initialize`.
const EOF_BEFORE_ANSWER_TEXT: &str = "unexpected EOF reading headers";

/// The text of the error (and of its log) when a restart stops at the
/// consecutive failure limit.
const RESTART_LIMIT_TEXT: &str = "too many consecutive failures";

/// The Python prelude of each mock LSP server script.
///
/// - `read_message()` reads one framed message from stdin and returns its
///   body (an empty string at EOF).
/// - `write_raw(text)` writes `text` to stdout and flushes it.
/// - `send(message)` writes one framed JSON message to stdout.
/// - `answer_initialize(request_id)` reads the `initialize` request, answers
///   it with `request_id`, and returns the body of the `initialized`
///   notification.
/// - `log(text)` writes one line to stderr and flushes it.
/// - `close_stdout()` closes the stdout file descriptor, so the daemon
///   reads EOF while the mock stays alive. (`sys.stdout.close()` does not
///   close the descriptor of a standard stream.)
/// - `wait_for_stdin_eof()` blocks until the daemon closes stdin.
const MOCK_PRELUDE: &str = r#"
import os, sys, re, json
def read_message():
    headers = ''
    while True:
        line = sys.stdin.readline()
        if line.strip() == '':
            break
        headers += line
    m = re.search(r'Content-Length:\s*(\d+)', headers)
    return sys.stdin.read(int(m.group(1))) if m else ''
def write_raw(text):
    sys.stdout.write(text)
    sys.stdout.flush()
def send(message):
    body = json.dumps(message)
    write_raw(f'Content-Length: {len(body)}\r\n\r\n{body}')
def answer_initialize(request_id):
    read_message()
    send({"jsonrpc": "2.0", "id": request_id, "result": {"capabilities": {}}})
    return read_message()
def log(text):
    sys.stderr.write(text + '\n')
    sys.stderr.flush()
def close_stdout():
    sys.stdout.flush()
    os.close(sys.stdout.fileno())
def wait_for_stdin_eof():
    sys.stdin.read()
"#;

// -- shared spec helpers --------------------------------------------------

/// Build an `OwnedLspServerSpec` that runs `command` with `args`.
fn spec_with(command: &str, args: &[&str], startup_timeout_secs: u64) -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: command.to_string(),
        args: args.iter().map(|arg| arg.to_string()).collect(),
        language_ids: vec!["test".to_string()],
        file_extensions: vec!["txt".to_string()],
        startup_timeout_secs,
        health_check_interval_secs: TEST_HEALTH_CHECK_INTERVAL_SECS,
        install_hint: format!("install {command}"),
        icon: None,
    }
}

/// Build a minimal `OwnedLspServerSpec` for `command` with no arguments.
fn test_spec(command: &str) -> OwnedLspServerSpec {
    spec_with(command, &[], DEFAULT_TEST_STARTUP_TIMEOUT_SECS)
}

/// Build a spec that spawns `cat`. `cat` echoes the `initialize` request,
/// which is valid framing, so the handshake passes.
fn cat_spec() -> OwnedLspServerSpec {
    spec_with(ECHO_PROGRAM, &[], SHORT_STARTUP_TIMEOUT_SECS)
}

/// Build a spec whose server keeps its pipes open and never writes, so the
/// handshake always hits the startup timeout.
fn silent_server_spec() -> OwnedLspServerSpec {
    spec_with(
        SILENT_PROGRAM,
        &[LONG_SLEEP_SECS],
        SHORT_STARTUP_TIMEOUT_SECS,
    )
}

/// Join [`MOCK_PRELUDE`] and the Python `body` into one mock server script.
fn mock_script(body: &str) -> String {
    format!("{MOCK_PRELUDE}{body}\n")
}

/// Build a spec that runs the Python `body` (after [`MOCK_PRELUDE`]) as the
/// LSP server.
fn mock_spec(body: &str) -> OwnedLspServerSpec {
    let script = mock_script(body);
    spec_with(
        PYTHON_PROGRAM,
        &["-c", &script],
        DEFAULT_TEST_STARTUP_TIMEOUT_SECS,
    )
}

/// The Python body of a mock that answers `initialize` with `response_id`,
/// reads `initialized` into the variable `initialized`, and then runs
/// `after`.
fn handshake_body(response_id: u64, after: &str) -> String {
    format!("initialized = answer_initialize({response_id})\n{after}")
}

/// Build a spec whose mock answers the handshake with `response_id` and
/// then runs `after`.
fn handshake_spec(response_id: u64, after: &str) -> OwnedLspServerSpec {
    mock_spec(&handshake_body(response_id, after))
}

/// Build a spec whose mock completes the handshake and then serves until
/// the daemon closes its stdin.
fn serving_handshake_spec() -> OwnedLspServerSpec {
    handshake_spec(INITIALIZE_REQUEST_ID, SERVE_UNTIL_STDIN_EOF)
}

/// Build a spec whose mock reads the `initialize` request and exits without
/// an answer, so the handshake reads EOF.
fn exits_after_request_spec() -> OwnedLspServerSpec {
    mock_spec("read_message()")
}

// -- shared daemon and process helpers ------------------------------------

/// Build a daemon for `spec` whose test does not use the workspace.
fn daemon_for(spec: OwnedLspServerSpec) -> LspDaemon {
    LspDaemon::new(spec, PathBuf::from(UNUSED_WORKSPACE_ROOT))
}

/// Build a daemon for `spec` in a new temporary workspace. The test must
/// keep the returned directory alive while the daemon runs.
fn daemon_in_tempdir(spec: OwnedLspServerSpec) -> (tempfile::TempDir, LspDaemon) {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let daemon = LspDaemon::new(spec, workspace.path().to_path_buf());
    (workspace, daemon)
}

/// Spawn `program` with piped stdio. The child is killed when its handle
/// drops.
fn spawn_piped(program: &str, args: &[&str]) -> Child {
    Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {program}: {e}"))
}

/// Spawn [`ECHO_PROGRAM`] with piped stdio.
fn spawn_echo() -> Child {
    spawn_piped(ECHO_PROGRAM, &[])
}

/// Spawn [`EXITING_PROGRAM`] with piped stdio.
fn spawn_exiting() -> Child {
    spawn_piped(EXITING_PROGRAM, &[])
}

/// Spawn [`SILENT_PROGRAM`] with piped stdio. The child outlives the test.
fn spawn_silent() -> Child {
    spawn_piped(SILENT_PROGRAM, &[LONG_SLEEP_SECS])
}

/// Build a daemon that holds `child`, with no handshake.
fn daemon_with_child(child: Child) -> LspDaemon {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));
    daemon.child = Some(child);
    daemon
}

/// Wait until the child of `daemon` exits. A later `try_wait` then reports
/// the exit, so the next health check sees it.
async fn wait_for_child_exit(daemon: &mut LspDaemon) {
    daemon
        .child
        .as_mut()
        .expect("the daemon holds a child")
        .wait()
        .await
        .expect("wait for the child to exit");
}

/// Kill `child` and reap it, so no process or pipe of the child stays
/// after the test.
async fn kill_and_reap_child(child: &mut Child) {
    child.kill().await.expect("kill the child");
}

/// Kill the child of `daemon` and reap it. The child handle stays in the
/// daemon, so the next health check sees the exit.
async fn kill_and_reap(daemon: &mut LspDaemon) {
    kill_and_reap_child(daemon.child.as_mut().expect("the daemon holds a child")).await;
}

/// The program that reports the status of a process.
const PS_PROGRAM: &str = "ps";

/// Return the pid of the child of `daemon`.
#[track_caller]
fn child_pid(daemon: &LspDaemon) -> u32 {
    daemon
        .child
        .as_ref()
        .and_then(Child::id)
        .expect("the daemon holds a live child")
}

/// Return true when the process table holds `pid`. A process that exited
/// but that nothing reaped (a zombie) is still in the table.
fn process_is_in_table(pid: u32) -> bool {
    !process_state(pid).is_empty()
}

/// Return the `ps` state of `pid`, or an empty string when the process table
/// does not hold `pid`.
fn process_state(pid: u32) -> String {
    let output = std::process::Command::new(PS_PROGRAM)
        .args(["-o", "stat=", "-p", &pid.to_string()])
        .output()
        .expect("run ps");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

/// A `cat` process whose pipes back a client that a test put into a
/// daemon's shared client slot. Drop kills and reaps the process.
struct InstalledClient {
    helper: std::process::Child,
}

impl Drop for InstalledClient {
    fn drop(&mut self) {
        // The daemon may already have dropped the client, which closes the
        // pipes and makes `cat` exit, so a kill error is expected here.
        let _ = self.helper.kill();
        let _ = self.helper.wait();
    }
}

/// Put a live client, backed by a separate `program` process run with
/// `args`, into the shared client slot of `daemon`.
fn install_client(daemon: &LspDaemon, program: &str, args: &[&str]) -> InstalledClient {
    let mut helper = std::process::Command::new(program)
        .args(args)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap_or_else(|e| panic!("spawn {program} helper: {e}"));
    let stdin = helper.stdin.take().expect("helper stdin");
    let stdout = helper.stdout.take().expect("helper stdout");
    *daemon.shared_client().lock().unwrap() = Some(LspJsonRpcClient::new(stdin, stdout));
    InstalledClient { helper }
}

/// Put a live client, backed by a separate `cat` process, into the shared
/// client slot of `daemon`.
fn install_cat_client(daemon: &LspDaemon) -> InstalledClient {
    install_client(daemon, ECHO_PROGRAM, &[])
}

/// A program that writes its format argument to stdout and exits.
const PRINTF_PROGRAM: &str = "printf";

/// A `printf` format that writes a header block whose Content-Length is too
/// large for a reader to trust.
const UNTRUSTED_HEADER_FORMAT: &str = "Content-Length: 18446744073709551615\\r\\n\\r\\n";

/// Put a client into the shared client slot of `daemon` whose server sent a
/// header block that is not trusted, and read from it one time. The client is
/// then out of step.
fn install_out_of_step_client(daemon: &LspDaemon) -> InstalledClient {
    let installed = install_client(daemon, PRINTF_PROGRAM, &[UNTRUSTED_HEADER_FORMAT]);
    let mut guard = daemon.client().expect("the client is installed");
    let client = guard.as_mut().expect("the slot holds the client");
    let result = client.read_message();
    assert!(
        matches!(result, Err(LspError::OutOfStep(_))),
        "the read must put the client out of step, got: {result:?}"
    );
    drop(guard);
    installed
}

/// Wait until `pid` is no longer a running process: it is gone from the
/// process table, or it is a zombie that nothing reaped yet.
async fn wait_until_not_running(pid: u32) {
    let deadline = Instant::now() + SIGNAL_TIMEOUT;
    while process_is_running(pid) {
        assert!(Instant::now() < deadline, "process {pid} is still running");
        tokio::time::sleep(Duration::from_millis(POLL_INTERVAL_MS)).await;
    }
}

/// The time between two checks of a process state, in milliseconds.
const POLL_INTERVAL_MS: u64 = 20;

/// The `ps` state letter of a zombie process.
const ZOMBIE_STATE: &str = "Z";

/// Return true when `pid` is in the process table and is not a zombie.
fn process_is_running(pid: u32) -> bool {
    let state = process_state(pid);
    !state.is_empty() && !state.starts_with(ZOMBIE_STATE)
}

/// Spawn a task that returns the first state that `daemon` sends after this
/// call.
fn spawn_next_state_observer(daemon: &LspDaemon) -> tokio::task::JoinHandle<LspDaemonState> {
    let mut rx = daemon.state_rx();
    drop(rx.borrow_and_update());
    tokio::spawn(async move {
        rx.changed()
            .await
            .expect("the daemon holds the state sender");
        rx.borrow_and_update().clone()
    })
}

/// Assert that `result` is a `HandshakeFailed` error whose message contains
/// `expected`, and return the message.
#[track_caller]
fn assert_handshake_failed(result: Result<(), LspError>, expected: &str) -> String {
    match result {
        Err(LspError::HandshakeFailed(msg)) => {
            assert!(
                msg.contains(expected),
                "expected {expected:?} in the handshake error, got: {msg}"
            );
            msg
        }
        other => panic!("expected HandshakeFailed, got: {other:?}"),
    }
}

// -- backoff and state tests ----------------------------------------------

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
        health_check_interval_secs: TEST_HEALTH_CHECK_INTERVAL_SECS,
        install_hint: "Install rust-analyzer: rustup component add rust-analyzer".to_string(),
        icon: None,
    };
    let daemon = daemon_for(spec);
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert_eq!(daemon.command(), "rust-analyzer");
}

// -- start() tests -------------------------------------------------------

#[tokio::test]
async fn test_start_binary_not_found() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));

    let result = daemon.start().await;
    assert!(
        matches!(result, Err(LspError::BinaryNotFound { .. })),
        "expected BinaryNotFound, got: {result:?}"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

#[tokio::test]
async fn test_start_binary_not_found_preserves_hint() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));

    let result = daemon.start().await;
    match result {
        Err(LspError::BinaryNotFound {
            command,
            install_hint,
        }) => {
            assert_eq!(command, MISSING_BINARY);
            assert!(install_hint.contains(MISSING_BINARY));
        }
        other => panic!("expected BinaryNotFound, got: {other:?}"),
    }
}

// -- client() tests ------------------------------------------------------

#[test]
fn test_client_returns_none_when_not_started() {
    let daemon = daemon_for(test_spec(IDLE_SERVER));
    assert!(daemon.client().is_none());
}

#[test]
fn test_shared_client_returns_arc() {
    let daemon = daemon_for(test_spec(IDLE_SERVER));
    let shared = daemon.shared_client();
    // The shared client should be lockable and contain None
    let guard = shared.lock().unwrap();
    assert!(guard.is_none());
}

/// Poison the shared client mutex of `daemon` with a panic inside a lock.
fn poison_shared_client(daemon: &LspDaemon) {
    let shared = daemon.shared_client();
    let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = shared.lock().unwrap();
        panic!("intentional panic to poison mutex");
    }));
}

#[test]
fn test_client_recovers_from_poisoned_mutex() {
    let daemon = daemon_for(test_spec(IDLE_SERVER));
    poison_shared_client(&daemon);

    // client() should still work via the poison-recovery path
    // The inner Option is still None, so it returns None
    assert!(daemon.client().is_none());
}

#[test]
fn test_client_recovers_from_poisoned_mutex_with_some_value() {
    // This test covers the poisoned mutex path where the inner Option IS Some.
    let daemon = daemon_for(test_spec(IDLE_SERVER));
    let _client = install_cat_client(&daemon);
    poison_shared_client(&daemon);

    // client() should recover from the poisoned mutex.
    // The inner Option is Some, so it should return Some.
    assert!(
        daemon.client().is_some(),
        "expected Some from poisoned mutex with client present"
    );
}

// -- health_check() tests ------------------------------------------------

#[test]
fn test_health_check_returns_false_when_no_child() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));
    assert!(!daemon.health_check());
}

// -- shutdown() tests ----------------------------------------------------

#[tokio::test]
async fn test_shutdown_when_not_started() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));

    // Shutdown should be a no-op that transitions to NotStarted
    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

// -- restart_with_backoff() tests ----------------------------------------

#[tokio::test]
async fn test_restart_with_backoff_exhausted() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));

    // Simulate MAX_CONSECUTIVE_FAILURES failures
    daemon.consecutive_failures = MAX_CONSECUTIVE_FAILURES;

    let result = daemon.restart_with_backoff().await;
    assert_handshake_failed(result, RESTART_LIMIT_TEXT);
}

/// A restart that finds no binary reports `NotFound` and adds no failure.
#[tokio::test]
async fn test_restart_with_backoff_reports_not_found_for_a_missing_binary() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));

    let result = daemon.restart_with_backoff().await;
    assert!(
        matches!(result, Err(LspError::BinaryNotFound { .. })),
        "expected BinaryNotFound, got: {result:?}"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
    assert_eq!(daemon.consecutive_failures, 0);
}

/// The restart waits the backoff for the current failure count, then
/// reports the missing binary. `BinaryNotFound` adds no failure, so the
/// count stays the same.
#[tokio::test]
async fn test_restart_with_backoff_waits_then_reports_binary_not_found() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    // Set to 1 failure (below MAX) so restart_with_backoff proceeds
    daemon.consecutive_failures = 1;

    let start = std::time::Instant::now();
    let result = daemon.restart_with_backoff().await;
    let elapsed = start.elapsed();

    assert!(
        matches!(result, Err(LspError::BinaryNotFound { .. })),
        "expected BinaryNotFound, got: {result:?}"
    );
    let backoff = backoff_duration(1);
    assert!(
        elapsed >= backoff,
        "expected at least the {backoff:?} backoff, got: {elapsed:?}"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
    assert_eq!(daemon.consecutive_failures, 1);
}

// -- force_restart() tests -----------------------------------------------

#[tokio::test]
async fn test_force_restart_resets_failures() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = 3;

    // force_restart resets failures to 0 then calls start()
    let result = daemon.force_restart().await;
    assert!(result.is_err()); // binary not found
                              // consecutive_failures was reset to 0 before start(), but start() didn't
                              // call record_failure because BinaryNotFound doesn't go through record_failure
    assert_eq!(daemon.consecutive_failures, 0);
}

#[tokio::test]
async fn test_force_restart_after_failure() {
    // Simulate a daemon that failed, then force_restart
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    daemon.consecutive_failures = 4;
    daemon.record_failure("previous crash".into());

    // force_restart resets consecutive_failures to 0 then calls start()
    let result = daemon.force_restart().await;
    assert!(result.is_err()); // binary still not found
                              // But consecutive_failures was reset before the start attempt
    assert_eq!(daemon.consecutive_failures, 0);
}

// -- state_rx() tests ----------------------------------------------------

#[tokio::test]
async fn test_state_rx_observes_transitions() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
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
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));
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
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));

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
    let daemon = daemon_for(test_spec(IDLE_SERVER));
    let debug = format!("{daemon:?}");
    assert!(debug.contains(IDLE_SERVER));
    assert!(debug.contains("NotStarted"));
}

// -- Lifecycle tests with real processes ---------------------------------
//
// These tests use simple commands (cat, sleep, Python mocks) to exercise the
// daemon lifecycle without requiring a full LSP server implementation.

/// A server that exits after it reads the `initialize` request makes the
/// handshake read EOF, so `start()` fails with `HandshakeFailed` and
/// records one failure.
#[tokio::test]
async fn test_start_fails_when_the_server_exits_before_answering() {
    let (_workspace, mut daemon) = daemon_in_tempdir(exits_after_request_spec());

    let result = daemon.start().await;
    assert_handshake_failed(result, EOF_BEFORE_ANSWER_TEXT);
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
    let (_workspace, mut daemon) = daemon_in_tempdir(cat_spec());

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
    let (workspace, mut daemon) = daemon_in_tempdir(cat_spec());
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
    let (workspace, mut daemon) = daemon_in_tempdir(cat_spec());

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
    let mut daemon = daemon_with_child(spawn_echo());

    // Process should be alive
    assert!(daemon.health_check());

    kill_and_reap(&mut daemon).await;

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
    let mut daemon = daemon_with_child(spawn_echo());

    // Shutting down should clean up the child
    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert!(daemon.child.is_none());
    assert!(daemon.client().is_none());
}

/// `start()` on a failing server leaves `Failed` as the last state that the
/// state receiver reports.
#[tokio::test]
async fn test_state_rx_reports_failed_after_failed_start() {
    let (_workspace, mut daemon) = daemon_in_tempdir(exits_after_request_spec());
    let mut rx = daemon.state_rx();

    assert_eq!(*rx.borrow(), LspDaemonState::NotStarted);

    let _ = daemon.start().await;

    assert!(rx.has_changed().expect("the daemon holds the state sender"));
    let final_state = rx.borrow_and_update().clone();
    assert!(
        matches!(final_state, LspDaemonState::Failed { .. }),
        "expected Failed, got: {final_state:?}"
    );
}

/// `shutdown()` on a daemon with a child sends `ShuttingDown` before the
/// final `NotStarted`. The child never exits, so the state stays
/// `ShuttingDown` for the grace period, and an observer that waits on the
/// next change sees it.
#[tokio::test]
async fn test_shutdown_with_child_transitions_through_shutting_down() {
    let mut daemon = daemon_with_child(spawn_silent());
    daemon.set_shutdown_grace(SHORT_SHUTDOWN_GRACE);
    let observer = spawn_next_state_observer(&daemon);

    daemon.shutdown().await;

    let first_state = observer.await.expect("the observer task completes");
    assert_eq!(first_state, LspDaemonState::ShuttingDown);
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

#[tokio::test]
async fn test_multiple_consecutive_failures_track_correctly() {
    let (_workspace, mut daemon) = daemon_in_tempdir(exits_after_request_spec());

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

    let err = LspError::OutOfStep("length too large".to_string());
    assert!(err.to_string().contains("out of step: length too large"));

    let err = LspError::ProjectDetection("no projects".to_string());
    assert!(err.to_string().contains("no projects"));

    let err = LspError::DaemonNotFound("missing-cmd".to_string());
    assert!(err.to_string().contains("missing-cmd"));
}

// -- start() timeout path ---------------------------------------------------

/// A server that keeps its pipes open and never writes makes the handshake
/// wait out the startup timeout. `start()` then returns `Timeout` with the
/// spec's startup timeout and records the failure.
#[tokio::test]
async fn test_start_times_out_when_the_server_never_answers() {
    let spec = silent_server_spec();
    let startup_timeout = spec.startup_timeout();
    let (_workspace, mut daemon) = daemon_in_tempdir(spec);

    let result = daemon.start().await;
    match result {
        Err(LspError::Timeout(timeout)) => assert_eq!(timeout, startup_timeout),
        other => panic!("expected Timeout, got: {other:?}"),
    }
    match daemon.state() {
        LspDaemonState::Failed { reason, attempts } => {
            assert!(
                reason.contains("timed out"),
                "expected 'timed out' in reason, got: {reason}"
            );
            assert_eq!(attempts, 1);
        }
        other => panic!("expected Failed, got: {other:?}"),
    }
    assert!(daemon.child.is_none(), "a timed-out server is not kept");
}

// -- shutdown with a real running child -----------------------------------

#[tokio::test]
async fn test_shutdown_kills_long_running_child() {
    // `sleep` ignores the closed pipes and never exits, so graceful
    // shutdown always hits the timeout branch. Shorten the grace period so
    // the test exercises that branch (state → NotStarted, child killed and
    // reaped) without waiting out the full production grace.
    let mut daemon = daemon_with_child(spawn_silent());
    daemon.set_shutdown_grace(SHORT_SHUTDOWN_GRACE);
    let pid = child_pid(&daemon);

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
    assert!(daemon.child.is_none());
    assert!(
        !process_is_in_table(pid),
        "shutdown must kill and reap the child after the grace"
    );
}

// -- file_extensions accessor test ----------------------------------------

#[test]
fn test_file_extensions_accessor() {
    let extensions = vec!["rs".to_string(), "toml".to_string(), "lock".to_string()];
    let mut spec = test_spec(IDLE_SERVER);
    spec.file_extensions = extensions.clone();
    let daemon = daemon_for(spec);
    assert_eq!(daemon.file_extensions(), extensions.as_slice());
}

#[test]
fn test_file_extensions_empty() {
    let mut spec = test_spec(IDLE_SERVER);
    spec.file_extensions = vec![];
    let daemon = daemon_for(spec);
    assert!(daemon.file_extensions().is_empty());
}

// -- Card 1: additional start() failure path coverage --------------------

/// A command that is not on `PATH` hits the binary-not-found branch. The
/// test also asserts that `consecutive_failures` is not incremented on
/// `BinaryNotFound`, which is the documented contract for
/// `set_state(NotFound)`.
#[tokio::test]
async fn test_start_binary_not_found_does_not_increment_failures() {
    let mut daemon = daemon_for(test_spec(MISSING_BINARY));
    assert_eq!(daemon.consecutive_failures, 0);

    let _ = daemon.start().await;
    // BinaryNotFound goes through set_state(NotFound), NOT record_failure.
    // The counter must remain at 0.
    assert_eq!(daemon.consecutive_failures, 0);
    assert_eq!(daemon.state(), LspDaemonState::NotFound);
}

/// The stderr line the stderr filter test mock writes before the handshake.
const STDERR_BEFORE_HANDSHAKE: &str = "mock lsp: starting up";

/// The stderr line the stderr filter test mock writes after the handshake.
const STDERR_AFTER_HANDSHAKE: &str = "mock lsp: ready";

/// After a good start, the stderr drain task gives each line that the
/// server writes to stderr to the injected stderr filter, in order: the
/// lines written before the handshake and the lines written after it.
#[tokio::test]
async fn test_start_spawns_stderr_filter_task_on_success() {
    let body = format!(
        "log('{STDERR_BEFORE_HANDSHAKE}')\n{}",
        handshake_body(
            INITIALIZE_REQUEST_ID,
            &format!("log('{STDERR_AFTER_HANDSHAKE}')\n{SERVE_UNTIL_STDIN_EOF}")
        )
    );
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(&body));
    let (line_tx, mut line_rx) = tokio::sync::mpsc::unbounded_channel();
    daemon.set_stderr_filter(Arc::new(move |line: &str| {
        // The receiver lives until the end of the test.
        let _ = line_tx.send(line.to_string());
        false
    }));

    let result = daemon.start().await;
    assert!(
        result.is_ok(),
        "expected mock LSP to handshake successfully: {result:?}"
    );

    for expected in [STDERR_BEFORE_HANDSHAKE, STDERR_AFTER_HANDSHAKE] {
        let line = tokio::time::timeout(SIGNAL_TIMEOUT, line_rx.recv())
            .await
            .expect("the drain task gives the line to the filter")
            .expect("the filter holds the line sender");
        assert_eq!(line, expected);
    }

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

/// Verify that `start()` on a success path takes stdin and stdout from
/// the child process and creates an `LspJsonRpcClient`. This exercises
/// the successful branch of the pipe-conversion `match` block
/// (`(Ok(stdin_fd), Ok(stdout_fd))` arm).
#[tokio::test]
async fn test_start_success_creates_shared_client() {
    let (_workspace, mut daemon) = daemon_in_tempdir(serving_handshake_spec());

    let result = daemon.start().await;
    assert!(result.is_ok(), "expected start to succeed: {result:?}");

    // After a successful start, the shared client should contain Some(_).
    // This confirms the pipe conversion succeeded.
    assert!(
        daemon.shared_client().lock().unwrap().is_some(),
        "expected shared client to be Some after successful start"
    );

    // And the child handle is retained for shutdown.
    assert!(daemon.child.is_some());

    daemon.shutdown().await;
}

/// When `start()` succeeds, a `Running { pid, since_epoch_ms }` state
/// with a sensible timestamp is emitted. This covers the success-path
/// transition in `start()`.
#[tokio::test]
async fn test_start_success_sets_running_state_with_pid_and_timestamp() {
    let (_workspace, mut daemon) = daemon_in_tempdir(serving_handshake_spec());

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

/// Return the pid of a `Running` daemon.
#[track_caller]
fn running_pid(daemon: &LspDaemon) -> u32 {
    match daemon.state() {
        LspDaemonState::Running { pid, .. } => pid,
        other => panic!("expected Running, got: {other:?}"),
    }
}

/// Successful start followed by a second start() documents that the
/// daemon does not refuse concurrent starts — it overwrites the child
/// handle. This covers the path where `start()` is entered while a
/// child already exists: the old child is killed and reaped.
#[tokio::test]
async fn test_start_after_previous_successful_start() {
    let (_workspace, mut daemon) = daemon_in_tempdir(serving_handshake_spec());

    let r1 = daemon.start().await;
    assert!(r1.is_ok(), "first start failed: {r1:?}");
    let first_pid = running_pid(&daemon);

    // Now start again without shutdown — new child overwrites old
    let r2 = daemon.start().await;
    assert!(r2.is_ok(), "second start failed: {r2:?}");
    let second_pid = running_pid(&daemon);
    assert_ne!(first_pid, second_pid, "second start should spawn a new pid");
    assert!(
        !process_is_in_table(first_pid),
        "the second start must kill and reap the first child"
    );

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
    let mut daemon = daemon_with_child(spawn_exiting());

    // Put a live client into the shared client slot so we can verify
    // health_check clears it. The client is wired to a separate `cat`
    // process so it does not touch the child under test.
    let _client = install_cat_client(&daemon);
    wait_for_child_exit(&mut daemon).await;

    // Observe health_check
    let alive = daemon.health_check();
    assert!(!alive, "expected health_check to return false");

    // Contract: child cleared
    assert!(daemon.child.is_none(), "child should be None after exit");

    // Contract: client cleared
    assert!(
        daemon.shared_client().lock().unwrap().is_none(),
        "shared client should be None after health_check detects exit"
    );

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
}

/// After `health_check` detects an exit and drops the child, a subsequent
/// call with no child is the early-return path. It must not mutate state
/// or the failure counter.
#[tokio::test]
async fn test_health_check_none_path_does_not_mutate_state() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));

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
    let mut daemon = daemon_with_child(spawn_silent());

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

    kill_and_reap(&mut daemon).await;
}

/// `graceful_shutdown` on a child whose stdin is already closed must
/// still wait for the child to exit and return Ok when it does. This
/// exercises the `child.stdin.as_mut() == None` branch at the top.
#[tokio::test]
async fn test_graceful_shutdown_with_no_stdin_still_waits() {
    let mut child = spawn_exiting();
    drop(child.stdin.take());

    let result = LspDaemon::graceful_shutdown(&mut child).await;
    assert!(
        result.is_ok(),
        "expected graceful_shutdown to succeed: {result:?}"
    );
}

/// `graceful_shutdown` happy path: cat takes input, shutdown request +
/// exit notification close its stdin (cat exits), wait() returns Ok.
#[tokio::test]
async fn test_graceful_shutdown_normal_flow_with_cat() {
    let mut child = spawn_echo();

    let result = LspDaemon::graceful_shutdown(&mut child).await;
    assert!(
        result.is_ok(),
        "expected graceful_shutdown with cat to succeed: {result:?}"
    );
}

// -- Card 3: additional restart_with_backoff / shutdown coverage ---------

/// `shutdown()` on a process that refuses to exit within the grace
/// window (by ignoring stdin close) must wait out the grace period and
/// then return with state NotStarted. This covers the `Err(_)` (timeout)
/// branch of the shutdown outcome match. The grace period is shortened via
/// `set_shutdown_grace` so the timeout branch is exercised in
/// milliseconds rather than the full production grace.
#[tokio::test]
async fn test_shutdown_timeout_path_waits_the_grace_then_not_started() {
    // sleep never responds to stdin close — shutdown will hit the
    // grace-period timeout.
    let mut daemon = daemon_with_child(spawn_silent());
    daemon.set_shutdown_grace(SHORT_SHUTDOWN_GRACE);

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
        elapsed >= SHORT_SHUTDOWN_GRACE,
        "expected at least the grace period {SHORT_SHUTDOWN_GRACE:?} (timeout path), got: {elapsed:?}"
    );
}

/// `shutdown()` after a client has been explicitly installed must clear
/// the shared client regardless of whether a child exists.
#[tokio::test]
async fn test_shutdown_clears_client_with_installed_value() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));
    let _client = install_cat_client(&daemon);
    let shared = daemon.shared_client();
    assert!(shared.lock().unwrap().is_some(), "the client is installed");

    // Shutdown with no child — should take the early-return path
    // but still clear the client.
    daemon.shutdown().await;

    assert!(
        shared.lock().unwrap().is_none(),
        "shutdown must clear client even when no child"
    );
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}

/// Calling `shutdown()` multiple times in a row on a never-started
/// daemon is idempotent — state remains NotStarted.
#[tokio::test]
async fn test_shutdown_is_idempotent_when_not_started() {
    let mut daemon = daemon_for(test_spec(IDLE_SERVER));

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);

    daemon.shutdown().await;
    assert_eq!(daemon.state(), LspDaemonState::NotStarted);
}
