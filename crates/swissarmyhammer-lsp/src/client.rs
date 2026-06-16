//! LSP JSON-RPC transport.
//!
//! Owns the wire-level JSON-RPC 2.0 client used to talk to a language-server
//! child process over stdin/stdout pipes with Content-Length framing.
//!
//! The [`LspTransport`] trait abstracts the three wire operations
//! (`send_request`, `send_notification`, `read_message`) so higher-level state
//! machines — the open-document/diagnostics flow in `swissarmyhammer-code-context`
//! and the diagnostics session work — can be unit-tested against an in-memory
//! fake transport without spawning a real language server. The real
//! [`LspJsonRpcClient`] implements the trait over child-process stdio.

use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{ChildStdin, ChildStdout};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolInformation};

use crate::error::LspError;

/// Timeout for a single LSP request/response round-trip.
///
/// If the LSP server does not produce a matching response within this window
/// (e.g., it silently ignores the request), `send_request` returns an error
/// instead of blocking the worker forever.
const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Abstraction over the LSP JSON-RPC wire protocol.
///
/// Implemented by the real stdio client ([`LspJsonRpcClient`]) and by in-memory
/// fakes in tests. The three methods are the minimal wire surface higher-level
/// LSP flows need: issue a request and await its matching response, fire a
/// notification, or read the next unsolicited server message (e.g. a
/// `textDocument/publishDiagnostics` notification).
pub trait LspTransport {
    /// Send a JSON-RPC request and block until the matching response arrives.
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError>;

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError>;

    /// Read the next JSON-RPC message from the server.
    ///
    /// This is used to drain server-initiated notifications (for example,
    /// `textDocument/publishDiagnostics`) that arrive outside the
    /// request/response cycle.
    fn read_message(&mut self) -> Result<Value, LspError>;
}

/// JSON-RPC request/response handler for LSP communication.
///
/// Communicates with an LSP server process over stdin/stdout pipes using
/// the JSON-RPC 2.0 protocol with Content-Length framing.
pub struct LspJsonRpcClient {
    /// Writable pipe to the LSP server's stdin.
    stdin: ChildStdin,
    /// Buffered reader over the LSP server's stdout.
    reader: BufReader<ChildStdout>,
    /// Raw file descriptor for the stdout pipe, used to poll for readability
    /// before blocking reads (enables timeout support).
    #[cfg(unix)]
    stdout_fd: std::os::unix::io::RawFd,
    /// Current request ID (incremented for each request).
    request_id: u32,
}

impl LspJsonRpcClient {
    /// Create a new JSON-RPC client from an LSP process's stdin and stdout pipes.
    ///
    /// The caller retains ownership of the `Child` handle for lifecycle management
    /// (health checks via `try_wait()`, shutdown via `kill()`). This client only
    /// needs the I/O pipes to send requests and read responses.
    ///
    /// # Arguments
    /// * `stdin` - The child process's stdin pipe (obtained via `child.stdin.take()`)
    /// * `stdout` - The child process's stdout pipe (obtained via `child.stdout.take()`)
    pub fn new(stdin: ChildStdin, stdout: ChildStdout) -> Self {
        #[cfg(unix)]
        let stdout_fd = {
            use std::os::unix::io::AsRawFd;
            stdout.as_raw_fd()
        };

        Self {
            stdin,
            reader: BufReader::new(stdout),
            #[cfg(unix)]
            stdout_fd,
            request_id: 1,
        }
    }

    /// Send a JSON-RPC request and read the response.
    ///
    /// Uses Content-Length framing per the LSP specification. The response
    /// read is bounded by [`LSP_REQUEST_TIMEOUT`] — if no matching response
    /// arrives within that window an `LspError` is returned instead of
    /// blocking indefinitely.
    pub fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
        let expected_id = self.request_id;
        self.request_id += 1;

        self.write_jsonrpc_request(method, params, expected_id)?;
        debug!("Sent LSP request: {} (id={})", method, expected_id);

