//! Hover information with layered resolution.
//!
//! Provides type info and documentation for a given position using three
//! data layers in priority order:
//!
//! 1. **Live LSP** -- sends `textDocument/hover` to a running LSP server.
//! 2. **LSP index** -- returns the `detail` field from persisted symbols.
//! 3. **Tree-sitter** -- returns the source chunk containing the position.
//!
//! The caller receives the best available data along with a [`SourceLayer`]
//! indicating which layer produced the result.

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::layered_context::{LayeredContext, LspRange, SourceLayer, SymbolInfo};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_hover` operation.
#[derive(Debug, Clone)]
pub struct GetHoverOptions {
    /// Path to the file (relative to workspace root).
    pub file_path: String,
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
}

/// Result of a hover operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HoverResult {
    /// The hover content (markdown, type signature, or source text).
    pub contents: String,
    /// The range the hover applies to, if available.
    pub range: Option<LspRange>,
    /// Symbol information from the layer, if available.
    pub symbol: Option<SymbolInfo>,
    /// Which data layer provided the result.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get hover information for a position in a file.
///
/// Tries live LSP first, then the LSP symbol index, then the tree-sitter
/// chunk index. Returns `None` if no layer has data for the position.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - The file path, line, and character to hover over.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails in a way that
/// is not a graceful "no data" response.
pub fn get_hover(
    ctx: &LayeredContext,
    opts: &GetHoverOptions,
) -> Result<Option<HoverResult>, crate::error::CodeContextError> {
    // Layer 1: Live LSP
    if ctx.has_live_lsp() {
        if let Some(result) = try_live_lsp(ctx, opts)? {
            return Ok(Some(result));
        }
    }

    // Layer 2: LSP index
    if let Some(result) = try_lsp_index(ctx, opts) {
        return Ok(Some(result));
    }

    // Layer 3: Tree-sitter
    if let Some(result) = try_treesitter(ctx, opts) {
        return Ok(Some(result));
    }

    Ok(None)
}

// ---------------------------------------------------------------------------
// Layer 1: Live LSP
// ---------------------------------------------------------------------------

/// Attempt to get hover data from a live LSP server.
///
/// Sends didOpen, textDocument/hover, and didClose atomically under a single
/// mutex hold to prevent interleaving with the indexing worker.
fn try_live_lsp(
    ctx: &LayeredContext,
    opts: &GetHoverOptions,
) -> Result<Option<HoverResult>, crate::error::CodeContextError> {
    let uri = file_path_to_uri(&opts.file_path);

    let response = ctx.lsp_request_with_document(
        &opts.file_path,
        "textDocument/hover",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": opts.line, "character": opts.character }
        }),
    )?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };

    let contents = parse_hover_contents(&response);
    if contents.is_empty() {
        return Ok(None);
    }

    let range = parse_hover_range(&response);

    // Enrich with symbol info if we have a range
    let symbol = range.as_ref().and_then(|r| {
        let enrichment = ctx.enrich_location(&opts.file_path, r);
        enrichment.symbol
    });

    Ok(Some(HoverResult {
        contents,
        range,
        symbol,
        source_layer: SourceLayer::LiveLsp,
    }))
}

// ---------------------------------------------------------------------------
// Layer 2: LSP index
// ---------------------------------------------------------------------------

/// Attempt to get hover data from the persisted LSP symbol index.
///
/// Returns the symbol's `detail` field (e.g. type signature) as hover content.
fn try_lsp_index(ctx: &LayeredContext, opts: &GetHoverOptions) -> Option<HoverResult> {
    let range = LspRange {
        start_line: opts.line,
        start_character: opts.character,
        end_line: opts.line,
        end_character: opts.character,
    };
    let symbol = ctx.lsp_symbol_at(&opts.file_path, &range)?;
    let detail = symbol.detail.clone().unwrap_or_else(|| {
        // Fall back to name + kind if no detail
        format!("{} ({})", symbol.name, symbol.kind)
    });

    Some(HoverResult {
        contents: detail,
        range: Some(symbol.range.clone()),
        symbol: Some(symbol),
        source_layer: SourceLayer::LspIndex,
    })
}

// ---------------------------------------------------------------------------
// Layer 3: Tree-sitter
// ---------------------------------------------------------------------------

/// Attempt to get hover data from the tree-sitter chunk index.
///
/// Returns the chunk's source text as a last resort.
fn try_treesitter(ctx: &LayeredContext, opts: &GetHoverOptions) -> Option<HoverResult> {
    let chunk = ctx.ts_chunk_at(&opts.file_path, opts.line)?;

    Some(HoverResult {
        contents: chunk.text.clone(),
        range: Some(LspRange {
            start_line: chunk.start_line,
            start_character: 0,
            end_line: chunk.end_line,
            end_character: 0,
        }),
        symbol: None,
        source_layer: SourceLayer::TreeSitter,
    })
}

// ---------------------------------------------------------------------------
// LSP hover response parsing
// ---------------------------------------------------------------------------

/// Parse the `contents` field from an LSP hover response.
///
/// Handles three LSP formats:
/// - `MarkupContent { kind, value }` -- returns the `value` string.
/// - `MarkedString` (string or `{ language, value }`) -- returns the value.
/// - Array of `MarkedString` -- joins with double newlines.
pub fn parse_hover_contents(response: &serde_json::Value) -> String {
    let contents = match response.get("contents") {
        Some(c) => c,
        None => return String::new(),
    };

    // Case 1: MarkupContent { kind, value }
    if let Some(value) = contents.get("value").and_then(|v| v.as_str()) {
        return value.to_string();
    }

    // Case 2: Plain string (MarkedString shorthand)
    if let Some(s) = contents.as_str() {
        return s.to_string();
    }

    // Case 3: Array of MarkedString
    if let Some(arr) = contents.as_array() {
        let parts: Vec<String> = arr
            .iter()
            .filter_map(|item| {
                // Each item is either a string or { language, value }
                if let Some(s) = item.as_str() {
                    Some(s.to_string())
                } else if let Some(value) = item.get("value").and_then(|v| v.as_str()) {
                    let lang = item.get("language").and_then(|l| l.as_str()).unwrap_or("");
                    if lang.is_empty() {
                        Some(value.to_string())
                    } else {
                        Some(format!("```{}\n{}\n```", lang, value))
                    }
                } else {
                    None
                }
            })
            .collect();
        return parts.join("\n\n");
    }

    String::new()
}

/// Parse the `range` field from an LSP hover response into an `LspRange`.
pub fn parse_hover_range(response: &serde_json::Value) -> Option<LspRange> {
    let range = response.get("range")?;
    let start = range.get("start")?;
    let end = range.get("end")?;

    Some(LspRange {
        start_line: start.get("line")?.as_u64()? as u32,
        start_character: start.get("character")?.as_u64()? as u32,
        end_line: end.get("line")?.as_u64()? as u32,
        end_character: end.get("character")?.as_u64()? as u32,
    })
}

// ---------------------------------------------------------------------------
// Helpers -- canonical versions live in lsp_helpers, re-exported here for
// backward compatibility with existing imports.
// ---------------------------------------------------------------------------

pub(crate) use super::lsp_helpers::file_path_to_uri;

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ops::lsp_helpers::language_id_from_path;
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

    // --- parse_hover_contents tests ---

    #[test]
    fn test_parse_markup_content() {
        let response = serde_json::json!({
            "contents": {
                "kind": "markdown",
                "value": "```rust\nfn main()\n```\n\nThe entry point."
            }
        });
        let result = parse_hover_contents(&response);
        assert!(result.contains("fn main()"));
        assert!(result.contains("The entry point."));
    }

    #[test]
    fn test_parse_marked_string_plain() {
        let response = serde_json::json!({
            "contents": "fn foo() -> i32"
        });
        let result = parse_hover_contents(&response);
        assert_eq!(result, "fn foo() -> i32");
    }

    #[test]
    fn test_parse_marked_string_object() {
        let response = serde_json::json!({
            "contents": {
                "language": "rust",
                "value": "fn bar() -> bool"
            }
        });
        // This has a "value" key, so it matches MarkupContent path
        let result = parse_hover_contents(&response);
        assert_eq!(result, "fn bar() -> bool");
    }

    #[test]
    fn test_parse_marked_string_array() {
        let response = serde_json::json!({
            "contents": [
                { "language": "rust", "value": "fn baz()" },
                "Documentation for baz"
            ]
        });
        let result = parse_hover_contents(&response);
        assert!(result.contains("```rust"));
        assert!(result.contains("fn baz()"));
        assert!(result.contains("Documentation for baz"));
    }

    #[test]
    fn test_parse_hover_contents_empty() {
        let response = serde_json::json!({});
        let result = parse_hover_contents(&response);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_hover_contents_null_contents() {
        let response = serde_json::json!({ "contents": null });
        let result = parse_hover_contents(&response);
        assert!(result.is_empty());
    }

    #[test]
    fn test_parse_hover_contents_empty_array() {
        let response = serde_json::json!({ "contents": [] });
        let result = parse_hover_contents(&response);
        assert!(result.is_empty());
    }

    // --- parse_hover_range tests ---

    #[test]
    fn test_parse_hover_range_present() {
        let response = serde_json::json!({
            "range": {
                "start": { "line": 10, "character": 5 },
                "end": { "line": 10, "character": 15 }
            }
        });
        let range = parse_hover_range(&response).unwrap();
        assert_eq!(range.start_line, 10);
        assert_eq!(range.start_character, 5);
        assert_eq!(range.end_line, 10);
        assert_eq!(range.end_character, 15);
    }

    #[test]
    fn test_parse_hover_range_absent() {
        let response = serde_json::json!({});
        assert!(parse_hover_range(&response).is_none());
    }

    // --- Fallback to LSP index ---

    #[test]
    fn test_fallback_to_lsp_index_with_detail() {
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
        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap().unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.contents, "fn() -> MyStruct");
        assert!(result.symbol.is_some());
        assert_eq!(result.symbol.unwrap().name, "process");
    }

    #[test]
    fn test_fallback_to_lsp_index_without_detail() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "Config",
            23,
            None,
            "src/main.rs",
            1,
            0,
            30,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap().unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.contents, "Config (struct)");
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
        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap().unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert!(result.contents.contains("fn main()"));
        assert!(result.symbol.is_none());
    }

    // --- No data ---

    #[test]
    fn test_no_data_returns_none() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 10,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap();
        assert!(result.is_none());
    }

    // --- source_layer correctness ---

    #[test]
    fn test_source_layer_lsp_index_when_both_present() {
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
        let opts = GetHoverOptions {
            file_path: "src/lib.rs".to_string(),
            line: 5,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap().unwrap();
        // LSP index takes priority over tree-sitter (no live LSP available)
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
    }

    // --- Helper tests ---

    #[test]
    fn test_file_path_to_uri_absolute() {
        let uri = file_path_to_uri("/home/user/project/src/main.rs");
        assert_eq!(uri, "file:///home/user/project/src/main.rs");
    }

    #[test]
    fn test_language_id_from_path_known() {
        assert_eq!(language_id_from_path("main.rs"), "rust");
        assert_eq!(language_id_from_path("app.py"), "python");
        assert_eq!(language_id_from_path("index.ts"), "typescript");
        assert_eq!(language_id_from_path("main.go"), "go");
    }

    #[test]
    fn test_language_id_from_path_unknown() {
        assert_eq!(language_id_from_path("file.xyz"), "plaintext");
        assert_eq!(language_id_from_path("noext"), "plaintext");
    }

    // --- parse_hover_contents edge cases ---

    #[test]
    fn test_parse_hover_contents_marked_string_empty_language() {
        // When an array item has {language: "", value: "..."}, the value should
        // be returned without backtick wrapping (empty language means plain text).
        let response = serde_json::json!({
            "contents": [
                { "language": "", "value": "some code here" }
            ]
        });
        let result = parse_hover_contents(&response);
        assert_eq!(result, "some code here");
        assert!(
            !result.contains("```"),
            "empty language should not produce backtick fencing"
        );
    }

    #[test]
    fn test_parse_hover_contents_mixed_empty_and_nonempty_language() {
        // Verify empty-language and non-empty-language items are handled
        // correctly when mixed in the same array.
        let response = serde_json::json!({
            "contents": [
                { "language": "rust", "value": "fn typed()" },
                { "language": "", "value": "plain text" }
            ]
        });
        let result = parse_hover_contents(&response);
        assert!(result.contains("```rust\nfn typed()\n```"));
        assert!(result.contains("plain text"));
        assert!(
            !result.contains("```\nplain text"),
            "empty-language item should not get backtick fencing"
        );
    }

    #[test]
    fn test_parse_hover_contents_unrecognized_items_skipped() {
        // Array items that are neither strings nor {language, value} objects
        // should be silently skipped.
        let response = serde_json::json!({
            "contents": [
                42,
                true,
                null,
                { "random": "data without value key" },
                "valid string item"
            ]
        });
        let result = parse_hover_contents(&response);
        // Only the valid string item should survive.
        assert_eq!(result, "valid string item");
    }

    #[test]
    fn test_parse_hover_contents_all_items_unrecognized() {
        // When every array item is unrecognized, the result should be empty.
        let response = serde_json::json!({
            "contents": [
                42,
                null,
                { "unexpected": "shape" },
                false
            ]
        });
        let result = parse_hover_contents(&response);
        assert!(
            result.is_empty(),
            "all-unrecognized array should produce empty string"
        );
    }

    // --- try_live_lsp graceful degradation ---

    #[test]
    fn test_try_live_lsp_skipped_when_shared_client_contains_none() {
        // When a SharedLspClient is present but contains None (no connected
        // process), has_live_lsp() returns false and get_hover should fall
        // through to index layers without error.
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 0);

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 1,
            character: 0,
        };
        // No index data exists, so the result should be None (graceful fall-through).
        let result = get_hover(&ctx, &opts).unwrap();
        assert!(
            result.is_none(),
            "should fall through all layers and return None"
        );
    }

    #[test]
    fn test_try_live_lsp_skipped_falls_to_lsp_index() {
        // When has_live_lsp() is false but LSP index data exists, the hover
        // result should come from the LSP index layer.
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_lsp_symbol(
            &conn,
            "sym1",
            "example",
            12,
            Some("fn example() -> bool"),
            "src/main.rs",
            1,
            0,
            10,
            1,
        );

        let shared: crate::lsp_worker::SharedLspClient =
            std::sync::Arc::new(std::sync::Mutex::new(None));
        let ctx = LayeredContext::new(&conn, Some(&shared));

        let opts = GetHoverOptions {
            file_path: "src/main.rs".to_string(),
            line: 5,
            character: 0,
        };
        let result = get_hover(&ctx, &opts).unwrap().unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.contents, "fn example() -> bool");
    }

    // --- HoverResult serialization ---

    #[test]
    fn test_hover_result_serializable() {
        let result = HoverResult {
            contents: "fn foo()".to_string(),
            range: Some(LspRange {
                start_line: 1,
                start_character: 0,
                end_line: 5,
                end_character: 1,
            }),
            symbol: None,
            source_layer: SourceLayer::LiveLsp,
        };
        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: HoverResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.contents, "fn foo()");
        assert_eq!(roundtrip.source_layer, SourceLayer::LiveLsp);
    }
}
