//! Find all references to a symbol using layered resolution.
//!
//! Uses [`LayeredContext`] to query three data layers in priority order:
//! 1. **Live LSP** -- `textDocument/references` for full cross-file results
//! 2. **LSP index** -- `lsp_callers_of` for call-edge-based references
//! 3. **Tree-sitter** -- `ts_chunks_matching` for text-based search
//!
//! Each reference is enriched with the enclosing symbol via
//! [`LayeredContext::enrich_location`], and results are grouped by file.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::error::CodeContextError;
use crate::layered_context::{LayeredContext, LspRange, SourceLayer, SymbolInfo};
use crate::ops::lsp_helpers::{file_path_to_uri, uri_to_file_path};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for [`get_references`].
#[derive(Debug, Clone)]
pub struct GetReferencesOptions {
    /// Path to the file containing the symbol.
    pub file_path: String,
    /// 0-based line number of the symbol.
    pub line: u32,
    /// 0-based character offset of the symbol.
    pub character: u32,
    /// Whether to include the declaration itself in the results.
    pub include_declaration: bool,
    /// Maximum number of references to return (None = unlimited).
    pub max_results: Option<usize>,
}

/// A single reference location with optional enclosing symbol.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceLocation {
    /// File containing the reference.
    pub file_path: String,
    /// Range of the reference in the file.
    pub range: LspRange,
    /// The enclosing symbol (e.g., the function this reference is inside).
    pub enclosing_symbol: Option<SymbolInfo>,
}

/// References grouped by file path.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileReferenceGroup {
    /// The file path shared by all references in this group.
    pub file_path: String,
    /// References within this file.
    pub references: Vec<ReferenceLocation>,
}

/// Result of a references query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferencesResult {
    /// All reference locations (after truncation by max_results).
    pub references: Vec<ReferenceLocation>,
    /// Total number of references found before truncation.
    pub total_count: usize,
    /// References grouped by file.
    pub by_file: Vec<FileReferenceGroup>,
    /// Which data layer provided the results.
    pub source_layer: SourceLayer,
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Find all references to the symbol at the given location.
///
/// Tries live LSP first, falls back to LSP index call edges, then to
/// tree-sitter text search. Each reference is enriched with the enclosing
/// symbol. Results are grouped by file and truncated to `max_results`.
///
/// # Arguments
/// * `ctx` - Layered context providing access to all data layers.
/// * `options` - Query parameters (file path, position, limits).
///
/// # Returns
/// A [`ReferencesResult`] with references, grouping, count, and source layer.
pub fn get_references(
    ctx: &LayeredContext,
    options: &GetReferencesOptions,
) -> Result<ReferencesResult, CodeContextError> {
    // Layer 1: Live LSP
    if let Some(result) = try_live_lsp(ctx, options)? {
        return Ok(result);
    }

    // Layer 2: LSP index (call edges as proxy)
    if let Some(result) = try_lsp_index(ctx, options) {
        return Ok(result);
    }

    // Layer 3: Tree-sitter text search
    Ok(try_treesitter(ctx, options))
}

// ---------------------------------------------------------------------------
// Layer implementations
// ---------------------------------------------------------------------------

/// Try to find references via live LSP `textDocument/references`.
///
/// Sends didOpen, the references request, and didClose atomically under a
/// single mutex hold to prevent interleaving with the indexing worker.
fn try_live_lsp(
    ctx: &LayeredContext,
    options: &GetReferencesOptions,
) -> Result<Option<ReferencesResult>, CodeContextError> {
    if !ctx.has_live_lsp() {
        return Ok(None);
    }

    let uri = file_path_to_uri(&options.file_path);

    let params = serde_json::json!({
        "textDocument": { "uri": &uri },
        "position": {
            "line": options.line,
            "character": options.character
        },
        "context": {
            "includeDeclaration": options.include_declaration
        }
    });

    let response =
        ctx.lsp_request_with_document(&options.file_path, "textDocument/references", params)?;

    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };

    // Parse the LSP response into reference locations
    let locations = parse_lsp_locations(&response);
    if locations.is_empty() {
        return Ok(None);
    }

    let enriched = enrich_all(ctx, locations);
    Ok(Some(build_result(
        enriched,
        options.max_results,
        SourceLayer::LiveLsp,
    )))
}

