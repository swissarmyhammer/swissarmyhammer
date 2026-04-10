//! Live workspace symbol search with layered resolution.
//!
//! Searches for symbols across the entire workspace using three data layers
//! in priority order:
//!
//! 1. **Live LSP** -- sends `workspace/symbol` to a running LSP server.
//! 2. **LSP index** -- searches persisted symbols by name via `lsp_symbols_by_name`.
//! 3. **Tree-sitter** -- text matching against chunk contents via `ts_chunks_matching`.
//!
//! The caller receives the best available data along with a [`SourceLayer`]
//! indicating which layer produced each result.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{LayeredContext, LspRange, SourceLayer, SymbolInfo};
use crate::ops::lsp_helpers::uri_to_file_path;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `workspace_symbol_live` operation.
#[derive(Debug, Clone)]
pub struct WorkspaceSymbolLiveOptions {
    /// The query string to search for.
    pub query: String,
    /// Maximum number of results to return.
    pub max_results: usize,
}

/// A single result from the workspace symbol search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSymbolResult {
    /// Symbol information.
    pub symbol: SymbolInfo,
    /// Which data layer provided this result.
    pub source_layer: SourceLayer,
}

/// Full result set from the workspace symbol search.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceSymbolLiveResult {
    /// The matched symbols, up to `max_results`.
    pub symbols: Vec<WorkspaceSymbolResult>,
    /// Which layer produced the results. If symbols come from multiple layers
    /// this reflects the highest-priority layer that contributed.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Search for symbols across the workspace using layered resolution.
///
/// Tries live LSP first (`workspace/symbol`), then the LSP symbol index,
/// then the tree-sitter chunk index. Returns results from the first layer
/// that produces any matches.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The query and max_results configuration.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn workspace_symbol_live(
    ctx: &LayeredContext,
    opts: &WorkspaceSymbolLiveOptions,
) -> Result<WorkspaceSymbolLiveResult, crate::error::CodeContextError> {
    // Layer 1: Live LSP
    if ctx.has_live_lsp() {
        let results = try_live_lsp(ctx, opts)?;
        if !results.is_empty() {
            return Ok(WorkspaceSymbolLiveResult {
                symbols: results,
                source_layer: SourceLayer::LiveLsp,
            });
        }
    }

    // Layer 2: LSP index
    let results = try_lsp_index(ctx, opts);
    if !results.is_empty() {
        return Ok(WorkspaceSymbolLiveResult {
            symbols: results,
            source_layer: SourceLayer::LspIndex,
        });
    }

    // Layer 3: Tree-sitter
    let results = try_treesitter(ctx, opts);
    if !results.is_empty() {
        return Ok(WorkspaceSymbolLiveResult {
            symbols: results,
            source_layer: SourceLayer::TreeSitter,
        });
    }

    Ok(WorkspaceSymbolLiveResult {
        symbols: vec![],
        source_layer: SourceLayer::None,
    })
}

// ---------------------------------------------------------------------------
// Layer 1: Live LSP
// ---------------------------------------------------------------------------

/// Attempt to search symbols via a live LSP server.
///
/// Sends `workspace/symbol` with the query string. Parses the response
/// (an array of `SymbolInformation`) into `WorkspaceSymbolResult` entries.
fn try_live_lsp(
    ctx: &LayeredContext,
    opts: &WorkspaceSymbolLiveOptions,
) -> Result<Vec<WorkspaceSymbolResult>, crate::error::CodeContextError> {
    let response = ctx.lsp_request(
        "workspace/symbol",
        json!({
            "query": opts.query
        }),
    )?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(vec![]),
    };

    let symbols = parse_workspace_symbols(&response);
    let truncated: Vec<WorkspaceSymbolResult> = symbols
        .into_iter()
        .take(opts.max_results)
        .map(|sym| WorkspaceSymbolResult {
            symbol: sym,
            source_layer: SourceLayer::LiveLsp,
        })
        .collect();

    Ok(truncated)
}

// ---------------------------------------------------------------------------
// Layer 2: LSP index
// ---------------------------------------------------------------------------

