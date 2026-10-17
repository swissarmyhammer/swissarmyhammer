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
use std::io::{ErrorKind, Read, Write};
use std::path::Path;
use std::process::{ChildStdin, ChildStdout};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

use lsp_types::{DocumentSymbol, DocumentSymbolResponse, SymbolInformation};

use crate::error::LspError;
use crate::framing::{self, FrameBuffer};

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
    /// The pipes to the LSP server: its stdin to write, and its stdout to
    /// read. The raw file descriptor of stdout is polled for readability
    /// before each read (see [`stdout_ready`]).
    wire: Wire<ChildStdin, ChildStdout>,
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
            wire: Wire::new(stdin, stdout),
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
        self.wire.send(&request)
    }

    /// Read JSON-RPC messages until the reply to request `expected_id` arrives.
    ///
    /// See [`Wire::read_reply`] for how each other message is handled. The
    /// whole read is bounded by [`lsp_request_timeout`], also the read of a
    /// partial frame; on a timeout the server gets a `$/cancelRequest` for
    /// the request.
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
        self.wire
            .read_reply(&pending, |stdout| stdout_ready(stdout, deadline))
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

        self.wire.send(&notification)?;
        debug!("Sent LSP notification: {}", method);
        Ok(())
    }

    /// Read the next framed JSON-RPC message from the server's stdout.
    ///
    /// Unlike [`send_request`](Self::send_request), this does not filter by id —
    /// it returns the next notification or reply that the server sends, which
    /// lets callers drain server-initiated notifications. A request from the
    /// server is never returned: it gets a [`METHOD_NOT_FOUND`] error reply
    /// with its own id, the same as in [`send_request`](Self::send_request),
    /// and the read continues. The whole read, also of a partial frame, is
    /// bounded by [`lsp_request_timeout`].
    pub fn read_message(&mut self) -> Result<Value, LspError> {
        let timeout = lsp_request_timeout();
        let deadline = Instant::now() + timeout;
        self.wire
            .read_unsolicited(timeout, |stdout| stdout_ready(stdout, deadline))
    }

    /// Tell if the server sent bytes that put this client out of step.
    ///
    /// After a header block that is not trusted, for example a Content-Length
    /// that is too large, the position of the next frame is not known. Each
    /// later read then gives an [`LspError::OutOfStep`] error, so the owner of
    /// the client must stop the server and start it again.
    pub fn is_out_of_step(&self) -> bool {
        self.wire.is_out_of_step()
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
        self.send_notification("initialized", json!({}))?;

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
        self.send_notification(
            "textDocument/didOpen",
            json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": language_id,
                    "version": 1,
                    "text": text
                }
            }),
        )?;
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
        self.send_notification(
            "textDocument/didClose",
            json!({
                "textDocument": {
                    "uri": uri
                }
            }),
        )?;
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

        // Send exit notification (no response expected). A failure is not an
        // error: the server may already be gone.
        if let Err(e) = self.send_notification("exit", Value::Null) {
            debug!("LSP exit notification not sent: {}", e);
        }

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
/// from the pipe. The client keeps no unread bytes of its own outside the
/// [`FrameBuffer`], and [`Wire::read_frame`] reads that buffer before it asks
/// here. A `poll` error counts as no data, so the caller times out instead of
/// looping for ever.
#[cfg(unix)]
fn stdout_ready(stdout: &ChildStdout, deadline: Instant) -> bool {
    use std::os::unix::io::AsRawFd;

    let fd = stdout.as_raw_fd();
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
fn stdout_ready(_stdout: &ChildStdout, _deadline: Instant) -> bool {
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

/// The most bytes that one read from the server's stdout takes.
const READ_CHUNK_BYTES: usize = 8 * 1024;

/// Tell if `message` is a request from the server: it has a `method` and an
/// `id`.
fn is_server_request(message: &Value) -> bool {
    message.get("method").is_some() && message.get("id").is_some()
}

/// Classify one incoming message relative to the pending request `expected_id`.
fn classify_response(message: &Value, expected_id: u32) -> ResponseMatch {
    if is_server_request(message) {
        return ResponseMatch::ServerRequest;
    }
    if message.get("method").is_some() {
        return ResponseMatch::Notification;
    }
    match message.get("id") {
        Some(id) if id_matches(id, expected_id) => ResponseMatch::Match,
        _ => ResponseMatch::Stale,
    }
}

/// Why a frame read stopped with no message.
enum ReadStop {
    /// No data came before the deadline.
    TimedOut,
    /// The read failed, or the stream ended.
    Failed(LspError),
}

/// The I/O of one connection to a language server.
///
/// The client writes each framed message to `writer`. It reads raw bytes from
/// `reader` into `incoming`, and takes each complete frame from there, so a
/// read never blocks on a partial frame. The type is generic over the two
/// ends, so the read loops are unit-tested over in-memory bytes.
struct Wire<W, R> {
    /// The server's stdin.
    writer: W,
    /// The server's stdout.
    reader: R,
    /// The bytes read from `reader` that are not a message yet.
    incoming: FrameBuffer,
}

impl<W: Write, R: Read> Wire<W, R> {
    /// A wire with no unread byte.
    fn new(writer: W, reader: R) -> Self {
        Self {
            writer,
            reader,
            incoming: FrameBuffer::default(),
        }
    }

    /// Write `message` as one frame, and flush it.
    fn send(&mut self, message: &Value) -> Result<(), LspError> {
        framing::write_frame(&mut self.writer, message)
    }

    /// Tell if the server sent bytes that put the read side out of step
    /// (see [`FrameBuffer`]). The wire cannot read a message again.
    fn is_out_of_step(&self) -> bool {
        self.incoming.is_out_of_step()
    }

    /// Read the next complete message.
    ///
    /// A complete frame in `incoming` comes first. Otherwise the read takes
    /// more bytes from `reader`, but only after `ready` tells that data comes
    /// before the deadline. So a stalled server, also one that stops in the
    /// middle of a frame, stops the read at the deadline. A partial frame
    /// stays in `incoming` for the next read, so the stream stays in step.
    fn read_frame(&mut self, ready: &mut impl FnMut(&R) -> bool) -> Result<Value, ReadStop> {
        let mut chunk = [0u8; READ_CHUNK_BYTES];
        loop {
            if let Some(message) = self.incoming.take_message().map_err(ReadStop::Failed)? {
                return Ok(message);
            }
            if !ready(&self.reader) {
                return Err(ReadStop::TimedOut);
            }
            match self.reader.read(&mut chunk) {
                Ok(0) => return Err(ReadStop::Failed(self.incoming.eof_error())),
                Ok(count) => self.incoming.push(&chunk[..count]),
                Err(e) if e.kind() == ErrorKind::Interrupted => {}
                Err(e) => return Err(ReadStop::Failed(LspError::JsonRpc(format!("read: {e}")))),
            }
        }
    }

    /// Read messages until the reply to `pending` arrives.
    ///
    /// `ready` tells if the reader has data before the deadline of the
    /// request. Each other message is handled here and then dropped, so no
    /// message stays in a buffer:
    ///
    /// - A notification is skipped.
    /// - A request from the server gets a [`METHOD_NOT_FOUND`] error reply
    ///   with its own id (a number or a string), so the server does not wait
    ///   for ever.
    /// - A reply to an earlier request (one that timed out) is discarded.
    ///
    /// When no data comes before the deadline, also in the middle of a frame,
    /// the server gets a `$/cancelRequest` for the request, and the result is
    /// a timeout error.
    fn read_reply(
        &mut self,
        pending: &PendingRequest<'_>,
        mut ready: impl FnMut(&R) -> bool,
    ) -> Result<Value, LspError> {
        loop {
            let message = match self.read_frame(&mut ready) {
                Ok(message) => message,
                Err(ReadStop::Failed(e)) => return Err(e),
                Err(ReadStop::TimedOut) => {
                    self.send_cancel_request(pending.id);
                    return Err(LspError::JsonRpc(format!(
                        "LSP request '{}' (id={}) timed out after {}s",
                        pending.method,
                        pending.id,
                        pending.timeout.as_secs()
                    )));
                }
            };
            match classify_response(&message, pending.id) {
                ResponseMatch::Match => return Ok(message),
                ResponseMatch::Notification => {
                    trace!("Skipping LSP notification: {}", message_method(&message))
                }
                ResponseMatch::ServerRequest => self.answer_server_request(&message),
                ResponseMatch::Stale => debug!(
                    "Discarded a late LSP reply (id={}) while waiting for id={}",
                    message["id"], pending.id
                ),
            }
        }
    }

    /// Read the next message that is not a request from the server.
    ///
    /// Each request from the server gets a [`METHOD_NOT_FOUND`] error reply
    /// with its own id, the same as in [`read_reply`](Self::read_reply), and
    /// the read continues. When no data comes before the deadline, also in the
    /// middle of a frame, the result is a timeout error.
    fn read_unsolicited(
        &mut self,
        timeout: Duration,
        mut ready: impl FnMut(&R) -> bool,
    ) -> Result<Value, LspError> {
        loop {
            match self.read_frame(&mut ready) {
                Ok(message) if is_server_request(&message) => self.answer_server_request(&message),
                Ok(message) => return Ok(message),
                Err(ReadStop::Failed(e)) => return Err(e),
                Err(ReadStop::TimedOut) => {
                    return Err(LspError::JsonRpc(format!(
                        "LSP read_message timed out after {}s",
                        timeout.as_secs()
                    )))
                }
            }
        }
    }

    /// Tell the server to stop work on request `id`, which timed out.
    ///
    /// A write failure is logged and not returned: the caller already reports
    /// the timeout, which is the error that matters.
    fn send_cancel_request(&mut self, id: u32) {
        let cancel = json!({
            "jsonrpc": "2.0",
            "method": CANCEL_REQUEST_METHOD,
            "params": { "id": id },
        });
        match self.send(&cancel) {
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
    /// stays a string. A write failure is logged and not returned, so the read
    /// continues.
    fn answer_server_request(&mut self, request: &Value) {
        let method = message_method(request);
        let reply = json!({
            "jsonrpc": "2.0",
            "id": request["id"],
            "error": {
                "code": METHOD_NOT_FOUND,
                "message": format!("method not supported by the client: {method}"),
            },
        });
        if let Err(e) = self.send(&reply) {
            warn!("Failed to answer the LSP server request {}: {}", method, e);
        }
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

/// The `method` of a message, or `"unknown"` when it has none.
fn message_method(message: &Value) -> &str {
    message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{FakeTransport, WireMessage};
    use std::io::Cursor;

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
    fn fake_transport_records_requests_and_notifications_in_wire_order() {
        let mut transport =
            FakeTransport::default().with_fallback_response(json!({"result": null}));

        transport
            .send_notification("textDocument/didOpen", json!({}))
            .expect("notification");
        transport
            .send_request("textDocument/hover", json!({}))
            .expect("request");
        transport
            .send_notification("textDocument/didClose", json!({}))
            .expect("notification");

        assert_eq!(
            transport.wire,
            vec![
                WireMessage::Notification("textDocument/didOpen".into()),
                WireMessage::Request("textDocument/hover".into()),
                WireMessage::Notification("textDocument/didClose".into()),
            ]
        );
    }

    #[test]
    fn fake_transport_answers_by_method_then_with_the_fallback() {
        let symbols = json!({"result": ["symbol"]});
        let fallback = json!({"result": null});
        let mut transport = FakeTransport::default()
            .with_method_response("textDocument/documentSymbol", symbols.clone())
            .with_fallback_response(fallback.clone());

        let by_method = transport.send_request("textDocument/documentSymbol", json!({}));
        let other = transport.send_request("textDocument/prepareCallHierarchy", json!({}));

        assert_eq!(by_method.expect("the method answer"), symbols);
        assert_eq!(other.expect("the fallback answer"), fallback);
    }

    #[test]
    fn fake_transport_fail_requests_records_the_request_and_fails() {
        let mut transport = FakeTransport {
            fail_requests: true,
            ..FakeTransport::default().with_fallback_response(json!({"result": null}))
        };

        let err = transport
            .send_request("textDocument/documentSymbol", json!({}))
            .expect_err("a forced request failure");

        assert!(matches!(err, LspError::JsonRpc(_)));
        assert_eq!(
            transport.wire,
            vec![WireMessage::Request("textDocument/documentSymbol".into())]
        );
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

    /// The request timeout that the read-loop tests give to [`Wire::read_reply`]
    /// and [`Wire::read_unsolicited`].
    const TEST_TIMEOUT: Duration = Duration::from_secs(1);

    /// A [`Wire`] over in-memory bytes: the client writes into a `Vec`, and
    /// reads the server's bytes from a `Cursor`.
    type TestWire = Wire<Vec<u8>, Cursor<Vec<u8>>>;

    /// A wire whose server has sent `bytes`.
    fn wire_over(bytes: Vec<u8>) -> TestWire {
        Wire::new(Vec::new(), Cursor::new(bytes))
    }

    /// The reader is ready while it holds bytes that the client did not read.
    /// After the last byte, the deadline passes with no data.
    fn ready_while_data(reader: &Cursor<Vec<u8>>) -> bool {
        (reader.position() as usize) < reader.get_ref().len()
    }

    /// Frame each message with Content-Length headers, as a server writes it.
    fn framed(messages: &[Value]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for message in messages {
            framing::write_frame(&mut bytes, message).expect("frame a message");
        }
        bytes
    }

    /// Read back each framed message that the client wrote.
    fn written_messages(bytes: &[u8]) -> Vec<Value> {
        let mut frames = FrameBuffer::default();
        frames.push(bytes);
        let mut messages = Vec::new();
        while let Some(message) = frames.take_message().expect("a framed message") {
            messages.push(message);
        }
        messages
    }

    /// The pending request `id`, with the test timeout.
    fn pending(id: u32) -> PendingRequest<'static> {
        PendingRequest {
            method: "textDocument/hover",
            id,
            timeout: TEST_TIMEOUT,
        }
    }

    /// Run [`Wire::read_reply`] for request `id` over `incoming`, with a reader
    /// that is ready while it holds data. Give the result and the written bytes.
    fn reply_for(id: u32, incoming: &[Value]) -> (Result<Value, LspError>, Vec<u8>) {
        let mut wire = wire_over(framed(incoming));
        let result = wire.read_reply(&pending(id), ready_while_data);
        (result, wire.writer)
    }

    /// The first `cut` bytes of the frame of `message`, as a server writes
    /// them before it stalls.
    fn half_frame(message: &Value, cut: usize) -> Vec<u8> {
        let mut bytes = framed(std::slice::from_ref(message));
        bytes.truncate(cut);
        bytes
    }

    #[test]
    fn a_partial_frame_still_times_out_and_cancels() {
        const STALLED_ID: u32 = 8;
        let reply = json!({"jsonrpc": "2.0", "id": STALLED_ID, "result": "late"});
        let header_and_part_of_the_body = framing::encode(&reply).unwrap().header.len() + 4;
        let mut wire = wire_over(half_frame(&reply, header_and_part_of_the_body));

        let result = wire.read_reply(&pending(STALLED_ID), ready_while_data);

        assert!(matches!(result, Err(LspError::JsonRpc(_))));
        let messages = written_messages(&wire.writer);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["method"], "$/cancelRequest");
        assert_eq!(messages[0]["params"]["id"], STALLED_ID);
    }

    #[test]
    fn a_partial_frame_completes_on_a_later_read() {
        const HEADER_ONLY_BYTES: usize = 10;
        let first = json!({"jsonrpc": "2.0", "id": 1, "result": "first"});
        let second = json!({"jsonrpc": "2.0", "id": 2, "result": "second"});
        let whole = framed(&[first, second.clone()]);
        let mut wire = wire_over(whole[..HEADER_ONLY_BYTES].to_vec());
        let timed_out = wire.read_reply(&pending(1), ready_while_data);
        wire.reader
            .get_mut()
            .extend_from_slice(&whole[HEADER_ONLY_BYTES..]);

        let result = wire.read_reply(&pending(2), ready_while_data);

        assert!(timed_out.is_err());
        assert_eq!(result.expect("the second reply"), second);
    }

    #[test]
    fn a_content_length_larger_than_the_maximum_puts_the_wire_out_of_step() {
        let reply = json!({"jsonrpc": "2.0", "id": 1, "result": "after"});
        let mut bytes = b"Content-Length: 18446744073709551615\r\n\r\n".to_vec();
        bytes.extend(framed(&[reply]));
        let mut wire = wire_over(bytes);

        let first = wire.read_reply(&pending(1), ready_while_data);
        let second = wire.read_unsolicited(TEST_TIMEOUT, ready_while_data);

        assert!(
            matches!(first, Err(LspError::OutOfStep(_))),
            "got: {first:?}"
        );
        assert!(
            matches!(second, Err(LspError::OutOfStep(_))),
            "got: {second:?}"
        );
        assert!(wire.is_out_of_step());
        assert!(
            wire.writer.is_empty(),
            "no cancel goes to an out-of-step server"
        );
    }

    #[test]
    fn read_message_answers_a_server_request_and_gives_the_next_message() {
        let server_request = json!({
            "jsonrpc": "2.0", "id": "config-1", "method": "workspace/configuration", "params": {}
        });
        let diagnostics = json!({
            "jsonrpc": "2.0", "method": "textDocument/publishDiagnostics", "params": {}
        });
        let mut wire = wire_over(framed(&[server_request, diagnostics.clone()]));

        let message = wire.read_unsolicited(TEST_TIMEOUT, ready_while_data);

        assert_eq!(message.expect("the notification"), diagnostics);
        let answers = written_messages(&wire.writer);
        assert_eq!(answers.len(), 1);
        assert_eq!(answers[0]["id"], "config-1");
        assert_eq!(answers[0]["error"]["code"], METHOD_NOT_FOUND);
    }

    #[test]
    fn read_message_times_out_on_a_partial_frame() {
        const HEADER_ONLY_BYTES: usize = 10;
        let notification = json!({"jsonrpc": "2.0", "method": "window/logMessage"});
        let mut wire = wire_over(half_frame(&notification, HEADER_ONLY_BYTES));

        let result = wire.read_unsolicited(TEST_TIMEOUT, ready_while_data);

        assert!(matches!(result, Err(LspError::JsonRpc(_))));
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
