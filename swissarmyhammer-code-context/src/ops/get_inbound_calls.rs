//! Inbound call hierarchy -- "who calls this function?"
//!
//! Uses layered resolution via [`LayeredContext`]:
//!
//! 1. **Live LSP** -- `textDocument/prepareCallHierarchy` then
//!    `callHierarchy/incomingCalls`, recursive up to `depth`.
//!    Cross-references with `ctx.lsp_callers_of()` for completeness.
//! 2. **LSP index** -- reverse-traverse indexed call edges via
//!    `ctx.lsp_callers_of()`.
//! 3. **Tree-sitter** -- `ctx.ts_callers_of()` for tree-sitter-derived edges.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::error::CodeContextError;
use crate::layered_context::{LayeredContext, LspRange, SourceLayer};
use crate::ops::lsp_helpers::{file_path_to_uri, parse_lsp_range, uri_to_file_path};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Options for the `get_inbound_calls` operation.
#[derive(Debug, Clone)]
pub struct GetInboundCallsOptions {
    /// Path to the file (relative to workspace root).
    pub file_path: String,
    /// Zero-based line number.
    pub line: u32,
    /// Zero-based character offset within the line.
    pub character: u32,
    /// Recursive depth for caller traversal (clamped to 1..=5).
    pub depth: u32,
}

/// Result of an inbound calls lookup.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundCallsResult {
    /// Name of the target symbol whose callers were resolved.
    pub target: String,
    /// The callers found.
    pub callers: Vec<InboundCallEntry>,
    /// Which data layer provided the result.
    pub source_layer: SourceLayer,
}

/// A single inbound call entry, potentially with recursive sub-callers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundCallEntry {
    /// Name of the calling symbol.
    pub symbol_name: String,
    /// File containing the calling symbol.
    pub file_path: String,
    /// Range of the calling symbol definition.
    pub range: LspRange,
    /// Ranges within the caller where the target is invoked.
    pub call_sites: Vec<LspRange>,
    /// Depth at which this caller was discovered (1-based).
    pub depth: u32,
    /// Recursive callers of this caller (populated when depth > 1).
    pub callers: Vec<InboundCallEntry>,
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

/// Get inbound calls (callers) for a symbol at a position in a file.
///
/// Tries live LSP first (two-phase call hierarchy protocol), then the
/// LSP call edge index, then tree-sitter call edges. Returns an empty
/// result with `SourceLayer::None` if no layer has data.
///
/// # Arguments
/// * `ctx` - The layered context providing access to all data layers.
/// * `opts` - File path, line, character, and depth.
///
/// # Errors
/// Returns a `CodeContextError` if an LSP request fails.
pub fn get_inbound_calls(
    ctx: &LayeredContext,
    opts: &GetInboundCallsOptions,
) -> Result<InboundCallsResult, CodeContextError> {
    let depth = opts.depth.clamp(1, 5);

    // Layer 1: Live LSP
    if ctx.has_live_lsp() {
        if let Some(result) = try_live_lsp(ctx, opts, depth)? {
            return Ok(result);
        }
    }

    // Layer 2: LSP index
    if let Some(result) = try_lsp_index(ctx, opts, depth) {
        return Ok(result);
    }

    // Layer 3: Tree-sitter
    if let Some(result) = try_treesitter(ctx, opts, depth) {
        return Ok(result);
    }

    Ok(InboundCallsResult {
        target: String::new(),
        callers: Vec::new(),
        source_layer: SourceLayer::None,
    })
}

// ---------------------------------------------------------------------------
// Layer 1: Live LSP
// ---------------------------------------------------------------------------

/// Attempt to get inbound calls from a live LSP server.
///
/// Uses the two-phase call hierarchy protocol:
/// 1. `textDocument/prepareCallHierarchy` to get a `CallHierarchyItem`
/// 2. `callHierarchy/incomingCalls` to get callers, recursive up to `depth`
///
/// After getting live results, cross-references with `ctx.lsp_callers_of()`
/// to pick up callers from files the live LSP may have missed.
fn try_live_lsp(
    ctx: &LayeredContext,
    opts: &GetInboundCallsOptions,
    depth: u32,
) -> Result<Option<InboundCallsResult>, CodeContextError> {
    let uri = file_path_to_uri(&opts.file_path);

    // Atomic didOpen + prepareCallHierarchy + didClose
    let prepare_response = ctx.lsp_request_with_document(
        &opts.file_path,
        "textDocument/prepareCallHierarchy",
        json!({
            "textDocument": { "uri": uri },
            "position": { "line": opts.line, "character": opts.character }
        }),
    )?;

    let prepare_response = match prepare_response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(None),
    };

    let items = parse_call_hierarchy_items(&prepare_response);
    if items.is_empty() {
        return Ok(None);
    }

    let target_item = &items[0];
    let target_name = target_item.name.clone();

    // Recursively fetch incoming calls
    let mut visited = HashSet::new();
    visited.insert(format!("{}:{}", target_item.uri, target_item.name));
    let callers = fetch_incoming_calls_recursive(ctx, target_item, depth, 1, &mut visited)?;

    // Cross-reference with indexed callers for completeness
    let callers = cross_reference_with_index(ctx, &target_name, &opts.file_path, callers);

    Ok(Some(InboundCallsResult {
        target: target_name,
        callers,
        source_layer: SourceLayer::LiveLsp,
    }))
}

/// A parsed call hierarchy item from LSP.
#[derive(Debug, Clone)]
struct CallHierarchyItem {
    name: String,
    uri: String,
    range: LspRange,
    /// The full JSON value, needed for passing back to incomingCalls.
    json: serde_json::Value,
}

/// Parse the response from `textDocument/prepareCallHierarchy`.
///
/// Returns a list of `CallHierarchyItem` from either a single item or an array.
fn parse_call_hierarchy_items(response: &serde_json::Value) -> Vec<CallHierarchyItem> {
    let items = if response.is_array() {
        response.as_array().cloned().unwrap_or_default()
    } else if response.is_object() {
        vec![response.clone()]
    } else {
        return Vec::new();
    };

    items
        .into_iter()
        .filter_map(|item| {
            let name = item.get("name")?.as_str()?.to_string();
            let uri = item.get("uri")?.as_str()?.to_string();
            let range = parse_lsp_range(item.get("range")?)?;
            Some(CallHierarchyItem {
                name,
                uri,
                range,
                json: item,
            })
        })
        .collect()
}