/// Try to find references via LSP index call edges.
///
/// Looks up the symbol at the given position, then finds all callers
/// via `lsp_callers_of`. Returns None if no symbol or callers are found.
fn try_lsp_index(ctx: &LayeredContext, options: &GetReferencesOptions) -> Option<ReferencesResult> {
    // Find the symbol at the given position
    let range = LspRange {
        start_line: options.line,
        start_character: options.character,
        end_line: options.line,
        end_character: options.character + 1,
    };

    let symbol = ctx.lsp_symbol_at(&options.file_path, &range)?;

    // The qualified_path from lsp_symbol_at is the raw `id` column from lsp_symbols,
    // which is already the full symbol ID (e.g., "lsp:src/lib.rs:process").
    // Use it directly if available, otherwise construct from file + name.
    let symbol_id = match &symbol.qualified_path {
        Some(qpath) => qpath.clone(),
        None => format!("lsp:{}:{}", options.file_path, symbol.name),
    };

    let callers = ctx.lsp_callers_of(&symbol_id);
    if callers.is_empty() {
        return None;
    }

    // Convert call edges to reference locations (use the first call site range,
    // or the symbol range if no call sites are recorded)
    let locations: Vec<(String, LspRange)> = callers
        .into_iter()
        .flat_map(|edge| {
            let file = edge.symbol.file_path.clone();
            if edge.call_sites.is_empty() {
                vec![(file, edge.symbol.range)]
            } else {
                edge.call_sites
                    .into_iter()
                    .map(|cs| (file.clone(), cs))
                    .collect()
            }
        })
        .collect();

    let enriched = enrich_all(ctx, locations);
    Some(build_result(
        enriched,
        options.max_results,
        SourceLayer::LspIndex,
    ))
}

/// Fall back to tree-sitter text search for the symbol name.
///
/// Extracts the symbol name from the file at the given position using
/// the tree-sitter index, then searches all chunks for that name.
fn try_treesitter(ctx: &LayeredContext, options: &GetReferencesOptions) -> ReferencesResult {
    // Try to get the symbol name from the chunk at the cursor position
    let symbol_name = ctx
        .ts_chunk_at(&options.file_path, options.line)
        .and_then(|chunk| extract_identifier_at_line(&chunk.text, options.line, chunk.start_line));

    let search_term = match symbol_name {
        Some(name) => name,
        None => {
            return ReferencesResult {
                references: Vec::new(),
                total_count: 0,
                by_file: Vec::new(),
                source_layer: SourceLayer::None,
            };
        }
    };

    // Search limit: use a generous internal limit, then truncate to max_results
    let search_limit = options.max_results.map(|m| m * 3).unwrap_or(200);
    let chunks = ctx.ts_chunks_matching(&search_term, search_limit);

    let locations: Vec<(String, LspRange)> = chunks
        .into_iter()
        .map(|chunk| {
            let range = LspRange {
                start_line: chunk.start_line,
                start_character: 0,
                end_line: chunk.start_line,
                end_character: 0,
            };
            (chunk.file_path, range)
        })
        .collect();

    let enriched = enrich_all(ctx, locations);
    build_result(enriched, options.max_results, SourceLayer::TreeSitter)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse an LSP `textDocument/references` response into (file_path, range) pairs.
fn parse_lsp_locations(response: &serde_json::Value) -> Vec<(String, LspRange)> {
    let arr = match response.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|loc| {
            let uri = loc.get("uri")?.as_str()?;
            let file_path_str = uri_to_file_path(uri);
            let range = loc.get("range")?;
            let start = range.get("start")?;
            let end = range.get("end")?;

            Some((
                file_path_str,
                LspRange {
                    start_line: start.get("line")?.as_u64()? as u32,
                    start_character: start.get("character")?.as_u64()? as u32,
                    end_line: end.get("line")?.as_u64()? as u32,
                    end_character: end.get("character")?.as_u64()? as u32,
                },
            ))
        })
        .collect()
}

/// Enrich each location with its enclosing symbol.
fn enrich_all(ctx: &LayeredContext, locations: Vec<(String, LspRange)>) -> Vec<ReferenceLocation> {
    locations
        .into_iter()
        .map(|(file_path, range)| {
            let enrichment = ctx.enrich_location(&file_path, &range);
            ReferenceLocation {
                file_path,
                range,
                enclosing_symbol: enrichment.symbol,
            }
        })
        .collect()
}

/// Build the final result with grouping, truncation, and total count.
fn build_result(
    references: Vec<ReferenceLocation>,
    max_results: Option<usize>,
    source_layer: SourceLayer,
) -> ReferencesResult {
    let total_count = references.len();

    let truncated = match max_results {
        Some(max) if references.len() > max => references[..max].to_vec(),
        _ => references,
    };

    let by_file = group_by_file(&truncated);

    ReferencesResult {
        references: truncated,
        total_count,
        by_file,
        source_layer,
    }
}

/// Group reference locations by file path, preserving order of first appearance.
fn group_by_file(references: &[ReferenceLocation]) -> Vec<FileReferenceGroup> {
    let mut groups: BTreeMap<&str, Vec<ReferenceLocation>> = BTreeMap::new();

    for r in references {
        groups.entry(&r.file_path).or_default().push(r.clone());
    }

    groups
        .into_iter()
        .map(|(file_path, refs)| FileReferenceGroup {
            file_path: file_path.to_string(),
            references: refs,
        })
        .collect()
}

