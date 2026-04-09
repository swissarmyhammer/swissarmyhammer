//! LSP JSON-RPC communication and symbol collection.
//!
//! Handles JSON-RPC protocol with LSP server processes.
//! Sends requests for symbols and collects results for database persistence.

use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{ChildStdin, ChildStdout};
use std::time::{Duration, Instant};
use tracing::{debug, trace, warn};

/// Timeout for a single LSP request/response round-trip.
///
/// If the LSP server does not produce a matching response within this window
/// (e.g., it silently ignores the request), `send_request` returns an error
/// instead of blocking the worker forever.
const LSP_REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

use crate::error::CodeContextError;
use crate::lsp_indexer::{flatten_symbols, mark_lsp_indexed, write_edges, write_symbols, CallEdge};
use lsp_types::{
    CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSymbol, DocumentSymbolResponse,
    SymbolInformation,
};

/// Result of collecting symbols from LSP server for a file.
#[derive(Debug)]
pub struct LspCollectionResult {
    /// File path that was processed
    pub file_path: String,
    /// Number of symbols collected
    pub symbol_count: usize,
    /// Any error that occurred
    pub error: Option<String>,
}

/// Collect and persist LSP symbols for a file.
///
/// This is a simplified version that takes a list of DocumentSymbols
/// (e.g., from an LSP server response) and persists them to the database.
///
/// # Arguments
/// * `conn` - Database connection
/// * `file_path` - Path to the file
/// * `symbols` - DocumentSymbols from LSP server
///
/// # Returns
/// Number of symbols written to database
pub fn collect_and_persist_symbols(
    conn: &Connection,
    file_path: &str,
    symbols: &[DocumentSymbol],
) -> Result<usize, CodeContextError> {
    // Flatten nested DocumentSymbols into FlatSymbol format
    let flat_symbols = flatten_symbols(file_path, symbols);
    let symbol_count = flat_symbols.len();

    // Write symbols to database
    if symbol_count > 0 {
        write_symbols(conn, file_path, &flat_symbols)?;
    }

    // Mark file as lsp_indexed
    mark_lsp_indexed(conn, file_path)?;

    debug!(
        "Collected and persisted {} symbols for {}",
        symbol_count, file_path
    );
    Ok(symbol_count)
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
pub fn parse_document_symbols(response: &Value) -> Result<Vec<DocumentSymbol>, CodeContextError> {
    // Check for JSON-RPC error first
    if let Some(error) = response.get("error") {
        return Err(CodeContextError::LspError(format!("LSP error: {}", error)));
    }

    // Extract the "result" field from the JSON-RPC response
    let result = match response.get("result") {
        Some(Value::Array(arr)) => arr,
        Some(Value::Null) | None => return Ok(Vec::new()),
        Some(other) => {
            return Err(CodeContextError::LspError(format!(
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
            CodeContextError::LspError(format!("failed to parse documentSymbol response: {}", e))
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
    pub fn send_request(&mut self, method: &str, params: Value) -> Result<Value, CodeContextError> {
        // Format JSON-RPC 2.0 request
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": self.request_id,
        });

        let expected_id = self.request_id;
        self.request_id += 1;

        // Encode with Content-Length header
        let json_str = request.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        // Write request
        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| CodeContextError::LspError(format!("write failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| CodeContextError::LspError(format!("flush failed: {}", e)))?;

        debug!("Sent LSP request: {} (id={})", method, expected_id);

        // Read response — loop to skip notifications (no "id" field).
        // Each iteration polls the fd for readability before attempting a
        // blocking read, enforcing LSP_REQUEST_TIMEOUT across the entire loop.
        let deadline = Instant::now() + LSP_REQUEST_TIMEOUT;

        loop {
            // Wait for data to be available, respecting the deadline
            self.wait_for_readable(deadline).map_err(|_| {
                CodeContextError::LspError(format!(
                    "LSP request '{}' (id={}) timed out after {}s",
                    method,
                    expected_id,
                    LSP_REQUEST_TIMEOUT.as_secs()
                ))
            })?;

            let response = read_jsonrpc_response(&mut self.reader)?;

            // Notifications have no "id" field — skip them
            if response.get("id").is_none() {
                trace!(
                    "Skipping LSP notification: {}",
                    response
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("unknown")
                );
                continue;
            }

            // Verify response ID matches
            if let Some(id) = response.get("id") {
                if id.as_u64() == Some(expected_id as u64) {
                    return Ok(response);
                }
                // Server-initiated requests or stale responses — skip and keep reading
                warn!(
                    "Unexpected response id: expected {}, got {} (method: {}). Skipping.",
                    expected_id,
                    id,
                    response
                        .get("method")
                        .and_then(|v| v.as_str())
                        .unwrap_or("none")
                );
                continue;
            }
        }
    }

    /// Send a JSON-RPC notification (fire-and-forget, no response expected).
    ///
    /// Per the JSON-RPC 2.0 spec, notifications omit the `id` field and the
    /// server must not reply. This avoids the timeout penalty of `send_request`
    /// for methods like `textDocument/didOpen` and `textDocument/didClose`.
    pub fn send_notification(
        &mut self,
        method: &str,
        params: Value,
    ) -> Result<(), CodeContextError> {
        let notification = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
        });

        let json_str = notification.to_string();
        let msg = format!("Content-Length: {}\r\n\r\n{}", json_str.len(), json_str);

        self.stdin
            .write_all(msg.as_bytes())
            .map_err(|e| CodeContextError::LspError(format!("write notification failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| CodeContextError::LspError(format!("flush notification failed: {}", e)))?;

        debug!("Sent LSP notification: {}", method);
        Ok(())
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

    /// Collect symbols from the LSP server for a given file.
    ///
    /// Sends textDocument/documentSymbol request and parses response.
    pub fn collect_file_symbols(
        &mut self,
        file_path: &Path,
    ) -> Result<LspCollectionResult, CodeContextError> {
        let file_path_str = file_path.to_string_lossy().to_string();

        // Build file URI for LSP (file:///absolute/path)
        let uri = format!("file://{}", file_path_str);

        debug!("Collecting symbols for {}", file_path_str);

        // Send textDocument/documentSymbol request
        let params = json!({
            "textDocument": {
                "uri": uri
            }
        });

        match self.send_request("textDocument/documentSymbol", params) {
            Ok(response) => match parse_document_symbols(&response) {
                Ok(symbols) => {
                    let symbol_count = count_symbols_recursive(&symbols);
                    Ok(LspCollectionResult {
                        file_path: file_path_str,
                        symbol_count,
                        error: None,
                    })
                }
                Err(e) => Ok(LspCollectionResult {
                    file_path: file_path_str,
                    symbol_count: 0,
                    error: Some(e.to_string()),
                }),
            },
            Err(e) => Ok(LspCollectionResult {
                file_path: file_path_str,
                symbol_count: 0,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Collect symbols and persist them to the database.
    ///
    /// Combines `collect_file_symbols` with database writes.
    pub fn collect_and_persist_file_symbols(
        &mut self,
        conn: &Connection,
        file_path: &Path,
        relative_path: &str,
    ) -> Result<LspCollectionResult, CodeContextError> {
        let file_path_str = file_path.to_string_lossy().to_string();
        let uri = format!("file://{}", file_path_str);

        let params = json!({
            "textDocument": {
                "uri": uri
            }
        });

        match self.send_request("textDocument/documentSymbol", params) {
            Ok(response) => match parse_document_symbols(&response) {
                Ok(symbols) => {
                    let count = collect_and_persist_symbols(conn, relative_path, &symbols)?;
                    Ok(LspCollectionResult {
                        file_path: file_path_str,
                        symbol_count: count,
                        error: None,
                    })
                }
                Err(e) => Ok(LspCollectionResult {
                    file_path: file_path_str,
                    symbol_count: 0,
                    error: Some(e.to_string()),
                }),
            },
            Err(e) => Ok(LspCollectionResult {
                file_path: file_path_str,
                symbol_count: 0,
                error: Some(e.to_string()),
            }),
        }
    }

    /// Initialize the LSP server and wait for initialization response.
    pub fn initialize(&mut self, workspace_root: &Path) -> Result<(), CodeContextError> {
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
            return Err(CodeContextError::LspError(format!(
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
            .map_err(|e| CodeContextError::LspError(format!("write initialized failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| CodeContextError::LspError(format!("flush initialized failed: {}", e)))?;

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
    ) -> Result<(), CodeContextError> {
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
            .map_err(|e| CodeContextError::LspError(format!("write didOpen failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| CodeContextError::LspError(format!("flush didOpen failed: {}", e)))?;

        debug!("Sent textDocument/didOpen for {}", file_path.display());
        Ok(())
    }

    /// Send a `textDocument/didClose` notification to the LSP server.
    ///
    /// This informs the server that the client is no longer interested in
    /// the document. Should be called after indexing to avoid "duplicate
    /// didOpen" warnings on re-indexing.
    pub fn send_did_close(&mut self, file_path: &Path) -> Result<(), CodeContextError> {
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
            .map_err(|e| CodeContextError::LspError(format!("write didClose failed: {}", e)))?;
        self.stdin
            .flush()
            .map_err(|e| CodeContextError::LspError(format!("flush didClose failed: {}", e)))?;

        debug!("Sent textDocument/didClose for {}", file_path.display());
        Ok(())
    }

    /// Send the LSP `shutdown` request followed by `exit` notification.
    ///
    /// This cleanly terminates the LSP server. The caller is responsible for
    /// waiting on the `Child` process handle after calling this method.
    pub fn shutdown(mut self) -> Result<(), CodeContextError> {
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

    /// Collect outgoing call edges for a file using LSP call hierarchy.
    ///
    /// For each function/method symbol, prepares a call hierarchy item,
    /// then queries outgoing calls. Returns edges suitable for `write_edges`.
    pub fn collect_call_edges(
        &mut self,
        file_path: &Path,
        relative_path: &str,
    ) -> Result<Vec<CallEdge>, CodeContextError> {
        let file_path_str = file_path.to_string_lossy().to_string();
        let uri = format!("file://{}", file_path_str);

        // First get document symbols to find function/method positions
        let symbol_params = json!({
            "textDocument": { "uri": &uri }
        });

        let symbol_response = self.send_request("textDocument/documentSymbol", symbol_params)?;
        let symbols = parse_document_symbols(&symbol_response)?;
        let flat = flatten_symbols(relative_path, &symbols);

        let mut all_edges = Vec::new();

        // For each callable symbol, prepare call hierarchy and get outgoing calls
        for sym in &flat {
            use lsp_types::SymbolKind;
            match sym.kind {
                SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => {}
                _ => continue,
            }

            // Prepare call hierarchy at the symbol's position
            let prepare_params = json!({
                "textDocument": { "uri": &uri },
                "position": { "line": sym.start_line, "character": sym.start_char }
            });

            let prepare_response =
                match self.send_request("textDocument/prepareCallHierarchy", prepare_params) {
                    Ok(r) => r,
                    Err(_) => continue, // Server may not support call hierarchy
                };

            let items = parse_call_hierarchy_items(&prepare_response)?;
            if items.is_empty() {
                continue;
            }

            // Get outgoing calls for the first (primary) item
            let outgoing_params = json!({
                "item": serde_json::to_value(&items[0])
                    .map_err(|e| CodeContextError::LspError(format!("serialize item: {}", e)))?
            });

            let outgoing_response =
                match self.send_request("callHierarchy/outgoingCalls", outgoing_params) {
                    Ok(r) => r,
                    Err(_) => continue,
                };

            let outgoing_calls = parse_outgoing_calls(&outgoing_response)?;

            for call in &outgoing_calls {
                let callee_file = uri_to_relative_path(call.to.uri.as_str(), file_path);
                let callee_qpath = call.to.name.clone();
                let callee_id = format!("lsp:{}:{}", callee_file, callee_qpath);

                let from_ranges_json =
                    serde_json::to_string(&call.from_ranges).unwrap_or_else(|_| "[]".to_string());

                all_edges.push(CallEdge {
                    caller_id: sym.id.clone(),
                    callee_id,
                    caller_file: relative_path.to_string(),
                    callee_file,
                    from_ranges: from_ranges_json,
                    source: "lsp".to_string(),
                });
            }
        }

        debug!(
            "Collected {} LSP call edges for {}",
            all_edges.len(),
            relative_path
        );
        Ok(all_edges)
    }

    /// Collect call edges and persist them to the database.
    pub fn collect_and_persist_call_edges(
        &mut self,
        conn: &Connection,
        file_path: &Path,
        relative_path: &str,
    ) -> Result<usize, CodeContextError> {
        let edges = self.collect_call_edges(file_path, relative_path)?;
        if edges.is_empty() {
            return Ok(0);
        }
        write_edges(conn, relative_path, &edges)
    }
}

/// Parse a `textDocument/prepareCallHierarchy` response into CallHierarchyItem array.
pub fn parse_call_hierarchy_items(
    response: &Value,
) -> Result<Vec<CallHierarchyItem>, CodeContextError> {
    if let Some(error) = response.get("error") {
        return Err(CodeContextError::LspError(format!("LSP error: {}", error)));
    }

    match response.get("result") {
        Some(Value::Array(arr)) => serde_json::from_value(Value::Array(arr.clone()))
            .map_err(|e| CodeContextError::LspError(format!("parse CallHierarchyItem: {}", e))),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(other) => Err(CodeContextError::LspError(format!(
            "unexpected prepareCallHierarchy result: {}",
            other
        ))),
    }
}

/// Parse a `callHierarchy/outgoingCalls` response.
pub fn parse_outgoing_calls(
    response: &Value,
) -> Result<Vec<CallHierarchyOutgoingCall>, CodeContextError> {
    if let Some(error) = response.get("error") {
        return Err(CodeContextError::LspError(format!("LSP error: {}", error)));
    }

    match response.get("result") {
        Some(Value::Array(arr)) => serde_json::from_value(Value::Array(arr.clone()))
            .map_err(|e| CodeContextError::LspError(format!("parse OutgoingCalls: {}", e))),
        Some(Value::Null) | None => Ok(Vec::new()),
        Some(other) => Err(CodeContextError::LspError(format!(
            "unexpected outgoingCalls result: {}",
            other
        ))),
    }
}

/// Convert a file:// URI to a relative path, falling back to the URI path.
fn uri_to_relative_path(uri: &str, reference_path: &Path) -> String {
    let path_str = uri.strip_prefix("file://").unwrap_or(uri);
    let abs_path = std::path::Path::new(path_str);

    // Try to find a common ancestor to make it relative
    if let Some(parent) = reference_path.parent() {
        // Walk up to find workspace root (heuristic: look for common prefix)
        let mut ancestor = parent;
        loop {
            if let Ok(rel) = abs_path.strip_prefix(ancestor) {
                return rel.to_string_lossy().to_string();
            }
            match ancestor.parent() {
                Some(p) => ancestor = p,
                None => break,
            }
        }
    }

    path_str.to_string()
}

/// Count symbols recursively including children.
fn count_symbols_recursive(symbols: &[DocumentSymbol]) -> usize {
    symbols
        .iter()
        .map(|s| {
            1 + s
                .children
                .as_ref()
                .map_or(0, |c| count_symbols_recursive(c))
        })
        .sum()
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

/// Read a single JSON-RPC message from a reader using Content-Length framing.
fn read_jsonrpc_response<R: BufRead>(reader: &mut R) -> Result<Value, CodeContextError> {
    let mut content_length: Option<usize> = None;

    // Read headers until blank line
    loop {
        let mut line = String::new();
        let n = reader
            .read_line(&mut line)
            .map_err(|e| CodeContextError::LspError(format!("read header: {}", e)))?;
        if n == 0 {
            return Err(CodeContextError::LspError(
                "unexpected EOF reading headers".into(),
            ));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            break;
        }

        if let Some(val) = trimmed.strip_prefix("Content-Length:") {
            content_length =
                Some(val.trim().parse::<usize>().map_err(|e| {
                    CodeContextError::LspError(format!("bad Content-Length: {}", e))
                })?);
        }
    }

    let length = content_length
        .ok_or_else(|| CodeContextError::LspError("missing Content-Length header".into()))?;

    let mut body = vec![0u8; length];
    reader
        .read_exact(&mut body)
        .map_err(|e| CodeContextError::LspError(format!("read body: {}", e)))?;

    serde_json::from_slice(&body)
        .map_err(|e| CodeContextError::LspError(format!("json decode: {}", e)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range, SymbolKind};

    #[test]
    fn test_lsp_collection_result() {
        let result = LspCollectionResult {
            file_path: "src/main.rs".to_string(),
            symbol_count: 5,
            error: None,
        };
        assert_eq!(result.file_path, "src/main.rs");
        assert_eq!(result.symbol_count, 5);
        assert!(result.error.is_none());
    }

    #[test]
    fn test_lsp_collection_with_error() {
        let result = LspCollectionResult {
            file_path: "src/bad.rs".to_string(),
            symbol_count: 0,
            error: Some("timeout".to_string()),
        };
        assert_eq!(result.file_path, "src/bad.rs");
        assert!(result.error.is_some());
    }

    #[test]
    fn test_parse_document_symbols_from_jsonrpc_response() {
        // Simulate a real LSP JSON-RPC response with hierarchical DocumentSymbol[]
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "MyStruct",
                    "detail": "pub struct",
                    "kind": 23,  // SymbolKind::STRUCT
                    "range": {
                        "start": {"line": 0, "character": 0},
                        "end": {"line": 20, "character": 1}
                    },
                    "selectionRange": {
                        "start": {"line": 0, "character": 11},
                        "end": {"line": 0, "character": 19}
                    },
                    "children": [
                        {
                            "name": "field1",
                            "kind": 8,  // SymbolKind::FIELD
                            "range": {
                                "start": {"line": 1, "character": 4},
                                "end": {"line": 1, "character": 20}
                            },
                            "selectionRange": {
                                "start": {"line": 1, "character": 4},
                                "end": {"line": 1, "character": 10}
                            }
                        },
                        {
                            "name": "new",
                            "detail": "pub fn",
                            "kind": 6,  // SymbolKind::METHOD
                            "range": {
                                "start": {"line": 5, "character": 4},
                                "end": {"line": 10, "character": 5}
                            },
                            "selectionRange": {
                                "start": {"line": 5, "character": 11},
                                "end": {"line": 5, "character": 14}
                            }
                        }
                    ]
                },
                {
                    "name": "main",
                    "kind": 12,  // SymbolKind::FUNCTION
                    "range": {
                        "start": {"line": 22, "character": 0},
                        "end": {"line": 30, "character": 1}
                    },
                    "selectionRange": {
                        "start": {"line": 22, "character": 3},
                        "end": {"line": 22, "character": 7}
                    }
                }
            ]
        });

        let symbols = parse_document_symbols(&response).unwrap();
        assert_eq!(symbols.len(), 2, "Should have 2 top-level symbols");

        // Verify first symbol (MyStruct)
        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::STRUCT);
        assert_eq!(symbols[0].detail, Some("pub struct".to_string()));
        assert_eq!(symbols[0].range.start.line, 0);
        assert_eq!(symbols[0].range.end.line, 20);

        // Verify children
        let children = symbols[0].children.as_ref().unwrap();
        assert_eq!(children.len(), 2);
        assert_eq!(children[0].name, "field1");
        assert_eq!(children[0].kind, SymbolKind::FIELD);
        assert_eq!(children[1].name, "new");
        assert_eq!(children[1].kind, SymbolKind::METHOD);

        // Verify second symbol (main)
        assert_eq!(symbols[1].name, "main");
        assert_eq!(symbols[1].kind, SymbolKind::FUNCTION);
        assert!(symbols[1].children.is_none());
    }

    #[test]
    fn test_parse_document_symbols_null_result() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": null
        });
        let symbols = parse_document_symbols(&response).unwrap();
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_parse_document_symbols_empty_array() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });
        let symbols = parse_document_symbols(&response).unwrap();
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_parse_document_symbols_error_response() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {
                "code": -32600,
                "message": "Invalid Request"
            }
        });
        let result = parse_document_symbols(&response);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_document_symbols_flat_symbol_information() {
        // Simulate a response with SymbolInformation[] (legacy flat format)
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "MyStruct",
                    "kind": 23,  // SymbolKind::STRUCT
                    "location": {
                        "uri": "file:///workspace/src/lib.rs",
                        "range": {
                            "start": {"line": 0, "character": 0},
                            "end": {"line": 10, "character": 1}
                        }
                    }
                },
                {
                    "name": "my_fn",
                    "kind": 12,  // SymbolKind::FUNCTION
                    "location": {
                        "uri": "file:///workspace/src/lib.rs",
                        "range": {
                            "start": {"line": 12, "character": 0},
                            "end": {"line": 20, "character": 1}
                        }
                    },
                    "containerName": "MyStruct"
                }
            ]
        });

        let symbols = parse_document_symbols(&response).unwrap();
        assert_eq!(symbols.len(), 2);

        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, SymbolKind::STRUCT);
        assert_eq!(symbols[0].range.start.line, 0);
        assert_eq!(symbols[0].range.end.line, 10);
        assert!(symbols[0].children.is_none());

        assert_eq!(symbols[1].name, "my_fn");
        assert_eq!(symbols[1].kind, SymbolKind::FUNCTION);
        // container_name is mapped to detail
        assert_eq!(symbols[1].detail, Some("MyStruct".to_string()));
        assert_eq!(symbols[1].range.start.line, 12);
    }

    #[test]
    fn test_count_symbols_recursive() {
        #[allow(deprecated)]
        let symbols = vec![DocumentSymbol {
            name: "Outer".to_string(),
            detail: None,
            kind: SymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            selection_range: Range::new(Position::new(0, 0), Position::new(0, 5)),
            children: Some(vec![
                DocumentSymbol {
                    name: "inner1".to_string(),
                    detail: None,
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(Position::new(1, 0), Position::new(3, 0)),
                    selection_range: Range::new(Position::new(1, 0), Position::new(1, 6)),
                    children: None,
                },
                DocumentSymbol {
                    name: "inner2".to_string(),
                    detail: None,
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(Position::new(4, 0), Position::new(6, 0)),
                    selection_range: Range::new(Position::new(4, 0), Position::new(4, 6)),
                    children: None,
                },
            ]),
        }];
        assert_eq!(count_symbols_recursive(&symbols), 3);
    }

    #[test]
    fn test_read_jsonrpc_response() {
        let body = json!({"jsonrpc": "2.0", "id": 1, "result": []});
        let body_str = body.to_string();
        let message = format!("Content-Length: {}\r\n\r\n{}", body_str.len(), body_str);

        let mut reader = std::io::BufReader::new(message.as_bytes());
        let response = read_jsonrpc_response(&mut reader).unwrap();

        assert_eq!(response["jsonrpc"], "2.0");
        assert_eq!(response["id"], 1);
    }

    #[test]
    fn test_read_jsonrpc_response_with_extra_headers() {
        let body = json!({"jsonrpc": "2.0", "id": 2, "result": null});
        let body_str = body.to_string();
        let message = format!(
            "Content-Length: {}\r\nContent-Type: application/vscode-jsonrpc; charset=utf-8\r\n\r\n{}",
            body_str.len(), body_str
        );

        let mut reader = std::io::BufReader::new(message.as_bytes());
        let response = read_jsonrpc_response(&mut reader).unwrap();

        assert_eq!(response["id"], 2);
    }

    #[test]
    fn test_read_jsonrpc_response_missing_content_length() {
        let message = b"SomeHeader: value\r\n\r\n{}";
        let mut reader = std::io::BufReader::new(&message[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_and_flatten_real_response() {
        // End-to-end: parse response → flatten → verify qualified paths
        let response = json!({
            "jsonrpc": "2.0",
            "id": 5,
            "result": [
                {
                    "name": "Config",
                    "kind": 23,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 15, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 17}},
                    "children": [
                        {
                            "name": "load",
                            "kind": 6,
                            "range": {"start": {"line": 3, "character": 4}, "end": {"line": 8, "character": 5}},
                            "selectionRange": {"start": {"line": 3, "character": 11}, "end": {"line": 3, "character": 15}}
                        },
                        {
                            "name": "save",
                            "kind": 6,
                            "range": {"start": {"line": 10, "character": 4}, "end": {"line": 14, "character": 5}},
                            "selectionRange": {"start": {"line": 10, "character": 11}, "end": {"line": 10, "character": 15}}
                        }
                    ]
                }
            ]
        });

        let symbols = parse_document_symbols(&response).unwrap();
        let flat = crate::lsp_indexer::flatten_symbols("src/config.rs", &symbols);

        assert_eq!(flat.len(), 3);
        assert_eq!(flat[0].qualified_path, "Config");
        assert_eq!(flat[0].id, "lsp:src/config.rs:Config");
        assert_eq!(flat[1].qualified_path, "Config::load");
        assert_eq!(flat[1].id, "lsp:src/config.rs:Config::load");
        assert_eq!(flat[2].qualified_path, "Config::save");
        assert_eq!(flat[2].id, "lsp:src/config.rs:Config::save");
    }

    #[test]
    fn test_parse_call_hierarchy_items() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": [
                {
                    "name": "process",
                    "kind": 12,
                    "uri": "file:///workspace/src/main.rs",
                    "range": {"start": {"line": 5, "character": 0}, "end": {"line": 15, "character": 1}},
                    "selectionRange": {"start": {"line": 5, "character": 3}, "end": {"line": 5, "character": 10}}
                }
            ]
        });

        let items = parse_call_hierarchy_items(&response).unwrap();
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "process");
        assert_eq!(items[0].kind, SymbolKind::FUNCTION);
        assert_eq!(items[0].uri.as_str(), "file:///workspace/src/main.rs");
    }

    #[test]
    fn test_parse_call_hierarchy_items_null() {
        let response = json!({"jsonrpc": "2.0", "id": 3, "result": null});
        let items = parse_call_hierarchy_items(&response).unwrap();
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_outgoing_calls() {
        let response = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "result": [
                {
                    "to": {
                        "name": "helper",
                        "kind": 12,
                        "uri": "file:///workspace/src/utils.rs",
                        "range": {"start": {"line": 10, "character": 0}, "end": {"line": 20, "character": 1}},
                        "selectionRange": {"start": {"line": 10, "character": 3}, "end": {"line": 10, "character": 9}}
                    },
                    "fromRanges": [
                        {"start": {"line": 8, "character": 4}, "end": {"line": 8, "character": 10}}
                    ]
                },
                {
                    "to": {
                        "name": "init",
                        "kind": 12,
                        "uri": "file:///workspace/src/lib.rs",
                        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
                        "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 7}}
                    },
                    "fromRanges": [
                        {"start": {"line": 6, "character": 4}, "end": {"line": 6, "character": 8}},
                        {"start": {"line": 12, "character": 4}, "end": {"line": 12, "character": 8}}
                    ]
                }
            ]
        });

        let calls = parse_outgoing_calls(&response).unwrap();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].to.name, "helper");
        assert_eq!(calls[0].from_ranges.len(), 1);
        assert_eq!(calls[1].to.name, "init");
        assert_eq!(calls[1].from_ranges.len(), 2);
    }

    #[test]
    fn test_parse_outgoing_calls_empty() {
        let response = json!({"jsonrpc": "2.0", "id": 4, "result": []});
        let calls = parse_outgoing_calls(&response).unwrap();
        assert!(calls.is_empty());
    }

    #[test]
    fn test_uri_to_relative_path() {
        let ref_path = std::path::Path::new("/workspace/src/main.rs");

        // Same directory — strips from nearest common parent (/workspace/src/)
        let rel = uri_to_relative_path("file:///workspace/src/utils.rs", ref_path);
        assert_eq!(rel, "utils.rs");

        // Different subdirectory — strips from /workspace/
        let rel = uri_to_relative_path("file:///workspace/lib/helper.rs", ref_path);
        assert_eq!(rel, "lib/helper.rs");

        // Different root — strips from / (always a common ancestor on unix)
        let rel = uri_to_relative_path("file:///other/project/foo.rs", ref_path);
        assert_eq!(rel, "other/project/foo.rs");
    }

    #[cfg(unix)]
    #[test]
    fn test_poll_fd_timeout() {
        // Verify that poll_fd returns 0 (timeout) on a pipe with no data.
        use std::os::unix::io::AsRawFd;

        let (reader, _writer) = std::os::unix::net::UnixStream::pair().unwrap();
        let fd = reader.as_raw_fd();

        let start = Instant::now();
        let ret = poll_fd(fd, 50); // 50ms timeout
        let elapsed = start.elapsed();

        assert_eq!(ret, 0, "poll should return 0 on timeout");
        assert!(
            elapsed >= Duration::from_millis(40),
            "poll should have waited ~50ms, took {:?}",
            elapsed
        );
        assert!(
            elapsed < Duration::from_secs(2),
            "poll took too long: {:?}",
            elapsed
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_poll_fd_ready() {
        // Verify that poll_fd returns > 0 when data is available.
        use std::os::unix::io::AsRawFd;

        let (reader, mut writer) = std::os::unix::net::UnixStream::pair().unwrap();
        // Write some data so the read end is immediately ready.
        std::io::Write::write_all(&mut writer, b"hello").unwrap();

        let fd = reader.as_raw_fd();
        let ret = poll_fd(fd, 1000);
        assert!(ret > 0, "poll should return >0 when data is available");
    }

    // ---------------------------------------------------------------------------
    // Tests for collect_and_persist_symbols (standalone DB function)
    // ---------------------------------------------------------------------------

    /// Open an in-memory SQLite DB with the code-context schema.
    fn open_test_db() -> rusqlite::Connection {
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::configure_connection(&conn).unwrap();
        crate::db::create_schema(&conn).unwrap();
        conn
    }

    /// Insert a placeholder row into `indexed_files` so FK constraints pass.
    fn seed_indexed_file(conn: &rusqlite::Connection, path: &str) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'deadbeef', 512, 999)",
            [path],
        )
        .unwrap();
    }

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_symbols_writes_to_db() {
        // Verify that collect_and_persist_symbols stores symbols and marks lsp_indexed.
        let conn = open_test_db();
        let file_path = "src/demo.rs";
        seed_indexed_file(&conn, file_path);

        let symbols = vec![
            DocumentSymbol {
                name: "Demo".to_string(),
                detail: None,
                kind: SymbolKind::STRUCT,
                tags: None,
                deprecated: None,
                range: Range::new(Position::new(0, 0), Position::new(10, 1)),
                selection_range: Range::new(Position::new(0, 0), Position::new(0, 4)),
                children: None,
            },
            DocumentSymbol {
                name: "run".to_string(),
                detail: None,
                kind: SymbolKind::FUNCTION,
                tags: None,
                deprecated: None,
                range: Range::new(Position::new(12, 0), Position::new(20, 1)),
                selection_range: Range::new(Position::new(12, 0), Position::new(12, 3)),
                children: None,
            },
        ];

        let count = collect_and_persist_symbols(&conn, file_path, &symbols).unwrap();
        assert_eq!(count, 2, "should have written 2 symbols");

        // Verify lsp_indexed flag was set
        let lsp_indexed: i32 = conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(lsp_indexed, 1, "lsp_indexed should be 1 after persist");
    }

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_symbols_empty_writes_nothing_but_marks_indexed() {
        // Even with no symbols, the file should be marked lsp_indexed.
        let conn = open_test_db();
        let file_path = "src/empty.rs";
        seed_indexed_file(&conn, file_path);

        let count = collect_and_persist_symbols(&conn, file_path, &[]).unwrap();
        assert_eq!(count, 0);

        let lsp_indexed: i32 = conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            lsp_indexed, 1,
            "lsp_indexed should still be 1 for empty file"
        );
    }

    // ---------------------------------------------------------------------------
    // Tests for read_jsonrpc_response edge cases
    // ---------------------------------------------------------------------------

    #[test]
    fn test_read_jsonrpc_response_eof_on_empty_input() {
        // An empty reader triggers EOF while reading headers.
        let input: &[u8] = b"";
        let mut reader = std::io::BufReader::new(input);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error on empty input (EOF)");
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("EOF") || err.contains("header"),
            "error should mention EOF or header, got: {}",
            err
        );
    }

    #[test]
    fn test_read_jsonrpc_response_eof_during_body() {
        // Content-Length claims 100 bytes but body is truncated.
        let message = b"Content-Length: 100\r\n\r\nshort";
        let mut reader = std::io::BufReader::new(&message[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error when body is truncated");
    }

    #[test]
    fn test_read_jsonrpc_response_invalid_json_body() {
        // Body is not valid JSON.
        let body = b"not valid json!!!";
        let message = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut data = message.into_bytes();
        data.extend_from_slice(body);
        let mut reader = std::io::BufReader::new(&data[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error on invalid JSON body");
    }

    // ---------------------------------------------------------------------------
    // Tests for parse_call_hierarchy_items and parse_outgoing_calls error paths
    // ---------------------------------------------------------------------------

    #[test]
    fn test_parse_call_hierarchy_items_error_response() {
        // JSON-RPC error field causes an Err result.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "error": {"code": -32601, "message": "Method not found"}
        });
        let result = parse_call_hierarchy_items(&response);
        assert!(result.is_err(), "expected Err for error response");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("LSP error"),
            "error should mention LSP error, got: {}",
            msg
        );
    }

    #[test]
    fn test_parse_call_hierarchy_items_unexpected_result_type() {
        // Non-null, non-array result type should return Err.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": "unexpected_string"
        });
        let result = parse_call_hierarchy_items(&response);
        assert!(result.is_err(), "expected Err for unexpected result type");
    }

    #[test]
    fn test_parse_outgoing_calls_error_response() {
        // JSON-RPC error field causes an Err result.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "error": {"code": -32600, "message": "Invalid Request"}
        });
        let result = parse_outgoing_calls(&response);
        assert!(result.is_err(), "expected Err for error response");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("LSP error"),
            "error should mention LSP error, got: {}",
            msg
        );
    }

    #[test]
    fn test_parse_outgoing_calls_unexpected_result_type() {
        // Non-null, non-array result type should return Err.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 4,
            "result": 42
        });
        let result = parse_outgoing_calls(&response);
        assert!(result.is_err(), "expected Err for unexpected result type");
    }

    #[test]
    fn test_parse_outgoing_calls_null() {
        // Null result returns empty Vec.
        let response = json!({"jsonrpc": "2.0", "id": 4, "result": null});
        let calls = parse_outgoing_calls(&response).unwrap();
        assert!(calls.is_empty());
    }

    // ---------------------------------------------------------------------------
    // Mock LSP stdio helpers and tests for initialize/shutdown
    // ---------------------------------------------------------------------------

    /// Spawn a mock LSP server using Python3.
    ///
    /// The server reads a single JSON-RPC request and writes back the
    /// given `response` JSON, then exits. This lets us test
    /// `LspJsonRpcClient` methods that require actual stdio round-trips.
    fn spawn_mock_lsp(responses: Vec<Value>) -> std::process::Child {
        // Build a Python3 script that handles N responses in sequence,
        // reading one request per response and replying.
        let mut script = String::from(
            "import sys, json\n\
             def read_msg():\n\
             \tcl = None\n\
             \twhile True:\n\
             \t\tline = sys.stdin.readline()\n\
             \t\tif not line: return None\n\
             \t\tline = line.strip()\n\
             \t\tif not line: break\n\
             \t\tif line.startswith('Content-Length:'):\n\
             \t\t\tcl = int(line.split(':', 1)[1].strip())\n\
             \tif cl is None: return None\n\
             \tbody = sys.stdin.read(cl)\n\
             \treturn json.loads(body)\n\
             def send_msg(obj):\n\
             \ts = json.dumps(obj)\n\
             \tsys.stdout.write(f'Content-Length: {len(s)}\\r\\n\\r\\n{s}')\n\
             \tsys.stdout.flush()\n",
        );

        for resp in &responses {
            let resp_json = resp.to_string().replace('\'', "\\'");
            script.push_str(&format!(
                "read_msg()\nsend_msg(json.loads('{}'))\n",
                resp_json
            ));
        }

        std::process::Command::new("python3")
            .arg("-c")
            .arg(&script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn mock LSP python3 process")
    }

    #[test]
    fn test_initialize_with_mock_server() {
        // initialize() should send the initialize request and succeed when
        // the mock server returns a valid response.
        let init_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": {"capabilities": {}}
        });

        let mut child = spawn_mock_lsp(vec![init_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.initialize(std::path::Path::new("/workspace"));
        assert!(result.is_ok(), "initialize should succeed: {:?}", result);

        let _ = child.wait();
    }

    #[test]
    fn test_initialize_returns_error_on_lsp_error_response() {
        // initialize() should propagate an LSP error from the server.
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32600, "message": "Invalid Request"}
        });

        let mut child = spawn_mock_lsp(vec![error_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.initialize(std::path::Path::new("/workspace"));
        assert!(
            result.is_err(),
            "initialize should fail on LSP error response"
        );

        let _ = child.wait();
    }

    #[test]
    fn test_shutdown_with_mock_server() {
        // shutdown() should send shutdown request and exit notification without error.
        let shutdown_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": null
        });

        let mut child = spawn_mock_lsp(vec![shutdown_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.shutdown();
        assert!(result.is_ok(), "shutdown should succeed: {:?}", result);

        let _ = child.wait();
    }

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_file_symbols_with_mock_server() {
        // collect_and_persist_file_symbols() should request documentSymbol, parse,
        // and persist the results to the database.
        let file_path = std::path::Path::new("/workspace/src/demo.rs");
        let relative_path = "src/demo.rs";

        // Mock server response for textDocument/documentSymbol
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "run",
                    "kind": 12,  // FUNCTION
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 6}}
                }
            ]
        });

        let mut child = spawn_mock_lsp(vec![symbol_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, relative_path);

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.collect_and_persist_file_symbols(&conn, file_path, relative_path);
        assert!(
            result.is_ok(),
            "collect_and_persist_file_symbols should succeed: {:?}",
            result
        );
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 1, "should have found 1 symbol");
        assert!(info.error.is_none());

        let _ = child.wait();
    }

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_call_edges_empty_symbols() {
        // collect_and_persist_call_edges() returns 0 when documentSymbol returns empty.
        let file_path = std::path::Path::new("/workspace/src/empty.rs");
        let relative_path = "src/empty.rs";

        // Mock server: empty documentSymbol response
        let empty_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let mut child = spawn_mock_lsp(vec![empty_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, relative_path);

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let count = client.collect_and_persist_call_edges(&conn, file_path, relative_path);
        assert!(count.is_ok(), "should succeed with empty symbols");
        assert_eq!(count.unwrap(), 0, "no edges for empty file");

        let _ = child.wait();
    }

    #[cfg(unix)]
    #[test]
    fn test_wait_for_readable_timeout() {
        // Verify that wait_for_readable returns Err on timeout.
        use std::process::{Command, Stdio};

        // Spawn a silent process whose stdout never produces data.
        let mut child = Command::new("cat")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("failed to spawn cat");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let client = LspJsonRpcClient::new(stdin, stdout);

        // Set a deadline 200ms from now
        let deadline = Instant::now() + Duration::from_millis(200);
        let result = client.wait_for_readable(deadline);

        assert!(result.is_err(), "should have timed out");
        // Verify it didn't take too long
        assert!(
            Instant::now() < deadline + Duration::from_secs(2),
            "timeout took far too long"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    // ---------------------------------------------------------------------------
    // Tests for send_did_open / send_did_close via mock LSP
    // ---------------------------------------------------------------------------

    /// Spawn a mock LSP that reads N messages without replying (for notifications).
    fn spawn_mock_notification_sink(count: usize) -> std::process::Child {
        let script = format!(
            "import sys, json\n\
             def read_msg():\n\
             \tcl = None\n\
             \twhile True:\n\
             \t\tline = sys.stdin.readline()\n\
             \t\tif not line: return None\n\
             \t\tline = line.strip()\n\
             \t\tif not line: break\n\
             \t\tif line.startswith('Content-Length:'):\n\
             \t\t\tcl = int(line.split(':', 1)[1].strip())\n\
             \tif cl is None: return None\n\
             \tbody = sys.stdin.read(cl)\n\
             \treturn json.loads(body)\n\
             for _ in range({count}):\n\
             \tread_msg()\n"
        );

        std::process::Command::new("python3")
            .arg("-c")
            .arg(&script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn mock notification sink")
    }

    #[test]
    fn test_send_did_open_with_mock_server() {
        // send_did_open is a notification — no response expected.
        // The mock just consumes the message without replying.
        let mut child = spawn_mock_notification_sink(1);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result =
            client.send_did_open(Path::new("/workspace/src/main.rs"), "rust", "fn main() {}");
        assert!(result.is_ok(), "send_did_open should succeed: {:?}", result);

        let _ = child.wait();
    }

    #[test]
    fn test_send_did_close_with_mock_server() {
        // send_did_close is a notification — no response expected.
        let mut child = spawn_mock_notification_sink(1);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.send_did_close(Path::new("/workspace/src/main.rs"));
        assert!(
            result.is_ok(),
            "send_did_close should succeed: {:?}",
            result
        );

        let _ = child.wait();
    }

    #[test]
    fn test_collect_file_symbols_with_mock_server() {
        // collect_file_symbols sends a documentSymbol request and parses response.
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "main",
                    "kind": 12,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 7}}
                }
            ]
        });

        let mut child = spawn_mock_lsp(vec![symbol_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.collect_file_symbols(Path::new("/workspace/src/main.rs"));
        assert!(
            result.is_ok(),
            "collect_file_symbols should succeed: {:?}",
            result
        );
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 1);
        assert!(info.error.is_none());

        let _ = child.wait();
    }

    #[test]
    fn test_collect_file_symbols_parse_error_returns_result_with_error() {
        // When the server returns an LSP error, collect_file_symbols should
        // still return Ok(LspCollectionResult) with the error field set.
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32600, "message": "Invalid Request"}
        });

        let mut child = spawn_mock_lsp(vec![error_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.collect_file_symbols(Path::new("/workspace/src/bad.rs"));
        assert!(result.is_ok(), "should return Ok with error field set");
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 0);
        assert!(info.error.is_some(), "error field should be populated");

        let _ = child.wait();
    }

    #[test]
    fn test_collect_and_persist_file_symbols_error_returns_result_with_error() {
        // When the LSP server returns an error, collect_and_persist_file_symbols
        // should return Ok(LspCollectionResult) with the error field set.
        let error_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "error": {"code": -32601, "message": "Method not found"}
        });

        let mut child = spawn_mock_lsp(vec![error_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, "src/err.rs");

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.collect_and_persist_file_symbols(
            &conn,
            Path::new("/workspace/src/err.rs"),
            "src/err.rs",
        );
        assert!(result.is_ok(), "should return Ok with error field set");
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 0);
        assert!(info.error.is_some());

        let _ = child.wait();
    }

    #[test]
    fn test_send_request_skips_notifications_before_response() {
        // The mock sends a notification (no "id" field) first, then the real response.
        // send_request should skip the notification and return the response.
        let script = r#"
import sys, json

def read_msg():
    cl = None
    while True:
        line = sys.stdin.readline()
        if not line: return None
        line = line.strip()
        if not line: break
        if line.startswith('Content-Length:'):
            cl = int(line.split(':', 1)[1].strip())
    if cl is None: return None
    body = sys.stdin.read(cl)
    return json.loads(body)

def send_msg(obj):
    s = json.dumps(obj)
    sys.stdout.write(f'Content-Length: {len(s)}\r\n\r\n{s}')
    sys.stdout.flush()

# Read the request
req = read_msg()

# Send a notification first (no "id" field)
send_msg({"jsonrpc": "2.0", "method": "window/logMessage", "params": {"type": 3, "message": "info"}})

# Then send the actual response
send_msg({"jsonrpc": "2.0", "id": req["id"], "result": {"capabilities": {}}})
"#;

        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn python3");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        // initialize calls send_request internally
        let result = client.initialize(Path::new("/workspace"));
        assert!(
            result.is_ok(),
            "should succeed after skipping notification: {:?}",
            result
        );

        let _ = child.wait();
    }

    #[test]
    fn test_send_request_accepts_mismatched_id_response() {
        // When the server returns a response with a wrong ID first, then the
        // correct one, send_request should skip the wrong ID and return the
        // correct response.
        let script = r#"
import sys, json

def read_msg():
    cl = None
    while True:
        line = sys.stdin.readline()
        if not line: return None
        line = line.strip()
        if not line: break
        if line.startswith('Content-Length:'):
            cl = int(line.split(':', 1)[1].strip())
    if cl is None: return None
    body = sys.stdin.read(cl)
    return json.loads(body)

def send_msg(obj):
    s = json.dumps(obj)
    sys.stdout.write(f'Content-Length: {len(s)}\r\n\r\n{s}')
    sys.stdout.flush()

# Read the request
req = read_msg()
req_id = req.get("id", 1)

# Send response with a different ID first (server-initiated request)
send_msg({"jsonrpc": "2.0", "id": 999, "method": "workspace/diagnostic/refresh"})

# Then send the correct response
send_msg({"jsonrpc": "2.0", "id": req_id, "result": {"capabilities": {}}})
"#;

        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn python3");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        // initialize() calls send_request — should skip the wrong-ID message
        // and return the correct response
        let result = client.initialize(Path::new("/workspace"));
        assert!(
            result.is_ok(),
            "should skip mismatched ID and find correct response: {:?}",
            result
        );

        let _ = child.wait();
    }

    #[test]
    fn test_collect_call_edges_with_functions() {
        // Test collect_call_edges with a response that has function symbols,
        // triggering the call hierarchy flow.
        // Response 1: documentSymbol with a function
        // Response 2: prepareCallHierarchy with an item
        // Response 3: outgoingCalls with a call
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "process",
                    "kind": 12,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 10}}
                }
            ]
        });
        let prepare_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": [
                {
                    "name": "process",
                    "kind": 12,
                    "uri": "file:///workspace/src/main.rs",
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 10}}
                }
            ]
        });
        let outgoing_response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": [
                {
                    "to": {
                        "name": "helper",
                        "kind": 12,
                        "uri": "file:///workspace/src/utils.rs",
                        "range": {"start": {"line": 5, "character": 0}, "end": {"line": 15, "character": 1}},
                        "selectionRange": {"start": {"line": 5, "character": 3}, "end": {"line": 5, "character": 9}}
                    },
                    "fromRanges": [
                        {"start": {"line": 3, "character": 4}, "end": {"line": 3, "character": 10}}
                    ]
                }
            ]
        });

        let mut child = spawn_mock_lsp(vec![symbol_response, prepare_response, outgoing_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let edges = client
            .collect_call_edges(Path::new("/workspace/src/main.rs"), "src/main.rs")
            .unwrap();

        assert_eq!(edges.len(), 1, "should find 1 call edge");
        assert!(edges[0].callee_id.contains("helper"));
        assert_eq!(edges[0].source, "lsp");

        let _ = child.wait();
    }

    #[test]
    fn test_collect_and_persist_call_edges_with_functions() {
        // End-to-end: collect call edges and persist them to the database.
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "caller_fn",
                    "kind": 12,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 12}}
                }
            ]
        });
        let prepare_response = json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": [
                {
                    "name": "caller_fn",
                    "kind": 12,
                    "uri": "file:///workspace/src/call.rs",
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 12}}
                }
            ]
        });
        let outgoing_response = json!({
            "jsonrpc": "2.0",
            "id": 3,
            "result": [
                {
                    "to": {
                        "name": "callee_fn",
                        "kind": 12,
                        "uri": "file:///workspace/src/target.rs",
                        "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
                        "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 12}}
                    },
                    "fromRanges": [
                        {"start": {"line": 3, "character": 4}, "end": {"line": 3, "character": 13}}
                    ]
                }
            ]
        });

        let mut child = spawn_mock_lsp(vec![symbol_response, prepare_response, outgoing_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, "src/call.rs");
        seed_indexed_file(&conn, "src/target.rs");

        // Pre-seed lsp_symbols for the caller (produced by flatten_symbols)
        // and callee so the FK constraints on lsp_call_edges are satisfied.
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/call.rs:caller_fn', 'caller_fn', 12, 'src/call.rs', 0, 3, 10, 1)",
            [],
        )
        .unwrap();

        // The callee_id is "lsp:<relative_callee_file>:<callee_name>"
        // uri_to_relative_path("file:///workspace/src/target.rs", Path::new("/workspace/src/call.rs"))
        // walks up from /workspace/src/ and strips, yielding "target.rs" (sibling).
        // But the caller_file's parent is /workspace/src, so target.rs -> "target.rs".
        // The callee_id = "lsp:target.rs:callee_fn"
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:target.rs:callee_fn', 'callee_fn', 12, 'src/target.rs', 0, 3, 5, 1)",
            [],
        )
        .unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let count = client
            .collect_and_persist_call_edges(
                &conn,
                Path::new("/workspace/src/call.rs"),
                "src/call.rs",
            )
            .unwrap();

        assert_eq!(count, 1, "should have persisted 1 edge");

        // Verify edge was written to the database
        let edge_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_call_edges WHERE caller_file = 'src/call.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(edge_count, 1, "edge should be in the database");

        let _ = child.wait();
    }

    #[test]
    fn test_collect_call_edges_skips_non_callable_symbols() {
        // Symbols that are not functions/methods/constructors should be skipped.
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "MyStruct",
                    "kind": 23,  // STRUCT — not callable
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 5, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 11}, "end": {"line": 0, "character": 19}}
                }
            ]
        });

        let mut child = spawn_mock_lsp(vec![symbol_response]);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let edges = client
            .collect_call_edges(Path::new("/workspace/src/types.rs"), "src/types.rs")
            .unwrap();

        assert!(
            edges.is_empty(),
            "struct symbols should produce no call edges"
        );

        let _ = child.wait();
    }

    #[test]
    fn test_parse_document_symbols_unexpected_result_type() {
        // Non-null, non-array result should return Err.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": "unexpected_string"
        });
        let result = parse_document_symbols(&response);
        assert!(result.is_err(), "expected Err for unexpected result type");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("unexpected"),
            "error should mention unexpected type, got: {}",
            msg
        );
    }

    #[test]
    fn test_parse_document_symbols_no_result_field() {
        // Response with no "result" and no "error" returns empty Vec.
        let response = json!({
            "jsonrpc": "2.0",
            "id": 1
        });
        let symbols = parse_document_symbols(&response).unwrap();
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_count_symbols_recursive_empty() {
        // Empty input should return 0.
        assert_eq!(count_symbols_recursive(&[]), 0);
    }

    #[test]
    fn test_uri_to_relative_path_no_file_prefix() {
        // URI without file:// prefix should be returned as-is.
        let ref_path = Path::new("/workspace/src/main.rs");
        let rel = uri_to_relative_path("/some/path/file.rs", ref_path);
        assert_eq!(rel, "some/path/file.rs");
    }

    #[test]
    fn test_read_jsonrpc_response_bad_content_length_value() {
        // Content-Length with a non-numeric value should return an error.
        let message = b"Content-Length: not_a_number\r\n\r\n";
        let mut reader = std::io::BufReader::new(&message[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error for bad Content-Length");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Content-Length"),
            "error should mention Content-Length, got: {}",
            msg
        );
    }

    #[test]
    fn test_lsp_collection_result_debug_impl() {
        // Exercise the Debug derive on LspCollectionResult.
        let result = LspCollectionResult {
            file_path: "src/test.rs".to_string(),
            symbol_count: 3,
            error: None,
        };
        let debug_str = format!("{:?}", result);
        assert!(debug_str.contains("src/test.rs"));
        assert!(debug_str.contains("3"));
    }

    #[test]
    fn test_collect_file_symbols_send_request_error_returns_result_with_error() {
        // When the LSP process exits before responding, send_request fails.
        // collect_file_symbols should wrap this in LspCollectionResult.error.
        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        // Give the process time to exit
        std::thread::sleep(Duration::from_millis(100));

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.collect_file_symbols(Path::new("/workspace/src/crash.rs"));
        assert!(result.is_ok(), "should return Ok with error field");
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 0);
        assert!(info.error.is_some(), "error field should be set on failure");

        let _ = child.wait();
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: read_jsonrpc_response error‐path assertions
    // ---------------------------------------------------------------------------

    #[test]
    fn test_read_jsonrpc_response_eof_error_message_content() {
        // Verify the error message specifically mentions "EOF" or "header".
        let input: &[u8] = b"";
        let mut reader = std::io::BufReader::new(input);
        let err = read_jsonrpc_response(&mut reader).unwrap_err().to_string();
        assert!(
            err.contains("EOF"),
            "error should mention EOF, got: {}",
            err
        );
    }

    #[test]
    fn test_read_jsonrpc_response_headers_then_eof_before_blank_line() {
        // Headers are present but stream ends before the blank separator line.
        // The read_line call returns 0 (EOF) inside the header loop.
        let input = b"Content-Length: 10\r\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error when EOF before blank line");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("EOF"),
            "error should mention EOF, got: {}",
            msg
        );
    }

    #[test]
    fn test_read_jsonrpc_response_missing_content_length_error_message() {
        // Only non-Content-Length headers followed by blank line.
        // Hits the "missing Content-Length header" error path.
        let input = b"X-Custom: something\r\n\r\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Content-Length"),
            "error should mention Content-Length, got: {}",
            msg
        );
    }

    #[test]
    fn test_read_jsonrpc_response_content_length_float() {
        // Content-Length with a float value should fail integer parsing.
        let input = b"Content-Length: 3.14\r\n\r\n";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err(), "expected error for float Content-Length");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("Content-Length"),
            "error should mention Content-Length, got: {}",
            msg
        );
    }

    #[test]
    fn test_read_jsonrpc_response_truncated_body_error_message() {
        // Content-Length claims 200 bytes but only 5 are available.
        // Verifies the "read body" error path.
        let input = b"Content-Length: 200\r\n\r\nhello";
        let mut reader = std::io::BufReader::new(&input[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("body") || msg.contains("read"),
            "error should mention body read failure, got: {}",
            msg
        );
    }

    #[test]
    fn test_read_jsonrpc_response_json_decode_error_message() {
        // Body is valid bytes but not valid JSON.
        // Verifies the "json decode" error path.
        let body = b"{invalid json!!!}";
        let header = format!("Content-Length: {}\r\n\r\n", body.len());
        let mut data = header.into_bytes();
        data.extend_from_slice(body);
        let mut reader = std::io::BufReader::new(&data[..]);
        let result = read_jsonrpc_response(&mut reader);
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("json decode"),
            "error should mention json decode, got: {}",
            msg
        );
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: uri_to_relative_path no common ancestor
    // ---------------------------------------------------------------------------

    #[test]
    fn test_uri_to_relative_path_empty_reference_path() {
        // When reference_path has no parent (empty path), the function falls
        // through to returning the raw path string.
        let ref_path = Path::new("");
        let result = uri_to_relative_path("file:///some/project/file.rs", ref_path);
        assert_eq!(result, "/some/project/file.rs");
    }

    #[test]
    fn test_uri_to_relative_path_relative_uri_no_common_ancestor() {
        // A relative URI path cannot be stripped by any absolute ancestor,
        // so the ancestor walk exhausts to root and falls through.
        let ref_path = Path::new("/workspace/src/main.rs");
        let result = uri_to_relative_path("relative/path/file.rs", ref_path);
        assert_eq!(result, "relative/path/file.rs");
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: send_request write-failure
    // ---------------------------------------------------------------------------

    #[test]
    fn test_send_request_write_failure_on_dead_process() {
        // When the child process exits immediately, writing to its stdin pipe
        // should fail, producing an LspError with "write failed" or "flush failed".
        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        // Wait for the process to fully exit so the pipe is broken
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(50));

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.send_request("test/method", json!({}));
        assert!(result.is_err(), "write to dead process should fail");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("write") || msg.contains("flush") || msg.contains("Broken pipe"),
            "error should mention write/flush failure, got: {}",
            msg
        );
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: send_notification write/flush failure
    // ---------------------------------------------------------------------------

    #[test]
    fn test_send_notification_write_failure_on_dead_process() {
        // When the child process exits immediately, send_notification should
        // fail on write_all or flush with an LspError.
        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg("import sys; sys.exit(0)")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        // Wait for process to exit so pipe is broken
        let _ = child.wait();
        std::thread::sleep(Duration::from_millis(50));

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.send_notification("textDocument/didOpen", json!({}));
        assert!(result.is_err(), "notification to dead process should fail");
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("write") || msg.contains("flush") || msg.contains("notification"),
            "error should mention write/flush failure, got: {}",
            msg
        );
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: send_request notification-skip without method field
    // ---------------------------------------------------------------------------

    #[test]
    fn test_send_request_skips_notification_without_method_field() {
        // When the server sends a notification that has no "method" field,
        // send_request should still skip it (hitting the unwrap_or("unknown")
        // fallback) and return the correct response.
        let script = r#"
import sys, json

def read_msg():
    cl = None
    while True:
        line = sys.stdin.readline()
        if not line: return None
        line = line.strip()
        if not line: break
        if line.startswith('Content-Length:'):
            cl = int(line.split(':', 1)[1].strip())
    if cl is None: return None
    body = sys.stdin.read(cl)
    return json.loads(body)

def send_msg(obj):
    s = json.dumps(obj)
    sys.stdout.write(f'Content-Length: {len(s)}\r\n\r\n{s}')
    sys.stdout.flush()

# Read the request
req = read_msg()

# Send a notification without a "method" field (unusual but possible)
send_msg({"jsonrpc": "2.0", "params": {"data": "noise"}})

# Then send the actual response
send_msg({"jsonrpc": "2.0", "id": req["id"], "result": {"ok": True}})
"#;

        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn python3");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.send_request("test/method", json!({}));
        assert!(
            result.is_ok(),
            "should skip notification without method and return response: {:?}",
            result
        );
        let response = result.unwrap();
        assert_eq!(response["result"]["ok"], true);

        let _ = child.wait();
    }

    // ---------------------------------------------------------------------------
    // Coverage gap tests: send_request wrong-ID response without method field
    // ---------------------------------------------------------------------------

    #[test]
    fn test_send_request_skips_wrong_id_without_method_field() {
        // When the server sends a response with a wrong ID and no "method"
        // field, send_request should skip it (hitting the unwrap_or("none")
        // fallback in the warn!) and eventually return the correct response.
        let script = r#"
import sys, json

def read_msg():
    cl = None
    while True:
        line = sys.stdin.readline()
        if not line: return None
        line = line.strip()
        if not line: break
        if line.startswith('Content-Length:'):
            cl = int(line.split(':', 1)[1].strip())
    if cl is None: return None
    body = sys.stdin.read(cl)
    return json.loads(body)

def send_msg(obj):
    s = json.dumps(obj)
    sys.stdout.write(f'Content-Length: {len(s)}\r\n\r\n{s}')
    sys.stdout.flush()

# Read the request
req = read_msg()
req_id = req.get("id", 1)

# Send response with wrong ID and NO method field
send_msg({"jsonrpc": "2.0", "id": 9999, "result": {"stale": True}})

# Then send the correct response
send_msg({"jsonrpc": "2.0", "id": req_id, "result": {"correct": True}})
"#;

        let mut child = std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn python3");

        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = client.send_request("test/method", json!({}));
        assert!(
            result.is_ok(),
            "should skip wrong-ID response and return correct one: {:?}",
            result
        );
        let response = result.unwrap();
        assert_eq!(response["result"]["correct"], true);

        let _ = child.wait();
    }
}
