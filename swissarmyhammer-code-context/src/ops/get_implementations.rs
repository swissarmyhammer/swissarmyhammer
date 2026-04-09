//! Find implementations of a trait/interface using layered resolution.
//!
//! Uses `LayeredContext` to try multiple data sources in priority order:
//! 1. **Live LSP** -- `textDocument/implementation` request for full cross-file results
//! 2. **LSP index** -- skipped (no equivalent relationship stored)
//! 3. **Tree-sitter** -- heuristic `impl TraitName` pattern search

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{DefinitionLocation, LayeredContext, LspRange, SourceLayer};
use crate::ops::lsp_helpers::{file_path_to_uri, parse_lsp_range, uri_to_file_path};

/// Options for the `get_implementations` operation.
#[derive(Debug, Clone)]
pub struct GetImplementationsOptions {
    /// Path to the file containing the symbol.
    pub file_path: String,
    /// Zero-based line number of the symbol.
    pub line: u32,
    /// Zero-based character offset of the symbol.
    pub character: u32,
    /// Maximum number of results to return.
    pub max_results: Option<usize>,
}

/// Result of the `get_implementations` operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetImplementationsResult {
    /// Implementation locations found.
    pub implementations: Vec<DefinitionLocation>,
    /// Which data layer provided the results.
    pub source_layer: SourceLayer,
}

