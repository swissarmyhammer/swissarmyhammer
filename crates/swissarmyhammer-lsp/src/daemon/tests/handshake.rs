//! Tests for the `initialize` handshake of [`LspDaemon`] and for the stderr
//! capture that decorates a failed handshake.

use super::*;

/// A response id that differs from the id of the `initialize` request.
const WRONG_RESPONSE_ID: u64 = 999;

/// The stderr line of the mock that exits after it reads the request.
const EXIT_STDERR_TEXT: &str = "mock LSP fatal error: config not found";

/// The stderr line of the mock that closes stdout after it reads the
/// request.
const CLOSE_STDERR_TEXT: &str = "FATAL: mock LSP refuses to initialize";

/// The stderr text of the `capture_stderr` read test.
const CAPTURED_STDERR_TEXT: &str = "mock-error-text";

// -- initialize_handshake error path tests --------------------------------
//
// These use Python mocks (see `MOCK_PRELUDE`). Each mock reads the
// `initialize` request first, so the write of the request always succeeds,
// and then sends a bad response.

#[tokio::test]
async fn test_initialize_handshake_bad_json_response() {
    // Valid Content-Length framing but an invalid JSON body.
    let body = r#"read_message()
bad = 'this is not json at all!!!'
write_raw(f'Content-Length: {len(bad)}\r\n\r\n{bad}')
wait_for_stdin_eof()"#;
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(body));

    let result = daemon.start().await;
    assert_handshake_failed(result, "json decode");
}

#[tokio::test]
async fn test_initialize_handshake_missing_content_length_response() {
    // A blank line with no Content-Length header ends the headers.
    let body = r#"read_message()
write_raw('\r\n')
wait_for_stdin_eof()"#;
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(body));

    let result = daemon.start().await;
    assert_handshake_failed(result, "missing Content-Length header");
}

#[tokio::test]
async fn test_initialize_handshake_truncated_body() {
    // A Content-Length that is larger than the body, then EOF.
    let body = r#"read_message()
write_raw('Content-Length: 1000\r\n\r\nhello')
close_stdout()
wait_for_stdin_eof()"#;
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(body));

    let result = daemon.start().await;
    assert_handshake_failed(result, "read body");
}

/// A server that writes to stderr and then exits without an answer makes
/// the handshake read EOF. The error message then carries the stderr text,
/// which proves the stderr capture path.
#[tokio::test]
async fn test_initialize_handshake_process_exits_with_stderr() {
    let body = format!("read_message()\nlog('{EXIT_STDERR_TEXT}')\nsys.exit(1)");
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(&body));

    let result = daemon.start().await;
    let msg = assert_handshake_failed(result, EOF_BEFORE_ANSWER_TEXT);
    assert!(
        msg.contains(&format!("stderr: {EXIT_STDERR_TEXT}")),
        "expected the stderr text in the error, got: {msg}"
    );
}

#[tokio::test]
async fn test_initialize_handshake_wrong_id_still_succeeds() {
    // initialize_handshake does NOT validate the response ID. This test
    // documents that behavior: a response with a wrong id is accepted
    // because the code only checks framing, not content.
    let spec = handshake_spec(WRONG_RESPONSE_ID, SERVE_UNTIL_STDIN_EOF);
    let (_workspace, mut daemon) = daemon_in_tempdir(spec);

    let result = daemon.start().await;
    // This succeeds because initialize_handshake doesn't validate the ID
    assert!(
        result.is_ok(),
        "expected wrong-id response to be accepted (no ID validation), got: {result:?}"
    );

    daemon.shutdown().await;
}

// -- Card 4: initialize_handshake and capture_stderr direct tests -------

/// Run `initialize_handshake` directly against `child`, with a new
/// temporary workspace.
async fn handshake_in_tempdir(child: &mut Child) -> Result<(), LspError> {
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    LspDaemon::initialize_handshake(child, workspace.path(), &test_spec(IDLE_SERVER)).await
}

/// Directly invoke `initialize_handshake` with an invalid (relative)
/// workspace path. `Url::from_file_path` rejects relative paths, so the
/// function should return `HandshakeFailed("invalid workspace path")`.
/// This exercises the `.map_err` branch of the `root_uri` construction.
#[tokio::test]
async fn test_initialize_handshake_invalid_workspace_path() {
    let mut child = spawn_echo();

    // A relative path — `Url::from_file_path` requires absolute paths
    let relative = PathBuf::from("not/an/absolute/path");
    let result =
        LspDaemon::initialize_handshake(&mut child, &relative, &test_spec(IDLE_SERVER)).await;

    assert_handshake_failed(result, "invalid workspace path");
    kill_and_reap_child(&mut child).await;
}

