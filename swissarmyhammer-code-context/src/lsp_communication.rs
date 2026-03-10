//! LSP JSON-RPC communication and symbol collection.
//!
//! Handles JSON-RPC protocol with LSP server processes.
//! Sends requests for symbols and collects results for database persistence.

use rusqlite::Connection;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::Child;
use tracing::{debug, warn};

use crate::error::CodeContextError;
use crate::lsp_indexer::{flatten_symbols, mark_lsp_indexed, write_edges, write_symbols, CallEdge};
use lsp_types::{CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSymbol};

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
/// We only handle the `DocumentSymbol[]` form here.
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

    // Try to parse as DocumentSymbol[]
    let symbols: Vec<DocumentSymbol> = serde_json::from_value(Value::Array(result.clone()))
        .map_err(|e| {
            CodeContextError::LspError(format!("failed to parse DocumentSymbol array: {}", e))
        })?;

    Ok(symbols)
}

/// JSON-RPC request/response handler for LSP communication.
pub struct LspJsonRpcClient {
    /// Child process handle
    process: Child,
    /// Current request ID (incremented for each request)
    request_id: u32,
}

impl LspJsonRpcClient {
    /// Create a new JSON-RPC client from an already-spawned LSP process.
    ///
    /// # Arguments
    /// * `process` - The spawned child process with stdin/stdout connected
    pub fn new(process: Child) -> Result<Self, CodeContextError> {
        Ok(Self {
            process,
            request_id: 1,
        })
    }

    /// Send a JSON-RPC request and read the response.
    ///
    /// Uses Content-Length framing per the LSP specification.
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, CodeContextError> {
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
        {
            let stdin = self
                .process
                .stdin
                .as_mut()
                .ok_or_else(|| CodeContextError::LspError("stdin unavailable".into()))?;
            stdin
                .write_all(msg.as_bytes())
                .map_err(|e| CodeContextError::LspError(format!("write failed: {}", e)))?;
            stdin
                .flush()
                .map_err(|e| CodeContextError::LspError(format!("flush failed: {}", e)))?;
        }

        debug!("Sent LSP request: {} (id={})", method, expected_id);

        // Read response — loop to skip notifications (no "id" field)
        let stdout = self
            .process
            .stdout
            .as_mut()
            .ok_or_else(|| CodeContextError::LspError("stdout unavailable".into()))?;
        let mut reader = BufReader::new(stdout);

        loop {
            let response = read_jsonrpc_response(&mut reader)?;

            // Notifications have no "id" field — skip them
            if response.get("id").is_none() {
                debug!(
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
                warn!(
                    "Unexpected response id: expected {}, got {}",
                    expected_id, id
                );
            }

            return Ok(response);
        }
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

        if let Some(stdin) = self.process.stdin.as_mut() {
            stdin.write_all(msg.as_bytes()).map_err(|e| {
                CodeContextError::LspError(format!("write initialized failed: {}", e))
            })?;
            stdin.flush().map_err(|e| {
                CodeContextError::LspError(format!("flush initialized failed: {}", e))
            })?;
        }

        debug!("LSP server initialized");
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
}
