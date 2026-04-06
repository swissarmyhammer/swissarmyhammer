//! Layered resolution context for code intelligence operations.
//!
//! Provides a single access point for three data layers:
//! 1. **Live LSP** -- real-time requests to a running LSP server
//! 2. **LSP index** -- persisted symbols and call edges from previous LSP sessions
//! 3. **Tree-sitter index** -- structural chunks extracted by tree-sitter
//!
//! Each layer has its own method family. Convenience methods like
//! [`LayeredContext::enrich_location`] try all layers in priority order.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CodeContextError;

/// Extract the `result` field from a JSON-RPC response envelope.
///
/// LSP responses are wrapped: `{"id": N, "jsonrpc": "2.0", "result": ...}`.
/// If the response contains an `error` field instead, convert it to a
/// `CodeContextError`. If neither `result` nor `error` is present, return
/// the response as-is (some callers may handle raw envelopes).
fn unwrap_lsp_result(response: Value) -> Result<Value, CodeContextError> {
    if let Some(error) = response.get("error") {
        let message = error
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("unknown LSP error");
        return Err(CodeContextError::LspError(message.to_string()));
    }
    // Return the result field if present, otherwise the whole response
    Ok(response.get("result").cloned().unwrap_or(response))
}
use crate::lsp_communication::LspJsonRpcClient;
use crate::lsp_worker::SharedLspClient;
use crate::ops::lsp_helpers::{file_path_to_uri, language_id_from_path};

// ---------------------------------------------------------------------------
// Shared types
// ---------------------------------------------------------------------------

/// A range in an LSP-style coordinate system (0-based lines and characters).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LspRange {
    pub start_line: u32,
    pub start_character: u32,
    pub end_line: u32,
    pub end_character: u32,
}

/// Information about a symbol from any data layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SymbolInfo {
    pub name: String,
    pub qualified_path: Option<String>,
    pub kind: String,
    pub detail: Option<String>,
    pub file_path: String,
    pub range: LspRange,
}

/// A call edge connecting a symbol to its call sites.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CallEdgeInfo {
    pub symbol: SymbolInfo,
    pub call_sites: Vec<LspRange>,
}

/// A chunk of source code extracted by tree-sitter.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkInfo {
    pub text: String,
    pub file_path: String,
    pub start_line: u32,
    pub end_line: u32,
}

/// A set of text edits to apply to a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileEdit {
    pub file_path: String,
    pub text_edits: Vec<TextEdit>,
}

/// A single text replacement within a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub range: LspRange,
    pub new_text: String,
}

/// A location where a symbol is defined, with optional source text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefinitionLocation {
    pub file_path: String,
    pub range: LspRange,
    pub source_text: Option<String>,
    pub symbol: Option<SymbolInfo>,
}

/// Which data layer provided a result.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SourceLayer {
    /// Result came from a live LSP server.
    LiveLsp,
    /// Result came from the persisted LSP symbol index.
    LspIndex,
    /// Result came from the tree-sitter chunk index.
    TreeSitter,
    /// No layer had data.
    None,
}

/// Result of enriching a location with the best available data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnrichmentResult {
    pub symbol: Option<SymbolInfo>,
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// LayeredContext
// ---------------------------------------------------------------------------

/// Single access point for all three data layers.
///
/// Private `conn` and `lsp_client` fields ensure callers use typed methods
/// rather than reaching into the raw database or LSP client directly.
pub struct LayeredContext<'a> {
    conn: &'a Connection,
    lsp_client: Option<&'a SharedLspClient>,
}

impl<'a> LayeredContext<'a> {
    /// Create a new layered context.
    ///
    /// # Arguments
    /// * `conn` - Reference to the SQLite connection for index queries.
    /// * `lsp_client` - Optional shared LSP client for live requests.
    pub fn new(conn: &'a Connection, lsp_client: Option<&'a SharedLspClient>) -> Self {
        Self { conn, lsp_client }
    }

    // === Layer availability ===

    /// Returns true if a live LSP client is available and contains a connected client.
    pub fn has_live_lsp(&self) -> bool {
        match self.lsp_client {
            Some(client) => {
                if let Ok(guard) = client.try_lock() {
                    guard.is_some()
                } else {
                    false
                }
            }
            None => false,
        }
    }

    /// Returns true if the given file has been indexed by LSP.
    pub fn has_lsp_index(&self, file_path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get::<_, i32>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
    }

    /// Returns true if the given file has been indexed by tree-sitter.
    pub fn has_ts_index(&self, file_path: &str) -> bool {
        self.conn
            .query_row(
                "SELECT ts_indexed FROM indexed_files WHERE file_path = ?1",
                [file_path],
                |row| row.get::<_, i32>(0),
            )
            .map(|v| v == 1)
            .unwrap_or(false)
    }

    // === Layer 1: Live LSP ===

    /// Send an arbitrary LSP request. Returns `Ok(None)` if no live client is available.
    ///
    /// This is a graceful degradation -- callers should fall back to index layers
    /// when the live client is absent.
    pub fn lsp_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, CodeContextError> {
        let client = match self.lsp_client {
            Some(c) => c,
            None => return Ok(None),
        };
        let mut guard = client
            .lock()
            .map_err(|e| CodeContextError::LspError(format!("failed to lock LSP client: {}", e)))?;
        match guard.as_mut() {
            Some(rpc) => {
                let response = rpc.send_request(method, params)?;
                Ok(Some(unwrap_lsp_result(response)?))
            }
            None => Ok(None),
        }
    }

