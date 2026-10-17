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
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolInformation};

use crate::error::LspError;

/// Environment variable that overrides the LSP request timeout, in whole seconds.
///
/// See [`lsp_request_timeout`] for how it is read and applied.
const LSP_REQUEST_TIMEOUT_ENV: &str = "SAH_LSP_REQUEST_TIMEOUT_SECS";

/// Default LSP request timeout, in seconds, when the env override is absent.
///
/// This is the value the shipped production binary uses — it is only overridden
/// when [`LSP_REQUEST_TIMEOUT_ENV`] is set to a valid whole number of seconds.
const DEFAULT_LSP_REQUEST_TIMEOUT_SECS: u64 = 30;

/// Resolve the raw env value into a request timeout `Duration`.
///
/// Whitespace is trimmed before parsing. An absent (`None`), empty, or
/// unparseable value falls back to [`DEFAULT_LSP_REQUEST_TIMEOUT_SECS`] — the
/// override never fails loudly, it only widens (or restores) the budget.
///
/// Split out from [`lsp_request_timeout`] so the parse/fallback behavior is unit
/// testable without mutating process-global environment state.
fn parse_lsp_request_timeout(raw: Option<String>) -> Duration {
    let secs = raw
        .and_then(|value| value.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_LSP_REQUEST_TIMEOUT_SECS);
    Duration::from_secs(secs)
}

/// Timeout for a single LSP request/response round-trip.
///
/// If the LSP server does not produce a matching response within this window
/// (e.g., it silently ignores the request), the bounded read returns an
/// `LspError` instead of blocking the worker forever.
///
/// Defaults to [`DEFAULT_LSP_REQUEST_TIMEOUT_SECS`] seconds and can be raised by
/// setting the `SAH_LSP_REQUEST_TIMEOUT_SECS` environment variable to a whole
/// number of seconds. This exists because a cold `rust-analyzer` on a slow or
/// heavily-loaded host legitimately needs more than the default to answer a
/// `rename`/`definition` request; the operator can grant a larger budget without
/// changing the shipped default. The env var is read exactly once and cached for
/// the process lifetime via [`OnceLock`]; an unset or unparseable value falls
/// back to the default (see [`parse_lsp_request_timeout`]).
fn lsp_request_timeout() -> Duration {
    static TIMEOUT: OnceLock<Duration> = OnceLock::new();
    *TIMEOUT.get_or_init(|| parse_lsp_request_timeout(std::env::var(LSP_REQUEST_TIMEOUT_ENV).ok()))
}

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

/// Count how many recorded `(method, params)` entries match `method`.
///
/// The one counting path shared by the in-memory test transports, which each
/// record the wire traffic they are handed as a `Vec<(String, Value)>` and need
/// to assert how many requests or notifications of a given method were emitted.
/// Kept here, next to [`LspTransport`], so the test doubles in both this crate
/// (`FakeTransport`) and `swissarmyhammer-diagnostics` (`RecordingTransport`)
/// interpret the same data the same way rather than each re-implementing the
/// filter.
pub fn count_recorded_method(records: &[(String, Value)], method: &str) -> usize {
    records.iter().filter(|(m, _)| m == method).count()
}

