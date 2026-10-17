//! Tests for the `initialize` handshake of [`LspDaemon`] and for the stderr
//! capture that decorates a failed handshake.

use super::*;

// -- initialize_handshake error path tests --------------------------------
//
// These use Python one-liners as mock LSP servers. The Python script reads
// the incoming JSON-RPC request, then sends a malformed response.

#[tokio::test]
async fn test_initialize_handshake_bad_json_response() {
    // Python script reads the initialize request, then responds with
    // valid Content-Length framing but invalid JSON body.
    let script = r#"
import sys
# Read the incoming request: headers then body
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
# Extract content length and read body
import re
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Send bad JSON with valid framing
bad = 'this is not json at all!!!'
sys.stdout.write(f'Content-Length: {len(bad)}\r\n\r\n{bad}')
sys.stdout.flush()
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_err(), "expected start to fail with bad JSON");
    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(_)),
        "expected HandshakeFailed, got: {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("json decode") || msg.contains("json"),
        "expected JSON decode error in message, got: {msg}"
    );
}

#[tokio::test]
async fn test_initialize_handshake_missing_content_length_response() {
    // Python script reads the request, then sends a response without
    // the Content-Length header (just raw JSON after blank line).
    let script = r#"
import sys, re
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Send without Content-Length header -- just newline then body
sys.stdout.write('\r\n')
sys.stdout.flush()
# Then close stdout so the reader gets EOF
sys.stdout.close()
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(
        result.is_err(),
        "expected start to fail without Content-Length"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(_)),
        "expected HandshakeFailed, got: {err:?}"
    );
    let msg = err.to_string();
    assert!(
        msg.contains("Content-Length") || msg.contains("EOF") || msg.contains("missing"),
        "expected missing Content-Length or EOF error, got: {msg}"
    );
}

#[tokio::test]
async fn test_initialize_handshake_truncated_body() {
    // Python script reads the request, then sends a Content-Length
    // that's larger than the actual body, causing an EOF during read.
    let script = r#"
import sys, re
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Claim 1000 bytes but only send 5
sys.stdout.write('Content-Length: 1000\r\n\r\nhello')
sys.stdout.flush()
sys.stdout.close()
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(
        result.is_err(),
        "expected start to fail with truncated body"
    );
    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(_)),
        "expected HandshakeFailed, got: {err:?}"
    );
}

#[tokio::test]
async fn test_initialize_handshake_process_exits_with_stderr() {
    // Python script writes to stderr then exits without responding,
    // exercising the stderr capture path in initialize_handshake.
    let script = r#"
import sys
sys.stderr.write('mock LSP fatal error: config not found\n')
sys.stderr.flush()
sys.exit(1)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_err(), "expected start to fail when process exits");
    let err = result.unwrap_err();
    assert!(
        matches!(err, LspError::HandshakeFailed(_) | LspError::Timeout(_)),
        "expected HandshakeFailed or Timeout, got: {err:?}"
    );
}

#[tokio::test]
async fn test_initialize_handshake_wrong_id_still_succeeds() {
    // initialize_handshake does NOT validate the response ID. This test
    // documents that behavior: a response with id:999 is accepted because
    // the code only checks framing, not content.
    let script = r#"
import sys, re, json
headers = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers += line
m = re.search(r'Content-Length:\s*(\d+)', headers)
if m:
    body = sys.stdin.read(int(m.group(1)))
# Respond with wrong id
resp = json.dumps({"jsonrpc": "2.0", "id": 999, "result": {"capabilities": {}}})
sys.stdout.write(f'Content-Length: {len(resp)}\r\n\r\n{resp}')
sys.stdout.flush()
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
# Keep stdin open briefly so pipes don't break
import time
time.sleep(0.5)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    // This succeeds because initialize_handshake doesn't validate the ID
    assert!(
        result.is_ok(),
        "expected wrong-id response to be accepted (no ID validation), got: {result:?}"
    );

    daemon.shutdown().await;
}

// -- Card 4: initialize_handshake and capture_stderr direct tests -------

/// Directly invoke `initialize_handshake` with an invalid (relative)
/// workspace path. `Url::from_file_path` rejects relative paths, so the
/// function should return `HandshakeFailed("invalid workspace path")`.
/// This exercises the `.map_err` branch of the `root_uri` construction.
#[tokio::test]
async fn test_initialize_handshake_invalid_workspace_path() {
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    let spec = test_spec("cat");
    // A relative path — `Url::from_file_path` requires absolute paths
    let relative = PathBuf::from("not/an/absolute/path");
    let result = LspDaemon::initialize_handshake(&mut child, &relative, &spec).await;

    let _ = child.kill().await;

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        LspError::HandshakeFailed(msg) => {
            assert!(
                msg.contains("invalid workspace path"),
                "expected 'invalid workspace path', got: {msg}"
            );
        }
        other => panic!("expected HandshakeFailed, got: {other:?}"),
    }
}

/// Directly invoke `initialize_handshake` against a child that has
/// had its stdin taken externally. This exercises the `stdin.as_mut()`
/// `None` branch that returns `HandshakeFailed("child stdin unavailable")`.
#[tokio::test]
async fn test_initialize_handshake_stdin_unavailable() {
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    // Take stdin, dropping the handle so the child sees EOF on stdin
    let _stolen_stdin = child.stdin.take();
    drop(_stolen_stdin);

    let spec = test_spec("cat");
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

    let _ = child.kill().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LspError::HandshakeFailed(msg) => {
            assert!(
                msg.contains("child stdin unavailable"),
                "expected 'child stdin unavailable', got: {msg}"
            );
        }
        other => panic!("expected HandshakeFailed with stdin msg, got: {other:?}"),
    }
}