/// Recursively fetch incoming calls for a call hierarchy item.
fn fetch_incoming_calls_recursive(
    ctx: &LayeredContext,
    item: &CallHierarchyItem,
    max_depth: u32,
    current_depth: u32,
    visited: &mut HashSet<String>,
) -> Result<Vec<InboundCallEntry>, CodeContextError> {
    if current_depth > max_depth {
        return Ok(Vec::new());
    }

    let response = ctx.lsp_request("callHierarchy/incomingCalls", json!({ "item": item.json }))?;
    let response = match response {
        Some(v) if !v.is_null() => v,
        _ => return Ok(Vec::new()),
    };

    let incoming = parse_incoming_calls(&response);
    let mut entries = Vec::new();
    for call in incoming {
        if let Some(entry) = resolve_incoming_call(ctx, call, max_depth, current_depth, visited)? {
            entries.push(entry);
        }
    }
    Ok(entries)
}

/// Convert a single parsed `IncomingCall` into an `InboundCallEntry`,
/// performing cycle detection and optional recursion into sub-callers.
///
/// Returns `None` if the caller was already visited (cycle).
fn resolve_incoming_call(
    ctx: &LayeredContext,
    call: IncomingCall,
    max_depth: u32,
    current_depth: u32,
    visited: &mut HashSet<String>,
) -> Result<Option<InboundCallEntry>, CodeContextError> {
    let visit_key = format!("{}:{}", call.from_item.uri, call.from_item.name);
    if !visited.insert(visit_key) {
        return Ok(None);
    }

    let sub_callers = if current_depth < max_depth {
        fetch_incoming_calls_recursive(ctx, &call.from_item, max_depth, current_depth + 1, visited)?
    } else {
        Vec::new()
    };

    Ok(Some(InboundCallEntry {
        symbol_name: call.from_item.name.clone(),
        file_path: uri_to_file_path(&call.from_item.uri),
        range: call.from_item.range.clone(),
        call_sites: call.from_ranges,
        depth: current_depth,
        callers: sub_callers,
    }))
}

/// A parsed incoming call from LSP `callHierarchy/incomingCalls`.
#[derive(Debug)]
struct IncomingCall {
    from_item: CallHierarchyItem,
    from_ranges: Vec<LspRange>,
}

/// Parse the response from `callHierarchy/incomingCalls`.
///
/// Each entry has `from` (a CallHierarchyItem) and `fromRanges` (array of ranges).
fn parse_incoming_calls(response: &serde_json::Value) -> Vec<IncomingCall> {
    let arr = match response.as_array() {
        Some(a) => a,
        None => return Vec::new(),
    };

    arr.iter()
        .filter_map(|entry| {
            let from_json = entry.get("from")?;
            let from_item = {
                let name = from_json.get("name")?.as_str()?.to_string();
                let uri = from_json.get("uri")?.as_str()?.to_string();
                let range = parse_lsp_range(from_json.get("range")?)?;
                CallHierarchyItem {
                    name,
                    uri,
                    range,
                    json: from_json.clone(),
                }
            };

            let from_ranges = entry
                .get("fromRanges")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(parse_lsp_range).collect())
                .unwrap_or_default();

            Some(IncomingCall {
                from_item,
                from_ranges,
            })
        })
        .collect()
}

/// Cross-reference live LSP results with the indexed call edges.
///
/// Merges callers from `ctx.lsp_callers_of()` that were not already found
/// by the live LSP response.
fn cross_reference_with_index(
    ctx: &LayeredContext,
    target_name: &str,
    target_file: &str,
    mut callers: Vec<InboundCallEntry>,
) -> Vec<InboundCallEntry> {
    // Search for the symbol by name in the index
    let symbols = ctx.lsp_symbols_by_name(target_name, 10);
    for sym in &symbols {
        if sym.file_path != target_file {
            continue;
        }
        let symbol_id = sym.qualified_path.as_deref().unwrap_or(&sym.name);

        let indexed_callers = ctx.lsp_callers_of(symbol_id);
        let existing_names: HashSet<String> =
            callers.iter().map(|c| c.symbol_name.clone()).collect();

        for edge in indexed_callers {
            if !existing_names.contains(&edge.symbol.name) {
                callers.push(InboundCallEntry {
                    symbol_name: edge.symbol.name.clone(),
                    file_path: edge.symbol.file_path.clone(),
                    range: edge.symbol.range.clone(),
                    call_sites: edge.call_sites,
                    depth: 1,
                    callers: Vec::new(),
                });
            }
        }
        break; // only use the first matching symbol
    }

    callers
}

// ---------------------------------------------------------------------------
// Layer 2: LSP index
// ---------------------------------------------------------------------------

/// Attempt to get inbound calls from the persisted LSP call edge index.
///
/// Looks up the symbol at the cursor, then finds callers via
/// `ctx.lsp_callers_of()`. Recurses for each caller when depth > 1.
fn try_lsp_index(
    ctx: &LayeredContext,
    opts: &GetInboundCallsOptions,
    depth: u32,
) -> Option<InboundCallsResult> {
    let range = LspRange {
        start_line: opts.line,
        start_character: opts.character,
        end_line: opts.line,
        end_character: opts.character,
    };

    let symbol = ctx.lsp_symbol_at(&opts.file_path, &range)?;
    let symbol_id = symbol.qualified_path.as_deref().unwrap_or(&symbol.name);

    let mut visited = HashSet::new();
    visited.insert(symbol_id.to_string());

    let callers = collect_lsp_index_callers(ctx, symbol_id, depth, 1, &mut visited);

    if callers.is_empty() {
        return None;
    }

    Some(InboundCallsResult {
        target: symbol.name.clone(),
        callers,
        source_layer: SourceLayer::LspIndex,
    })
}

