//! Go-to-definition with layered resolution.
//!
//! Returns definition locations for a symbol at a given position using three
//! data layers in priority order:
//!
//! 1. **Live LSP** -- sends `textDocument/definition` to a running LSP server.
//! 2. **LSP index** -- looks up the symbol at the cursor from persisted symbols.
//! 3. **Tree-sitter** -- returns the chunk containing the cursor position.
//!
//! The caller receives the best available data along with a [`SourceLayer`]
//! indicating which layer produced the result.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{DefinitionLocation, LayeredContext, LspRange, SourceLayer};
use crate::ops::lsp_helpers::{
    file_path_to_uri, parse_lsp_range, read_source_range, uri_to_file_path,
};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_definition` operation.
#[derive(Debug, Clone)]
pub struct GetDefinitionOptions {
    /// Path to the file (relative to workspace root).
    pub file_path: String,
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
    /// Whether to include source text from disk at each definition location.
    pub include_source: bool,
}

/// Result of a definition lookup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetDefinitionResult {
    /// The definition locations found.
    pub locations: Vec<DefinitionLocation>,
    /// Which data layer provided the result.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get definition locations for a symbol at a position in a file.
///
/// Tries live LSP first, then the LSP symbol index, then the tree-sitter
/// chunk index. Returns an empty result with `SourceLayer::None` if no
/// layer has data for the position.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The file path, line, character, and include_source flag.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn get_definition(
    ctx: &LayeredContext,
    opts: &GetDefinitionOptions,
) -> Result<GetDefinitionResult, crate::error::CodeContextError> {
    // Layer 1: Live LSP
    if ctx.has_live_lsp() {
        if let Some(result) = try_live_lsp(ctx, opts)? {
            return Ok(result);
        }
    }

    // Layer 2: LSP index
    if let Some(result) = try_lsp_index(ctx, opts) {
        return Ok(result);
    }

    // Layer 3: Tree-sitter
    if let Some(result) = try_treesitter(ctx, opts) {
        return Ok(result);
    }

    Ok(GetDefinitionResult {
        locations: Vec::new(),
        source_layer: SourceLayer::None,
    })
}

// ---------------------------------------------------------------------------
// Layer 1: Live LSP
// ---------------------------------------------------------------------------

/// Attempt to get definition locations from a live LSP server.
///
/// Sends didOpen, textDocument/definition, and didClose atomically under a
/// single mutex hold to prevent interleaving with the indexing worker.
fn try_live_lsp(
    ctx: &LayeredContext,
    opts: &GetDefinitionOptions,
) -> Result<Option<GetDefinitionResult>, crate::error::CodeContextError> {
    let uri = file_path_to_uri(&opts.file_path);

    let response = ctx.lsp_request_with_document(
        &opts.file_path,
        "textDocument/definition",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": opts.line, "character": opts.character }
        }),
    )?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };

    let mut locations = parse_definition_locations(&response);
    if locations.is_empty() {
        return Ok(None);
    }

    // Read source text from disk if requested
    if opts.include_source {
        for loc in &mut locations {
            loc.source_text = read_source_range(&loc.file_path, &loc.range);
        }
    }

    // Enrich each location with symbol info
    for loc in &mut locations {
        let enrichment = ctx.enrich_location(&loc.file_path, &loc.range);
        loc.symbol = enrichment.symbol;
    }

    Ok(Some(GetDefinitionResult {
        locations,
        source_layer: SourceLayer::LiveLsp,
    }))
}

// ---------------------------------------------------------------------------
// Layer 2: LSP index
// ---------------------------------------------------------------------------

/// Attempt to get definition from the persisted LSP symbol index.
///
/// Returns the symbol at the cursor position as a definition location.
/// Less precise than live LSP (no cross-file jump) but works offline.
fn try_lsp_index(ctx: &LayeredContext, opts: &GetDefinitionOptions) -> Option<GetDefinitionResult> {
    let range = LspRange {
        start_line: opts.line,
        start_character: opts.character,
        end_line: opts.line,
        end_character: opts.character,
    };
    let symbol = ctx.lsp_symbol_at(&opts.file_path, &range)?;

    let mut loc = DefinitionLocation {
        file_path: symbol.file_path.clone(),
        range: symbol.range.clone(),
        source_text: None,
        symbol: Some(symbol),
    };

    if opts.include_source {
        loc.source_text = read_source_range(&loc.file_path, &loc.range);
    }

    Some(GetDefinitionResult {
        locations: vec![loc],
        source_layer: SourceLayer::LspIndex,
    })
}

// ---------------------------------------------------------------------------
// Layer 3: Tree-sitter
// ---------------------------------------------------------------------------

/// Attempt to get definition from the tree-sitter chunk index.
///
/// Returns the chunk containing the cursor as a last resort.
fn try_treesitter(
    ctx: &LayeredContext,
    opts: &GetDefinitionOptions,
) -> Option<GetDefinitionResult> {
    let chunk = ctx.ts_chunk_at(&opts.file_path, opts.line)?;

    let loc = DefinitionLocation {
        file_path: opts.file_path.clone(),
        range: LspRange {
            start_line: chunk.start_line,
            start_character: 0,
            end_line: chunk.end_line,
            end_character: 0,
        },
        source_text: if opts.include_source {
            Some(chunk.text.clone())
        } else {
            None
        },
        symbol: None,
    };

    Some(GetDefinitionResult {
        locations: vec![loc],
        source_layer: SourceLayer::TreeSitter,
    })
}

// ---------------------------------------------------------------------------
// LSP definition response parsing
// ---------------------------------------------------------------------------

/// Parse definition locations from an LSP textDocument/definition response.
///
/// Handles three response formats per the LSP spec:
/// - Single `Location { uri, range }`
/// - Array of `Location`
/// - Array of `LocationLink { targetUri, targetRange, targetSelectionRange }`
pub fn parse_definition_locations(response: &serde_json::Value) -> Vec<DefinitionLocation> {
    // Case 1: Single Location object
    if let Some(loc) = try_parse_location(response) {
        return vec![loc];
    }

    // Case 2 & 3: Array of Location or LocationLink
    if let Some(arr) = response.as_array() {
        let mut locations = Vec::new();
        for item in arr {
            if let Some(loc) = try_parse_location(item) {
                locations.push(loc);
            } else if let Some(loc) = try_parse_location_link(item) {
                locations.push(loc);
            }
        }
        return locations;
    }

    Vec::new()
}

/// Try to parse a single LSP Location `{ uri, range }`.
fn try_parse_location(value: &serde_json::Value) -> Option<DefinitionLocation> {
    let uri = value.get("uri")?.as_str()?;
    let range = parse_lsp_range(value.get("range")?)?;

    Some(DefinitionLocation {
        file_path: uri_to_file_path(uri),
        range,
        source_text: None,
        symbol: None,
    })
}

/// Try to parse an LSP LocationLink `{ targetUri, targetRange, targetSelectionRange }`.
fn try_parse_location_link(value: &serde_json::Value) -> Option<DefinitionLocation> {
    let uri = value.get("targetUri")?.as_str()?;
    // Prefer targetSelectionRange (the precise identifier range) over targetRange
    let range = value
        .get("targetSelectionRange")
        .and_then(parse_lsp_range)
        .or_else(|| value.get("targetRange").and_then(parse_lsp_range))?;

    Some(DefinitionLocation {
        file_path: uri_to_file_path(uri),
        range,
        source_text: None,
        symbol: None,
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

    // --- parse_definition_locations: single Location ---

    #[test]
    fn test_parse_single_location() {
        let response = serde_json::json!({
            "uri": "file:///src/main.rs",
            "range": {
                "start": { "line": 10, "character": 4 },
                "end": { "line": 10, "character": 20 }
            }
        });
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file_path, "/src/main.rs");
        assert_eq!(locations[0].range.start_line, 10);
        assert_eq!(locations[0].range.start_character, 4);
    }

    // --- parse_definition_locations: array of Location ---

    #[test]
    fn test_parse_location_array() {
        let response = serde_json::json!([
            {
                "uri": "file:///src/lib.rs",
                "range": {
                    "start": { "line": 5, "character": 0 },
                    "end": { "line": 5, "character": 15 }
                }
            },
            {
                "uri": "file:///src/utils.rs",
                "range": {
                    "start": { "line": 20, "character": 4 },
                    "end": { "line": 25, "character": 1 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].file_path, "/src/lib.rs");
        assert_eq!(locations[1].file_path, "/src/utils.rs");
    }

    // --- parse_definition_locations: LocationLink ---

    #[test]
    fn test_parse_location_link() {
        let response = serde_json::json!([
            {
                "targetUri": "file:///src/types.rs",
                "targetRange": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 10, "character": 1 }
                },
                "targetSelectionRange": {
                    "start": { "line": 1, "character": 11 },
                    "end": { "line": 1, "character": 20 }
                },
                "originSelectionRange": {
                    "start": { "line": 5, "character": 4 },
                    "end": { "line": 5, "character": 13 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file_path, "/src/types.rs");
        // Should use targetSelectionRange (more precise)
        assert_eq!(locations[0].range.start_line, 1);
        assert_eq!(locations[0].range.start_character, 11);
    }

    // --- parse_definition_locations: empty/null ---

    #[test]
    fn test_parse_empty_response() {
        let response = serde_json::json!(null);
        // null gets filtered before parse, but the parser handles it gracefully
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_empty_array() {
        let response = serde_json::json!([]);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    // --- Fallback to LSP index ---

    #[test]
    fn test_fallback_to_lsp_index() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "process",
            12,
            Some("fn() -> MyStruct"),
            "src/main.rs",
            5,
            0,
            20,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.locations.len(), 1);
        assert_eq!(result.locations[0].file_path, "src/main.rs");
        assert!(result.locations[0].symbol.is_some());
        assert_eq!(result.locations[0].symbol.as_ref().unwrap().name, "process");
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
            "fn main() {\n    println!(\"hello\");\n}",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
            include_source: true,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.locations.len(), 1);
        assert!(result.locations[0]
            .source_text
            .as_ref()
            .unwrap()
            .contains("fn main()"));
        assert!(result.locations[0].symbol.is_none());
    }

    // --- No data ---

    #[test]
    fn test_no_data_returns_empty() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert!(result.locations.is_empty());
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- source_layer correctness ---

    #[test]
    fn test_lsp_index_preferred_over_treesitter() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs", 1, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "foo",
            12,
            Some("fn(x: u32) -> bool"),
            "src/lib.rs",
            1,
            0,
            10,
            1,
        );
        insert_ts_chunk(&conn, "src/lib.rs", 1, 10, "fn foo(x: u32) -> bool {}");

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: "src/lib.rs".to_string(),
            line: 5,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        // LSP index takes priority over tree-sitter (no live LSP available)
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
    }

    // --- parse_lsp_range ---

    #[test]
    fn test_parse_lsp_range_valid() {
        let range_json = serde_json::json!({
            "start": { "line": 3, "character": 7 },
            "end": { "line": 3, "character": 15 }
        });
        let range = parse_lsp_range(&range_json).unwrap();
        assert_eq!(range.start_line, 3);
        assert_eq!(range.start_character, 7);
        assert_eq!(range.end_line, 3);
        assert_eq!(range.end_character, 15);
    }

    #[test]
    fn test_parse_lsp_range_missing_fields() {
        let range_json = serde_json::json!({ "start": { "line": 0 } });
        assert!(parse_lsp_range(&range_json).is_none());
    }

    // --- uri_to_file_path ---

    #[test]
    fn test_uri_to_file_path() {
        assert_eq!(
            uri_to_file_path("file:///home/user/project/src/main.rs"),
            "/home/user/project/src/main.rs"
        );
    }

    #[test]
    fn test_uri_non_file_scheme() {
        assert_eq!(
            uri_to_file_path("https://example.com/file.rs"),
            "https://example.com/file.rs"
        );
    }

    // --- try_lsp_index: include_source branch ---

    #[test]
    fn test_lsp_index_include_source_nonexistent_file() {
        // When include_source is true but the file doesn't exist on disk,
        // read_source_range returns None so source_text should be None.
        let conn = test_db();
        insert_file(&conn, "nonexistent/path.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "MyStruct",
            5, // class
            Some("pub struct MyStruct"),
            "nonexistent/path.rs",
            10,
            0,
            25,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: "nonexistent/path.rs".to_string(),
            line: 15,
            character: 0,
            include_source: true,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.locations.len(), 1);
        // File doesn't exist on disk, so source_text is None
        assert!(result.locations[0].source_text.is_none());
        assert!(result.locations[0].symbol.is_some());
        assert_eq!(
            result.locations[0].symbol.as_ref().unwrap().name,
            "MyStruct"
        );
    }

    #[test]
    fn test_lsp_index_include_source_real_file() {
        // When include_source is true and the file exists, source_text is populated.
        let conn = test_db();

        // Create a real temporary file so read_source_range can read it
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("test_src.rs");
        std::fs::write(
            &file_path,
            "line0\nline1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n",
        )
        .unwrap();
        let file_path_str = file_path.to_str().unwrap();

        insert_file(&conn, file_path_str, 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym_real",
            "greet",
            12, // function
            Some("fn greet()"),
            file_path_str,
            2,
            0,
            5,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetDefinitionOptions {
            file_path: file_path_str.to_string(),
            line: 3,
            character: 0,
            include_source: true,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.locations.len(), 1);
        // File exists, so source_text should be Some containing lines 2..=5
        let src = result.locations[0].source_text.as_ref().unwrap();
        assert!(src.contains("line2"));
        assert!(src.contains("line5"));
    }

    // --- try_parse_location_link: targetRange fallback ---

    #[test]
    fn test_parse_location_link_target_range_only() {
        // When targetSelectionRange is absent, targetRange should be used.
        let response = serde_json::json!([
            {
                "targetUri": "file:///src/models.rs",
                "targetRange": {
                    "start": { "line": 20, "character": 0 },
                    "end": { "line": 30, "character": 1 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file_path, "/src/models.rs");
        // Should fall back to targetRange since targetSelectionRange is absent
        assert_eq!(locations[0].range.start_line, 20);
        assert_eq!(locations[0].range.start_character, 0);
        assert_eq!(locations[0].range.end_line, 30);
        assert_eq!(locations[0].range.end_character, 1);
    }

    #[test]
    fn test_parse_location_link_prefers_selection_range_over_target_range() {
        // Verify that targetSelectionRange takes precedence when both are present.
        let response = serde_json::json!([
            {
                "targetUri": "file:///src/handler.rs",
                "targetRange": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 50, "character": 1 }
                },
                "targetSelectionRange": {
                    "start": { "line": 10, "character": 4 },
                    "end": { "line": 10, "character": 18 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        // targetSelectionRange should win
        assert_eq!(locations[0].range.start_line, 10);
        assert_eq!(locations[0].range.start_character, 4);
        assert_eq!(locations[0].range.end_line, 10);
        assert_eq!(locations[0].range.end_character, 18);
    }

    #[test]
    fn test_parse_location_link_missing_both_ranges() {
        // When both targetSelectionRange and targetRange are absent, parsing fails.
        let response = serde_json::json!([
            {
                "targetUri": "file:///src/orphan.rs"
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_location_link_missing_target_uri() {
        // When targetUri is absent, parsing should fail entirely.
        let response = serde_json::json!([
            {
                "targetRange": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 5, "character": 1 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    // --- No-live-LSP fallback paths ---

    #[test]
    fn test_no_live_lsp_skips_to_lsp_index() {
        // With no LSP client at all (None), has_live_lsp() returns false
        // and the code falls through to the LSP index layer.
        let conn = test_db();
        insert_file(&conn, "src/service.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "svc_sym",
            "handle_request",
            12,
            Some("fn handle_request()"),
            "src/service.rs",
            0,
            0,
            15,
            1,
        );

        // Explicitly pass None for lsp_client -- no live LSP
        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_live_lsp());

        let opts = GetDefinitionOptions {
            file_path: "src/service.rs".to_string(),
            line: 5,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        // Should skip live LSP and land on LSP index
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(
            result.locations[0].symbol.as_ref().unwrap().name,
            "handle_request"
        );
    }

    #[test]
    fn test_no_live_lsp_falls_through_to_treesitter() {
        // With no LSP client and no LSP index data, falls through to tree-sitter.
        let conn = test_db();
        insert_file(&conn, "src/utils.rs", 1, 0);
        insert_ts_chunk(
            &conn,
            "src/utils.rs",
            0,
            10,
            "fn helper() -> String { String::new() }",
        );

        let ctx = LayeredContext::new(&conn, None);
        assert!(!ctx.has_live_lsp());

        let opts = GetDefinitionOptions {
            file_path: "src/utils.rs".to_string(),
            line: 5,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
    }

    #[test]
    fn test_no_live_lsp_empty_mutex_falls_through() {
        // An LSP client mutex exists but contains None (no connected server).
        // has_live_lsp() should return false, skipping the live layer.
        use std::sync::{Arc, Mutex};

        let conn = test_db();
        insert_file(&conn, "src/app.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "app_sym",
            "main",
            12,
            Some("fn main()"),
            "src/app.rs",
            0,
            0,
            20,
            1,
        );

        // Create a SharedLspClient (Arc<Mutex<Option<...>>>) containing None
        let empty_client: crate::lsp_worker::SharedLspClient = Arc::new(Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&empty_client));
        assert!(!ctx.has_live_lsp());

        let opts = GetDefinitionOptions {
            file_path: "src/app.rs".to_string(),
            line: 10,
            character: 0,
            include_source: false,
        };
        let result = get_definition(&ctx, &opts).unwrap();
        // Even with a mutex present, no server inside means skip live LSP
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.locations[0].symbol.as_ref().unwrap().name, "main");
    }

    // --- GetDefinitionResult serialization ---

    #[test]
    fn test_result_serializable() {
        let result = GetDefinitionResult {
            locations: vec![DefinitionLocation {
                file_path: "/src/main.rs".to_string(),
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 5,
                    end_character: 1,
                },
                source_text: Some("fn main() {}".to_string()),
                symbol: None,
            }],
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: GetDefinitionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.locations.len(), 1);
        assert_eq!(roundtrip.source_layer, SourceLayer::LiveLsp);
    }
}
