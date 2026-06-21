//! LSP symbol collection and persistence.
//!
//! The wire-level JSON-RPC client ([`LspJsonRpcClient`]) and the
//! [`parse_document_symbols`] helper now live in `swissarmyhammer-lsp`; this
//! module owns the code-context-specific layer on top of that transport:
//! collecting document symbols and call-hierarchy edges from a language server
//! and persisting them into the index database. These are free functions over a
//! `&mut LspJsonRpcClient` so they can be unit-tested against the in-memory
//! fake transport in `swissarmyhammer-lsp`.

use rusqlite::Connection;
use serde_json::{json, Value};
use std::path::Path;
use tracing::debug;

use lsp_types::{CallHierarchyItem, CallHierarchyOutgoingCall, DocumentSymbol};
use swissarmyhammer_lsp::parse_document_symbols;
// Re-exported so existing `crate::lsp_communication::LspJsonRpcClient` paths in
// this crate keep resolving after the client moved into `swissarmyhammer-lsp`.
pub use swissarmyhammer_lsp::LspJsonRpcClient;

use crate::error::CodeContextError;
use crate::invalidation::{reextract_symbols, InvalidationAction};
use crate::lsp_indexer::{flatten_symbols, mark_lsp_indexed, write_edges, CallEdge, FlatSymbol};

/// Result of collecting symbols from LSP server for a file.
#[derive(Debug)]
pub struct LspCollectionResult {
    /// File path that was processed
    pub file_path: String,
    /// Number of symbols collected
    pub symbol_count: usize,
    /// Any error that occurred
    pub error: Option<String>,
    /// Invalidation actions the caller should apply after this collection.
    ///
    /// When a file's symbol set shrinks (deletions, renames), every file that
    /// previously called into a now-gone symbol needs its outgoing edges
    /// refreshed. These actions describe that propagation — the caller is
    /// responsible for applying them (typically by marking the affected files
    /// as `lsp_indexed = 0` so the worker picks them up on its next pass).
    pub pending_actions: Vec<InvalidationAction>,
}

/// Collect and persist LSP symbols for a file, tracking which dependent
/// files need their outgoing edges refreshed.
///
/// Uses [`reextract_symbols`] so that:
/// - The file's own symbol rows are replaced atomically (delete-then-insert).
/// - Files that had outgoing edges pointing at now-deleted symbol IDs are
///   captured in the returned actions for follow-up invalidation.
///
/// The file's own outgoing call edges are preserved; they are owned by a
/// separate call-hierarchy pass. Rows whose `caller_id` maps to a deleted
/// symbol are cleaned up by the `lsp_symbols` CASCADE.
///
/// # Arguments
/// * `conn` - Database connection
/// * `file_path` - Path to the file
/// * `symbols` - DocumentSymbols from LSP server
///
/// # Returns
/// A pair of (number of symbols written, invalidation actions for dependents).
pub fn collect_and_persist_symbols(
    conn: &Connection,
    file_path: &str,
    symbols: &[DocumentSymbol],
) -> Result<(usize, Vec<InvalidationAction>), CodeContextError> {
    // Flatten nested DocumentSymbols into FlatSymbol format
    let flat_symbols = flatten_symbols(file_path, symbols);
    let symbol_count = flat_symbols.len();

    // Run the invalidation-aware symbol re-extract. This always runs (even
    // for zero symbols) so deletions propagate correctly.
    let actions = reextract_symbols(conn, file_path, &flat_symbols)?;

    // Mark file as lsp_indexed
    mark_lsp_indexed(conn, file_path)?;

    debug!(
        "Collected and persisted {} symbols for {} ({} dependent files affected)",
        symbol_count,
        file_path,
        actions.len()
    );
    Ok((symbol_count, actions))
}

/// Collect symbols from the LSP server for a given file.
///
/// Sends `textDocument/documentSymbol` through the transport and parses the
/// response. Operates on a `&mut LspJsonRpcClient` so it can be exercised with
/// either a real language server or the in-memory fake transport.
pub fn collect_file_symbols(
    client: &mut LspJsonRpcClient,
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

    match client.send_request("textDocument/documentSymbol", params) {
        Ok(response) => match parse_document_symbols(&response) {
            Ok(symbols) => {
                let symbol_count = count_symbols_recursive(&symbols);
                Ok(LspCollectionResult {
                    file_path: file_path_str,
                    symbol_count,
                    error: None,
                    pending_actions: Vec::new(),
                })
            }
            Err(e) => Ok(LspCollectionResult {
                file_path: file_path_str,
                symbol_count: 0,
                error: Some(e.to_string()),
                pending_actions: Vec::new(),
            }),
        },
        Err(e) => Ok(LspCollectionResult {
            file_path: file_path_str,
            symbol_count: 0,
            error: Some(e.to_string()),
            pending_actions: Vec::new(),
        }),
    }
}