/// Recursively collect callers from the LSP index.
fn collect_lsp_index_callers(
    ctx: &LayeredContext,
    symbol_id: &str,
    max_depth: u32,
    current_depth: u32,
    visited: &mut HashSet<String>,
) -> Vec<InboundCallEntry> {
    if current_depth > max_depth {
        return Vec::new();
    }

    let edges = ctx.lsp_callers_of(symbol_id);
    let mut entries = Vec::new();

    for edge in edges {
        let caller_id = edge
            .symbol
            .qualified_path
            .as_deref()
            .unwrap_or(&edge.symbol.name);

        if !visited.insert(caller_id.to_string()) {
            continue;
        }

        let sub_callers = if current_depth < max_depth {
            collect_lsp_index_callers(ctx, caller_id, max_depth, current_depth + 1, visited)
        } else {
            Vec::new()
        };

        entries.push(InboundCallEntry {
            symbol_name: edge.symbol.name.clone(),
            file_path: edge.symbol.file_path.clone(),
            range: edge.symbol.range.clone(),
            call_sites: edge.call_sites,
            depth: current_depth,
            callers: sub_callers,
        });
    }

    entries
}

// ---------------------------------------------------------------------------
// Layer 3: Tree-sitter
// ---------------------------------------------------------------------------

/// Attempt to get inbound calls from tree-sitter call edges.
///
/// Looks up the symbol name at the cursor, then uses `ctx.ts_callers_of()`
/// to find tree-sitter-derived callers.
fn try_treesitter(
    ctx: &LayeredContext,
    opts: &GetInboundCallsOptions,
    depth: u32,
) -> Option<InboundCallsResult> {
    // Try ts_symbols_in_file first for accurate symbol names (from symbol_path).
    // Fall back to find_symbol which may return chunk text as the name.
    let symbol = find_ts_symbol_at_cursor(ctx, opts)
        .or_else(|| ctx.find_symbol(&opts.file_path, opts.line, opts.character))?;

    let symbol_id = symbol.qualified_path.as_deref().unwrap_or(&symbol.name);
    let callers = ctx.lsp_callers_of(symbol_id);
    if callers.is_empty() {
        return None;
    }

    let entries = callers
        .into_iter()
        .map(|edge| call_edge_to_entry(ctx, edge, depth))
        .collect();

    Some(InboundCallsResult {
        target: symbol.name,
        callers: entries,
        source_layer: SourceLayer::TreeSitter,
    })
}