    /// Send an LSP notification. No-op if no live client is available.
    pub fn lsp_notify(&self, method: &str, params: Value) -> Result<(), CodeContextError> {
        // Notifications are fire-and-forget. If no client, silently succeed.
        let client = match self.lsp_client {
            Some(c) => c,
            None => return Ok(()),
        };
        let mut guard = client
            .lock()
            .map_err(|e| CodeContextError::LspError(format!("failed to lock LSP client: {}", e)))?;
        if let Some(rpc) = guard.as_mut() {
            rpc.send_notification(method, params)?;
        }
        Ok(())
    }

    /// Send a single LSP request wrapped in didOpen/didClose, holding the mutex
    /// for the entire sequence.
    ///
    /// This prevents the indexing worker from interleaving its own requests
    /// between the didOpen, the request, and the didClose. Without this, the
    /// worker can steal responses or corrupt the pipe.
    ///
    /// Returns `Ok(None)` if no live LSP client is available (graceful degradation).
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to open/close around the request.
    /// * `method` - The LSP method to call (e.g. `"textDocument/hover"`).
    /// * `params` - The JSON parameters for the request.
    pub fn lsp_request_with_document(
        &self,
        file_path: &str,
        method: &str,
        params: Value,
    ) -> Result<Option<Value>, CodeContextError> {
        let client = match self.lsp_client {
            Some(c) => c,
            None => return Ok(None),
        };
        let mut guard = client
            .lock()
            .map_err(|e| CodeContextError::LspError(format!("failed to lock LSP client: {}", e)))?;
        let rpc = match guard.as_mut() {
            Some(rpc) => rpc,
            None => return Ok(None),
        };

        let uri = file_path_to_uri(file_path);
        let text = std::fs::read_to_string(file_path).unwrap_or_default();
        let lang = language_id_from_path(file_path);

        // didOpen
        rpc.send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": lang,
                    "version": 1,
                    "text": text
                }
            }),
        )?;

        // The actual request
        tracing::debug!(uri = %uri, method = %method, text_len = text.len(), "lsp_request_with_document");
        let response = rpc.send_request(method, params);
        match &response {
            Ok(v) => {
                tracing::debug!(response = %serde_json::to_string(v).unwrap_or_default().chars().take(300).collect::<String>(), "lsp_request_with_document OK")
            }
            Err(e) => tracing::warn!(error = %e, "lsp_request_with_document ERR"),
        }

        // didClose -- always sent, even if the request failed
        let _ = rpc.send_notification(
            "textDocument/didClose",
            serde_json::json!({
                "textDocument": { "uri": uri }
            }),
        );

        Ok(Some(unwrap_lsp_result(response?)?))
    }

    /// Execute multiple LSP requests wrapped in a single didOpen/didClose sequence,
    /// holding the mutex for the entire duration.
    ///
    /// The closure receives the RPC client directly and can send as many
    /// requests/notifications as needed. The didOpen is sent before the closure
    /// runs, and didClose is sent after (even if the closure returns an error).
    ///
    /// Returns `Ok(None)` if no live LSP client is available.
    ///
    /// # Arguments
    /// * `file_path` - Path to the file to open/close around the requests.
    /// * `f` - Closure that receives the RPC client and performs the requests.
    pub fn lsp_multi_request_with_document<F, T>(
        &self,
        file_path: &str,
        f: F,
    ) -> Result<Option<T>, CodeContextError>
    where
        F: FnOnce(&mut LspJsonRpcClient) -> Result<T, CodeContextError>,
    {
        let client = match self.lsp_client {
            Some(c) => c,
            None => return Ok(None),
        };
        let mut guard = client
            .lock()
            .map_err(|e| CodeContextError::LspError(format!("failed to lock LSP client: {}", e)))?;
        let rpc = match guard.as_mut() {
            Some(rpc) => rpc,
            None => return Ok(None),
        };

        let uri = file_path_to_uri(file_path);
        let text = std::fs::read_to_string(file_path).unwrap_or_default();
        let lang = language_id_from_path(file_path);

        // didOpen
        rpc.send_notification(
            "textDocument/didOpen",
            serde_json::json!({
                "textDocument": {
                    "uri": uri,
                    "languageId": lang,
                    "version": 1,
                    "text": text
                }
            }),
        )?;

        // Run the caller's requests
        let result = f(rpc);

        // didClose -- always sent, even if the closure failed
        let _ = rpc.send_notification(
            "textDocument/didClose",
            serde_json::json!({
                "textDocument": { "uri": uri }
            }),
        );

        result.map(Some)
    }

    // === Layer 2: LSP index (lsp_symbols, lsp_call_edges tables) ===

    /// Look up a symbol at the given range from the LSP index.
    pub fn lsp_symbol_at(&self, file_path: &str, range: &LspRange) -> Option<SymbolInfo> {
        self.conn
            .query_row(
                "SELECT id, name, kind, detail, start_line, start_char, end_line, end_char
                 FROM lsp_symbols
                 WHERE file_path = ?1
                   AND start_line <= ?2 AND end_line >= ?3
                 ORDER BY (end_line - start_line) ASC
                 LIMIT 1",
                rusqlite::params![file_path, range.start_line as i64, range.end_line as i64,],
                |row| {
                    Ok(SymbolInfo {
                        name: row.get(1)?,
                        qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                        kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                        detail: row.get(3)?,
                        file_path: file_path.to_string(),
                        range: LspRange {
                            start_line: row.get::<_, i32>(4)? as u32,
                            start_character: row.get::<_, i32>(5)? as u32,
                            end_line: row.get::<_, i32>(6)? as u32,
                            end_character: row.get::<_, i32>(7)? as u32,
                        },
                    })
                },
            )
            .ok()
    }

    /// List all symbols in a file from the LSP index.
    pub fn lsp_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT id, name, kind, detail, start_line, start_char, end_line, end_char
             FROM lsp_symbols WHERE file_path = ?1
             ORDER BY start_line",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([file_path], |row| {
            Ok(SymbolInfo {
                name: row.get(1)?,
                qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                detail: row.get(3)?,
                file_path: file_path.to_string(),
                range: LspRange {
                    start_line: row.get::<_, i32>(4)? as u32,
                    start_character: row.get::<_, i32>(5)? as u32,
                    end_line: row.get::<_, i32>(6)? as u32,
                    end_character: row.get::<_, i32>(7)? as u32,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Search for symbols by name from the LSP index.
    pub fn lsp_symbols_by_name(&self, query: &str, max: usize) -> Vec<SymbolInfo> {
        let pattern = format!("%{}%", query);
        let mut stmt = match self.conn.prepare(
            "SELECT id, name, kind, detail, file_path, start_line, start_char, end_line, end_char
             FROM lsp_symbols WHERE name LIKE ?1
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![pattern, max as i64], |row| {
            Ok(SymbolInfo {
                name: row.get(1)?,
                qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                detail: row.get(3)?,
                file_path: row.get(4)?,
                range: LspRange {
                    start_line: row.get::<_, i32>(5)? as u32,
                    start_character: row.get::<_, i32>(6)? as u32,
                    end_line: row.get::<_, i32>(7)? as u32,
                    end_character: row.get::<_, i32>(8)? as u32,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Find callers of a symbol from the LSP call edge index.
    pub fn lsp_callers_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo> {
        self.query_call_edges(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.caller_id = s.id
             WHERE e.callee_id = ?1",
            symbol_id,
        )
    }

    /// Find callees of a symbol from the LSP call edge index.
    pub fn lsp_callees_of(&self, symbol_id: &str) -> Vec<CallEdgeInfo> {
        self.query_call_edges(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.callee_id = s.id
             WHERE e.caller_id = ?1",
            symbol_id,
        )
    }

    /// Shared helper for caller/callee queries.
    fn query_call_edges(&self, sql: &str, symbol_id: &str) -> Vec<CallEdgeInfo> {
        let mut stmt = match self.conn.prepare(sql) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([symbol_id], |row| {
            let from_ranges_json: String = row.get(9)?;
            let call_sites = parse_from_ranges(&from_ranges_json);
            Ok(CallEdgeInfo {
                symbol: SymbolInfo {
                    name: row.get(1)?,
                    qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                    kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                    detail: row.get(3)?,
                    file_path: row.get(4)?,
                    range: LspRange {
                        start_line: row.get::<_, i32>(5)? as u32,
                        start_character: row.get::<_, i32>(6)? as u32,
                        end_line: row.get::<_, i32>(7)? as u32,
                        end_character: row.get::<_, i32>(8)? as u32,
                    },
                },
                call_sites,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // === Layer 3: Tree-sitter index (ts_chunks table) ===

    /// Find the chunk containing the given line from the tree-sitter index.
    pub fn ts_chunk_at(&self, file_path: &str, line: u32) -> Option<ChunkInfo> {
        self.conn
            .query_row(
                "SELECT text, start_line, end_line
                 FROM ts_chunks
                 WHERE file_path = ?1 AND start_line <= ?2 AND end_line >= ?2
                 ORDER BY (end_line - start_line) ASC
                 LIMIT 1",
                rusqlite::params![file_path, line as i64],
                |row| {
                    Ok(ChunkInfo {
                        text: row.get(0)?,
                        file_path: file_path.to_string(),
                        start_line: row.get::<_, i32>(1)? as u32,
                        end_line: row.get::<_, i32>(2)? as u32,
                    })
                },
            )
            .ok()
    }

    /// List all symbols in a file from the tree-sitter index.
    ///
    /// Converts ts_chunks with a `symbol_path` into SymbolInfo entries.
    pub fn ts_symbols_in_file(&self, file_path: &str) -> Vec<SymbolInfo> {
        let mut stmt = match self.conn.prepare(
            "SELECT text, start_line, end_line, symbol_path
             FROM ts_chunks
             WHERE file_path = ?1 AND symbol_path IS NOT NULL
             ORDER BY start_line",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map([file_path], |row| {
            let symbol_path: String = row.get(3)?;
            let name = symbol_path
                .rsplit("::")
                .next()
                .unwrap_or(&symbol_path)
                .to_string();
            Ok(SymbolInfo {
                name,
                qualified_path: Some(symbol_path),
                kind: "chunk".to_string(),
                detail: None,
                file_path: file_path.to_string(),
                range: LspRange {
                    start_line: row.get::<_, i32>(1)? as u32,
                    start_character: 0,
                    end_line: row.get::<_, i32>(2)? as u32,
                    end_character: 0,
                },
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Search for chunks matching a text query in the tree-sitter index.
    pub fn ts_chunks_matching(&self, query: &str, max: usize) -> Vec<ChunkInfo> {
        let pattern = format!("%{}%", query);
        let mut stmt = match self.conn.prepare(
            "SELECT text, file_path, start_line, end_line
             FROM ts_chunks WHERE text LIKE ?1
             LIMIT ?2",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        stmt.query_map(rusqlite::params![pattern, max as i64], |row| {
            Ok(ChunkInfo {
                text: row.get(0)?,
                file_path: row.get(1)?,
                start_line: row.get::<_, i32>(2)? as u32,
                end_line: row.get::<_, i32>(3)? as u32,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    /// Find callers of a symbol from the tree-sitter call edge index.
    pub fn ts_callers_of(&self, file_path: &str, symbol: &str) -> Vec<CallEdgeInfo> {
        // Tree-sitter call edges use the same lsp_call_edges table with source='treesitter'
        let mut stmt = match self.conn.prepare(
            "SELECT s.id, s.name, s.kind, s.detail, s.file_path,
                    s.start_line, s.start_char, s.end_line, s.end_char,
                    e.from_ranges
             FROM lsp_call_edges e
             JOIN lsp_symbols s ON e.caller_id = s.id
             WHERE e.callee_file = ?1 AND s.name LIKE ?2 AND e.source = 'treesitter'",
        ) {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let pattern = format!("%{}%", symbol);
        stmt.query_map(rusqlite::params![file_path, pattern], |row| {
            let from_ranges_json: String = row.get(9)?;
            let call_sites = parse_from_ranges(&from_ranges_json);
            Ok(CallEdgeInfo {
                symbol: SymbolInfo {
                    name: row.get(1)?,
                    qualified_path: row.get::<_, Option<String>>(0).ok().flatten(),
                    kind: symbol_kind_int_to_string(row.get::<_, i32>(2)?).to_string(),
                    detail: row.get(3)?,
                    file_path: row.get(4)?,
                    range: LspRange {
                        start_line: row.get::<_, i32>(5)? as u32,
                        start_character: row.get::<_, i32>(6)? as u32,
                        end_line: row.get::<_, i32>(7)? as u32,
                        end_character: row.get::<_, i32>(8)? as u32,
                    },
                },
                call_sites,
            })
        })
        .map(|rows| rows.filter_map(|r| r.ok()).collect())
        .unwrap_or_default()
    }

    // === Layered convenience ===

    /// Enrich a location by trying all layers in priority order.
    ///
    /// Tries: LSP index first (live LSP requires async and is skipped here),
    /// then tree-sitter. Returns the best available data with the source layer.
    pub fn enrich_location(&self, file_path: &str, range: &LspRange) -> EnrichmentResult {
        // Try LSP index first
        if let Some(symbol) = self.lsp_symbol_at(file_path, range) {
            return EnrichmentResult {
                symbol: Some(symbol),
                source_layer: SourceLayer::LspIndex,
            };
        }

        // Fall back to tree-sitter
        if let Some(chunk) = self.ts_chunk_at(file_path, range.start_line) {
            let name = chunk.text.lines().next().unwrap_or("").trim().to_string();
            return EnrichmentResult {
                symbol: Some(SymbolInfo {
                    name,
                    qualified_path: None,
                    kind: "chunk".to_string(),
                    detail: None,
                    file_path: file_path.to_string(),
                    range: LspRange {
                        start_line: chunk.start_line,
                        start_character: 0,
                        end_line: chunk.end_line,
                        end_character: 0,
                    },
                }),
                source_layer: SourceLayer::TreeSitter,
            };
        }

        EnrichmentResult {
            symbol: None,
            source_layer: SourceLayer::None,
        }
    }

    /// Find a symbol at a specific position by trying all layers.
    pub fn find_symbol(&self, file_path: &str, line: u32, char: u32) -> Option<SymbolInfo> {
        let range = LspRange {
            start_line: line,
            start_character: char,
            end_line: line,
            end_character: char,
        };
        let result = self.enrich_location(file_path, &range);
        result.symbol
    }

    /// Generate a human-readable notice about which layer provided data.
    ///
    /// Returns `None` for live LSP (best case -- no notice needed).
    pub fn layer_notice(source: SourceLayer) -> Option<String> {
        match source {
            SourceLayer::LiveLsp => None,
            SourceLayer::LspIndex => {
                Some("Results from LSP index (live LSP not available)".to_string())
            }
            SourceLayer::TreeSitter => {
                Some("Results from tree-sitter index only (LSP not available)".to_string())
            }
            SourceLayer::None => Some("No index data available for this location".to_string()),
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Convert an LSP SymbolKind integer to a human-readable string.
///
/// Returns a static string slice to avoid per-call allocation. Callers that
/// need an owned `String` (e.g. for `SymbolInfo.kind`) should call
/// `.to_string()` at the use site.
pub(crate) fn symbol_kind_int_to_string(kind: i32) -> &'static str {
    match kind {
        1 => "file",
        2 => "module",
        3 => "namespace",
        4 => "package",
        5 => "class",
        6 => "method",
        7 => "property",
        8 => "field",
        9 => "constructor",
        10 => "enum",
        11 => "interface",
        12 => "function",
        13 => "variable",
        14 => "constant",
        15 => "string",
        16 => "number",
        17 => "boolean",
        18 => "array",
        19 => "object",
        20 => "key",
        21 => "null",
        22 => "enum_member",
        23 => "struct",
        24 => "event",
        25 => "operator",
        26 => "type_parameter",
        _ => "unknown",
    }
}

/// Parse the `from_ranges` JSON column from lsp_call_edges into LspRange entries.
fn parse_from_ranges(json: &str) -> Vec<LspRange> {
    // Format is a JSON array of range objects like:
    // [{"start":{"line":10,"character":5},"end":{"line":10,"character":15}}]
    let parsed: Result<Vec<serde_json::Value>, _> = serde_json::from_str(json);
    match parsed {
        Ok(ranges) => ranges
            .iter()
            .filter_map(|r| {
                let start = r.get("start")?;
                let end = r.get("end")?;
                Some(LspRange {
                    start_line: start.get("line")?.as_u64()? as u32,
                    start_character: start.get("character")?.as_u64()? as u32,
                    end_line: end.get("line")?.as_u64()? as u32,
                    end_character: end.get("character")?.as_u64()? as u32,
                })
            })
            .collect(),
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_call_edge, insert_file, insert_ts_chunk, test_db};

    /// Insert an LSP symbol (without detail, for layered_context tests).
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: i32,
        start_char: i32,
        end_line: i32,
        end_char: i32,
    ) {
        crate::test_fixtures::insert_lsp_symbol(
            conn, id, name, kind, file_path, start_line, start_char, end_line, end_char, None,
        );
    }

    // --- Layer availability ---

    #[test]
    fn test_has_live_lsp_returns_false_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_live_lsp());
    }

    #[test]
    fn test_has_live_lsp_returns_false_when_client_is_none() {
        let conn = test_db();
        let shared: SharedLspClient = std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));
        assert!(!ctx.has_live_lsp());
    }

    #[test]
    fn test_has_lsp_index_returns_true_when_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.has_lsp_index("src/main.rs"));
    }

    #[test]
    fn test_has_lsp_index_returns_false_when_not_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);
        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_lsp_index("src/main.rs"));
    }

    #[test]
    fn test_has_ts_index_returns_true_when_indexed() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.has_ts_index("src/main.rs"));
    }

    // --- Layer 1: Live LSP ---

    #[test]
    fn test_lsp_request_returns_ok_none_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let result = ctx
            .lsp_request("textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_lsp_request_returns_ok_none_when_client_is_none() {
        let conn = test_db();
        let shared: SharedLspClient = std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let result = ctx
            .lsp_request("textDocument/hover", serde_json::json!({}))
            .unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_lsp_notify_succeeds_when_no_client() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        ctx.lsp_notify("textDocument/didOpen", serde_json::json!({}))
            .unwrap();
    }

    // --- Layer 2: LSP index ---

    #[test]
    fn test_lsp_symbol_at_returns_data_from_lsp_symbols() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let symbol = ctx.lsp_symbol_at("src/main.rs", &range);
        assert!(symbol.is_some());
        let sym = symbol.unwrap();
        assert_eq!(sym.name, "main");
        assert_eq!(sym.kind, "function");
    }

    #[test]
    fn test_lsp_symbol_at_returns_none_when_no_match() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        assert!(ctx.lsp_symbol_at("src/main.rs", &range).is_none());
    }

    #[test]
    fn test_lsp_symbols_in_file_returns_ordered_list() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "foo", 12, "src/lib.rs", 10, 0, 20, 1);
        insert_lsp_symbol(&conn, "sym2", "bar", 12, "src/lib.rs", 1, 0, 8, 1);

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.lsp_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "bar");
        assert_eq!(symbols[1].name, "foo");
    }

    #[test]
    fn test_lsp_symbols_by_name_finds_matches() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "process_request",
            12,
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym2",
            "handle_response",
            12,
            "src/lib.rs",
            20,
            0,
            30,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.lsp_symbols_by_name("process", 10);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].name, "process_request");
    }

    // --- Layer 3: Tree-sitter index ---

    #[test]
    fn test_ts_chunk_at_returns_data_from_ts_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/main.rs",
            5,
            20,
            "fn main() {\n    println!(\"hello\");\n}",
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let chunk = ctx.ts_chunk_at("src/main.rs", 10);
        assert!(chunk.is_some());
        let c = chunk.unwrap();
        assert_eq!(c.start_line, 5);
        assert_eq!(c.end_line, 20);
        assert!(c.text.contains("fn main()"));
    }

    #[test]
    fn test_ts_chunk_at_returns_none_when_no_match() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);

        let ctx = LayeredContext::new(&conn, None);
        assert!(ctx.ts_chunk_at("src/main.rs", 100).is_none());
    }

    #[test]
    fn test_ts_symbols_in_file_returns_named_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(&conn, "src/lib.rs", 1, 10, "fn foo() {}", Some("lib::foo"));
        insert_ts_chunk(&conn, "src/lib.rs", 15, 25, "fn bar() {}", Some("lib::bar"));
        insert_ts_chunk(&conn, "src/lib.rs", 30, 40, "// comment block", None);

        let ctx = LayeredContext::new(&conn, None);
        let symbols = ctx.ts_symbols_in_file("src/lib.rs");
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "foo");
        assert_eq!(symbols[1].name, "bar");
    }

    #[test]
    fn test_ts_chunks_matching_finds_by_text() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn hello_world() {}", None);
        insert_ts_chunk(&conn, "src/main.rs", 10, 15, "fn goodbye() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_chunks_matching("hello", 10);
        assert_eq!(results.len(), 1);
        assert!(results[0].text.contains("hello_world"));
    }

    // --- Layered convenience ---

    #[test]
    fn test_enrich_location_prefers_lsp_index_over_treesitter() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);
        insert_ts_chunk(&conn, "src/main.rs", 5, 20, "fn main() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert!(result.symbol.is_some());
        assert_eq!(result.symbol.unwrap().name, "main");
    }

    #[test]
    fn test_enrich_location_falls_back_to_treesitter_when_lsp_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 5, 20, "fn main() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert!(result.symbol.is_some());
    }

    #[test]
    fn test_enrich_location_returns_none_when_all_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 10,
            start_character: 0,
            end_line: 10,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }

    // --- Shared types serialization ---

    #[test]
    fn test_lsp_range_serializable() {
        let range = LspRange {
            start_line: 1,
            start_character: 5,
            end_line: 10,
            end_character: 20,
        };
        let json = serde_json::to_string(&range).unwrap();
        let roundtrip: LspRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, roundtrip);
    }

    #[test]
    fn test_source_layer_serializable() {
        let layer = SourceLayer::LspIndex;
        let json = serde_json::to_string(&layer).unwrap();
        let roundtrip: SourceLayer = serde_json::from_str(&json).unwrap();
        assert_eq!(layer, roundtrip);
    }

    #[test]
    fn test_enrichment_result_serializable() {
        let result = EnrichmentResult {
            symbol: Some(SymbolInfo {
                name: "test".to_string(),
                qualified_path: None,
                kind: "function".to_string(),
                detail: None,
                file_path: "test.rs".to_string(),
                range: LspRange {
                    start_line: 0,
                    start_character: 0,
                    end_line: 5,
                    end_character: 0,
                },
            }),
            source_layer: SourceLayer::TreeSitter,
        };
        let json = serde_json::to_string(&result).unwrap();
        let _roundtrip: EnrichmentResult = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_layer_notice_live_lsp_returns_none() {
        assert!(LayeredContext::layer_notice(SourceLayer::LiveLsp).is_none());
    }

    #[test]
    fn test_layer_notice_lsp_index_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::LspIndex);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("LSP index"));
    }

    #[test]
    fn test_layer_notice_treesitter_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::TreeSitter);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("tree-sitter"));
    }

    #[test]
    fn test_layer_notice_none_returns_message() {
        let notice = LayeredContext::layer_notice(SourceLayer::None);
        assert!(notice.is_some());
        assert!(notice.unwrap().contains("No index data"));
    }

    #[test]
    fn test_find_symbol_delegates_to_enrich() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(&conn, "sym1", "main", 12, "src/main.rs", 5, 0, 20, 1);

        let ctx = LayeredContext::new(&conn, None);
        let sym = ctx.find_symbol("src/main.rs", 10, 0);
        assert!(sym.is_some());
        assert_eq!(sym.unwrap().name, "main");
    }

    // --- Helper tests ---

    #[test]
    fn test_symbol_kind_int_to_string() {
        assert_eq!(symbol_kind_int_to_string(12), "function");
        assert_eq!(symbol_kind_int_to_string(5), "class");
        assert_eq!(symbol_kind_int_to_string(23), "struct");
        assert_eq!(symbol_kind_int_to_string(999), "unknown");
    }

    #[test]
    fn test_parse_from_ranges_valid() {
        let json = r#"[{"start":{"line":10,"character":5},"end":{"line":10,"character":15}}]"#;
        let ranges = parse_from_ranges(json);
        assert_eq!(ranges.len(), 1);
        assert_eq!(ranges[0].start_line, 10);
        assert_eq!(ranges[0].start_character, 5);
    }

    #[test]
    fn test_parse_from_ranges_empty() {
        let ranges = parse_from_ranges("[]");
        assert!(ranges.is_empty());
    }

    #[test]
    fn test_parse_from_ranges_invalid_json() {
        let ranges = parse_from_ranges("not json");
        assert!(ranges.is_empty());
    }

    // --- symbol_kind_int_to_string exhaustive coverage ---

    /// Verify every LSP SymbolKind integer (1-26) maps to the correct string,
    /// and that out-of-range values return "unknown".
    #[test]
    fn test_symbol_kind_int_to_string_all_variants() {
        let expected: &[(i32, &str)] = &[
            (1, "file"),
            (2, "module"),
            (3, "namespace"),
            (4, "package"),
            (5, "class"),
            (6, "method"),
            (7, "property"),
            (8, "field"),
            (9, "constructor"),
            (10, "enum"),
            (11, "interface"),
            (12, "function"),
            (13, "variable"),
            (14, "constant"),
            (15, "string"),
            (16, "number"),
            (17, "boolean"),
            (18, "array"),
            (19, "object"),
            (20, "key"),
            (21, "null"),
            (22, "enum_member"),
            (23, "struct"),
            (24, "event"),
            (25, "operator"),
            (26, "type_parameter"),
        ];
        for &(kind, label) in expected {
            assert_eq!(
                symbol_kind_int_to_string(kind),
                label,
                "SymbolKind {} should map to {:?}",
                kind,
                label,
            );
        }
    }

    /// Out-of-range values (0, negative, >26) all return "unknown".
    #[test]
    fn test_symbol_kind_int_to_string_unknown_cases() {
        for kind in [0, -1, 27, 100, i32::MAX, i32::MIN] {
            assert_eq!(
                symbol_kind_int_to_string(kind),
                "unknown",
                "SymbolKind {} should be unknown",
                kind,
            );
        }
    }

    // --- lsp_callees_of coverage ---

    /// When a caller symbol has call edges, lsp_callees_of returns the callee symbols.
    #[test]
    fn test_lsp_callees_of_returns_callee_symbols() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_file(&conn, "src/helper.rs", 1, 1);

        // The caller symbol
        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "do_work",
            12,
            "src/main.rs",
            1,
            0,
            10,
            1,
        );
        // Two callee symbols
        insert_lsp_symbol(
            &conn,
            "sym:callee_a",
            "helper_a",
            12,
            "src/helper.rs",
            1,
            0,
            5,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:callee_b",
            "helper_b",
            6,
            "src/helper.rs",
            10,
            0,
            20,
            1,
        );

        // Edges: caller -> callee_a and caller -> callee_b
        let from_ranges_json =
            r#"[{"start":{"line":3,"character":4},"end":{"line":3,"character":12}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:callee_a",
            "src/main.rs",
            "src/helper.rs",
            "lsp",
            from_ranges_json,
        );
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:callee_b",
            "src/main.rs",
            "src/helper.rs",
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let callees = ctx.lsp_callees_of("sym:caller");

        assert_eq!(callees.len(), 2);

        let names: Vec<&str> = callees.iter().map(|c| c.symbol.name.as_str()).collect();
        assert!(names.contains(&"helper_a"));
        assert!(names.contains(&"helper_b"));

        // Verify the first callee's call_sites were parsed from from_ranges
        let a = callees
            .iter()
            .find(|c| c.symbol.name == "helper_a")
            .unwrap();
        assert_eq!(a.call_sites.len(), 1);
        assert_eq!(a.call_sites[0].start_line, 3);
        assert_eq!(a.call_sites[0].start_character, 4);

        // Verify kind was translated through symbol_kind_int_to_string
        assert_eq!(a.symbol.kind, "function");
        let b = callees
            .iter()
            .find(|c| c.symbol.name == "helper_b")
            .unwrap();
        assert_eq!(b.symbol.kind, "method");
    }

    /// When a symbol has no outgoing call edges, lsp_callees_of returns an empty vec.
    #[test]
    fn test_lsp_callees_of_returns_empty_when_no_edges() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym:lonely",
            "lonely_fn",
            12,
            "src/main.rs",
            1,
            0,
            5,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let callees = ctx.lsp_callees_of("sym:lonely");
        assert!(callees.is_empty());
    }

    // --- ts_callers_of ---

    /// A single tree-sitter call edge is returned with correct symbol info and call sites.
    #[test]
    fn test_ts_callers_of_single_caller() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "caller1",
            "run_process",
            12,
            "src/main.rs",
            10,
            0,
            25,
            1,
        );
        insert_lsp_symbol(&conn, "callee1", "do_work", 12, "src/lib.rs", 1, 0, 5, 1);

        let ranges_json =
            r#"[{"start":{"line":15,"character":4},"end":{"line":15,"character":20}}]"#;
        insert_call_edge(
            &conn,
            "caller1",
            "callee1",
            "src/main.rs",
            "src/lib.rs",
            "treesitter",
            ranges_json,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "process");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].symbol.name, "run_process");
        assert_eq!(results[0].symbol.file_path, "src/main.rs");
        assert_eq!(results[0].symbol.kind, "function");
        assert_eq!(results[0].symbol.range.start_line, 10);
        assert_eq!(results[0].symbol.range.end_line, 25);
        assert_eq!(results[0].call_sites.len(), 1);
        assert_eq!(results[0].call_sites[0].start_line, 15);
        assert_eq!(results[0].call_sites[0].start_character, 4);
    }

    /// Multiple callers from different files are all returned.
    #[test]
    fn test_ts_callers_of_multiple_callers() {
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/a.rs", 1, 0);
        insert_file(&conn, "src/b.rs", 1, 0);

        insert_lsp_symbol(&conn, "c1", "invoke_target", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "c2", "call_target", 12, "src/b.rs", 5, 0, 15, 1);
        insert_lsp_symbol(&conn, "t1", "some_target", 12, "src/target.rs", 1, 0, 20, 1);

        let ranges1 = r#"[{"start":{"line":3,"character":0},"end":{"line":3,"character":10}}]"#;
        let ranges2 = r#"[{"start":{"line":8,"character":2},"end":{"line":8,"character":12}}]"#;
        insert_call_edge(
            &conn,
            "c1",
            "t1",
            "src/a.rs",
            "src/target.rs",
            "treesitter",
            ranges1,
        );
        insert_call_edge(
            &conn,
            "c2",
            "t1",
            "src/b.rs",
            "src/target.rs",
            "treesitter",
            ranges2,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/target.rs", "target");
        assert_eq!(results.len(), 2);

        let names: Vec<&str> = results.iter().map(|r| r.symbol.name.as_str()).collect();
        assert!(names.contains(&"invoke_target"));
        assert!(names.contains(&"call_target"));
    }

    /// When no edges exist for a file/symbol, ts_callers_of returns an empty vec.
    #[test]
    fn test_ts_callers_of_no_callers() {
        let conn = test_db();
        insert_file(&conn, "src/orphan.rs", 1, 0);

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/orphan.rs", "some_func");
        assert!(results.is_empty());
    }

    /// Edges with source != 'treesitter' are excluded from ts_callers_of results.
    #[test]
    fn test_ts_callers_of_ignores_non_treesitter_edges() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "lsp_caller",
            "do_stuff",
            12,
            "src/main.rs",
            1,
            0,
            10,
            1,
        );
        insert_lsp_symbol(&conn, "target", "the_target", 12, "src/lib.rs", 1, 0, 5, 1);

        let ranges = r#"[{"start":{"line":5,"character":0},"end":{"line":5,"character":8}}]"#;
        insert_call_edge(
            &conn,
            "lsp_caller",
            "target",
            "src/main.rs",
            "src/lib.rs",
            "lsp",
            ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "stuff");
        assert!(results.is_empty());
    }

    /// Multiple call sites within a single edge are all parsed and returned.
    #[test]
    fn test_ts_callers_of_parses_multiple_call_sites() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_file(&conn, "src/main.rs", 1, 0);

        insert_lsp_symbol(
            &conn,
            "multi",
            "run_process",
            12,
            "src/main.rs",
            1,
            0,
            50,
            1,
        );
        insert_lsp_symbol(&conn, "callee", "target_fn", 12, "src/lib.rs", 1, 0, 10, 1);

        let ranges = r#"[{"start":{"line":10,"character":4},"end":{"line":10,"character":15}},{"start":{"line":30,"character":8},"end":{"line":30,"character":19}}]"#;
        insert_call_edge(
            &conn,
            "multi",
            "callee",
            "src/main.rs",
            "src/lib.rs",
            "treesitter",
            ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let results = ctx.ts_callers_of("src/lib.rs", "process");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].call_sites.len(), 2);
        assert_eq!(results[0].call_sites[0].start_line, 10);
        assert_eq!(results[0].call_sites[0].start_character, 4);
        assert_eq!(results[0].call_sites[1].start_line, 30);
        assert_eq!(results[0].call_sites[1].start_character, 8);
    }

    // --- enrich_location additional coverage ---

    /// Verify the symbol fields produced by the tree-sitter fallback path.
    #[test]
    fn test_enrich_location_treesitter_symbol_fields() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/lib.rs",
            10,
            25,
            "fn process_data() {\n    let x = 42;\n}",
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 15,
            start_character: 0,
            end_line: 15,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/lib.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);

        let sym = result.symbol.expect("should have symbol");
        // Name is extracted from the first line of chunk text, trimmed.
        assert_eq!(sym.name, "fn process_data() {");
        assert_eq!(sym.kind, "chunk");
        assert_eq!(sym.file_path, "src/lib.rs");
        assert_eq!(sym.range.start_line, 10);
        assert_eq!(sym.range.end_line, 25);
        assert_eq!(sym.range.start_character, 0);
        assert_eq!(sym.range.end_character, 0);
    }

    /// A file that does not exist in the DB at all yields SourceLayer::None.
    #[test]
    fn test_enrich_location_file_not_in_db() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 0,
            start_character: 0,
            end_line: 0,
            end_character: 0,
        };
        let result = ctx.enrich_location("nonexistent.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }

    /// A range that falls outside all indexed chunks yields SourceLayer::None.
    #[test]
    fn test_enrich_location_range_outside_all_chunks() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(&conn, "src/main.rs", 1, 5, "fn small() {}", None);

        let ctx = LayeredContext::new(&conn, None);
        let range = LspRange {
            start_line: 500,
            start_character: 0,
            end_line: 500,
            end_character: 0,
        };
        let result = ctx.enrich_location("src/main.rs", &range);
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.symbol.is_none());
    }
}