/// Find implementations of the symbol at the given position.
///
/// Tries live LSP first (`textDocument/implementation`), then falls back to a
/// tree-sitter heuristic that searches for `impl <SymbolName>` chunks. Returns
/// an empty result (not an error) when no data is available.
pub fn get_implementations(
    ctx: &LayeredContext,
    opts: &GetImplementationsOptions,
) -> Result<GetImplementationsResult, crate::error::CodeContextError> {
    let max = opts.max_results.unwrap_or(20);

    // --- Layer 1: Live LSP ---
    if ctx.has_live_lsp() {
        let uri = file_path_to_uri(&opts.file_path);

        let response = ctx.lsp_request_with_document(
            &opts.file_path,
            "textDocument/implementation",
            json!({
                "textDocument": { "uri": uri },
                "position": { "line": opts.line, "character": opts.character }
            }),
        )?;

        if let Some(value) = response {
            let mut locations = parse_locations(&value);
            locations.truncate(max);

            // Enrich each location
            let implementations: Vec<DefinitionLocation> =
                locations.into_iter().map(|loc| enrich(ctx, loc)).collect();

            return Ok(GetImplementationsResult {
                implementations,
                source_layer: SourceLayer::LiveLsp,
            });
        }
    }

    // --- Layer 2: LSP index --- skipped (no equivalent)

    // --- Layer 3: Tree-sitter heuristic ---
    // Find the symbol name at the cursor position so we can search for `impl <Name>`
    if let Some(symbol) = ctx.find_symbol(&opts.file_path, opts.line, opts.character) {
        let query = format!("impl {}", symbol.name);
        let chunks = ctx.ts_chunks_matching(&query, max);

        let implementations: Vec<DefinitionLocation> = chunks
            .into_iter()
            .map(|chunk| {
                let range = LspRange {
                    start_line: chunk.start_line,
                    start_character: 0,
                    end_line: chunk.end_line,
                    end_character: 0,
                };
                let enriched = ctx.enrich_location(&chunk.file_path, &range);
                DefinitionLocation {
                    file_path: chunk.file_path,
                    range,
                    source_text: Some(chunk.text),
                    symbol: enriched.symbol,
                }
            })
            .collect();

        if !implementations.is_empty() {
            return Ok(GetImplementationsResult {
                implementations,
                source_layer: SourceLayer::TreeSitter,
            });
        }
    }

    // No data from any layer -- return empty, not an error.
    Ok(GetImplementationsResult {
        implementations: Vec::new(),
        source_layer: SourceLayer::None,
    })
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse an LSP response that may be a single `Location`, a `Location[]`,
/// or a `LocationLink[]` into a list of `DefinitionLocation` entries.
fn parse_locations(value: &serde_json::Value) -> Vec<DefinitionLocation> {
    let mut results = Vec::new();

    match value {
        serde_json::Value::Array(arr) => {
            for item in arr {
                if let Some(loc) = try_parse_location(item) {
                    results.push(loc);
                } else if let Some(loc) = try_parse_location_link(item) {
                    results.push(loc);
                }
            }
        }
        obj @ serde_json::Value::Object(_) => {
            if let Some(loc) = try_parse_location(obj) {
                results.push(loc);
            } else if let Some(loc) = try_parse_location_link(obj) {
                results.push(loc);
            }
        }
        serde_json::Value::Null => {
            // null means no results
        }
        _ => {}
    }

    results
}

/// Try to parse a JSON value as an LSP `Location` (`{ uri, range }`).
fn try_parse_location(value: &serde_json::Value) -> Option<DefinitionLocation> {
    let uri = value.get("uri")?.as_str()?;
    let range = value.get("range")?;
    let lsp_range = parse_lsp_range(range)?;
    Some(DefinitionLocation {
        file_path: uri_to_file_path(uri),
        range: lsp_range,
        source_text: None,
        symbol: None,
    })
}

/// Try to parse a JSON value as an LSP `LocationLink`.
///
/// Prefers `targetSelectionRange` (the precise identifier range) over
/// `targetRange` (the full body), matching the LSP spec recommendation and
/// the behaviour in get_definition.rs.
fn try_parse_location_link(value: &serde_json::Value) -> Option<DefinitionLocation> {
    let uri = value.get("targetUri")?.as_str()?;
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

/// Enrich a `DefinitionLocation` by querying the `LayeredContext` for symbol info.
fn enrich(ctx: &LayeredContext, mut loc: DefinitionLocation) -> DefinitionLocation {
    let enrichment = ctx.enrich_location(&loc.file_path, &loc.range);
    loc.symbol = enrichment.symbol;
    loc
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::lsp_helpers::language_id_from_path;

    #[test]
    fn parse_location_format() {
        let value = serde_json::json!([
            {
                "uri": "file:///src/main.rs",
                "range": {
                    "start": { "line": 10, "character": 4 },
                    "end": { "line": 20, "character": 1 }
                }
            }
        ]);
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file_path, "/src/main.rs");
        assert_eq!(locs[0].range.start_line, 10);
        assert_eq!(locs[0].range.start_character, 4);
        assert_eq!(locs[0].range.end_line, 20);
        assert_eq!(locs[0].range.end_character, 1);
    }

    #[test]
    fn parse_location_link_format() {
        let value = serde_json::json!([
            {
                "targetUri": "file:///src/impl.rs",
                "targetRange": {
                    "start": { "line": 5, "character": 0 },
                    "end": { "line": 15, "character": 1 }
                },
                "targetSelectionRange": {
                    "start": { "line": 5, "character": 4 },
                    "end": { "line": 5, "character": 12 }
                }
            }
        ]);
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file_path, "/src/impl.rs");
        // Should prefer targetSelectionRange (the precise identifier range)
        assert_eq!(locs[0].range.start_line, 5);
        assert_eq!(locs[0].range.start_character, 4);
        assert_eq!(locs[0].range.end_line, 5);
        assert_eq!(locs[0].range.end_character, 12);
    }

    #[test]
    fn parse_single_location_not_array() {
        let value = serde_json::json!({
            "uri": "file:///src/lib.rs",
            "range": {
                "start": { "line": 1, "character": 0 },
                "end": { "line": 10, "character": 0 }
            }
        });
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file_path, "/src/lib.rs");
    }

    #[test]
    fn parse_null_returns_empty() {
        let locs = parse_locations(&serde_json::Value::Null);
        assert!(locs.is_empty());
    }

    #[test]
    fn parse_mixed_location_and_link() {
        let value = serde_json::json!([
            {
                "uri": "file:///a.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 0 }
                }
            },
            {
                "targetUri": "file:///b.rs",
                "targetRange": {
                    "start": { "line": 2, "character": 0 },
                    "end": { "line": 3, "character": 0 }
                }
            }
        ]);
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 2);
        assert_eq!(locs[0].file_path, "/a.rs");
        assert_eq!(locs[1].file_path, "/b.rs");
    }

    #[test]
    fn empty_result_when_no_layers_available() {
        // Construct a LayeredContext with no LSP client and an empty database
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        crate::db::configure_connection(&conn).unwrap();
        crate::db::create_schema(&conn).unwrap();
        let ctx = LayeredContext::new(&conn, None);

        let opts = GetImplementationsOptions {
            file_path: "/nonexistent.rs".to_string(),
            line: 0,
            character: 0,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        assert!(result.implementations.is_empty());
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn max_results_truncation() {
        // Verify parse_locations + truncation works
        let value = serde_json::json!([
            { "uri": "file:///a.rs", "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } } },
            { "uri": "file:///b.rs", "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } } },
            { "uri": "file:///c.rs", "range": { "start": { "line": 0, "character": 0 }, "end": { "line": 1, "character": 0 } } },
        ]);
        let mut locs = parse_locations(&value);
        locs.truncate(2); // Simulate max_results = 2
        assert_eq!(locs.len(), 2);
    }

    #[test]
    fn uri_to_file_path_strips_prefix() {
        assert_eq!(
            uri_to_file_path("file:///usr/src/main.rs"),
            "/usr/src/main.rs"
        );
        assert_eq!(uri_to_file_path("/raw/path.rs"), "/raw/path.rs");
    }

    // --- Tree-sitter fallback (Layer 3) ---

    #[test]
    fn treesitter_finds_impl_blocks() {
        // LSP symbol at cursor gives find_symbol the name "Drawable",
        // then ts_chunks_matching finds "impl Drawable" chunks.
        let conn = crate::test_fixtures::test_db();
        crate::test_fixtures::insert_file(&conn, "src/traits.rs", 1, 0);

        // LSP symbol so find_symbol resolves the name "Drawable" at cursor
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:drawable",
            "Drawable",
            11,
            "src/traits.rs",
            0,
            0,
            5,
            1,
            None,
        );

        // Two impl blocks whose text contains "impl Drawable"
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/traits.rs",
            10,
            20,
            "impl Drawable for Circle { fn draw(&self) {} }",
            Some("traits::CircleDrawable"),
        );
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/traits.rs",
            25,
            35,
            "impl Drawable for Square { fn draw(&self) {} }",
            Some("traits::SquareDrawable"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetImplementationsOptions {
            file_path: "src/traits.rs".to_string(),
            line: 2,
            character: 6,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.implementations.len(), 2);

        // Both results come from the "impl Drawable" chunks
        let paths: Vec<&str> = result
            .implementations
            .iter()
            .map(|l| l.file_path.as_str())
            .collect();
        assert!(paths.iter().all(|p| *p == "src/traits.rs"));

        // Verify ranges correspond to the impl chunks
        let start_lines: Vec<u32> = result
            .implementations
            .iter()
            .map(|l| l.range.start_line)
            .collect();
        assert!(start_lines.contains(&10));
        assert!(start_lines.contains(&25));
    }

    #[test]
    fn treesitter_no_impls_found() {
        // Symbol found but no chunks match "impl <Name>".
        let conn = crate::test_fixtures::test_db();
        crate::test_fixtures::insert_file(&conn, "src/orphan.rs", 1, 0);

        // LSP symbol so find_symbol resolves the name
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:invisible",
            "Invisible",
            11,
            "src/orphan.rs",
            0,
            0,
            5,
            1,
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetImplementationsOptions {
            file_path: "src/orphan.rs".to_string(),
            line: 2,
            character: 6,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        // No impls -- falls through to SourceLayer::None
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.implementations.is_empty());
    }

    #[test]
    fn treesitter_struct_impl_blocks() {
        // Verify finding `impl StructName` blocks (not just traits).
        let conn = crate::test_fixtures::test_db();
        crate::test_fixtures::insert_file(&conn, "src/structs.rs", 1, 0);

        // LSP symbol for the struct at cursor
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:widget",
            "Widget",
            23,
            "src/structs.rs",
            0,
            0,
            3,
            1,
            None,
        );

        // Inherent impl block
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/structs.rs",
            5,
            15,
            "impl Widget { fn new() -> Self { Widget { x: 0 } } }",
            Some("structs::Widget_impl"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetImplementationsOptions {
            file_path: "src/structs.rs".to_string(),
            line: 1,
            character: 7,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.implementations.len(), 1);
        assert_eq!(result.implementations[0].range.start_line, 5);
        assert!(result.implementations[0]
            .source_text
            .as_ref()
            .unwrap()
            .contains("impl Widget"));
    }

    #[test]
    fn language_id_detection() {
        assert_eq!(language_id_from_path("src/main.rs"), "rust");
        assert_eq!(language_id_from_path("app.ts"), "typescript");
        assert_eq!(language_id_from_path("app.tsx"), "typescriptreact");
        assert_eq!(language_id_from_path("script.py"), "python");
        assert_eq!(language_id_from_path("readme.txt"), "plaintext");
    }

    // --- No-live-LSP falls through to treesitter ---

    #[test]
    fn no_live_lsp_falls_through_to_treesitter() {
        // When a SharedLspClient exists but wraps None (no connected LSP process),
        // has_live_lsp() returns false and get_implementations should skip the
        // live layer entirely, falling back to tree-sitter heuristic search.
        use std::sync::{Arc, Mutex};

        let conn = crate::test_fixtures::test_db();
        crate::test_fixtures::insert_file(&conn, "src/shapes.rs", 1, 0);

        // LSP symbol so find_symbol resolves the name "Renderable" at cursor
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym:renderable",
            "Renderable",
            11,
            "src/shapes.rs",
            0,
            0,
            5,
            1,
            None,
        );

        // A tree-sitter chunk containing "impl Renderable"
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/shapes.rs",
            10,
            20,
            "impl Renderable for Circle { fn render(&self) {} }",
            Some("shapes::CircleRenderable"),
        );

        // SharedLspClient present but wrapping None -- no connected LSP server
        let empty_client: crate::lsp_worker::SharedLspClient = Arc::new(Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&empty_client));
        assert!(!ctx.has_live_lsp());

        let opts = GetImplementationsOptions {
            file_path: "src/shapes.rs".to_string(),
            line: 2,
            character: 6,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        // Should fall through to tree-sitter, not return SourceLayer::None
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.implementations.len(), 1);
        assert!(result.implementations[0]
            .source_text
            .as_ref()
            .unwrap()
            .contains("impl Renderable"));
    }

    // --- parse_locations: single LocationLink object (not array) ---

    #[test]
    fn parse_single_location_link_not_array() {
        // When the LSP response is a single LocationLink object (not wrapped in
        // an array), parse_locations should still extract it. This exercises the
        // Object → try_parse_location fails → try_parse_location_link succeeds path.
        let value = serde_json::json!({
            "targetUri": "file:///src/widget.rs",
            "targetRange": {
                "start": { "line": 10, "character": 0 },
                "end": { "line": 20, "character": 1 }
            },
            "targetSelectionRange": {
                "start": { "line": 10, "character": 4 },
                "end": { "line": 10, "character": 10 }
            }
        });
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file_path, "/src/widget.rs");
        // Should prefer targetSelectionRange
        assert_eq!(locs[0].range.start_line, 10);
        assert_eq!(locs[0].range.start_character, 4);
        assert_eq!(locs[0].range.end_line, 10);
        assert_eq!(locs[0].range.end_character, 10);
    }

    // --- parse_locations: non-object, non-array, non-null value ---

    #[test]
    fn parse_locations_unexpected_type_returns_empty() {
        // When the LSP response is an unexpected JSON type (e.g. a string or
        // boolean), parse_locations should silently return an empty vec.
        let string_value = serde_json::json!("unexpected");
        assert!(parse_locations(&string_value).is_empty());

        let bool_value = serde_json::json!(true);
        assert!(parse_locations(&bool_value).is_empty());

        let number_value = serde_json::json!(42);
        assert!(parse_locations(&number_value).is_empty());
    }

    // --- SharedLspClient with None → verify fallthrough ---

    #[test]
    fn shared_lsp_client_with_none_returns_empty_when_no_treesitter() {
        // When a SharedLspClient exists but wraps None (no connected LSP
        // process) and no tree-sitter data is available, get_implementations
        // should return an empty result with SourceLayer::None -- not an error.
        use std::sync::{Arc, Mutex};

        let conn = crate::test_fixtures::test_db();
        let shared: crate::lsp_worker::SharedLspClient = Arc::new(Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = GetImplementationsOptions {
            file_path: "/nonexistent.rs".to_string(),
            line: 0,
            character: 0,
            max_results: None,
        };

        let result = get_implementations(&ctx, &opts).unwrap();
        assert!(
            result.implementations.is_empty(),
            "SharedLspClient(None) should behave like no LSP"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- parse_locations: LocationLink without targetSelectionRange ---

    #[test]
    fn parse_single_location_link_falls_back_to_target_range() {
        // When a LocationLink has no targetSelectionRange, the parser should
        // fall back to targetRange. This tests the or_else fallback in
        // try_parse_location_link.
        let value = serde_json::json!({
            "targetUri": "file:///src/models.rs",
            "targetRange": {
                "start": { "line": 30, "character": 0 },
                "end": { "line": 40, "character": 1 }
            }
        });
        let locs = parse_locations(&value);
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].file_path, "/src/models.rs");
        assert_eq!(locs[0].range.start_line, 30);
        assert_eq!(locs[0].range.start_character, 0);
        assert_eq!(locs[0].range.end_line, 40);
        assert_eq!(locs[0].range.end_character, 1);
    }
}