/// Collect symbols and persist them to the database.
///
/// Combines [`collect_file_symbols`] with database writes. The returned
/// [`LspCollectionResult::pending_actions`] lists files that had edges
/// to symbols that no longer exist in `relative_path` and therefore need
/// their outgoing edges refreshed.
pub fn collect_and_persist_file_symbols(
    client: &mut LspJsonRpcClient,
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

    match client.send_request("textDocument/documentSymbol", params) {
        Ok(response) => match parse_document_symbols(&response) {
            Ok(symbols) => {
                let (count, pending_actions) =
                    collect_and_persist_symbols(conn, relative_path, &symbols)?;
                Ok(LspCollectionResult {
                    file_path: file_path_str,
                    symbol_count: count,
                    error: None,
                    pending_actions,
                })
            }
            Err(e) => Ok(LspCollectionResult {
                file_path: file_path_str,
                symbol_count: 0,
                error: Some(e.to_string()),
                pending_actions: Vec::new(),
            }),
        },
        Err(e) => Ok(LspCollectionResult {
            file_path: file_path_str,
            symbol_count: 0,
            error: Some(e.to_string()),
            pending_actions: Vec::new(),
        }),
    }
}

/// Collect outgoing call edges for a file using LSP call hierarchy.
///
/// For each function/method symbol, prepares a call hierarchy item,
/// then queries outgoing calls. Returns edges suitable for `write_edges`.
pub fn collect_call_edges(
    client: &mut LspJsonRpcClient,
    file_path: &Path,
    relative_path: &str,
) -> Result<Vec<CallEdge>, CodeContextError> {
    let file_path_str = file_path.to_string_lossy().to_string();
    let uri = format!("file://{}", file_path_str);

    // First get document symbols to find function/method positions
    let symbol_params = json!({
        "textDocument": { "uri": &uri }
    });

    let symbol_response = client.send_request("textDocument/documentSymbol", symbol_params)?;
    let symbols = parse_document_symbols(&symbol_response)?;
    let flat = flatten_symbols(relative_path, &symbols);

    let mut all_edges = Vec::new();

    for sym in &flat {
        use lsp_types::SymbolKind;
        match sym.kind {
            SymbolKind::FUNCTION | SymbolKind::METHOD | SymbolKind::CONSTRUCTOR => {}
            _ => continue,
        }
        collect_edges_for_symbol(client, &uri, sym, file_path, relative_path, &mut all_edges)?;
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
    client: &mut LspJsonRpcClient,
    conn: &Connection,
    file_path: &Path,
    relative_path: &str,
) -> Result<usize, CodeContextError> {
    let edges = collect_call_edges(client, file_path, relative_path)?;
    if edges.is_empty() {
        return Ok(0);
    }
    write_edges(conn, relative_path, &edges)
}

/// Collect outgoing call edges for a single callable symbol.
fn collect_edges_for_symbol(
    client: &mut LspJsonRpcClient,
    uri: &str,
    sym: &FlatSymbol,
    file_path: &Path,
    relative_path: &str,
    edges: &mut Vec<CallEdge>,
) -> Result<(), CodeContextError> {
    let prepare_params = json!({
        "textDocument": { "uri": uri },
        "position": { "line": sym.start_line, "character": sym.start_char }
    });

    let prepare_response =
        match client.send_request("textDocument/prepareCallHierarchy", prepare_params) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

    let items = parse_call_hierarchy_items(&prepare_response)?;
    if items.is_empty() {
        return Ok(());
    }

    let outgoing_params = json!({
        "item": serde_json::to_value(&items[0])
            .map_err(|e| CodeContextError::LspError(format!("serialize item: {}", e)))?
    });

    let outgoing_response =
        match client.send_request("callHierarchy/outgoingCalls", outgoing_params) {
            Ok(r) => r,
            Err(_) => return Ok(()),
        };

    for call in &parse_outgoing_calls(&outgoing_response)? {
        let callee_file = uri_to_relative_path(call.to.uri.as_str(), file_path);
        let callee_id = format!("lsp:{}:{}", callee_file, call.to.name);
        let from_ranges_json =
            serde_json::to_string(&call.from_ranges).unwrap_or_else(|_| "[]".to_string());

        edges.push(CallEdge {
            caller_id: sym.id.clone(),
            callee_id,
            caller_file: relative_path.to_string(),
            callee_file,
            from_ranges: from_ranges_json,
            source: "lsp".to_string(),
        });
    }

    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use lsp_types::{Position, Range, SymbolKind};

    /// Spawn the crate's single mock LSP server, scripted to read a request and
    /// reply with `response`.
    ///
    /// Delegates to [`crate::test_fixtures::spawn_mock_lsp`] (read-a-request /
    /// send-`response`) so there is one mock-LSP spawn path and one kill-on-drop
    /// guard ([`crate::testing::KillOnDrop`]) crate-wide. Returns the guard, so
    /// callers must never block on the child themselves.
    fn spawn_mock_lsp(response: Value) -> crate::testing::KillOnDrop {
        crate::test_fixtures::spawn_mock_lsp(&[response])
    }

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
    fn test_lsp_collection_result() {
        let result = LspCollectionResult {
            file_path: "src/main.rs".to_string(),
            symbol_count: 5,
            error: None,
            pending_actions: Vec::new(),
        };
        assert_eq!(result.file_path, "src/main.rs");
        assert_eq!(result.symbol_count, 5);
        assert!(result.error.is_none());
        assert!(result.pending_actions.is_empty());
    }

    #[test]
    fn test_lsp_collection_with_error() {
        let result = LspCollectionResult {
            file_path: "src/bad.rs".to_string(),
            symbol_count: 0,
            error: Some("timeout".to_string()),
            pending_actions: Vec::new(),
        };
        assert_eq!(result.file_path, "src/bad.rs");
        assert!(result.error.is_some());
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

    // ---------------------------------------------------------------------------
    // Tests for collect_and_persist_symbols (standalone DB function)
    // ---------------------------------------------------------------------------

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

        let (count, actions) = collect_and_persist_symbols(&conn, file_path, &symbols).unwrap();
        assert_eq!(count, 2, "should have written 2 symbols");
        assert!(
            actions.is_empty(),
            "no prior symbols -> no dependent files to refresh"
        );

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

        let (count, actions) = collect_and_persist_symbols(&conn, file_path, &[]).unwrap();
        assert_eq!(count, 0);
        assert!(actions.is_empty(), "empty-in, empty-dependents-out");

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
    // Mock-LSP round-trip tests for the collection free functions
    // ---------------------------------------------------------------------------

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_file_symbols_with_mock_server() {
        // collect_and_persist_file_symbols() should request documentSymbol, parse,
        // and persist the results to the database.
        let file_path = std::path::Path::new("/workspace/src/demo.rs");
        let relative_path = "src/demo.rs";

        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "run",
                    "kind": 12,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 10, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 6}}
                }
            ]
        });

        let mut child = spawn_mock_lsp(symbol_response);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, relative_path);

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = collect_and_persist_file_symbols(&mut client, &conn, file_path, relative_path);
        assert!(
            result.is_ok(),
            "collect_and_persist_file_symbols should succeed: {:?}",
            result
        );
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 1, "should have found 1 symbol");
        assert!(info.error.is_none());
        // `child` (a KillOnDrop guard) is killed and reaped on drop.
    }

    #[test]
    #[allow(deprecated)]
    fn test_collect_and_persist_call_edges_empty_symbols() {
        // collect_and_persist_call_edges() returns 0 when documentSymbol returns empty.
        let file_path = std::path::Path::new("/workspace/src/empty.rs");
        let relative_path = "src/empty.rs";

        let empty_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let mut child = spawn_mock_lsp(empty_response);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, relative_path);

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let count = collect_and_persist_call_edges(&mut client, &conn, file_path, relative_path);
        assert!(count.is_ok(), "should succeed with empty symbols");
        assert_eq!(count.unwrap(), 0, "no edges for empty file");
        // `child` (a KillOnDrop guard) is killed and reaped on drop.
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

        let mut child = spawn_mock_lsp(symbol_response);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = collect_file_symbols(&mut client, Path::new("/workspace/src/main.rs"));
        assert!(
            result.is_ok(),
            "collect_file_symbols should succeed: {:?}",
            result
        );
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 1);
        assert!(info.error.is_none());
        // `child` (a KillOnDrop guard) is killed and reaped on drop.
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

        let mut child = spawn_mock_lsp(error_response);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = collect_file_symbols(&mut client, Path::new("/workspace/src/bad.rs"));
        assert!(result.is_ok(), "should return Ok with error field set");
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 0);
        assert!(info.error.is_some(), "error field should be populated");
        // `child` (a KillOnDrop guard) is killed and reaped on drop.
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

        let mut child = spawn_mock_lsp(error_response);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();

        let conn = open_test_db();
        seed_indexed_file(&conn, "src/err.rs");

        let mut client = LspJsonRpcClient::new(stdin, stdout);
        let result = collect_and_persist_file_symbols(
            &mut client,
            &conn,
            Path::new("/workspace/src/err.rs"),
            "src/err.rs",
        );
        assert!(result.is_ok(), "should return Ok with error field set");
        let info = result.unwrap();
        assert_eq!(info.symbol_count, 0);
        assert!(info.error.is_some());
        // `child` (a KillOnDrop guard) is killed and reaped on drop.
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
}
