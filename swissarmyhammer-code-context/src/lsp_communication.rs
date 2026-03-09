//! LSP JSON-RPC communication and symbol collection.
//!
//! Handles JSON-RPC protocol with LSP server processes.
//! Sends requests for symbols and collects results for database persistence.

use std::io::Write;
use std::path::Path;
use std::process::Child;
use serde_json::{json, Value};
use tracing::debug;
use rusqlite::Connection;

use crate::error::CodeContextError;
use crate::lsp_indexer::{flatten_symbols, write_symbols, mark_lsp_indexed};
use lsp_types::DocumentSymbol;

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

    debug!("Collected and persisted {} symbols for {}", symbol_count, file_path);
    Ok(symbol_count)
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
        // The process is already spawned with piped stdin/stdout
        // We'll use them for communication below
        Ok(Self {
            process,
            request_id: 1,
        })
    }

    /// Send a JSON-RPC request and get the response.
    ///
    /// Returns the response as a serde_json::Value.
    /// This is a simplified version that doesn't handle streaming well;
    /// a production version would use async/await and proper message framing.
    fn send_request(&mut self, method: &str, params: Value) -> Result<Value, CodeContextError> {
        // Format JSON-RPC 2.0 request
        let request = json!({
            "jsonrpc": "2.0",
            "method": method,
            "params": params,
            "id": self.request_id,
        });

        self.request_id += 1;

        // LSP uses Content-Length header + JSON body
        let json_str = request.to_string();
        let content_length = json_str.len();

        // Try to write to process stdin (if available)
        if let Some(mut stdin) = self.process.stdin.take() {
            let msg = format!("Content-Length: {}\r\n\r\n{}", content_length, json_str);
            debug!("Sending LSP request: {}", msg);

            match stdin.write_all(msg.as_bytes()) {
                Ok(_) => {
                    // Restore stdin for next request
                    self.process.stdin = Some(stdin);
                }
                Err(e) => {
                    self.process.stdin = Some(stdin);
                    return Err(CodeContextError::from(e));
                }
            }
        }

        // In a full implementation, we'd read the response here
        // For now, return placeholder
        Ok(json!({}))
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
            Ok(_response) => {
                // In a full implementation:
                // 1. Parse response to get DocumentSymbol array
                // 2. Flatten symbols using flatten_symbols()
                // 3. Write to database using write_symbols()
                // 4. Return actual symbol count

                // For now, return 0 symbols
                Ok(LspCollectionResult {
                    file_path: file_path_str,
                    symbol_count: 0,
                    error: None,
                })
            }
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

        // Build directory URI for LSP (file:///absolute/path)
        let root_str = workspace_root.to_string_lossy().to_string();
        let uri = format!("file://{}", root_str);

        // Send initialize request per LSP spec
        let params = json!({
            "processId": std::process::id() as i32,
            "rootPath": root_str,
            "rootUri": uri,
            "capabilities": {}
        });

        match self.send_request("initialize", params) {
            Ok(_response) => {
                // In a full implementation:
                // 1. Wait for "initialized" notification
                // 2. Verify capabilities
                debug!("LSP server initialized (placeholder)");
                Ok(())
            }
            Err(e) => {
                Err(e)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