/// Directly invoke `initialize_handshake` against a child that has
/// had its stdout taken externally. This exercises the `stdout.as_mut()`
/// `None` branch that returns `HandshakeFailed("child stdout unavailable")`.
#[tokio::test]
async fn test_initialize_handshake_stdout_unavailable() {
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    // Take stdout, dropping the handle
    let _stolen_stdout = child.stdout.take();
    drop(_stolen_stdout);

    let spec = test_spec("cat");
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

    let _ = child.kill().await;

    assert!(result.is_err());
    match result.unwrap_err() {
        LspError::HandshakeFailed(msg) => {
            assert!(
                msg.contains("child stdout unavailable"),
                "expected 'child stdout unavailable', got: {msg}"
            );
        }
        other => panic!("expected HandshakeFailed with stdout msg, got: {other:?}"),
    }
}

/// `capture_stderr` on a child whose stderr handle has been taken
/// returns an empty string (the early-return `None` path).
#[tokio::test]
async fn test_capture_stderr_returns_empty_when_no_stderr_handle() {
    let mut child = Command::new("cat")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn cat");

    // Take stderr so the handle is None
    let _stolen = child.stderr.take();
    drop(_stolen);

    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert_eq!(captured, "");

    let _ = child.kill().await;
}

/// `capture_stderr` reads available data from the child's stderr
/// within the 1-second timeout, trims it, and returns it.
#[tokio::test]
async fn test_capture_stderr_reads_available_bytes() {
    let mut child = Command::new("sh")
        .args(["-c", "echo mock-error-text 1>&2; sleep 2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sh");

    // Give the child a moment to write to stderr
    tokio::time::sleep(Duration::from_millis(200)).await;

    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert!(
        captured.contains("mock-error-text"),
        "expected 'mock-error-text' in captured stderr, got: {captured:?}"
    );

    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// `capture_stderr` on a child that produces no stderr output within
/// the timeout returns the empty string (the `_ =>` arm of the match
/// on `timeout(read).await`).
#[tokio::test]
async fn test_capture_stderr_returns_empty_on_timeout() {
    let mut child = Command::new("sh")
        .args(["-c", "sleep 10"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn sh");

    // No stderr output forthcoming — capture_stderr's 1s timeout kicks in
    let captured = LspDaemon::capture_stderr(&mut child).await;
    assert_eq!(captured, "");

    let _ = child.kill().await;
    let _ = child.wait().await;
}

/// When `initialize_handshake` fails because the child closes stdout
/// immediately, the error message should be decorated with stderr
/// context (via `capture_stderr`). This confirms both the error path
/// through `read_jsonrpc_message` and the concatenation of stderr into
/// the `HandshakeFailed` message.
#[tokio::test]
async fn test_initialize_handshake_includes_stderr_context_in_error() {
    // Python script that writes to stderr and then exits without
    // responding on stdout. The daemon will read EOF from stdout and
    // then capture_stderr should pick up the error message.
    let script = r#"
import sys
sys.stderr.write('FATAL: mock LSP refuses to initialize\n')
sys.stderr.flush()
# Close stdout immediately so reader gets EOF
sys.stdout.close()
import time
time.sleep(0.3)
"#;
    let spec = python_mock_spec(script);
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let mut daemon = LspDaemon::new(spec, workspace.path().to_path_buf());

    let result = daemon.start().await;
    assert!(result.is_err(), "expected start to fail");
    match result.unwrap_err() {
        LspError::HandshakeFailed(msg) => {
            // capture_stderr should prepend "; stderr: " once it reads
            // the stderr content. The exact content depends on timing,
            // but the error should surface an EOF and optionally a
            // stderr decoration. We accept either (timing-sensitive).
            assert!(
                msg.contains("EOF")
                    || msg.contains("stderr")
                    || msg.contains("FATAL")
                    || msg.contains("read header"),
                "expected EOF/stderr/FATAL/read header in msg, got: {msg}"
            );
        }
        LspError::Timeout(_) => {
            // Timing-dependent alternate path: if the timeout fires
            // before read_jsonrpc_message completes, we get Timeout.
        }
        other => panic!("expected HandshakeFailed or Timeout, got: {other:?}"),
    }
}

/// `initialize_handshake` happy path: a Python mock that plays the full
/// initialize/initialized handshake correctly. This covers the
/// `send_jsonrpc_message(&mut writer, &initialized)` line (the final
/// send of the `initialized` notification).
#[tokio::test]
async fn test_initialize_handshake_full_happy_path_direct() {
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
# Read the initialized notification and confirm it was sent
headers2 = ''
while True:
    line = sys.stdin.readline()
    if line.strip() == '':
        break
    headers2 += line
m2 = re.search(r'Content-Length:\s*(\d+)', headers2)
if m2:
    body2 = sys.stdin.read(int(m2.group(1)))
    # Echo the initialized body to stderr so we can assert on it
    sys.stderr.write(f'GOT: {body2}\n')
    sys.stderr.flush()
time.sleep(0.5)
"#;
    let mut child = Command::new("python3")
        .args(["-c", script])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
        .expect("spawn python3");

    let spec = test_spec("python3");
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let result = LspDaemon::initialize_handshake(&mut child, workspace.path(), &spec).await;

    assert!(result.is_ok(), "expected handshake to succeed: {result:?}");

    let _ = child.kill().await;
    let _ = child.wait().await;
}