        self.read_matching_response(method, expected_id)
    }

    /// Format and write a JSON-RPC 2.0 request with Content-Length framing.
    fn write_jsonrpc_request(
        &mut self,
        method: &str,
        params: Value,
        id: u32,
    ) -> Result<(), LspError> {
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": id,
        });

        let json_str = request.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| LspError::JsonRpc(format!("write failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| LspError::JsonRpc(format!("flush failed: {}", e)))?;

        Ok(())
    }

    /// Read JSON-RPC messages until one with a matching `id` arrives.
    ///
    /// Notifications (no `id`) and responses with mismatched IDs are skipped.
    /// The entire read loop is bounded by [`LSP_REQUEST_TIMEOUT`].
    fn read_matching_response(
        &mut self,
        method: &str,
        expected_id: u32,
    ) -> Result<Value, LspError> {
        let deadline = Instant::now() + LSP_REQUEST_TIMEOUT;

        loop {
            self.wait_for_readable(deadline).map_err(|_| {
                LspError::JsonRpc(format!(
                    "LSP request '{}' (id={}) timed out after {}s",
                    method,
                    expected_id,
                    LSP_REQUEST_TIMEOUT.as_secs()
                ))
            })?;

            let response = read_jsonrpc_response(&mut self.reader)?;

            match classify_response(&response, expected_id) {
                ResponseMatch::Match => return Ok(response),
                ResponseMatch::Notification | ResponseMatch::Mismatch => continue,
            }
        }
    }

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    ///
    /// Per the JSON-RPC 2.0 spec, notifications omit the `id` field and the
    /// server must not reply. This avoids the timeout penalty of `send_request`
    /// for methods like `textDocument/didOpen` and `textDocument/didClose`.
    pub fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let json_str = notification.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| LspError::JsonRpc(format!("write notification failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| LspError::JsonRpc(format!("flush notification failed: {}", e)))?;

        debug!("Sent LSP notification: {}", method);
        Ok(())
    }

    /// Read the next framed JSON-RPC message from the server's stdout.
    ///
    /// Unlike [`send_request`](Self::send_request), this does not filter by id —
    /// it returns whatever the server sends next, which lets callers drain
    /// server-initiated notifications. The read is bounded by
    /// [`LSP_REQUEST_TIMEOUT`].
    pub fn read_message(&mut self) -> Result<Value, LspError> {
        let deadline = Instant::now() + LSP_REQUEST_TIMEOUT;
        self.wait_for_readable(deadline).map_err(|_| {
            LspError::JsonRpc(format!(
                "LSP read_message timed out after {}s",
                LSP_REQUEST_TIMEOUT.as_secs()
            ))
        })?;
        read_jsonrpc_response(&mut self.reader)
    }

    /// Block until the stdout pipe has data available, or the deadline expires.
    ///
    /// Uses `poll(2)` on Unix to check readability without consuming any bytes
    /// from the pipe. Returns `Ok(())` when data is ready, `Err(())` on timeout.
    ///
    /// Note: If the BufReader already has buffered data, `poll` on the raw fd
    /// might not see it. However, in practice each JSON-RPC message is read
    /// completely by `read_jsonrpc_response`, so the buffer is drained before
    /// we poll again.
    #[cfg(unix)]
    fn wait_for_readable(&self, deadline: Instant) -> Result<(), ()> {
        // If the BufReader has buffered bytes, data is already available.
        if !self.reader.buffer().is_empty() {
            return Ok(());
        }

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(());
            }

            // Cap individual poll calls at 1 second so we re-check the deadline
            // even if the kernel rounds the timeout.
            let poll_ms = remaining.as_millis().min(1_000) as i32;

            let ready = poll_fd(self.stdout_fd, poll_ms);
            if ready > 0 {
                return Ok(());
            }
            if ready < 0 {
                // poll error — treat as timeout to avoid infinite loop
                return Err(());
            }
            // ready == 0 → poll timed out, loop to re-check deadline
        }
    }

    /// Non-Unix fallback: no timeout support, returns immediately.
    #[cfg(not(unix))]
    fn wait_for_readable(&self, _deadline: Instant) -> Result<(), ()> {
        Ok(())
    }

    /// Initialize the LSP server and wait for initialization response.
    pub fn initialize(&mut self, workspace_root: &Path) -> Result<(), LspError> {
        debug!("Initializing LSP server");

        let root_str = workspace_root.to_string_lossy().to_string();
        let uri = format!("file://{}", root_str);

        let params = json!({
            "processId": std::process::id() as i32,
            "rootPath": root_str,
            "rootUri": uri,
            "capabilities": {
                "textDocument": {
                    "documentSymbol": {
                        "hierarchicalDocumentSymbolSupport": true
                    }
                }
            }
        });

        let response = self.send_request("initialize", params)?;

        // Check for errors
        if let Some(error) = response.get("error") {
            return Err(LspError::HandshakeFailed(format!(
                "LSP initialize error: {}",
                error
            )));
        }

        // Send initialized notification (no response expected)
        let json_str = json!({
            "jsonrpc": "2.0",
            "method": "initialized",
            "params": {}
        })
        .to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| LspError::JsonRpc(format!("write initialized failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| LspError::JsonRpc(format!("flush initialized failed: {}", e)))?;

        debug!("LSP server initialized");
        Ok(())
    }

    /// Send a `textDocument/didOpen` notification to the LSP server.
    ///
    /// This informs the server that a document has been opened by the client,
    /// which is required before the server will respond to requests for that document.
    ///
    /// # Arguments
    /// * `file_path` - Absolute path to the file
    /// * `language_id` - Language identifier (e.g., "rust", "python")
    /// * `text` - Full text content of the file
    pub fn send_did_open(
        &mut self,
        file_path: &Path,
        language_id: &str,
        text: &str,
    ) -> Result<(), LspError> {
        let uri = format!("file://{}", file_path.to_string_lossy());

        let notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didOpen",
            "params": {
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }
        });

        let json_str = notification.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| LspError::JsonRpc(format!("write didOpen failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| LspError::JsonRpc(format!("flush didOpen failed: {}", e)))?;

        debug!("Sent textDocument/didOpen for {}", file_path.display());
        Ok(())
    }

    /// Send a `textDocument/didClose` notification to the LSP server.
    ///
    /// This informs the server that the client is no longer interested in
    /// the document. Should be called after indexing to avoid "duplicate
    /// didOpen" warnings on re-indexing.
    pub fn send_did_close(&mut self, file_path: &Path) -> Result<(), LspError> {
        let uri = format!("file://{}", file_path.to_string_lossy());

        let notification = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/didClose",
            "params": {
                "textDocument": {
                    "uri": uri
                }
            }
        });

        let json_str = notification.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| LspError::JsonRpc(format!("write didClose failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| LspError::JsonRpc(format!("flush didClose failed: {}", e)))?;

        debug!("Sent textDocument/didClose for {}", file_path.display());
        Ok(())
    }

    /// Send the LSP `shutdown` request followed by `exit` notification.
    ///
    /// This cleanly terminates the LSP server. The caller is responsible for
    /// waiting on the `Child` process handle after calling this method.
    pub fn shutdown(mut self) -> Result<(), LspError> {
        // Send shutdown request (expects a response)
        let _response = self.send_request("shutdown", json!(null))?;

        // Send exit notification (no response expected)
        let notification = json!({
            "jsonrpc": "2.0",
            "method": "exit"
        });
        let json_str = notification.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        let _ = self.stdin.write_all(msg.as_bytes());
        let _ = self.stdin.flush();

        debug!("LSP server shut down");
        Ok(())
    }
}

/// Shared handle to an LSP client that may or may not be available.
///
/// The `Option` is `None` when the LSP daemon hasn't started or is restarting.
/// A worker locks the mutex, checks for `Some`, and uses the client to send
/// requests. The daemon's owner is responsible for populating and clearing this.
pub type SharedLspClient = Arc<Mutex<Option<LspJsonRpcClient>>>;

impl LspTransport for LspJsonRpcClient {
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, LspError> {
        LspJsonRpcClient::send_request(self, method, params)
    }

    fn send_notification(&mut self, method: &str, params: Value) -> Result<(), LspError> {
        LspJsonRpcClient::send_notification(self, method, params)
    }

    fn read_message(&mut self) -> Result<Value, LspError> {
        LspJsonRpcClient::read_message(self)
    }
}

/// Parse a JSON-RPC response body into DocumentSymbol array.
///
/// LSP `textDocument/documentSymbol` can return either:
/// - `DocumentSymbol[]` (hierarchical, preferred)
/// - `SymbolInformation[]` (flat, legacy)
///
/// Both formats are supported. `SymbolInformation` items are converted to
/// flat `DocumentSymbol` entries (no children, `range` = `selection_range` =
/// the symbol's `location.range`).
pub fn parse_document_symbols(response: &Value) -> Result<Vec<DocumentSymbol>, LspError> {
    // Check for JSON-RPC error first
    if let Some(error) = response.get("error") {
        return Err(LspError::JsonRpc(format!("LSP error: {}", error)));
    }

    // Extract the "result" field from the JSON-RPC response
    let result = match response.get("result") {
        Some(Value::Array(arr)) => arr,
        Some(Value::Null) | None => return Ok(Vec::new()),
        Some(other) => {
            return Err(LspError::JsonRpc(format!(
                "unexpected documentSymbol result type: {}",
                other
            )));
        }
    };

    if result.is_empty() {
        return Ok(Vec::new());
    }

    // Use lsp-types' DocumentSymbolResponse which handles both formats
    let dsr: DocumentSymbolResponse = serde_json::from_value(Value::Array(result.clone()))
        .map_err(|e| {
            LspError::JsonRpc(format!("failed to parse documentSymbol response: {}", e))
        })?;

    match dsr {
        DocumentSymbolResponse::Nested(symbols) => Ok(symbols),
        DocumentSymbolResponse::Flat(infos) => Ok(symbol_information_to_document_symbols(infos)),
    }
}

/// Convert legacy `SymbolInformation[]` to `DocumentSymbol[]`.
///
/// Each `SymbolInformation` becomes a flat `DocumentSymbol` with no children.
/// The `range` and `selection_range` are both set to `location.range`.
#[allow(deprecated)]
fn symbol_information_to_document_symbols(infos: Vec<SymbolInformation>) -> Vec<DocumentSymbol> {
    infos
        .into_iter()
        .map(|si| DocumentSymbol {
            name: si.name,
            detail: si.container_name,
            kind: si.kind,
            tags: si.tags,
            deprecated: si.deprecated,
            range: si.location.range,
            selection_range: si.location.range,
            children: None,
        })
        .collect()
}

/// Call `poll(2)` on a single file descriptor, returning the number of
/// ready descriptors (0 = timeout, negative = error).
///
/// This uses an `extern "C"` declaration to avoid a `libc` crate dependency.
#[cfg(unix)]
fn poll_fd(fd: std::os::unix::io::RawFd, timeout_ms: i32) -> i32 {
    /// Minimal `pollfd` struct matching the POSIX definition.
    #[repr(C)]
    struct PollFd {
        fd: i32,
        events: i16,
        revents: i16,
    }

    /// POLLIN constant — data available for reading.
    const POLLIN: i16 = 0x0001;

    extern "C" {
        fn poll(fds: *mut PollFd, nfds: u64, timeout: i32) -> i32;
    }

    let mut pfd = PollFd {
        fd,
        events: POLLIN,
        revents: 0,
    };

    // SAFETY: We pass a valid pointer to a single PollFd, nfds=1, and a
    // non-negative timeout. The fd is owned by ChildStdout which outlives
    // this call.
    unsafe { poll(&mut pfd as *mut PollFd, 1, timeout_ms) }
}

/// Classification of an incoming JSON-RPC message relative to an expected ID.
enum ResponseMatch {
    /// The response `id` matches the expected ID.
    Match,
    /// A notification (no `id` field) — should be skipped.
    Notification,
    /// A response with a mismatched `id` — should be skipped.
    Mismatch,
}

/// Classify a JSON-RPC response as matching, a notification, or mismatched.
fn classify_response(response: &Value, expected_id: u32) -> ResponseMatch {
    match response.get("id") {
        None => {
            trace!(
                "Skipping LSP notification: {}",
                response
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
            );
            ResponseMatch::Notification
        }
        Some(id) if id.as_u64() == Some(expected_id as u64) => ResponseMatch::Match,
        Some(id) => {
            warn!(
                "Unexpected response id: expected {}, got {} (method: {}). Skipping.",
                expected_id,
                id,
                response
                    .get("method")
                    .and_then(|v| v.as_str())
                    .unwrap_or("none")
            );
            ResponseMatch::Mismatch
        }
    }
}

/// Read a single JSON-RPC message from a reader using Content-Length framing.
fn read_jsonrpc_response<R: BufRead>(reader: &mut R) -> Result<Value, LspError> {
    let mut content_length: Option<usize> = None;

    // Read headers until blank line
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| LspError::JsonRpc(format!("read header: {}", e)))?;
        if n == 0 {
            return Err(LspError::JsonRpc("unexpected EOF reading headers".into()));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length = Some(
                val.trim()
                    .parse::<usize>()
                    .map_err(|e| LspError::JsonRpc(format!("bad Content-Length: {}", e)))?,
            );
        }
    }

    let length =
        content_length.ok_or_else(|| LspError::JsonRpc("missing Content-Length header".into()))?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|e| LspError::JsonRpc(format!("read body: {}", e)))?;

    serde_json::from_slice(&body).map_err(|e| LspError::JsonRpc(format!("json decode: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::FakeTransport;

    #[test]
    fn fake_transport_records_request_and_replays_response() {
        let mut transport = FakeTransport::default().with_response(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"capabilities": {}}
        }));

        let response = transport
            .send_request("initialize", json!({"rootUri": "file:///tmp"}))
            .expect("scripted response should be returned");

        assert_eq!(response["result"]["capabilities"], json!({}));
        assert_eq!(transport.sent_requests.len(), 1);
        assert_eq!(transport.sent_requests[0].0, "initialize");
        assert_eq!(
            transport.sent_requests[0].1,
            json!({"rootUri": "file:///tmp"})
        );
    }

    #[test]
    fn fake_transport_records_notifications() {
        let mut transport = FakeTransport::default();

        transport
            .send_notification(
                "textDocument/didOpen",
                json!({"textDocument": {"uri": "file:///tmp/a.rs"}}),
            )
            .expect("notification should be accepted");

        assert_eq!(transport.sent_notifications.len(), 1);
        assert_eq!(transport.sent_notifications[0].0, "textDocument/didOpen");
    }

    #[test]
    fn fake_transport_read_message_drains_server_notifications() {
        let diagnostics = json!({
            "jsonrpc": "2.0",
            "method": "textDocument/publishDiagnostics",
            "params": {
                "uri": "file:///tmp/a.rs",
                "diagnostics": [{"message": "unused variable"}]
            }
        });
        let mut transport = FakeTransport::default().with_incoming(diagnostics.clone());

        let message = transport
            .read_message()
            .expect("scripted incoming message should be returned");

        assert_eq!(message, diagnostics);
        assert_eq!(message["method"], "textDocument/publishDiagnostics");
    }

    #[test]
    fn fake_transport_send_request_without_script_errors() {
        let mut transport = FakeTransport::default();
        let err = transport
            .send_request("textDocument/documentSymbol", json!({}))
            .expect_err("no scripted response should yield an error");
        assert!(matches!(err, LspError::JsonRpc(_)));
    }

    #[test]
    fn parse_document_symbols_reads_jsonrpc_result() {
        // A FakeTransport can drive the same parsing path the real client uses.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "MyStruct",
                    "kind": 23,
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 20, "character": 1}
                    },
                    "selectionRange": {
                        "start": {"line": 0, "character": 11},
                        "end": {"line": 0, "character": 19}
                    }
                }
            ]
        });

        let symbols = parse_document_symbols(&response).expect("should parse symbols");
        assert_eq!(symbols.len(), 1);
        assert_eq!(symbols[0].name, "MyStruct");
    }
}
