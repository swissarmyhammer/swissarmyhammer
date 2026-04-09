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
}