/// Directly invoke `initialize_handshake` against a child that has
/// had its stdin taken externally. This exercises the `stdin.as_mut()`
/// `None` branch that returns `HandshakeFailed("child stdin unavailable")`.
#[tokio::test]
async fn test_initialize_handshake_stdin_unavailable() {
    let mut child = spawn_echo();
    // Take stdin, dropping the handle so the child sees EOF on stdin
    drop(child.stdin.take());

    let result = handshake_in_tempdir(&mut child).await;
    assert_handshake_failed(result, "child stdin unavailable");
    kill_and_reap_child(&mut child).await;
}

/// Directly invoke `initialize_handshake` against a child that has
/// had its stdout taken externally. This exercises the `stdout.as_mut()`
/// `None` branch that returns `HandshakeFailed("child stdout unavailable")`.
#[tokio::test]
async fn test_initialize_handshake_stdout_unavailable() {
    let mut child = spawn_echo();
    drop(child.stdout.take());

    let result = handshake_in_tempdir(&mut child).await;
    assert_handshake_failed(result, "child stdout unavailable");
    kill_and_reap_child(&mut child).await;
}

/// `capture_stderr` on a child whose stderr handle has been taken
/// returns an empty string (the early-return `None` path).
#[tokio::test]
async fn test_capture_stderr_returns_empty_when_no_stderr_handle() {
    let mut child = spawn_echo();
    drop(child.stderr.take());

    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert_eq!(captured, "");
    kill_and_reap_child(&mut child).await;
}

/// `capture_stderr` reads available data from the child's stderr
/// within the 1-second timeout, trims it, and returns it. The read waits
/// for the data, so the test needs no sleep. The shell `exec`s the sleep, so
/// the kill at the end stops the one process that holds the pipes.
#[tokio::test]
async fn test_capture_stderr_reads_available_bytes() {
    let command = format!("echo {CAPTURED_STDERR_TEXT} 1>&2; exec sleep {LONG_SLEEP_SECS}");
    let mut child = spawn_piped("sh", &["-c", &command]);

    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert_eq!(captured, CAPTURED_STDERR_TEXT);
    kill_and_reap_child(&mut child).await;
}

/// `capture_stderr` on a child that produces no stderr output within
/// the timeout returns the empty string (the `_ =>` arm of the match
/// on `timeout(read).await`).
#[tokio::test]
async fn test_capture_stderr_returns_empty_on_timeout() {
    let mut child = spawn_silent();

    // No stderr output forthcoming — capture_stderr's 1s timeout kicks in
    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert_eq!(captured, "");
    kill_and_reap_child(&mut child).await;
}

/// When `initialize_handshake` fails because the child closes stdout
/// while it stays alive, the error message is decorated with stderr
/// context (via `capture_stderr`). This confirms both the error path
/// through `read_jsonrpc_message` and the concatenation of stderr into
/// the `HandshakeFailed` message.
#[tokio::test]
async fn test_initialize_handshake_includes_stderr_context_in_error() {
    // The mock writes to stderr before it closes stdout, so the stderr
    // text is in the pipe when the daemon reads EOF.
    let body = format!(
        "read_message()\nlog('{CLOSE_STDERR_TEXT}')\nclose_stdout()\n{SERVE_UNTIL_STDIN_EOF}"
    );
    let (_workspace, mut daemon) = daemon_in_tempdir(mock_spec(&body));

    let result = daemon.start().await;
    let msg = assert_handshake_failed(result, EOF_BEFORE_ANSWER_TEXT);
    assert!(
        msg.contains(&format!("; stderr: {CLOSE_STDERR_TEXT}")),
        "expected the stderr decoration in the error, got: {msg}"
    );
}

/// `initialize_handshake` happy path: a Python mock that plays the full
/// initialize/initialized handshake correctly. The mock echoes the
/// `initialized` notification to stderr, and the test reads it back. This
/// covers the `send_jsonrpc_message(&mut writer, &initialized)` line (the
/// final send of the `initialized` notification).
#[tokio::test]
async fn test_initialize_handshake_full_happy_path_direct() {
    let script = mock_script(&handshake_body(
        INITIALIZE_REQUEST_ID,
        &format!("log(f'GOT: {{initialized}}')\n{SERVE_UNTIL_STDIN_EOF}"),
    ));
    let mut child = spawn_piped(PYTHON_PROGRAM, &["-c", &script]);

    let result = handshake_in_tempdir(&mut child).await;
    assert!(result.is_ok(), "expected handshake to succeed: {result:?}");

    let stderr = child.stderr.take().expect("child stderr");
    let echoed = tokio::time::timeout(SIGNAL_TIMEOUT, BufReader::new(stderr).lines().next_line())
        .await
        .expect("the mock echoes the initialized notification")
        .expect("read the mock stderr")
        .expect("the mock writes one stderr line");
    assert!(
        echoed.starts_with("GOT: ") && echoed.contains(r#""method":"initialized""#),
        "expected the initialized notification, got: {echoed}"
    );
    kill_and_reap_child(&mut child).await;
}
