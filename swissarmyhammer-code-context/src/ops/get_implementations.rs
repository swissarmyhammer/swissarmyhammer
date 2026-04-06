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

    #[test]
    fn language_id_detection() {
        assert_eq!(language_id_from_path("src/main.rs"), "rust");
        assert_eq!(language_id_from_path("app.ts"), "typescript");
        assert_eq!(language_id_from_path("app.tsx"), "typescriptreact");
        assert_eq!(language_id_from_path("script.py"), "python");
        assert_eq!(language_id_from_path("readme.txt"), "plaintext");
    }
}