/// JSON-RPC request/response handler for LSP communication.
///
/// Communicates with an LSP server process over stdin/stdout pipes using
/// the JSON-RPC 2.0 protocol with Content-Length framing.
pub struct LspJsonRpcClient {
    /// Writable pipe to the LSP server's stdin.
    stdin: ChildStdin,
    /// Buffered reader over the LSP server's stdout. Its raw file descriptor
    /// is polled for readability before blocking reads (see [`stdout_ready`]).
    reader: BufReader<ChildStdout>,
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
        Self {
            stdin,
            reader: BufReader::new(stdout),
            request_id: 1,
        }
    }

    /// Send a JSON-RPC request and read the response.
    ///
    /// Uses Content-Length framing per the LSP specification. The response
    /// read is bounded by [`lsp_request_timeout`] — if no matching response
    /// arrives within that window an `LspError` is returned instead of
    /// blocking indefinitely.
    ///
    /// # Errors
    ///
    /// Returns an [`LspError::JsonRpc`] if the write fails or no matching
    /// response arrives within the request timeout.
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
        write_message(&mut self.stdin, &request)
    }

    /// Read JSON-RPC messages until the reply to request `expected_id` arrives.
    ///
    /// See [`read_reply`] for how each other message is handled. The entire
    /// read loop is bounded by [`lsp_request_timeout`]; on a timeout the
    /// server gets a `$/cancelRequest` for the request.
    fn read_matching_response(
        &mut self,
        method: &str,
        expected_id: u32,
    ) -> Result<Value, LspError> {
        let pending = PendingRequest {
            method,
            id: expected_id,
            timeout: lsp_request_timeout(),
        };
        let deadline = Instant::now() + pending.timeout;
        read_reply(&mut self.stdin, &mut self.reader, &pending, |reader| {
            stdout_ready(reader, deadline)
        })
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

        write_message(&mut self.stdin, &notification)?;
        debug!("Sent LSP notification: {}", method);
        Ok(())
    }

    /// Read the next framed JSON-RPC message from the server's stdout.
    ///
    /// Unlike [`send_request`](Self::send_request), this does not filter by id —
    /// it returns whatever the server sends next, which lets callers drain
    /// server-initiated notifications. The read is bounded by
    /// [`lsp_request_timeout`].
    pub fn read_message(&mut self) -> Result<Value, LspError> {
        let timeout = lsp_request_timeout();
        let deadline = Instant::now() + timeout;
        if !stdout_ready(&self.reader, deadline) {
            return Err(LspError::JsonRpc(format!(
                "LSP read_message timed out after {}s",
                timeout.as_secs()
            )));
        }
        read_jsonrpc_response(&mut self.reader)
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

/// The longest single `poll(2)` wait, in milliseconds. A wait is capped so the
/// deadline is checked again even if the kernel rounds the timeout.
#[cfg(unix)]
const MAX_POLL_MS: u128 = 1_000;

/// Tell if the server's stdout has data to read before `deadline`.
///
/// Uses `poll(2)` on Unix to check readability without consuming any bytes
/// from the pipe. Bytes already in the `BufReader` buffer count as data, as
/// `poll` on the raw fd does not see them. A `poll` error counts as no data,
/// so the caller times out instead of looping for ever.
#[cfg(unix)]
fn stdout_ready(reader: &BufReader<ChildStdout>, deadline: Instant) -> bool {
    use std::os::unix::io::AsRawFd;

    if !reader.buffer().is_empty() {
        return true;
    }
    let fd = reader.get_ref().as_raw_fd();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return false;
        }
        let poll_ms = remaining.as_millis().min(MAX_POLL_MS) as i32;
        match poll_fd(fd, poll_ms) {
            ready if ready > 0 => return true,
            ready if ready < 0 => return false,
            // The poll timed out: check the deadline again.
            _ => {}
        }
    }
}

/// Non-Unix fallback: no timeout support, data always counts as ready.
#[cfg(not(unix))]
fn stdout_ready(_reader: &BufReader<ChildStdout>, _deadline: Instant) -> bool {
    true
}

/// The JSON-RPC error code for a method that the receiver does not support.
const METHOD_NOT_FOUND: i64 = -32601;

/// The LSP notification that tells the server to stop work on a request.
const CANCEL_REQUEST_METHOD: &str = "$/cancelRequest";

/// A request that waits for its reply.
struct PendingRequest<'a> {
    /// The method of the request, for the log and the timeout error.
    method: &'a str,
    /// The id that the reply carries.
    id: u32,
    /// How long the request waits for its reply.
    timeout: Duration,
}

/// What one incoming message is, relative to the pending request.
enum ResponseMatch {
    /// The reply to the pending request.
    Match,
    /// A notification from the server: a `method` and no `id`.
    Notification,
    /// A request from the server: a `method` and an `id`. It is never the
    /// reply to the pending request, also when the two ids are equal.
    ServerRequest,
    /// A reply to an earlier request, for example one that timed out.
    Stale,
}

/// Classify one incoming message relative to the pending request `expected_id`.
fn classify_response(message: &Value, expected_id: u32) -> ResponseMatch {
    let id = message.get("id");
    if message.get("method").is_some() {
        return match id {
            Some(_) => ResponseMatch::ServerRequest,
            None => ResponseMatch::Notification,
        };
    }
    match id {
        Some(id) if id_matches(id, expected_id) => ResponseMatch::Match,
        _ => ResponseMatch::Stale,
    }
}