/// Search symbols from the persisted LSP symbol index.
fn try_lsp_index(
    ctx: &LayeredContext,
    opts: &WorkspaceSymbolLiveOptions,
) -> Vec<WorkspaceSymbolResult> {
    ctx.lsp_symbols_by_name(&opts.query, opts.max_results)
        .into_iter()
        .map(|sym| WorkspaceSymbolResult {
            symbol: sym,
            source_layer: SourceLayer::LspIndex,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Layer 3: Tree-sitter
// ---------------------------------------------------------------------------

/// Search symbols from the tree-sitter chunk index.
///
/// Converts matching chunks into `SymbolInfo` entries. Since chunks don't
/// have structured symbol names, we use the chunk text as a fallback.
fn try_treesitter(
    ctx: &LayeredContext,
    opts: &WorkspaceSymbolLiveOptions,
) -> Vec<WorkspaceSymbolResult> {
    ctx.ts_chunks_matching(&opts.query, opts.max_results)
        .into_iter()
        .map(|chunk| {
            let name = chunk
                .text
                .lines()
                .next()
                .unwrap_or(&chunk.text)
                .trim()
                .chars()
                .take(80)
                .collect::<String>();
            WorkspaceSymbolResult {
                symbol: SymbolInfo {
                    name,
                    qualified_path: None,
                    kind: "chunk".to_string(),
                    detail: None,
                    file_path: chunk.file_path,
                    range: LspRange {
                        start_line: chunk.start_line,
                        start_character: 0,
                        end_line: chunk.end_line,
                        end_character: 0,
                    },
                },
                source_layer: SourceLayer::TreeSitter,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// LSP workspace/symbol response parsing
// ---------------------------------------------------------------------------

/// Parse an LSP `workspace/symbol` response into `SymbolInfo` entries.
///
/// Handles both the `SymbolInformation[]` format (LSP 3.x) and the
/// `WorkspaceSymbol[]` format (LSP 3.17+). Both share the same essential
/// fields: `name`, `kind`, `location` (or `location.uri` + `location.range`).
pub fn parse_workspace_symbols(response: &serde_json::Value) -> Vec<SymbolInfo> {
    let items = match response.as_array() {
        Some(arr) => arr,
        None => return vec![],
    };

    items.iter().filter_map(parse_symbol_information).collect()
}

/// Parse a single `SymbolInformation` or `WorkspaceSymbol` JSON object.
fn parse_symbol_information(item: &serde_json::Value) -> Option<SymbolInfo> {
    let name = item.get("name")?.as_str()?.to_string();
    let kind_int = item.get("kind")?.as_u64()? as i32;
    let kind = crate::layered_context::symbol_kind_int_to_string(kind_int).to_string();

    let detail = item
        .get("detail")
        .and_then(|d| d.as_str())
        .map(|s| s.to_string());

    let container_name = item
        .get("containerName")
        .and_then(|c| c.as_str())
        .map(|s| s.to_string());

    let qualified_path = container_name.map(|c| format!("{}::{}", c, name));

    // location field (SymbolInformation has { uri, range })
    let location = item.get("location")?;
    let uri = location.get("uri")?.as_str()?;
    let file_path = uri_to_file_path(uri);

    let range = location.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;

    Some(SymbolInfo {
        name,
        qualified_path,
        kind,
        detail,
        file_path,
        range: LspRange {
            start_line: start.get("line")?.as_u64()? as u32,
            start_character: start.get("character")?.as_u64()? as u32,
            end_line: end.get("line")?.as_u64()? as u32,
            end_character: end.get("character")?.as_u64()? as u32,
        },
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_file, test_db};
    use rusqlite::Connection;

    /// Insert an LSP symbol (detail-before-file_path order used by these tests).
    #[allow(clippy::too_many_arguments)]
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        detail: Option<&str>,
        file_path: &str,
        start_line: i32,
        start_char: i32,
        end_line: i32,
        end_char: i32,
    ) {
        crate::test_fixtures::insert_lsp_symbol(
            conn, id, name, kind, file_path, start_line, start_char, end_line, end_char, detail,
        );
    }

    /// Insert a tree-sitter chunk (no symbol_path needed by these tests).
    fn insert_ts_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: i32,
        end_line: i32,
        text: &str,
    ) {
        crate::test_fixtures::insert_ts_chunk(conn, file_path, start_line, end_line, text, None);
    }

    // --- workspace/symbol response parsing tests ---

    #[test]
    fn test_parse_workspace_symbols_standard_format() {
        let response = serde_json::json!([
            {
                "name": "MyStruct",
                "kind": 23,
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": {
                        "start": { "line": 10, "character": 0 },
                        "end": { "line": 20, "character": 1 }
                    }
                }
            },
            {
                "name": "process",
                "kind": 12,
                "containerName": "MyStruct",
                "detail": "fn(x: u32) -> bool",
                "location": {
                    "uri": "file:///src/lib.rs",
                    "range": {
                        "start": { "line": 15, "character": 4 },
                        "end": { "line": 18, "character": 5 }
                    }
                }
            }
        ]);

        let symbols = parse_workspace_symbols(&response);
        assert_eq!(symbols.len(), 2);

        assert_eq!(symbols[0].name, "MyStruct");
        assert_eq!(symbols[0].kind, "struct");
        assert_eq!(symbols[0].file_path, "/src/lib.rs");
        assert_eq!(symbols[0].range.start_line, 10);
        assert!(symbols[0].qualified_path.is_none());

        assert_eq!(symbols[1].name, "process");
        assert_eq!(symbols[1].kind, "function");
        assert_eq!(symbols[1].detail.as_deref(), Some("fn(x: u32) -> bool"));
        assert_eq!(
            symbols[1].qualified_path.as_deref(),
            Some("MyStruct::process")
        );
    }

    #[test]
    fn test_parse_workspace_symbols_empty_response() {
        let response = serde_json::json!([]);
        let symbols = parse_workspace_symbols(&response);
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_parse_workspace_symbols_null_response() {
        let response = serde_json::json!(null);
        let symbols = parse_workspace_symbols(&response);
        assert!(symbols.is_empty());
    }

    #[test]
    fn test_parse_workspace_symbols_malformed_item_skipped() {
        let response = serde_json::json!([
            { "name": "valid", "kind": 12, "location": {
                "uri": "file:///a.rs",
                "range": { "start": {"line": 0, "character": 0}, "end": {"line": 1, "character": 0} }
            }},
            { "bad": "data" },
            { "name": "also_valid", "kind": 5, "location": {
                "uri": "file:///b.rs",
                "range": { "start": {"line": 5, "character": 0}, "end": {"line": 10, "character": 0} }
            }}
        ]);
        let symbols = parse_workspace_symbols(&response);
        assert_eq!(symbols.len(), 2);
        assert_eq!(symbols[0].name, "valid");
        assert_eq!(symbols[1].name, "also_valid");
    }

    // --- Fallback to LSP index ---

    #[test]
    fn test_fallback_to_lsp_index() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "process_data",
            12,
            Some("fn() -> Result"),
            "src/main.rs",
            5,
            0,
            20,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym2",
            "process_event",
            12,
            None,
            "src/main.rs",
            25,
            0,
            40,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = WorkspaceSymbolLiveOptions {
            query: "process".to_string(),
            max_results: 10,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.symbols.len(), 2);
        assert!(result
            .symbols
            .iter()
            .all(|s| s.source_layer == SourceLayer::LspIndex));
    }

    // --- Fallback to tree-sitter ---

    #[test]
    fn test_fallback_to_treesitter() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/main.rs",
            5,
            20,
            "fn process_data() {\n    println!(\"hello\");\n}",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = WorkspaceSymbolLiveOptions {
            query: "process".to_string(),
            max_results: 10,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.symbols[0].symbol.kind, "chunk");
    }

    // --- max_results truncation ---

    #[test]
    fn test_max_results_truncation() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        for i in 0..5 {
            insert_lsp_symbol(
                &conn,
                &format!("sym{}", i),
                &format!("handler_{}", i),
                12,
                None,
                "src/main.rs",
                i * 10,
                0,
                (i + 1) * 10,
                1,
            );
        }

        let ctx = LayeredContext::new(&conn, None);
        let opts = WorkspaceSymbolLiveOptions {
            query: "handler".to_string(),
            max_results: 2,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert!(
            result.symbols.len() <= 2,
            "expected at most 2, got {}",
            result.symbols.len()
        );
    }

    // --- No data returns empty ---

    #[test]
    fn test_no_data_returns_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let opts = WorkspaceSymbolLiveOptions {
            query: "nonexistent".to_string(),
            max_results: 10,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert!(result.symbols.is_empty());
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- uri_to_file_path ---

    #[test]
    fn test_uri_to_file_path_strips_scheme() {
        assert_eq!(
            uri_to_file_path("file:///home/user/project/src/main.rs"),
            "/home/user/project/src/main.rs"
        );
    }

    #[test]
    fn test_uri_to_file_path_passthrough() {
        assert_eq!(uri_to_file_path("src/main.rs"), "src/main.rs");
    }

    // --- WorkspaceSymbolLiveResult serialization ---

    #[test]
    fn test_result_serializable() {
        let result = WorkspaceSymbolLiveResult {
            symbols: vec![WorkspaceSymbolResult {
                symbol: SymbolInfo {
                    name: "foo".to_string(),
                    qualified_path: None,
                    kind: "function".to_string(),
                    detail: None,
                    file_path: "src/lib.rs".to_string(),
                    range: LspRange {
                        start_line: 1,
                        start_character: 0,
                        end_line: 5,
                        end_character: 1,
                    },
                },
                source_layer: SourceLayer::LspIndex,
            }],
            source_layer: SourceLayer::LspIndex,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: WorkspaceSymbolLiveResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.symbols.len(), 1);
        assert_eq!(roundtrip.symbols[0].symbol.name, "foo");
        assert_eq!(roundtrip.source_layer, SourceLayer::LspIndex);
    }

    // --- SharedLspClient with None inner (no connected LSP process) ---

    #[test]
    fn test_shared_lsp_client_with_none_inner_falls_through() {
        // When a SharedLspClient is present but wraps None (no connected LSP
        // process), has_live_lsp() returns false and workspace_symbol_live
        // should fall through to index layers without error.
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = WorkspaceSymbolLiveOptions {
            query: "something".to_string(),
            max_results: 10,
        };
        // No index data exists, so result should be empty.
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert!(
            result.symbols.is_empty(),
            "SharedLspClient(None) with no index data should produce empty results"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_shared_lsp_client_with_none_inner_falls_to_lsp_index() {
        // When has_live_lsp() is false but LSP index data exists, the result
        // should come from the LSP index layer.
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "find_handler",
            12,
            Some("fn find_handler() -> bool"),
            "src/main.rs",
            5,
            0,
            20,
            1,
        );

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = WorkspaceSymbolLiveOptions {
            query: "find".to_string(),
            max_results: 10,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(
            result.source_layer,
            SourceLayer::LspIndex,
            "should fall through to LSP index when live LSP has None inner"
        );
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].symbol.name, "find_handler");
        assert_eq!(result.symbols[0].source_layer, SourceLayer::LspIndex);
    }

    #[test]
    fn test_shared_lsp_client_with_none_inner_falls_to_treesitter() {
        // When has_live_lsp() is false and no LSP index data exists but
        // tree-sitter data does, should fall through to tree-sitter.
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/main.rs",
            1,
            10,
            "fn find_handler() {\n    // body\n}",
        );

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = WorkspaceSymbolLiveOptions {
            query: "find".to_string(),
            max_results: 10,
        };
        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(
            result.source_layer,
            SourceLayer::TreeSitter,
            "should fall through to tree-sitter when live LSP is None and no LSP index data"
        );
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].source_layer, SourceLayer::TreeSitter);
    }

    // --- Mock LSP helper for live-path tests ---

    /// Spawn a Python process that acts as a mock LSP server.
    ///
    /// Each entry in the responses array is either `null` (read a
    /// notification, no reply) or a JSON-RPC response object (read a
    /// request, reply with this object).
    fn spawn_mock_lsp(responses: &[serde_json::Value]) -> std::process::Child {
        let temp_dir = tempfile::tempdir().expect("failed to create temp dir for mock LSP");
        let response_file = temp_dir.path().join("mock_responses.json");
        std::fs::write(&response_file, serde_json::to_string(responses).unwrap())
            .expect("failed to write mock responses file");

        let script = "\
            import sys, json, os\n\
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
            \tsys.stdout.flush()\n\
            with open(os.environ['MOCK_RESPONSE_FILE']) as f:\n\
            \tresponses = json.load(f)\n\
            for resp in responses:\n\
            \tread_msg()\n\
            \tif resp is not None:\n\
            \t\tsend_msg(resp)\n";

        // Leak the tempdir so it outlives the child process.
        std::mem::forget(temp_dir);

        std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .env("MOCK_RESPONSE_FILE", &response_file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn mock LSP python3 process")
    }

    /// Create a `SharedLspClient` from a mock LSP child process.
    fn mock_lsp_client(child: &mut std::process::Child) -> crate::lsp_worker::SharedLspClient {
        use crate::lsp_communication::LspJsonRpcClient;
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let client = LspJsonRpcClient::new(stdin, stdout);
        std::sync::Arc::new(std::sync::Mutex::new(Some(client)))
    }

    // --- Live LSP workspace/symbol tests ---

    #[test]
    fn test_live_lsp_returns_workspace_symbols() {
        // Mock LSP returns a valid workspace/symbol response with two symbols.
        // workspace_symbol_live should return them with SourceLayer::LiveLsp.
        let ws_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "MyStruct",
                    "kind": 23,
                    "location": {
                        "uri": "file:///src/models.rs",
                        "range": {
                            "start": { "line": 5, "character": 0 },
                            "end": { "line": 25, "character": 1 }
                        }
                    }
                },
                {
                    "name": "new",
                    "kind": 12,
                    "containerName": "MyStruct",
                    "detail": "fn() -> MyStruct",
                    "location": {
                        "uri": "file:///src/models.rs",
                        "range": {
                            "start": { "line": 10, "character": 4 },
                            "end": { "line": 15, "character": 5 }
                        }
                    }
                }
            ]
        });

        let responses = vec![ws_response];
        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = WorkspaceSymbolLiveOptions {
            query: "MyStruct".to_string(),
            max_results: 50,
        };

        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LiveLsp);
        assert_eq!(result.symbols.len(), 2);

        assert_eq!(result.symbols[0].symbol.name, "MyStruct");
        assert_eq!(result.symbols[0].symbol.kind, "struct");
        assert_eq!(result.symbols[0].symbol.file_path, "/src/models.rs");
        assert_eq!(result.symbols[0].source_layer, SourceLayer::LiveLsp);

        assert_eq!(result.symbols[1].symbol.name, "new");
        assert_eq!(result.symbols[1].symbol.kind, "function");
        assert_eq!(
            result.symbols[1].symbol.qualified_path.as_deref(),
            Some("MyStruct::new")
        );
        assert_eq!(
            result.symbols[1].symbol.detail.as_deref(),
            Some("fn() -> MyStruct")
        );
        assert_eq!(result.symbols[1].source_layer, SourceLayer::LiveLsp);
    }

    #[test]
    fn test_live_lsp_null_response_falls_through() {
        // Mock LSP returns a null result for workspace/symbol.
        // workspace_symbol_live should fall through to index layers.
        let ws_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": null
        });

        let responses = vec![ws_response];
        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = WorkspaceSymbolLiveOptions {
            query: "anything".to_string(),
            max_results: 10,
        };

        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        // No index data, so should end up empty after falling through all layers.
        assert!(
            result.symbols.is_empty(),
            "null LSP response with no index data should produce empty results"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_live_lsp_empty_array_falls_through() {
        // Mock LSP returns an empty array for workspace/symbol.
        // workspace_symbol_live should fall through to index layers.
        let ws_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let responses = vec![ws_response];
        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = WorkspaceSymbolLiveOptions {
            query: "anything".to_string(),
            max_results: 10,
        };

        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert!(
            result.symbols.is_empty(),
            "empty LSP response with no index data should produce empty results"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_live_lsp_empty_array_falls_through_to_lsp_index() {
        // Mock LSP returns an empty array but LSP index has data.
        // workspace_symbol_live should fall through to LSP index.
        let ws_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let responses = vec![ws_response];
        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "my_handler",
            12,
            None,
            "src/main.rs",
            10,
            0,
            20,
            1,
        );
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = WorkspaceSymbolLiveOptions {
            query: "handler".to_string(),
            max_results: 10,
        };

        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(
            result.source_layer,
            SourceLayer::LspIndex,
            "empty live LSP response should fall through to LSP index"
        );
        assert_eq!(result.symbols.len(), 1);
        assert_eq!(result.symbols[0].symbol.name, "my_handler");
    }

    #[test]
    fn test_live_lsp_truncates_to_max_results() {
        // Mock LSP returns 5 symbols but max_results is 2.
        // workspace_symbol_live should return only 2.
        let mut items = vec![];
        for i in 0..5 {
            items.push(serde_json::json!({
                "name": format!("sym_{}", i),
                "kind": 12,
                "location": {
                    "uri": format!("file:///src/mod{}.rs", i),
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 10, "character": 0 }
                    }
                }
            }));
        }
        let ws_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": items
        });

        let responses = vec![ws_response];
        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = WorkspaceSymbolLiveOptions {
            query: "sym".to_string(),
            max_results: 2,
        };

        let result = workspace_symbol_live(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LiveLsp);
        assert_eq!(
            result.symbols.len(),
            2,
            "should truncate to max_results=2, got {}",
            result.symbols.len()
        );
        // Verify they are the first two items, preserving order.
        assert_eq!(result.symbols[0].symbol.name, "sym_0");
        assert_eq!(result.symbols[1].symbol.name, "sym_1");
    }

    // --- Additional parse_workspace_symbols edge cases ---

    #[test]
    fn test_parse_workspace_symbols_with_missing_range_fields_skipped() {
        // An item with a location but missing range sub-fields should be
        // skipped by parse_symbol_information's filter_map.
        let response = serde_json::json!([
            {
                "name": "good",
                "kind": 6,
                "location": {
                    "uri": "file:///a.rs",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 5, "character": 0 }
                    }
                }
            },
            {
                "name": "missing_end",
                "kind": 6,
                "location": {
                    "uri": "file:///b.rs",
                    "range": {
                        "start": { "line": 0, "character": 0 }
                    }
                }
            },
            {
                "name": "missing_range",
                "kind": 6,
                "location": {
                    "uri": "file:///c.rs"
                }
            }
        ]);
        let symbols = parse_workspace_symbols(&response);
        assert_eq!(symbols.len(), 1, "only the fully valid item should parse");
        assert_eq!(symbols[0].name, "good");
    }

    #[test]
    fn test_parse_workspace_symbols_non_array_response() {
        // A response that is an object (not an array) should return empty.
        let response = serde_json::json!({
            "name": "not_an_array",
            "kind": 12
        });
        let symbols = parse_workspace_symbols(&response);
        assert!(symbols.is_empty(), "non-array response should return empty");
    }

    #[test]
    fn test_parse_workspace_symbols_missing_name_skipped() {
        // An item without a "name" field should be skipped.
        let response = serde_json::json!([
            {
                "kind": 12,
                "location": {
                    "uri": "file:///a.rs",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 1, "character": 0 }
                    }
                }
            }
        ]);
        let symbols = parse_workspace_symbols(&response);
        assert!(symbols.is_empty(), "item without name should be skipped");
    }

    #[test]
    fn test_parse_workspace_symbols_missing_kind_skipped() {
        // An item without a "kind" field should be skipped.
        let response = serde_json::json!([
            {
                "name": "foo",
                "location": {
                    "uri": "file:///a.rs",
                    "range": {
                        "start": { "line": 0, "character": 0 },
                        "end": { "line": 1, "character": 0 }
                    }
                }
            }
        ]);
        let symbols = parse_workspace_symbols(&response);
        assert!(symbols.is_empty(), "item without kind should be skipped");
    }

    #[test]
    fn test_parse_workspace_symbols_missing_location_skipped() {
        // An item without a "location" field should be skipped.
        let response = serde_json::json!([
            {
                "name": "foo",
                "kind": 12
            }
        ]);
        let symbols = parse_workspace_symbols(&response);
        assert!(
            symbols.is_empty(),
            "item without location should be skipped"
        );
    }
}
