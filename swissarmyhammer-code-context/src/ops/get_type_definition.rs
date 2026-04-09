//! Go-to-type-definition via live LSP only.
//!
//! Type definition is inherently a live LSP feature -- there is no meaningful
//! index-based equivalent. When no live LSP is available, this operation
//! returns an empty result with `SourceLayer::None` (not an error).

use serde::{Deserialize, Serialize};
use serde_json::json;

#[cfg(test)]
use crate::layered_context::LspRange;
use crate::layered_context::{DefinitionLocation, LayeredContext, SourceLayer};
use crate::ops::get_definition::parse_definition_locations;
use crate::ops::lsp_helpers::{file_path_to_uri, read_source_range};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_type_definition` operation.
#[derive(Debug, Clone)]
pub struct GetTypeDefinitionOptions {
    /// Path to the file (relative to workspace root).
    pub file_path: String,
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
    /// Whether to include source text from disk at each definition location.
    pub include_source: bool,
}

/// Result of a type definition lookup operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTypeDefinitionResult {
    /// The type definition locations found.
    pub locations: Vec<DefinitionLocation>,
    /// Which data layer provided the result.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get type definition locations for a symbol at a position in a file.
///
/// Uses live LSP only -- type definition has no meaningful index equivalent.
/// Returns an empty result with `SourceLayer::None` when no live LSP is
/// available (graceful degradation, not an error).
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The file path, line, character, and include_source flag.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn get_type_definition(
    ctx: &LayeredContext,
    opts: &GetTypeDefinitionOptions,
) -> Result<GetTypeDefinitionResult, crate::error::CodeContextError> {
    if !ctx.has_live_lsp() {
        return Ok(GetTypeDefinitionResult {
            locations: Vec::new(),
            source_layer: SourceLayer::None,
        });
    }

    let uri = file_path_to_uri(&opts.file_path);

    let response = ctx.lsp_request_with_document(
        &opts.file_path,
        "textDocument/typeDefinition",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": opts.line, "character": opts.character }
        }),
    )?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => {
            return Ok(GetTypeDefinitionResult {
                locations: Vec::new(),
                source_layer: SourceLayer::None,
            })
        }
    };

    let mut locations = parse_definition_locations(&response);
    if locations.is_empty() {
        return Ok(GetTypeDefinitionResult {
            locations: Vec::new(),
            source_layer: SourceLayer::None,
        });
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

    Ok(GetTypeDefinitionResult {
        locations,
        source_layer: SourceLayer::LiveLsp,
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_context::SymbolInfo;
    use crate::test_fixtures::test_db;

    // --- No live LSP returns empty, not error ---

    #[test]
    fn test_no_live_lsp_returns_empty() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);
        let opts = GetTypeDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 5,
            include_source: false,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(result.locations.is_empty());
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- SharedLspClient with None inside (no connected process) ---

    #[test]
    fn test_shared_client_with_none_inside_returns_empty() {
        // A SharedLspClient Arc exists but contains None (no connected LSP process).
        // has_live_lsp() should return false and get_type_definition should
        // produce an empty result with SourceLayer::None -- not an error.
        let conn = test_db();
        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = GetTypeDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 1,
            character: 0,
            include_source: false,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(
            result.locations.is_empty(),
            "SharedLspClient(None) should behave like no LSP"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_shared_client_with_none_inside_include_source_true() {
        // Even with include_source: true, a None-inside SharedLspClient
        // should short-circuit before any file I/O.
        let conn = test_db();
        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = GetTypeDefinitionOptions {
            file_path: "nonexistent/file.rs".to_string(),
            line: 0,
            character: 0,
            include_source: true,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(
            result.locations.is_empty(),
            "should return empty before attempting to read any file"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- Type definition does NOT fall back to index layers ---
    // Unlike get_definition, get_type_definition is live-LSP-only.
    // When there is no live LSP, it should return empty even if
    // index data exists at the requested position.

    #[test]
    fn test_no_fallback_to_lsp_index() {
        // Populate the LSP index with a symbol covering the queried position,
        // but provide no live LSP. The result should be empty because
        // get_type_definition does not consult the index layers.
        let conn = test_db();
        crate::test_fixtures::insert_file(&conn, "src/main.rs", 0, 1);
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym1",
            "MyStruct",
            5, // Class
            "src/main.rs",
            5,
            0,
            20,
            1,
            Some("struct MyStruct"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetTypeDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
            include_source: false,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(
            result.locations.is_empty(),
            "type definition should NOT fall back to LSP index"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_no_fallback_to_treesitter() {
        // Populate the tree-sitter index with a chunk covering the queried
        // position, but provide no live LSP. The result should be empty.
        let conn = test_db();
        crate::test_fixtures::insert_file(&conn, "src/main.rs", 1, 0);
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/main.rs",
            5,
            20,
            "fn main() { println!(\"hello\"); }",
            None,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetTypeDefinitionOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
            include_source: true,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(
            result.locations.is_empty(),
            "type definition should NOT fall back to tree-sitter"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_no_fallback_with_both_indexes_populated() {
        // Both LSP index and tree-sitter index have data, but no live LSP.
        // get_type_definition should still return empty.
        let conn = test_db();
        crate::test_fixtures::insert_file(&conn, "src/lib.rs", 1, 1);
        crate::test_fixtures::insert_lsp_symbol(
            &conn,
            "sym1",
            "Config",
            5, // Class
            "src/lib.rs",
            1,
            0,
            30,
            1,
            Some("struct Config"),
        );
        crate::test_fixtures::insert_ts_chunk(
            &conn,
            "src/lib.rs",
            1,
            30,
            "struct Config { field: u32 }",
            Some("Config"),
        );

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = GetTypeDefinitionOptions {
            file_path: "src/lib.rs".to_string(),
            line: 15,
            character: 5,
            include_source: true,
        };
        let result = get_type_definition(&ctx, &opts).unwrap();
        assert!(
            result.locations.is_empty(),
            "no live LSP means no results, even with rich index data"
        );
        assert_eq!(result.source_layer, SourceLayer::None);
    }

    // --- Result serialization ---

    #[test]
    fn test_result_serializable() {
        let result = GetTypeDefinitionResult {
            locations: vec![DefinitionLocation {
                file_path: "/src/types.rs".to_string(),
                range: LspRange {
                    start_line: 10,
                    start_character: 0,
                    end_line: 25,
                    end_character: 1,
                },
                source_text: None,
                symbol: None,
            }],
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: GetTypeDefinitionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.locations.len(), 1);
        assert_eq!(roundtrip.source_layer, SourceLayer::LiveLsp);
    }

    #[test]
    fn test_result_serializable_with_source_text_and_symbol() {
        // Verify round-trip when all optional fields are populated.
        let result = GetTypeDefinitionResult {
            locations: vec![DefinitionLocation {
                file_path: "/src/models.rs".to_string(),
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 20,
                    end_character: 1,
                },
                source_text: Some("pub struct Foo {\n    bar: i32,\n}".to_string()),
                symbol: Some(SymbolInfo {
                    name: "Foo".to_string(),
                    qualified_path: Some("models::Foo".to_string()),
                    kind: "struct".to_string(),
                    detail: Some("pub struct Foo".to_string()),
                    file_path: "/src/models.rs".to_string(),
                    range: LspRange {
                        start_line: 1,
                        start_character: 0,
                        end_line: 20,
                        end_character: 1,
                    },
                }),
            }],
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: GetTypeDefinitionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.locations.len(), 1);
        let loc = &roundtrip.locations[0];
        assert!(
            loc.source_text.is_some(),
            "source_text should survive round-trip"
        );
        assert!(
            loc.source_text.as_ref().unwrap().contains("pub struct Foo"),
            "source_text content should be preserved"
        );
        let sym = loc
            .symbol
            .as_ref()
            .expect("symbol should survive round-trip");
        assert_eq!(sym.name, "Foo");
        assert_eq!(sym.qualified_path.as_deref(), Some("models::Foo"));
        assert_eq!(sym.kind, "struct");
        assert_eq!(sym.detail.as_deref(), Some("pub struct Foo"));
    }

    #[test]
    fn test_result_serializable_empty_locations() {
        // An empty result with SourceLayer::None should serialize cleanly.
        let result = GetTypeDefinitionResult {
            locations: Vec::new(),
            source_layer: SourceLayer::None,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: GetTypeDefinitionResult = serde_json::from_str(&json).unwrap();
        assert!(roundtrip.locations.is_empty());
        assert_eq!(roundtrip.source_layer, SourceLayer::None);
    }

    #[test]
    fn test_result_serializable_multiple_locations() {
        // Multiple locations in a single result should all survive round-trip.
        let result = GetTypeDefinitionResult {
            locations: vec![
                DefinitionLocation {
                    file_path: "/src/a.rs".to_string(),
                    range: LspRange {
                        start_line: 1,
                        start_character: 0,
                        end_line: 5,
                        end_character: 1,
                    },
                    source_text: None,
                    symbol: None,
                },
                DefinitionLocation {
                    file_path: "/src/b.rs".to_string(),
                    range: LspRange {
                        start_line: 10,
                        start_character: 4,
                        end_line: 15,
                        end_character: 1,
                    },
                    source_text: Some("impl Trait for B {}".to_string()),
                    symbol: None,
                },
            ],
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: GetTypeDefinitionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.locations.len(), 2);
        assert_eq!(roundtrip.locations[0].file_path, "/src/a.rs");
        assert_eq!(roundtrip.locations[1].file_path, "/src/b.rs");
        assert!(roundtrip.locations[1].source_text.is_some());
    }

    // --- parse_definition_locations reuse ---
    // These test the shared parser from get_definition, exercised through type_definition context.

    #[test]
    fn test_parse_single_location_via_shared_parser() {
        let response = serde_json::json!({
            "uri": "file:///src/types.rs",
            "range": {
                "start": { "line": 5, "character": 0 },
                "end": { "line": 15, "character": 1 }
            }
        });
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file_path, "/src/types.rs");
    }

    #[test]
    fn test_parse_location_link_via_shared_parser() {
        let response = serde_json::json!([{
            "targetUri": "file:///src/models.rs",
            "targetRange": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 30, "character": 1 }
            },
            "targetSelectionRange": {
                "start": { "line": 2, "character": 11 },
                "end": { "line": 2, "character": 20 }
            }
        }]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        // Uses targetSelectionRange
        assert_eq!(locations[0].range.start_line, 2);
        assert_eq!(locations[0].range.start_character, 11);
    }

    #[test]
    fn test_parse_array_of_locations() {
        // LSP may return an array of Location objects for multiple type definitions.
        let response = serde_json::json!([
            {
                "uri": "file:///src/types.rs",
                "range": {
                    "start": { "line": 5, "character": 0 },
                    "end": { "line": 15, "character": 1 }
                }
            },
            {
                "uri": "file:///src/traits.rs",
                "range": {
                    "start": { "line": 30, "character": 4 },
                    "end": { "line": 45, "character": 1 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].file_path, "/src/types.rs");
        assert_eq!(locations[0].range.start_line, 5);
        assert_eq!(locations[1].file_path, "/src/traits.rs");
        assert_eq!(locations[1].range.start_line, 30);
        assert_eq!(locations[1].range.start_character, 4);
    }

    #[test]
    fn test_parse_location_link_without_target_selection_range() {
        // When targetSelectionRange is absent, the parser should fall back
        // to targetRange.
        let response = serde_json::json!([{
            "targetUri": "file:///src/fallback.rs",
            "targetRange": {
                "start": { "line": 10, "character": 0 },
                "end": { "line": 20, "character": 1 }
            }
        }]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert_eq!(locations[0].file_path, "/src/fallback.rs");
        assert_eq!(locations[0].range.start_line, 10);
        assert_eq!(locations[0].range.end_line, 20);
    }

    #[test]
    fn test_parse_null_response() {
        let response = serde_json::json!(null);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_empty_array_response() {
        let response = serde_json::json!([]);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_mixed_location_and_location_link() {
        // An array containing both Location and LocationLink items.
        let response = serde_json::json!([
            {
                "uri": "file:///src/regular.rs",
                "range": {
                    "start": { "line": 1, "character": 0 },
                    "end": { "line": 5, "character": 1 }
                }
            },
            {
                "targetUri": "file:///src/linked.rs",
                "targetRange": {
                    "start": { "line": 10, "character": 0 },
                    "end": { "line": 20, "character": 1 }
                },
                "targetSelectionRange": {
                    "start": { "line": 12, "character": 4 },
                    "end": { "line": 12, "character": 14 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].file_path, "/src/regular.rs");
        assert_eq!(locations[1].file_path, "/src/linked.rs");
        assert_eq!(
            locations[1].range.start_line, 12,
            "LocationLink should use targetSelectionRange"
        );
    }

    #[test]
    fn test_parse_unrecognized_object_ignored() {
        // An object that is neither Location nor LocationLink should be
        // safely skipped.
        let response = serde_json::json!([
            { "something": "irrelevant" },
            {
                "uri": "file:///src/valid.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 1, "character": 0 }
                }
            }
        ]);
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1, "unrecognized items should be skipped");
        assert_eq!(locations[0].file_path, "/src/valid.rs");
    }

    #[test]
    fn test_parse_string_response() {
        // A bare string is neither Location nor array -- should produce empty.
        let response = serde_json::json!("unexpected");
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    #[test]
    fn test_parse_number_response() {
        // A number is not a valid LSP response format -- should produce empty.
        let response = serde_json::json!(42);
        let locations = parse_definition_locations(&response);
        assert!(locations.is_empty());
    }

    // --- Parsed locations have no source_text or symbol by default ---

    #[test]
    fn test_parsed_locations_have_no_source_text_or_symbol() {
        // parse_definition_locations should set source_text and symbol to None;
        // enrichment happens in a later pass within get_type_definition.
        let response = serde_json::json!({
            "uri": "file:///src/types.rs",
            "range": {
                "start": { "line": 0, "character": 0 },
                "end": { "line": 10, "character": 1 }
            }
        });
        let locations = parse_definition_locations(&response);
        assert_eq!(locations.len(), 1);
        assert!(
            locations[0].source_text.is_none(),
            "parser should not populate source_text"
        );
        assert!(
            locations[0].symbol.is_none(),
            "parser should not populate symbol"
        );
    }
}