/// Tell if a reply `id` is the request id `expected_id`.
///
/// The client sends numeric ids. A server can echo an id as a number or as a
/// string of that number, so both forms match.
fn id_matches(id: &Value, expected_id: u32) -> bool {
    let expected = u64::from(expected_id);
    id.as_u64() == Some(expected)
        || id.as_str().and_then(|text| text.parse::<u64>().ok()) == Some(expected)
}

/// Read messages until the reply to `pending` arrives.
///
/// `ready` tells if the reader has data before the deadline of the request.
/// Each other message is handled here and then dropped, so no message stays in
/// a buffer:
///
/// - A notification is skipped.
/// - A request from the server gets a [`METHOD_NOT_FOUND`] error reply with
///   its own id (a number or a string), so the server does not wait for ever.
/// - A reply to an earlier request (one that timed out) is discarded.
///
/// When no data comes before the deadline, the server gets a
/// `$/cancelRequest` for the request, and the result is a timeout error.
fn read_reply<W: Write, R: BufRead>(
    writer: &mut W,
    reader: &mut R,
    pending: &PendingRequest<'_>,
    mut ready: impl FnMut(&R) -> bool,
) -> Result<Value, LspError> {
    loop {
        if !ready(reader) {
            send_cancel_request(writer, pending.id);
            return Err(LspError::JsonRpc(format!(
                "LSP request '{}' (id={}) timed out after {}s",
                pending.method,
                pending.id,
                pending.timeout.as_secs()
            )));
        }
        let message = read_jsonrpc_response(reader)?;
        match classify_response(&message, pending.id) {
            ResponseMatch::Match => return Ok(message),
            ResponseMatch::Notification => {
                trace!("Skipping LSP notification: {}", message_method(&message))
            }
            ResponseMatch::ServerRequest => answer_server_request(writer, &message),
            ResponseMatch::Stale => debug!(
                "Discarded a late LSP reply (id={}) while waiting for id={}",
                message["id"], pending.id
            ),
        }
    }
}