/// Convert a [`CallEdgeInfo`] from the index into an [`InboundCallEntry`],
/// optionally recursing one level to gather sub-callers when `depth > 1`.
fn call_edge_to_entry(
    ctx: &LayeredContext,
    edge: crate::layered_context::CallEdgeInfo,
    depth: u32,
) -> InboundCallEntry {
    let sub_callers = if depth > 1 {
        let sub_id = edge
            .symbol
            .qualified_path
            .as_deref()
            .unwrap_or(&edge.symbol.name);
        ctx.lsp_callers_of(sub_id)
            .into_iter()
            .map(|sub| InboundCallEntry {
                symbol_name: sub.symbol.name,
                file_path: sub.symbol.file_path,
                range: sub.symbol.range,
                call_sites: sub.call_sites,
                depth: 2,
                callers: Vec::new(),
            })
            .collect()
    } else {
        Vec::new()
    };

    InboundCallEntry {
        symbol_name: edge.symbol.name,
        file_path: edge.symbol.file_path,
        range: edge.symbol.range,
        call_sites: edge.call_sites,
        depth: 1,
        callers: sub_callers,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Find a tree-sitter symbol at the cursor position using `ts_symbols_in_file`.
///
/// Returns the narrowest symbol whose range contains the cursor. This gives
/// accurate symbol names derived from `symbol_path` rather than raw chunk text.
fn find_ts_symbol_at_cursor(
    ctx: &LayeredContext,
    opts: &GetInboundCallsOptions,
) -> Option<crate::layered_context::SymbolInfo> {
    let symbols = ctx.ts_symbols_in_file(&opts.file_path);
    symbols
        .into_iter()
        .filter(|s| s.range.start_line <= opts.line && s.range.end_line >= opts.line)
        .min_by_key(|s| s.range.end_line - s.range.start_line)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_call_edge, insert_file, insert_ts_chunk, test_db};
    use rusqlite::Connection;

    /// Insert an LSP symbol (without detail, for inbound_calls tests).
    #[allow(clippy::too_many_arguments)]
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

    // --- prepareCallHierarchy response parsing ---

    #[test]
    fn test_parse_call_hierarchy_items_array() {
        let response = serde_json::json!([
            {
                "name": "process_request",
                "kind": 12,
                "uri": "file:///src/handler.rs",
                "range": {
                    "start": { "line": 10, "character": 0 },
                    "end": { "line": 25, "character": 1 }
                },
                "selectionRange": {
                    "start": { "line": 10, "character": 4 },
                    "end": { "line": 10, "character": 19 }
                }
            }
        ]);

        let items = parse_call_hierarchy_items(&response);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].name, "process_request");
        assert_eq!(items[0].uri, "file:///src/handler.rs");
        assert_eq!(items[0].range.start_line, 10);
        assert_eq!(items[0].range.end_line, 25);
    }

    #[test]
    fn test_parse_call_hierarchy_items_empty() {
        let response = serde_json::json!([]);
        let items = parse_call_hierarchy_items(&response);
        assert!(items.is_empty());
    }

    #[test]
    fn test_parse_call_hierarchy_items_null() {
        let response = serde_json::json!(null);
        let items = parse_call_hierarchy_items(&response);
        assert!(items.is_empty());
    }

    // --- incomingCalls response parsing ---

    #[test]
    fn test_parse_incoming_calls_response() {
        let response = serde_json::json!([
            {
                "from": {
                    "name": "main",
                    "kind": 12,
                    "uri": "file:///src/main.rs",
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 10, "character": 1 }
                    }
                },
                "fromRanges": [
                    {
                        "start": { "line": 5, "character": 4 },
                        "end": { "line": 5, "character": 20 }
                    }
                ]
            },
            {
                "from": {
                    "name": "handle_request",
                    "kind": 12,
                    "uri": "file:///src/handler.rs",
                    "range": {
                        "start": { "line": 20, "character": 0 },
                        "end": { "line": 30, "character": 1 }
                    }
                },
                "fromRanges": [
                    {
                        "start": { "line": 25, "character": 8 },
                        "end": { "line": 25, "character": 24 }
                    },
                    {
                        "start": { "line": 28, "character": 8 },
                        "end": { "line": 28, "character": 24 }
                    }
                ]
            }
        ]);

        let calls = parse_incoming_calls(&response);
        assert_eq!(calls.len(), 2);

        assert_eq!(calls[0].from_item.name, "main");
        assert_eq!(calls[0].from_ranges.len(), 1);
        assert_eq!(calls[0].from_ranges[0].start_line, 5);

        assert_eq!(calls[1].from_item.name, "handle_request");
        assert_eq!(calls[1].from_ranges.len(), 2);
    }

    #[test]
    fn test_parse_incoming_calls_empty() {
        let response = serde_json::json!([]);
        let calls = parse_incoming_calls(&response);
        assert!(calls.is_empty());
    }

    // --- LSP index fallback ---

    #[test]
    fn test_lsp_index_callers_single_depth() {
        let conn = test_db();
        insert_file(&conn, "src/main.rs", 0, 1);
        insert_file(&conn, "src/handler.rs", 0, 1);

        insert_lsp_symbol(
            &conn,
            "sym:target",
            "process",
            12,
            "src/handler.rs",
            10,
            0,
            25,
            1,
        );
        insert_lsp_symbol(&conn, "sym:caller", "main", 12, "src/main.rs", 1, 0, 20, 1);

        let from_ranges = r#"[{"start":{"line":5,"character":4},"end":{"line":5,"character":20}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:target",
            "src/main.rs",
            "src/handler.rs",
            "lsp",
            from_ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/handler.rs".to_string(),
            line: 15,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.target, "process");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "main");
        assert_eq!(result.callers[0].depth, 1);
        assert_eq!(result.callers[0].call_sites.len(), 1);
        assert_eq!(result.callers[0].call_sites[0].start_line, 5);
    }

    #[test]
    fn test_lsp_index_callers_recursive_depth_2() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 0, 1);
        insert_file(&conn, "src/b.rs", 0, 1);
        insert_file(&conn, "src/c.rs", 0, 1);

        // C calls B, B calls A. We ask for callers of A.
        insert_lsp_symbol(&conn, "sym:a", "func_a", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:b", "func_b", 12, "src/b.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:c", "func_c", 12, "src/c.rs", 1, 0, 10, 1);

        insert_call_edge(&conn, "sym:b", "sym:a", "src/b.rs", "src/a.rs", "lsp", "[]");
        insert_call_edge(&conn, "sym:c", "sym:b", "src/c.rs", "src/b.rs", "lsp", "[]");

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/a.rs".to_string(),
            line: 5,
            character: 0,
            depth: 2,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.callers.len(), 1); // func_b
        assert_eq!(result.callers[0].symbol_name, "func_b");
        assert_eq!(result.callers[0].depth, 1);

        // func_b should have func_c as its caller at depth 2
        assert_eq!(result.callers[0].callers.len(), 1);
        assert_eq!(result.callers[0].callers[0].symbol_name, "func_c");
        assert_eq!(result.callers[0].callers[0].depth, 2);
    }

    #[test]
    fn test_depth_clamping_max_5() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 0, 1);
        insert_file(&conn, "src/b.rs", 0, 1);

        insert_lsp_symbol(&conn, "sym:a", "func_a", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:b", "func_b", 12, "src/b.rs", 1, 0, 10, 1);

        insert_call_edge(&conn, "sym:b", "sym:a", "src/b.rs", "src/a.rs", "lsp", "[]");

        let ctx = LayeredContext::new(&conn, None);

        // depth=100 should be clamped to 5
        let opts = GetInboundCallsOptions {
            file_path: "src/a.rs".to_string(),
            line: 5,
            character: 0,
            depth: 100,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        // Should still work (clamped to 5, but only 1 level of callers exists)
        assert_eq!(result.callers.len(), 1);
    }

    #[test]
    fn test_depth_clamping_min_1() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 0, 1);
        insert_file(&conn, "src/b.rs", 0, 1);

        insert_lsp_symbol(&conn, "sym:a", "func_a", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:b", "func_b", 12, "src/b.rs", 1, 0, 10, 1);

        insert_call_edge(&conn, "sym:b", "sym:a", "src/b.rs", "src/a.rs", "lsp", "[]");

        let ctx = LayeredContext::new(&conn, None);

        // depth=0 should be clamped to 1
        let opts = GetInboundCallsOptions {
            file_path: "src/a.rs".to_string(),
            line: 5,
            character: 0,
            depth: 0,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.callers.len(), 1);
        // At depth=1 (clamped from 0), no recursion into sub-callers
        assert!(result.callers[0].callers.is_empty());
    }

    // --- Tree-sitter fallback ---

    #[test]
    fn test_treesitter_callers() {
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/caller.rs", 1, 0);

        // Tree-sitter chunk for the target so find_symbol can locate it
        insert_ts_chunk(
            &conn,
            "src/target.rs",
            5,
            15,
            "fn process() {}",
            Some("target::process"),
        );

        // Both symbols must exist for the foreign key constraint.
        // Place the target at the cursor position (line 10) -- the LSP index
        // layer will find it and return callers (treesitter-sourced edges are
        // stored in the same table). The important thing is callers are found.
        insert_lsp_symbol(
            &conn,
            "ts:target",
            "process",
            12,
            "src/target.rs",
            5,
            0,
            15,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "ts:caller",
            "handle",
            12,
            "src/caller.rs",
            1,
            0,
            10,
            1,
        );
        insert_call_edge(
            &conn,
            "ts:caller",
            "ts:target",
            "src/caller.rs",
            "src/target.rs",
            "treesitter",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        // Treesitter edges are in the same lsp_call_edges table, so the LSP
        // index layer picks them up when it can find the symbol.
        assert!(
            result.source_layer == SourceLayer::LspIndex
                || result.source_layer == SourceLayer::TreeSitter
        );
        assert_eq!(result.target, "process");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "handle");
    }

    // --- No data ---

    #[test]
    fn test_no_data_returns_empty() {
        let conn = test_db();
        insert_file(&conn, "src/empty.rs", 0, 0);

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/empty.rs".to_string(),
            line: 5,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.callers.is_empty());
    }

    // --- Serialization ---

    #[test]
    fn test_result_serializable() {
        let result = InboundCallsResult {
            target: "process".to_string(),
            callers: vec![InboundCallEntry {
                symbol_name: "main".to_string(),
                file_path: "src/main.rs".to_string(),
                range: LspRange {
                    start_line: 1,
                    start_character: 0,
                    end_line: 10,
                    end_character: 1,
                },
                call_sites: vec![LspRange {
                    start_line: 5,
                    start_character: 4,
                    end_line: 5,
                    end_character: 20,
                }],
                depth: 1,
                callers: Vec::new(),
            }],
            source_layer: SourceLayer::LiveLsp,
        };

        let json = serde_json::to_string(&result).unwrap();
        let roundtrip: InboundCallsResult = serde_json::from_str(&json).unwrap();
        assert_eq!(roundtrip.target, "process");
        assert_eq!(roundtrip.callers.len(), 1);
        assert_eq!(roundtrip.source_layer, SourceLayer::LiveLsp);
    }

    // --- Tree-sitter fallback (try_treesitter) ---

    #[test]
    fn test_try_treesitter_callers_found() {
        // Set up a scenario where try_lsp_index returns None (no LSP symbol at
        // cursor) but try_treesitter finds callers via ts_chunks + call edges.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/caller.rs", 1, 0);

        // ts_chunk for the target with symbol_path so find_ts_symbol_at_cursor works
        insert_ts_chunk(
            &conn,
            "src/target.rs",
            5,
            15,
            "fn do_work() { /* body */ }",
            Some("target::do_work"),
        );

        // Both caller and callee need LSP symbols for the FK constraint.
        // Place the target LSP symbol at lines 50-60 (away from cursor at 10)
        // so lsp_symbol_at won't find it and try_lsp_index returns None.
        insert_lsp_symbol(
            &conn,
            "target::do_work",
            "do_work",
            12,
            "src/target.rs",
            50,
            0,
            60,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "ts:caller_fn",
            "invoke",
            12,
            "src/caller.rs",
            1,
            0,
            10,
            1,
        );

        // Call edge: caller_fn calls target::do_work
        insert_call_edge(
            &conn,
            "ts:caller_fn",
            "target::do_work",
            "src/caller.rs",
            "src/target.rs",
            "treesitter",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.target, "do_work");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "invoke");
        assert_eq!(result.callers[0].depth, 1);
        assert!(result.callers[0].callers.is_empty());
    }

    #[test]
    fn test_try_treesitter_no_callers() {
        // Tree-sitter finds the symbol but there are no call edges pointing to it.
        let conn = test_db();
        insert_file(&conn, "src/lonely.rs", 1, 0);

        insert_ts_chunk(
            &conn,
            "src/lonely.rs",
            1,
            10,
            "fn orphan() {}",
            Some("lonely::orphan"),
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/lonely.rs".to_string(),
            line: 5,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        // No callers found -- falls through to SourceLayer::None
        assert_eq!(result.source_layer, SourceLayer::None);
        assert!(result.callers.is_empty());
    }

    #[test]
    fn test_try_treesitter_depth_greater_than_1() {
        // Verify recursion: A <- B <- C with depth=2 via tree-sitter path.
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 1, 0);
        insert_file(&conn, "src/b.rs", 1, 0);
        insert_file(&conn, "src/c.rs", 1, 0);

        // ts_chunk for target at cursor (lines 1-10, cursor at 5)
        insert_ts_chunk(&conn, "src/a.rs", 1, 10, "fn alpha() {}", Some("a::alpha"));

        // Target LSP symbol at lines 50-60 (away from cursor at 5) so
        // lsp_symbol_at misses and try_lsp_index returns None.
        insert_lsp_symbol(&conn, "a::alpha", "alpha", 12, "src/a.rs", 50, 0, 60, 1);
        // Callers need LSP symbols for the JOIN in lsp_callers_of
        insert_lsp_symbol(&conn, "ts:beta", "beta", 12, "src/b.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "ts:gamma", "gamma", 12, "src/c.rs", 1, 0, 10, 1);

        // B calls A, C calls B
        insert_call_edge(
            &conn,
            "ts:beta",
            "a::alpha",
            "src/b.rs",
            "src/a.rs",
            "treesitter",
            "[]",
        );
        insert_call_edge(
            &conn,
            "ts:gamma",
            "ts:beta",
            "src/c.rs",
            "src/b.rs",
            "treesitter",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/a.rs".to_string(),
            line: 5,
            character: 0,
            depth: 2,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.target, "alpha");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "beta");
        assert_eq!(result.callers[0].depth, 1);

        // depth=2: beta's callers should include gamma
        assert_eq!(result.callers[0].callers.len(), 1);
        assert_eq!(result.callers[0].callers[0].symbol_name, "gamma");
        assert_eq!(result.callers[0].callers[0].depth, 2);
    }

    // --- Recursive tree construction ---

    #[test]
    fn test_recursive_result_tree_construction() {
        let conn = test_db();
        insert_file(&conn, "src/a.rs", 0, 1);
        insert_file(&conn, "src/b.rs", 0, 1);
        insert_file(&conn, "src/c.rs", 0, 1);
        insert_file(&conn, "src/d.rs", 0, 1);

        // Chain: D -> C -> B -> A
        insert_lsp_symbol(&conn, "sym:a", "func_a", 12, "src/a.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:b", "func_b", 12, "src/b.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:c", "func_c", 12, "src/c.rs", 1, 0, 10, 1);
        insert_lsp_symbol(&conn, "sym:d", "func_d", 12, "src/d.rs", 1, 0, 10, 1);

        insert_call_edge(&conn, "sym:b", "sym:a", "src/b.rs", "src/a.rs", "lsp", "[]");
        insert_call_edge(&conn, "sym:c", "sym:b", "src/c.rs", "src/b.rs", "lsp", "[]");
        insert_call_edge(&conn, "sym:d", "sym:c", "src/d.rs", "src/c.rs", "lsp", "[]");

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/a.rs".to_string(),
            line: 5,
            character: 0,
            depth: 3,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(result.target, "func_a");

        // Level 1: func_b calls func_a
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "func_b");
        assert_eq!(result.callers[0].depth, 1);

        // Level 2: func_c calls func_b
        assert_eq!(result.callers[0].callers.len(), 1);
        assert_eq!(result.callers[0].callers[0].symbol_name, "func_c");
        assert_eq!(result.callers[0].callers[0].depth, 2);

        // Level 3: func_d calls func_c
        assert_eq!(result.callers[0].callers[0].callers.len(), 1);
        assert_eq!(
            result.callers[0].callers[0].callers[0].symbol_name,
            "func_d"
        );
        assert_eq!(result.callers[0].callers[0].callers[0].depth, 3);
    }

    // --- cross_reference_with_index ---

    #[test]
    fn test_cross_reference_with_index_appends_indexed_callers() {
        // When live LSP returns zero callers, cross_reference_with_index should
        // pick up callers from the persisted call edge index and append them.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 0, 1);
        insert_file(&conn, "src/caller.rs", 0, 1);

        // Target symbol at cursor -- lsp_symbols_by_name will find it.
        insert_lsp_symbol(
            &conn,
            "sym:target",
            "process",
            12,
            "src/target.rs",
            10,
            0,
            25,
            1,
        );
        // Caller symbol needed for the JOIN in lsp_callers_of.
        insert_lsp_symbol(
            &conn,
            "sym:indexed_caller",
            "bootstrap",
            12,
            "src/caller.rs",
            1,
            0,
            20,
            1,
        );

        let from_ranges = r#"[{"start":{"line":8,"character":4},"end":{"line":8,"character":20}}]"#;
        insert_call_edge(
            &conn,
            "sym:indexed_caller",
            "sym:target",
            "src/caller.rs",
            "src/target.rs",
            "lsp",
            from_ranges,
        );

        let ctx = LayeredContext::new(&conn, None);

        // Pass an empty callers vec — the function should append the indexed caller.
        let callers = cross_reference_with_index(&ctx, "process", "src/target.rs", Vec::new());

        assert_eq!(
            callers.len(),
            1,
            "expected one indexed caller to be appended"
        );
        assert_eq!(callers[0].symbol_name, "bootstrap");
        assert_eq!(callers[0].file_path, "src/caller.rs");
        assert_eq!(callers[0].depth, 1);
        assert_eq!(callers[0].call_sites.len(), 1);
        assert_eq!(callers[0].call_sites[0].start_line, 8);
    }

    #[test]
    fn test_cross_reference_with_index_skips_already_present() {
        // When a caller is already in the live LSP results, cross_reference
        // should not duplicate it.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 0, 1);
        insert_file(&conn, "src/caller.rs", 0, 1);

        insert_lsp_symbol(
            &conn,
            "sym:target",
            "process",
            12,
            "src/target.rs",
            10,
            0,
            25,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "bootstrap",
            12,
            "src/caller.rs",
            1,
            0,
            20,
            1,
        );

        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:target",
            "src/caller.rs",
            "src/target.rs",
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);

        // Pre-populate callers with a caller named "bootstrap" (same as index).
        let existing = vec![InboundCallEntry {
            symbol_name: "bootstrap".to_string(),
            file_path: "src/caller.rs".to_string(),
            range: LspRange {
                start_line: 1,
                start_character: 0,
                end_line: 20,
                end_character: 1,
            },
            call_sites: Vec::new(),
            depth: 1,
            callers: Vec::new(),
        }];

        let callers = cross_reference_with_index(&ctx, "process", "src/target.rs", existing);

        assert_eq!(
            callers.len(),
            1,
            "should not duplicate caller already present"
        );
        assert_eq!(callers[0].symbol_name, "bootstrap");
    }

    #[test]
    fn test_cross_reference_with_index_no_matching_symbol() {
        // When lsp_symbols_by_name finds no symbol for the target, the
        // function should return the callers list unchanged.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 0, 1);

        let ctx = LayeredContext::new(&conn, None);

        let callers = cross_reference_with_index(&ctx, "nonexistent", "src/target.rs", Vec::new());

        assert!(
            callers.is_empty(),
            "should return empty when no symbol in index"
        );
    }

    // --- try_lsp_index direct tests ---

    #[test]
    fn test_try_lsp_index_returns_some_when_callers_exist() {
        // Insert symbol at cursor + call edge → try_lsp_index returns
        // Some(InboundCallsResult) with SourceLayer::LspIndex.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 0, 1);
        insert_file(&conn, "src/caller.rs", 0, 1);

        // Place the symbol at cursor position (line 10, covering lines 5-15)
        insert_lsp_symbol(
            &conn,
            "sym:target",
            "serve",
            12,
            "src/target.rs",
            5,
            0,
            15,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "main",
            12,
            "src/caller.rs",
            1,
            0,
            20,
            1,
        );

        let from_ranges =
            r#"[{"start":{"line":12,"character":4},"end":{"line":12,"character":15}}]"#;
        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:target",
            "src/caller.rs",
            "src/target.rs",
            "lsp",
            from_ranges,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = try_lsp_index(&ctx, &opts, 1);
        assert!(result.is_some(), "expected Some when callers exist");
        let result = result.unwrap();
        assert_eq!(result.source_layer, SourceLayer::LspIndex);
        assert_eq!(result.target, "serve");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "main");
        assert_eq!(result.callers[0].call_sites.len(), 1);
        assert_eq!(result.callers[0].call_sites[0].start_line, 12);
    }

    #[test]
    fn test_try_lsp_index_returns_none_when_no_symbol() {
        // No LSP symbol at cursor → try_lsp_index returns None.
        let conn = test_db();
        insert_file(&conn, "src/empty.rs", 0, 1);

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/empty.rs".to_string(),
            line: 5,
            character: 0,
            depth: 1,
        };

        assert!(
            try_lsp_index(&ctx, &opts, 1).is_none(),
            "expected None when no symbol at cursor"
        );
    }

    #[test]
    fn test_try_lsp_index_returns_none_when_no_callers() {
        // Symbol exists at cursor but no call edges point to it.
        let conn = test_db();
        insert_file(&conn, "src/lonely.rs", 0, 1);

        insert_lsp_symbol(
            &conn,
            "sym:lonely",
            "lonely_fn",
            12,
            "src/lonely.rs",
            1,
            0,
            10,
            1,
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/lonely.rs".to_string(),
            line: 5,
            character: 0,
            depth: 1,
        };

        assert!(
            try_lsp_index(&ctx, &opts, 1).is_none(),
            "expected None when symbol exists but has no callers"
        );
    }

    // --- try_treesitter direct tests ---

    #[test]
    fn test_try_treesitter_returns_some_with_treesitter_layer() {
        // Insert ts_chunk + treesitter-sourced call edge. Place the LSP symbol
        // far from cursor so try_lsp_index won't find it, but the ts_chunk
        // covers the cursor so find_ts_symbol_at_cursor succeeds.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/caller.rs", 1, 0);

        // ts_chunk covering cursor at line 10 (lines 5-15)
        insert_ts_chunk(
            &conn,
            "src/target.rs",
            5,
            15,
            "fn handle() { /* body */ }",
            Some("target::handle"),
        );

        // LSP symbols needed for the call edge JOIN. Place target at lines
        // 50-60 so lsp_symbol_at at line 10 misses it.
        insert_lsp_symbol(
            &conn,
            "target::handle",
            "handle",
            12,
            "src/target.rs",
            50,
            0,
            60,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "ts:dispatcher",
            "dispatch",
            12,
            "src/caller.rs",
            1,
            0,
            10,
            1,
        );

        insert_call_edge(
            &conn,
            "ts:dispatcher",
            "target::handle",
            "src/caller.rs",
            "src/target.rs",
            "treesitter",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = try_treesitter(&ctx, &opts, 1);
        assert!(
            result.is_some(),
            "expected Some when treesitter callers exist"
        );
        let result = result.unwrap();
        assert_eq!(result.source_layer, SourceLayer::TreeSitter);
        assert_eq!(result.target, "handle");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "dispatch");
        assert_eq!(result.callers[0].depth, 1);
        assert!(result.callers[0].callers.is_empty());
    }

    #[test]
    fn test_try_treesitter_returns_none_when_no_chunk() {
        // No ts_chunk at cursor → find_ts_symbol_at_cursor returns None.
        let conn = test_db();
        insert_file(&conn, "src/empty.rs", 1, 0);

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/empty.rs".to_string(),
            line: 5,
            character: 0,
            depth: 1,
        };

        assert!(
            try_treesitter(&ctx, &opts, 1).is_none(),
            "expected None when no ts_chunk at cursor"
        );
    }

    // --- No-live-LSP fallback path (integration) ---

    #[test]
    fn test_no_live_lsp_falls_through_to_lsp_index() {
        // When no live LSP client is configured, the entry point should skip
        // the live LSP layer and fall through to the LSP index layer.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 0, 1);
        insert_file(&conn, "src/caller.rs", 0, 1);

        insert_lsp_symbol(
            &conn,
            "sym:target",
            "compute",
            12,
            "src/target.rs",
            5,
            0,
            15,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:caller",
            "orchestrate",
            12,
            "src/caller.rs",
            1,
            0,
            20,
            1,
        );

        insert_call_edge(
            &conn,
            "sym:caller",
            "sym:target",
            "src/caller.rs",
            "src/target.rs",
            "lsp",
            "[]",
        );

        // No LSP client → ctx.has_live_lsp() is false
        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(
            result.source_layer,
            SourceLayer::LspIndex,
            "should fall through to LspIndex when no live LSP"
        );
        assert_eq!(result.target, "compute");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "orchestrate");
    }

    #[test]
    fn test_no_live_lsp_falls_through_to_treesitter() {
        // When no live LSP client is configured and no LSP symbol at cursor,
        // should fall through to the tree-sitter layer.
        let conn = test_db();
        insert_file(&conn, "src/target.rs", 1, 0);
        insert_file(&conn, "src/caller.rs", 1, 0);

        // ts_chunk at cursor so find_ts_symbol_at_cursor succeeds
        insert_ts_chunk(
            &conn,
            "src/target.rs",
            5,
            15,
            "fn render() {}",
            Some("target::render"),
        );

        // LSP symbol far from cursor (lines 50-60) so lsp_symbol_at misses
        insert_lsp_symbol(
            &conn,
            "target::render",
            "render",
            12,
            "src/target.rs",
            50,
            0,
            60,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "ts:view",
            "update_view",
            12,
            "src/caller.rs",
            1,
            0,
            10,
            1,
        );

        insert_call_edge(
            &conn,
            "ts:view",
            "target::render",
            "src/caller.rs",
            "src/target.rs",
            "treesitter",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, None);
        let opts = GetInboundCallsOptions {
            file_path: "src/target.rs".to_string(),
            line: 10,
            character: 0,
            depth: 1,
        };

        let result = get_inbound_calls(&ctx, &opts).unwrap();
        assert_eq!(
            result.source_layer,
            SourceLayer::TreeSitter,
            "should fall through to TreeSitter when no LSP symbol at cursor"
        );
        assert_eq!(result.target, "render");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "update_view");
    }

    // --- Live LSP with mock server ---

    /// Spawn a Python process that acts as a mock LSP server.
    ///
    /// The script reads JSON-RPC messages from stdin and sends back canned
    /// responses loaded from a JSON file. `null` entries consume a
    /// notification without replying; non-null entries reply to a request.
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

    /// Create a temp file so `lsp_request_with_document` can read it.
    fn create_temp_source_file() -> tempfile::TempDir {
        use std::io::Write;
        let dir = tempfile::tempdir().expect("failed to create temp dir");
        let file = dir.path().join("test.rs");
        let mut f = std::fs::File::create(&file).unwrap();
        writeln!(f, "fn main() {{}}").unwrap();
        dir
    }

    #[test]
    fn test_try_live_lsp_returns_callers() {
        // Mock LSP returns a prepareCallHierarchy item, then an incomingCalls
        // response with one caller.
        //
        // Protocol sequence:
        // 1. didOpen notification (null — consume, don't reply)
        // 2. prepareCallHierarchy request → response with one item
        // 3. didClose notification (null — consume, don't reply)
        // 4. incomingCalls request → response with one caller
        let prepare_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [{
                "name": "process",
                "kind": 12,
                "uri": "file:///test.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 14 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 3 },
                    "end": { "line": 0, "character": 10 }
                }
            }]
        });

        let incoming_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": [{
                "from": {
                    "name": "caller_fn",
                    "kind": 12,
                    "uri": "file:///caller.rs",
                    "range": {
                        "start": { "line": 5, "character": 0 },
                        "end": { "line": 10, "character": 1 }
                    }
                },
                "fromRanges": [{
                    "start": { "line": 7, "character": 4 },
                    "end": { "line": 7, "character": 11 }
                }]
            }]
        });

        let responses = vec![
            serde_json::Value::Null, // didOpen notification
            prepare_response,        // prepareCallHierarchy request
            serde_json::Value::Null, // didClose notification
            incoming_response,       // incomingCalls request
        ];

        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = GetInboundCallsOptions {
            file_path: file_path.to_str().unwrap().to_string(),
            line: 0,
            character: 5,
            depth: 1,
        };

        let result = try_live_lsp(&ctx, &opts, 1).unwrap();
        assert!(result.is_some(), "expected Some from live LSP");
        let result = result.unwrap();
        assert_eq!(result.source_layer, SourceLayer::LiveLsp);
        assert_eq!(result.target, "process");
        assert_eq!(result.callers.len(), 1);
        assert_eq!(result.callers[0].symbol_name, "caller_fn");
        assert_eq!(result.callers[0].call_sites.len(), 1);
        assert_eq!(result.callers[0].call_sites[0].start_line, 7);

        let _ = child.wait();
    }

    #[test]
    fn test_try_live_lsp_null_prepare_returns_none() {
        // Mock LSP returns null for prepareCallHierarchy → try_live_lsp
        // returns Ok(None).
        let null_prepare = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": null
        });

        let responses = vec![
            serde_json::Value::Null, // didOpen notification
            null_prepare,            // prepareCallHierarchy returns null
            serde_json::Value::Null, // didClose notification
        ];

        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = GetInboundCallsOptions {
            file_path: file_path.to_str().unwrap().to_string(),
            line: 0,
            character: 5,
            depth: 1,
        };

        let result = try_live_lsp(&ctx, &opts, 1).unwrap();
        assert!(
            result.is_none(),
            "expected None when prepareCallHierarchy returns null"
        );

        let _ = child.wait();
    }

    #[test]
    fn test_try_live_lsp_empty_prepare_returns_none() {
        // Mock LSP returns an empty array for prepareCallHierarchy → None.
        let empty_prepare = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let responses = vec![
            serde_json::Value::Null, // didOpen notification
            empty_prepare,           // prepareCallHierarchy returns []
            serde_json::Value::Null, // didClose notification
        ];

        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");

        let conn = test_db();
        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = GetInboundCallsOptions {
            file_path: file_path.to_str().unwrap().to_string(),
            line: 0,
            character: 5,
            depth: 1,
        };

        let result = try_live_lsp(&ctx, &opts, 1).unwrap();
        assert!(
            result.is_none(),
            "expected None when prepareCallHierarchy returns empty array"
        );

        let _ = child.wait();
    }

    #[test]
    fn test_try_live_lsp_with_cross_reference() {
        // Mock LSP returns a caller via incomingCalls, and the index has an
        // additional caller that was not returned by live LSP. The cross-
        // reference should merge both.
        let prepare_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [{
                "name": "target_fn",
                "kind": 12,
                "uri": "file:///test.rs",
                "range": {
                    "start": { "line": 0, "character": 0 },
                    "end": { "line": 0, "character": 14 }
                },
                "selectionRange": {
                    "start": { "line": 0, "character": 3 },
                    "end": { "line": 0, "character": 10 }
                }
            }]
        });

        let incoming_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "result": [{
                "from": {
                    "name": "live_caller",
                    "kind": 12,
                    "uri": "file:///live.rs",
                    "range": {
                        "start": { "line": 1, "character": 0 },
                        "end": { "line": 5, "character": 1 }
                    }
                },
                "fromRanges": [{
                    "start": { "line": 3, "character": 4 },
                    "end": { "line": 3, "character": 15 }
                }]
            }]
        });

        let responses = vec![
            serde_json::Value::Null, // didOpen
            prepare_response,        // prepareCallHierarchy
            serde_json::Value::Null, // didClose
            incoming_response,       // incomingCalls
        ];

        let mut child = spawn_mock_lsp(&responses);
        let shared = mock_lsp_client(&mut child);

        let temp_dir = create_temp_source_file();
        let file_path = temp_dir.path().join("test.rs");
        let file_path_str = file_path.to_str().unwrap().to_string();

        let conn = test_db();
        insert_file(&conn, &file_path_str, 0, 1);
        insert_file(&conn, "src/indexed_caller.rs", 0, 1);

        // Index has a symbol matching "target_fn" in the same file
        insert_lsp_symbol(
            &conn,
            "sym:target_fn",
            "target_fn",
            12,
            &file_path_str,
            0,
            0,
            14,
            1,
        );
        insert_lsp_symbol(
            &conn,
            "sym:indexed_caller",
            "indexed_caller",
            12,
            "src/indexed_caller.rs",
            1,
            0,
            10,
            1,
        );

        insert_call_edge(
            &conn,
            "sym:indexed_caller",
            "sym:target_fn",
            "src/indexed_caller.rs",
            &file_path_str,
            "lsp",
            "[]",
        );

        let ctx = LayeredContext::new(&conn, Some(&shared));
        let opts = GetInboundCallsOptions {
            file_path: file_path_str,
            line: 0,
            character: 5,
            depth: 1,
        };

        let result = try_live_lsp(&ctx, &opts, 1).unwrap();
        assert!(result.is_some(), "expected Some from live LSP");
        let result = result.unwrap();
        assert_eq!(result.source_layer, SourceLayer::LiveLsp);
        assert_eq!(result.target, "target_fn");

        // Should have both the live caller and the indexed caller
        assert_eq!(
            result.callers.len(),
            2,
            "expected 2 callers: 1 live + 1 indexed"
        );

        let names: Vec<&str> = result
            .callers
            .iter()
            .map(|c| c.symbol_name.as_str())
            .collect();
        assert!(names.contains(&"live_caller"), "should contain live_caller");
        assert!(
            names.contains(&"indexed_caller"),
            "should contain indexed_caller from cross-reference"
        );

        let _ = child.wait();
    }
}