/// Extract an identifier from the given line of chunk text.
///
/// Uses a simple heuristic: finds the first word-like token on the target line.
/// Returns None if the line is not within the chunk or contains no identifiers.
fn extract_identifier_at_line(
    chunk_text: &str,
    target_line: u32,
    chunk_start_line: u32,
) -> Option<String> {
    let line_offset = (target_line - chunk_start_line) as usize;
    let line = chunk_text.lines().nth(line_offset)?;

    // Find the longest word-like token (alphanumeric + underscore)
    line.split(|c: char| !c.is_alphanumeric() && c != '_')
        .filter(|w| {
            !w.is_empty()
                && w.chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
        })
        .max_by_key(|w| w.len())
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layered_context::LayeredContext;
    use crate::test_fixtures::{insert_file_simple as insert_file, test_db};
    use rusqlite::Connection;

    /// Insert an LSP symbol.
    fn insert_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: i32,
        end_line: i32,
    ) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, 0)",
            rusqlite::params![id, name, kind, file_path, start_line, end_line],
        )
        .unwrap();
    }

    /// Insert a call edge with from_ranges.
    fn insert_edge(
        conn: &Connection,
        caller_id: &str,
        callee_id: &str,
        caller_file: &str,
        callee_file: &str,
        from_ranges: &str,
    ) {
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges, source)
             VALUES (?1, ?2, ?3, ?4, ?5, 'lsp')",
            rusqlite::params![caller_id, callee_id, caller_file, callee_file, from_ranges],
        )
        .unwrap();
    }

    /// Insert a tree-sitter chunk.
    fn insert_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: i32,
        end_line: i32,
        text: &str,
        symbol_path: Option<&str>,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path)
             VALUES (?1, 0, 100, ?2, ?3, ?4, ?5)",
            rusqlite::params![file_path, start_line, end_line, text, symbol_path],
        )
        .unwrap();
    }

    #[test]
    fn test_grouping_by_file() {
        let refs = vec![
            ReferenceLocation {
                file_path: "src/a.rs".to_string(),
                range: LspRange {
                    start_line: 10,
                    start_character: 0,
                    end_line: 10,
                    end_character: 5,
                },
                enclosing_symbol: None,
            },
            ReferenceLocation {
                file_path: "src/b.rs".to_string(),
                range: LspRange {
                    start_line: 20,
                    start_character: 0,
                    end_line: 20,
                    end_character: 5,
                },
                enclosing_symbol: None,
            },
            ReferenceLocation {
                file_path: "src/a.rs".to_string(),
                range: LspRange {
                    start_line: 15,
                    start_character: 0,
                    end_line: 15,
                    end_character: 5,
                },
                enclosing_symbol: None,
            },
        ];

        let groups = group_by_file(&refs);

        assert_eq!(groups.len(), 2, "should have two file groups");

        let a_group = groups.iter().find(|g| g.file_path == "src/a.rs").unwrap();
        assert_eq!(a_group.references.len(), 2, "src/a.rs should have 2 refs");

        let b_group = groups.iter().find(|g| g.file_path == "src/b.rs").unwrap();
        assert_eq!(b_group.references.len(), 1, "src/b.rs should have 1 ref");
    }

    #[test]
    fn test_max_results_truncation_preserves_total_count() {
        let refs: Vec<ReferenceLocation> = (0..10)
            .map(|i| ReferenceLocation {
                file_path: format!("src/file_{}.rs", i),
                range: LspRange {
                    start_line: i,
                    start_character: 0,
                    end_line: i,
                    end_character: 5,
                },
                enclosing_symbol: None,
            })
            .collect();

        let result = build_result(refs, Some(3), SourceLayer::TreeSitter);

        assert_eq!(
            result.total_count, 10,
            "total_count should be 10 (pre-truncation)"
        );
        assert_eq!(result.references.len(), 3, "should be truncated to 3");
        assert_eq!(
            result.by_file.len(),
            3,
            "groups should reflect truncated set"
        );
    }

    #[test]
    fn test_enrichment_via_enrich_location() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs");

        // Insert a symbol that covers lines 5-15 (the "enclosing" function)
        insert_symbol(
            &conn,
            "lsp:src/main.rs:main",
            "main",
            12, // Function
            "src/main.rs",
            5,
            15,
        );

        let ctx = LayeredContext::new(&conn, None);

        // A reference at line 10 should be enriched with the "main" function
        let locations = vec![(
            "src/main.rs".to_string(),
            LspRange {
                start_line: 10,
                start_character: 0,
                end_line: 10,
                end_character: 5,
            },
        )];

        let enriched = enrich_all(&ctx, locations);

        assert_eq!(enriched.len(), 1);
        let r = &enriched[0];
        assert!(r.enclosing_symbol.is_some(), "should have enclosing symbol");
        assert_eq!(r.enclosing_symbol.as_ref().unwrap().name, "main");
    }

    #[test]
    fn test_fallback_to_lsp_callers_of() {
        let conn = test_db();
        insert_file(&conn, "src/lib.rs");
        insert_file(&conn, "src/caller.rs");

        // The target symbol (callee)
        insert_symbol(
            &conn,
            "lsp:src/lib.rs:process",
            "process",
            12,
            "src/lib.rs",
            0,
            10,
        );

        // A caller symbol
        insert_symbol(
            &conn,
            "lsp:src/caller.rs:handle_request",
            "handle_request",
            12,
            "src/caller.rs",
            0,
            20,
        );

        // Call edge: handle_request calls process
        let from_ranges = r#"[{"start":{"line":5,"character":4},"end":{"line":5,"character":11}}]"#;
        insert_edge(
            &conn,
            "lsp:src/caller.rs:handle_request",
            "lsp:src/lib.rs:process",
            "src/caller.rs",
            "src/lib.rs",
            from_ranges,
        );

        let ctx = LayeredContext::new(&conn, None);

        let options = GetReferencesOptions {
            file_path: "src/lib.rs".to_string(),
            line: 0,
            character: 0,
            include_declaration: false,
            max_results: None,
        };

        let result = get_references(&ctx, &options).unwrap();

        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert!(
            !result.references.is_empty(),
            "should find call-site references"
        );
        assert_eq!(result.references[0].file_path, "src/caller.rs");
        // The call site is at line 5, char 4
        assert_eq!(result.references[0].range.start_line, 5);
        assert_eq!(result.references[0].range.start_character, 4);
    }

    #[test]
    fn test_treesitter_fallback() {
        let conn = test_db();
        insert_file(&conn, "src/target.rs");
        insert_file(&conn, "src/user.rs");

        // No LSP symbols or edges -- only tree-sitter chunks
        insert_chunk(
            &conn,
            "src/target.rs",
            5,
            10,
            "fn my_function() {\n    // body\n}",
            Some("my_function"),
        );

        // A chunk in another file that mentions "my_function"
        insert_chunk(
            &conn,
            "src/user.rs",
            1,
            5,
            "use crate::my_function;\nfn caller() { my_function(); }",
            Some("caller"),
        );

        let ctx = LayeredContext::new(&conn, None);

        let options = GetReferencesOptions {
            file_path: "src/target.rs".to_string(),
            line: 5,
            character: 3,
            include_declaration: false,
            max_results: None,
        };

        let result = get_references(&ctx, &options).unwrap();

        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        // Should find chunks matching "my_function" -- at least the user.rs chunk
        assert!(
            result.total_count >= 1,
            "should find at least one tree-sitter match"
        );
    }

    #[test]
    fn test_no_results_returns_none_layer() {
        let conn = test_db();
        let ctx = LayeredContext::new(&conn, None);

        let options = GetReferencesOptions {
            file_path: "nonexistent.rs".to_string(),
            line: 0,
            character: 0,
            include_declaration: false,
            max_results: None,
        };

        let result = get_references(&ctx, &options).unwrap();

        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.references.is_empty());
        assert_eq!(result.total_count, 0);
    }

    #[test]
    fn test_parse_lsp_locations() {
        let response = serde_json::json!([
            {
                "uri": "file:///src/a.rs",
                "range": {
                    "start": { "line": 10, "character": 5 },
                    "end": { "line": 10, "character": 15 }
                }
            },
            {
                "uri": "file:///src/b.rs",
                "range": {
                    "start": { "line": 20, "character": 0 },
                    "end": { "line": 20, "character": 8 }
                }
            }
        ]);

        let locations = parse_lsp_locations(&response);

        assert_eq!(locations.len(), 2);
        assert_eq!(locations[0].0, "/src/a.rs");
        assert_eq!(locations[0].1.start_line, 10);
        assert_eq!(locations[0].1.start_character, 5);
        assert_eq!(locations[1].0, "/src/b.rs");
        assert_eq!(locations[1].1.start_line, 20);
    }

    #[test]
    fn test_extract_identifier_at_line() {
        let text = "fn my_function() {\n    let x = 42;\n}";
        let id = extract_identifier_at_line(text, 0, 0);
        assert_eq!(id, Some("my_function".to_string()));

        let id2 = extract_identifier_at_line(text, 1, 0);
        // "let" or "x" -- should pick a word
        assert!(id2.is_some());

        // Out of bounds
        let id3 = extract_identifier_at_line(text, 100, 0);
        assert!(id3.is_none());
    }
}