/// The `method` of a message, or `"unknown"` when it has none.
fn message_method(message: &Value) -> &str {
    message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

/// Tell the server to stop work on request `id`, which timed out.
///
/// A write failure is logged and not returned: the caller already reports the
/// timeout, which is the error that matters.
fn send_cancel_request<W: Write>(writer: &mut W, id: u32) {
    let cancel = json!({
        "jsonrpc": "2.0",
        "method": CANCEL_REQUEST_METHOD,
        "params": { "id": id },
    });
    match write_message(writer, &cancel) {
        Ok(()) => debug!("Sent {} for LSP request id={}", CANCEL_REQUEST_METHOD, id),
        Err(e) => warn!(
            "Failed to send {} for id={}: {}",
            CANCEL_REQUEST_METHOD, id, e
        ),
    }
}

/// Answer a request from the server with a [`METHOD_NOT_FOUND`] error.
///
/// The reply carries the id of the request without change, so a string id
/// stays a string. A write failure is logged and not returned, so the wait for
/// the pending reply continues.
fn answer_server_request<W: Write>(writer: &mut W, request: &Value) {
    let method = message_method(request);
    let reply = json!({
        "jsonrpc": "2.0",
        "id": request["id"],
        "error": {
            "code": METHOD_NOT_FOUND,
            "message": format!("method not supported by the client: {method}"),
        },
    });
    if let Err(e) = write_message(writer, &reply) {
        warn!("Failed to answer the LSP server request {}: {}", method, e);
    }
}

/// Write one JSON-RPC message with Content-Length framing, and flush it.
fn write_message<W: Write>(writer: &mut W, message: &Value) -> Result<(), LspError> {
    let body = message.to_string();
    let framed = format!("Content-Length: {}\r\n\r\n{}", body.len(), body);
    writer
        .write_all(framed.as_bytes())
        .map_err(|e| LspError::JsonRpc(format!("write failed: {}", e)))?;
    writer
        .flush()
        .map_err(|e| LspError::JsonRpc(format!("flush failed: {}", e)))
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

    #[test]
    fn parse_lsp_request_timeout_defaults_when_env_absent() {
        assert_eq!(
            parse_lsp_request_timeout(None),
            Duration::from_secs(DEFAULT_LSP_REQUEST_TIMEOUT_SECS)
        );
    }

    #[test]
    fn parse_lsp_request_timeout_honors_valid_override() {
        assert_eq!(
            parse_lsp_request_timeout(Some("120".to_string())),
            Duration::from_secs(120)
        );
    }

    #[test]
    fn parse_lsp_request_timeout_trims_whitespace() {
        assert_eq!(
            parse_lsp_request_timeout(Some("  90 \n".to_string())),
            Duration::from_secs(90)
        );
    }

    /// The request timeout that the read-loop tests give to [`read_reply`].
    const TEST_TIMEOUT: Duration = Duration::from_secs(1);

    /// Frame each message with Content-Length headers, as a server writes it.
    fn framed(messages: &[Value]) -> std::io::Cursor<Vec<u8>> {
        let mut bytes = Vec::new();
        for message in messages {
            write_message(&mut bytes, message).expect("frame a message");
        }
        std::io::Cursor::new(bytes)
    }

    /// Read back each framed message that the client wrote.
    fn written_messages(bytes: &[u8]) -> Vec<Value> {
        let mut reader = std::io::Cursor::new(bytes);
        let mut messages = Vec::new();
        while (reader.position() as usize) < bytes.len() {
            messages.push(read_jsonrpc_response(&mut reader).expect("a framed message"));
        }
        messages
    }

    /// Run [`read_reply`] for request `id` over `incoming`, with a reader that
    /// is ready while it holds data. Give the result and the written bytes.
    fn reply_for(id: u32, incoming: &[Value]) -> (Result<Value, LspError>, Vec<u8>) {
        let mut reader = framed(incoming);
        let mut writer = Vec::new();
        let pending = PendingRequest {
            method: "textDocument/hover",
            id,
            timeout: TEST_TIMEOUT,
        };
        let result = read_reply(&mut writer, &mut reader, &pending, |reader| {
            (reader.position() as usize) < reader.get_ref().len()
        });
        (result, writer)
    }

    #[test]
    fn a_timed_out_request_sends_cancel_request_with_its_id() {
        const TIMED_OUT_ID: u32 = 7;

        let (result, written) = reply_for(TIMED_OUT_ID, &[]);

        assert!(matches!(result, Err(LspError::JsonRpc(_))));
        let messages = written_messages(&written);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["method"], "$/cancelRequest");
        assert_eq!(messages[0]["params"]["id"], TIMED_OUT_ID);
        assert!(
            messages[0].get("id").is_none(),
            "a cancel is a notification"
        );
    }

    #[test]
    fn a_late_reply_to_a_timed_out_request_is_discarded() {
        let late = json!({"jsonrpc": "2.0", "id": 1, "result": "late"});
        let fresh = json!({"jsonrpc": "2.0", "id": 2, "result": "fresh"});

        let (result, written) = reply_for(2, &[late, fresh.clone()]);

        assert_eq!(result.expect("the fresh reply"), fresh);
        assert!(written.is_empty(), "a late reply gets no answer");
    }

    #[test]
    fn a_server_request_with_the_pending_id_is_not_taken_as_the_reply() {
        let server_request = json!({
            "jsonrpc": "2.0", "id": 3, "method": "workspace/configuration", "params": {}
        });
        let reply = json!({"jsonrpc": "2.0", "id": 3, "result": "reply"});

        let (result, written) = reply_for(3, &[server_request, reply.clone()]);

        assert_eq!(result.expect("the real reply"), reply);
        let answers = written_messages(&written);
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0]["id"], 3);
        assert_eq!(answers[0]["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn a_server_request_with_a_string_id_is_answered_with_that_id() {
        let server_request = json!({
            "jsonrpc": "2.0", "id": "progress-1", "method": "window/workDoneProgress/create"
        });
        let reply = json!({"jsonrpc": "2.0", "id": 4, "result": null});

        let (result, written) = reply_for(4, &[server_request, reply.clone()]);

        assert_eq!(result.expect("the real reply"), reply);
        let answers = written_messages(&written);
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0]["id"], "progress-1");
    }

    #[test]
    fn a_reply_with_the_pending_id_as_a_string_is_taken() {
        let reply = json!({"jsonrpc": "2.0", "id": "5", "result": "ok"});

        let (result, _) = reply_for(5, std::slice::from_ref(&reply));

        assert_eq!(result.expect("the string-id reply"), reply);
    }

    #[test]
    fn parse_lsp_request_timeout_falls_back_when_unparseable() {
        assert_eq!(
            parse_lsp_request_timeout(Some("not-a-number".to_string())),
            Duration::from_secs(DEFAULT_LSP_REQUEST_TIMEOUT_SECS)
        );
    }
}
