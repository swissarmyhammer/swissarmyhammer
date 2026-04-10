//! Tree-sitter call graph heuristic.
//!
//! When no LSP server is available for a language, this module generates
//! *approximate* call edges by parsing source code with tree-sitter, walking
//! the AST for call expression nodes, and matching callee names against known
//! `symbol_path` values in `ts_chunks`.
//!
//! **Limitations**: This is a heuristic. It will miss dynamic dispatch, get
//! confused by name collisions across modules, and cannot resolve fully
//! qualified paths precisely. But it provides useful signal when LSP is
//! unavailable.

use rusqlite::Connection;
use tree_sitter::{Language, Node, Parser};

use crate::error::CodeContextError;
use crate::lsp_indexer::CallEdge;

/// A call site extracted from source code by tree-sitter.
#[derive(Debug, Clone)]
pub struct CallSite {
    /// The name of the called function or method.
    pub callee_name: String,
    /// Byte offset where the call starts.
    pub start_byte: usize,
    /// Byte offset where the call ends.
    pub end_byte: usize,
    /// Line where the call starts (0-based).
    pub start_line: u32,
    /// Line where the call ends (0-based).
    pub end_line: u32,
}

/// A callee name resolved against `ts_chunks` symbol paths.
#[derive(Debug, Clone)]
pub struct ResolvedCallee {
    /// The callee name that was looked up.
    pub callee_name: String,
    /// File path of the chunk containing the matching symbol.
    pub file_path: String,
    /// Full symbol path from the matching chunk.
    pub symbol_path: String,
}

/// Extract the callee name from a call expression node.
///
/// Handles several patterns:
/// - Simple calls: `foo()` -> `"foo"`
/// - Method calls: `self.bar()` / `obj.method()` -> `"bar"` / `"method"`
/// - Scoped calls: `MyStruct::new()` -> `"MyStruct::new"`
///
/// Returns `None` if no recognisable callee can be extracted.
fn extract_callee_name(node: Node, source: &[u8]) -> Option<String> {
    // Try "function" field first (Rust call_expression, JS/TS call_expression, Go)
    // Then "method" field (Rust method_call_expression)
    // Then "function" for Python call nodes
    let callee = node
        .child_by_field_name("function")
        .or_else(|| node.child_by_field_name("method"));

    let callee = match callee {
        Some(c) => c,
        None => {
            // Python `call` nodes have the callee as the first named child
            if node.kind() == "call" {
                node.named_child(0)?
            } else {
                return None;
            }
        }
    };

    let text = callee.utf8_text(source).ok()?;

    // For field/member expressions (e.g. self.bar, obj.method), take
    // the part after the last dot.
    if text.contains('.') {
        let after_dot = text.rsplit('.').next()?;
        if after_dot.is_empty() {
            return None;
        }
        Some(after_dot.to_string())
    } else {
        Some(text.to_string())
    }
}

/// Extract call site names from source code using tree-sitter.
///
/// Parses the source with the given language, walks the entire AST, and
/// returns a list of [`CallSite`] values for every recognised call expression.
pub fn extract_call_names(source: &str, language: Language) -> Vec<CallSite> {
    let mut parser = Parser::new();
    if parser.set_language(&language).is_err() {
        return Vec::new();
    }

    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => return Vec::new(),
    };

    let source_bytes = source.as_bytes();
    let mut sites = Vec::new();
    let mut cursor = tree.walk();

    walk_tree(&mut cursor, source_bytes, &mut sites);
    sites
}

/// Recursively walk the tree-sitter AST and collect call sites.
fn walk_tree(cursor: &mut tree_sitter::TreeCursor, source: &[u8], sites: &mut Vec<CallSite>) {
    loop {
        let node = cursor.node();
        let kind = node.kind();

        // Check if this node represents a call expression.
        // Different languages use different node types:
        //   Rust: call_expression, method_call_expression
        //   Python: call
        //   JS/TS/Go/Java/C/C++: call_expression
        if kind == "call_expression" || kind == "method_call_expression" || kind == "call" {
            if let Some(name) = extract_callee_name(node, source) {
                sites.push(CallSite {
                    callee_name: name,
                    start_byte: node.start_byte(),
                    end_byte: node.end_byte(),
                    start_line: node.start_position().row as u32,
                    end_line: node.end_position().row as u32,
                });
            }
        }

        // Descend into children first (depth-first).
        if cursor.goto_first_child() {
            continue;
        }

        // Try siblings, then backtrack up the tree.
        loop {
            if cursor.goto_next_sibling() {
                break;
            }
            if !cursor.goto_parent() {
                return;
            }
        }
    }
}

/// Look up `symbol_path` values in `ts_chunks` that match the given callee names.
///
/// A callee name matches if:
/// 1. `symbol_path` equals the callee name exactly, **or**
/// 2. `symbol_path` ends with `::<callee_name>` (suffix match).
///
/// Returns matching [`ResolvedCallee`] triples.
pub fn resolve_callees(
    conn: &Connection,
    callee_names: &[String],
) -> Result<Vec<ResolvedCallee>, CodeContextError> {
    if callee_names.is_empty() {
        return Ok(Vec::new());
    }

    let mut results = Vec::new();

    let mut stmt = conn.prepare_cached(
        "SELECT DISTINCT file_path, symbol_path FROM ts_chunks
         WHERE symbol_path IS NOT NULL
           AND (symbol_path = ?1 OR symbol_path LIKE '%::' || ?1)",
    )?;

    for name in callee_names {
        let rows = stmt.query_map([name], |row| {
            Ok(ResolvedCallee {
                callee_name: name.clone(),
                file_path: row.get(0)?,
                symbol_path: row.get(1)?,
            })
        })?;

        for row in rows {
            results.push(row?);
        }
    }

    Ok(results)
}

/// Ensure synthetic `lsp_symbols` entries exist for `ts_chunks` with symbol paths.
///
/// For each chunk in `ts_chunks` that has a non-null `symbol_path`, inserts a
/// synthetic entry into `lsp_symbols` with id format `"ts:{file_path}:{symbol_path}"`.
/// Uses `INSERT OR IGNORE` so existing entries (from LSP) are not overwritten.
///
/// Returns the number of symbols created.
pub fn ensure_ts_symbols(conn: &Connection, file_path: &str) -> Result<usize, CodeContextError> {
    let count = conn.execute(
        "INSERT OR IGNORE INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
         SELECT
             'ts:' || file_path || ':' || symbol_path,
             CASE
                 WHEN INSTR(symbol_path, '::') > 0
                 THEN SUBSTR(symbol_path, LENGTH(symbol_path) - LENGTH(REPLACE(symbol_path, '::', '')) + 2)
                 ELSE symbol_path
             END,
             12,
             file_path,
             start_line,
             0,
             end_line,
             0
         FROM ts_chunks
         WHERE file_path = ?1 AND symbol_path IS NOT NULL",
        [file_path],
    )?;

    Ok(count)
}

/// Generate call edges for a file using the tree-sitter heuristic.
///
/// Parses the file with tree-sitter, extracts call names, resolves them
/// against `ts_chunks`, and returns [`CallEdge`] structs ready for
/// [`crate::lsp_indexer::write_edges`].
///
/// Before generating edges this function ensures synthetic `lsp_symbols`
/// entries exist for all chunks involved (both caller and callee sides)
/// so that the foreign-key constraints on `lsp_call_edges` are satisfied.
pub fn generate_ts_call_edges(
    conn: &Connection,
    file_path: &str,
    source: &str,
    language: Language,
) -> Result<Vec<CallEdge>, CodeContextError> {
    let call_sites = extract_call_names(source, language);
    if call_sites.is_empty() {
        return Ok(Vec::new());
    }

    // Collect unique callee names.
    let mut unique_names: Vec<String> = call_sites.iter().map(|s| s.callee_name.clone()).collect();
    unique_names.sort();
    unique_names.dedup();

    let resolved = resolve_callees(conn, &unique_names)?;
    if resolved.is_empty() {
        return Ok(Vec::new());
    }

    // Ensure synthetic lsp_symbols exist for the caller file's chunks.
    ensure_ts_symbols(conn, file_path)?;

    // Also ensure symbols for every resolved callee file.
    let mut callee_files: Vec<&str> = resolved.iter().map(|r| r.file_path.as_str()).collect();
    callee_files.sort();
    callee_files.dedup();
    for cf in &callee_files {
        ensure_ts_symbols(conn, cf)?;
    }

    let edges = map_call_sites_to_edges(conn, file_path, &call_sites, &resolved)?;
    Ok(edges)
}

/// Map each call site to at most one [`CallEdge`] by finding the enclosing
/// chunk (caller) and the first matching resolved callee.
fn map_call_sites_to_edges(
    conn: &Connection,
    file_path: &str,
    call_sites: &[CallSite],
    resolved: &[ResolvedCallee],
) -> Result<Vec<CallEdge>, CodeContextError> {
    let mut caller_stmt = conn.prepare_cached(
        "SELECT symbol_path FROM ts_chunks
         WHERE file_path = ?1 AND symbol_path IS NOT NULL
           AND start_byte <= ?2 AND end_byte >= ?3
         ORDER BY (end_byte - start_byte) ASC
         LIMIT 1",
    )?;

    let mut edges = Vec::new();
    for site in call_sites {
        if let Some(edge) = build_edge_for_site(&mut caller_stmt, file_path, site, resolved)? {
            edges.push(edge);
        }
    }
    Ok(edges)
}

/// Build a single [`CallEdge`] for one call site, or `None` if the site has
/// no enclosing chunk or no matching callee.
fn build_edge_for_site(
    caller_stmt: &mut rusqlite::CachedStatement<'_>,
    file_path: &str,
    site: &CallSite,
    resolved: &[ResolvedCallee],
) -> Result<Option<CallEdge>, CodeContextError> {
    let caller_symbol: Option<String> = caller_stmt
        .query_row(
            rusqlite::params![file_path, site.start_byte as i64, site.end_byte as i64],
            |row| row.get(0),
        )
        .ok();

    let caller_symbol = match caller_symbol {
        Some(s) => s,
        None => return Ok(None),
    };

    let caller_id = format!("ts:{file_path}:{caller_symbol}");

    // Take the first matching callee (deterministic by DB ordering).
    for r in resolved
        .iter()
        .filter(|r| r.callee_name == site.callee_name)
    {
        let callee_id = format!("ts:{}:{}", r.file_path, r.symbol_path);
        if caller_id == callee_id {
            continue; // Skip self-edges.
        }
        let from_ranges = format!("[[{},{},{},{}]]", site.start_line, 0, site.end_line, 0);
        return Ok(Some(CallEdge {
            caller_id,
            callee_id,
            caller_file: file_path.to_string(),
            callee_file: r.file_path.clone(),
            from_ranges,
            source: "treesitter".to_string(),
        }));
    }

    Ok(None)
}

/// Write tree-sitter heuristic edges for a file, replacing any previous
/// tree-sitter edges for that file while preserving LSP-sourced edges.
///
/// Returns the number of edges inserted.
pub fn write_ts_edges(
    conn: &Connection,
    caller_file: &str,
    edges: &[CallEdge],
) -> Result<usize, CodeContextError> {
    // Only delete tree-sitter edges, not LSP edges.
    conn.execute(
        "DELETE FROM lsp_call_edges WHERE caller_file = ?1 AND source = 'treesitter'",
        [caller_file],
    )?;

    let mut stmt = conn.prepare_cached(
        "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges, source)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for edge in edges {
        stmt.execute(rusqlite::params![
            edge.caller_id,
            edge.callee_id,
            edge.caller_file,
            edge.callee_file,
            edge.from_ranges,
            edge.source,
        ])?;
    }

    Ok(edges.len())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;

    /// Open an in-memory DB with the full schema.
    fn open_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        db::configure_connection(&conn).unwrap();
        db::create_schema(&conn).unwrap();
        conn
    }

    /// Insert a file row so foreign-key constraints are satisfied.
    fn seed_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'00112233', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Insert a ts_chunk with a symbol path.
    #[allow(clippy::too_many_arguments)]
    fn seed_chunk(
        conn: &Connection,
        file_path: &str,
        start_byte: i64,
        end_byte: i64,
        start_line: i64,
        end_line: i64,
        text: &str,
        symbol_path: &str,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            rusqlite::params![file_path, start_byte, end_byte, start_line, end_line, text, symbol_path],
        )
        .unwrap();
    }

    fn rust_language() -> Language {
        tree_sitter_rust::LANGUAGE.into()
    }

    // ── extract_call_names tests ────────────────────────────────────

    #[test]
    fn test_extract_call_names_simple() {
        let source = "fn main() { foo(); bar(); }";
        let sites = extract_call_names(source, rust_language());

        let names: Vec<&str> = sites.iter().map(|s| s.callee_name.as_str()).collect();
        assert!(names.contains(&"foo"), "expected 'foo' in {names:?}");
        assert!(names.contains(&"bar"), "expected 'bar' in {names:?}");
        assert_eq!(names.len(), 2);
    }

    #[test]
    fn test_extract_call_names_method() {
        let source = r#"
            struct S;
            impl S {
                fn test(&self) {
                    self.method();
                }
            }
        "#;
        let sites = extract_call_names(source, rust_language());

        let names: Vec<&str> = sites.iter().map(|s| s.callee_name.as_str()).collect();
        assert!(names.contains(&"method"), "expected 'method' in {names:?}");
    }

    #[test]
    fn test_extract_call_names_scoped() {
        let source = r#"
            fn test() {
                MyStruct::new();
            }
        "#;
        let sites = extract_call_names(source, rust_language());

        let names: Vec<&str> = sites.iter().map(|s| s.callee_name.as_str()).collect();
        // The scoped call `MyStruct::new()` should extract the full text
        // "MyStruct::new" since it does not contain a dot.
        assert!(
            names.contains(&"MyStruct::new"),
            "expected 'MyStruct::new' in {names:?}"
        );
    }

    // ── resolve_callees tests ───────────────────────────────────────

    #[test]
    fn test_resolve_callees() {
        let conn = open_memory_db();
        seed_file(&conn, "src/lib.rs");
        seed_chunk(&conn, "src/lib.rs", 0, 100, 0, 10, "fn foo() {}", "foo");
        seed_chunk(
            &conn,
            "src/lib.rs",
            110,
            200,
            12,
            20,
            "fn bar() {}",
            "MyStruct::bar",
        );

        let names = vec!["foo".to_string(), "bar".to_string()];
        let resolved = resolve_callees(&conn, &names).unwrap();

        // "foo" should match exactly.
        assert!(
            resolved
                .iter()
                .any(|r| r.callee_name == "foo" && r.symbol_path == "foo"),
            "expected exact match for 'foo'"
        );

        // "bar" should match via suffix "::bar".
        assert!(
            resolved
                .iter()
                .any(|r| r.callee_name == "bar" && r.symbol_path == "MyStruct::bar"),
            "expected suffix match for 'bar'"
        );
    }

    #[test]
    fn test_no_match_no_error() {
        let conn = open_memory_db();
        seed_file(&conn, "src/lib.rs");
        seed_chunk(&conn, "src/lib.rs", 0, 100, 0, 10, "fn foo() {}", "foo");

        let names = vec!["nonexistent_function".to_string()];
        let resolved = resolve_callees(&conn, &names).unwrap();
        assert!(
            resolved.is_empty(),
            "expected empty result for unknown callee"
        );
    }

    // ── generate_ts_call_edges tests ────────────────────────────────

    #[test]
    fn test_generate_ts_call_edges() {
        let conn = open_memory_db();

        // File A has a function `main` that calls `foo`.
        seed_file(&conn, "src/main.rs");
        let caller_source = "fn main() { foo(); }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            caller_source.len() as i64,
            0,
            0,
            caller_source,
            "main",
        );

        // File B defines `foo`.
        seed_file(&conn, "src/lib.rs");
        seed_chunk(
            &conn,
            "src/lib.rs",
            0,
            50,
            0,
            5,
            "fn foo() { println!(\"hello\"); }",
            "foo",
        );

        let edges =
            generate_ts_call_edges(&conn, "src/main.rs", caller_source, rust_language()).unwrap();

        assert_eq!(edges.len(), 1, "expected exactly one edge");
        assert_eq!(edges[0].caller_id, "ts:src/main.rs:main");
        assert_eq!(edges[0].callee_id, "ts:src/lib.rs:foo");
        assert_eq!(edges[0].caller_file, "src/main.rs");
        assert_eq!(edges[0].callee_file, "src/lib.rs");
    }

    #[test]
    fn test_edges_have_treesitter_source() {
        let conn = open_memory_db();

        seed_file(&conn, "src/main.rs");
        let source = "fn main() { helper(); }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            source.len() as i64,
            0,
            0,
            source,
            "main",
        );

        seed_file(&conn, "src/util.rs");
        seed_chunk(
            &conn,
            "src/util.rs",
            0,
            30,
            0,
            3,
            "fn helper() {}",
            "helper",
        );

        let edges = generate_ts_call_edges(&conn, "src/main.rs", source, rust_language()).unwrap();

        assert!(!edges.is_empty(), "expected at least one edge");
        for edge in &edges {
            assert_eq!(
                edge.source, "treesitter",
                "expected source 'treesitter', got '{}'",
                edge.source
            );
        }
    }

    #[test]
    fn test_write_ts_edges_persists() {
        let conn = open_memory_db();

        seed_file(&conn, "src/main.rs");
        let source = "fn main() { do_work(); }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            source.len() as i64,
            0,
            0,
            source,
            "main",
        );

        seed_file(&conn, "src/work.rs");
        seed_chunk(
            &conn,
            "src/work.rs",
            0,
            40,
            0,
            4,
            "fn do_work() {}",
            "do_work",
        );

        let edges = generate_ts_call_edges(&conn, "src/main.rs", source, rust_language()).unwrap();

        let count = write_ts_edges(&conn, "src/main.rs", &edges).unwrap();
        assert_eq!(count, 1);

        // Verify the edge was persisted.
        let (caller_id, callee_id, source_col): (String, String, String) = conn
            .query_row(
                "SELECT caller_id, callee_id, source FROM lsp_call_edges WHERE caller_file = 'src/main.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(caller_id, "ts:src/main.rs:main");
        assert_eq!(callee_id, "ts:src/work.rs:do_work");
        assert_eq!(source_col, "treesitter");
    }

    // ── extract_callee_name edge cases ──────────────────────────────

    #[test]
    fn test_extract_callee_name_non_call_node_returns_none() {
        // When extract_callee_name receives a node that is not a call
        // expression and has no "function"/"method" field, it returns None.
        // This exercises the `else { return None }` branch.
        let source = "fn main() {}";
        let mut parser = Parser::new();
        parser.set_language(&rust_language()).unwrap();
        let tree = parser.parse(source, None).unwrap();

        // Walk to a node that is NOT a call expression (e.g., "fn_item"
        // or "function_item"). Such a node will have neither "function"
        // nor "method" fields and is not "call" kind.
        let root = tree.root_node();
        let fn_node = root.child(0).expect("expected function_item node");
        assert_ne!(fn_node.kind(), "call");
        assert_ne!(fn_node.kind(), "call_expression");
        assert_ne!(fn_node.kind(), "method_call_expression");

        let result = extract_callee_name(fn_node, source.as_bytes());
        assert!(
            result.is_none(),
            "expected None for a non-call node, got {result:?}"
        );
    }

    #[test]
    fn test_extract_callee_name_trailing_dot_returns_none() {
        // When the callee text ends with a dot (e.g., "foo."), the part
        // after the last dot is empty and extract_callee_name returns None.
        // This exercises the after_dot.is_empty() branch.
        //
        // In Rust, "foo.()" is a syntax error, but tree-sitter still
        // produces a partial parse. We verify the behaviour via
        // extract_call_names: tree-sitter may or may not produce a call
        // node with trailing-dot callee text, so we test at the function
        // boundary of extract_call_names and verify no call site is emitted
        // with an empty name.
        let source = "fn main() { foo.(); }";
        let sites = extract_call_names(source, rust_language());
        // Regardless of how tree-sitter handles the malformed source,
        // no call site should have an empty callee_name.
        for site in &sites {
            assert!(
                !site.callee_name.is_empty(),
                "callee_name must not be empty"
            );
        }
    }

    // ── extract_call_names edge cases ────────────────────────────────

    #[test]
    fn test_extract_call_names_empty_source() {
        // Parsing empty source should produce no call sites.
        let sites = extract_call_names("", rust_language());
        assert!(sites.is_empty(), "expected no call sites for empty source");
    }

    #[test]
    fn test_extract_call_names_no_calls() {
        // Source with no call expressions yields an empty list.
        let source = "fn main() { let x = 42; }";
        let sites = extract_call_names(source, rust_language());
        assert!(
            sites.is_empty(),
            "expected no call sites for source without calls"
        );
    }

    // ── resolve_callees edge cases ─────────────────────────────────

    #[test]
    fn test_resolve_callees_empty_names_returns_empty() {
        // Calling resolve_callees with an empty callee list should
        // short-circuit and return Ok(vec![]).
        let conn = open_memory_db();
        let result = resolve_callees(&conn, &[]).unwrap();
        assert!(
            result.is_empty(),
            "expected empty result for empty callee list"
        );
    }

    // ── generate_ts_call_edges edge cases ──────────────────────────

    #[test]
    fn test_generate_ts_call_edges_call_outside_chunk_is_skipped() {
        // When a call site falls outside any known ts_chunk, it should
        // be skipped (no edge emitted, no error).
        let conn = open_memory_db();

        // Source has main() calling helper(), but the ts_chunks DB only
        // has a chunk for helper() in another file — NOT for main().
        // That means the call site has no enclosing caller chunk.
        seed_file(&conn, "src/main.rs");
        seed_file(&conn, "src/util.rs");

        let caller_source = "fn main() { helper(); }";
        // Deliberately do NOT seed a chunk for "src/main.rs" so the call
        // site in main has no enclosing chunk.
        seed_chunk(
            &conn,
            "src/util.rs",
            0,
            30,
            0,
            3,
            "fn helper() {}",
            "helper",
        );

        let edges =
            generate_ts_call_edges(&conn, "src/main.rs", caller_source, rust_language()).unwrap();

        assert!(
            edges.is_empty(),
            "expected no edges when call site is outside any chunk"
        );
    }

    #[test]
    fn test_generate_ts_call_edges_multiple_callees_takes_first_only() {
        // When multiple chunks match the same callee name, only the first
        // match (by DB ordering) should produce an edge per call site.
        let conn = open_memory_db();

        seed_file(&conn, "src/main.rs");
        seed_file(&conn, "src/a.rs");
        seed_file(&conn, "src/b.rs");

        let caller_source = "fn main() { helper(); }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            caller_source.len() as i64,
            0,
            0,
            caller_source,
            "main",
        );

        // Two different files define "helper", so resolve_callees will
        // return two matches. The code should break after the first.
        seed_chunk(&conn, "src/a.rs", 0, 30, 0, 3, "fn helper() {}", "helper");
        seed_chunk(&conn, "src/b.rs", 0, 30, 0, 3, "fn helper() {}", "helper");

        let edges =
            generate_ts_call_edges(&conn, "src/main.rs", caller_source, rust_language()).unwrap();

        assert_eq!(
            edges.len(),
            1,
            "expected exactly one edge despite multiple callee matches, got {}",
            edges.len()
        );
    }

    #[test]
    fn test_generate_ts_call_edges_self_call_skipped() {
        // When the caller and callee are the same symbol, the self-edge
        // should be skipped (e.g., recursive call within the same chunk).
        let conn = open_memory_db();

        seed_file(&conn, "src/main.rs");

        // Source: main() calls main() recursively. Because the call site
        // is inside the "main" chunk and callee resolves to "main" in the
        // same file, caller_id == callee_id → skipped.
        let source = "fn main() { main(); }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            source.len() as i64,
            0,
            0,
            source,
            "main",
        );

        let edges = generate_ts_call_edges(&conn, "src/main.rs", source, rust_language()).unwrap();

        assert!(
            edges.is_empty(),
            "expected no edges for self-recursive call"
        );
    }

    #[test]
    fn test_generate_ts_call_edges_no_calls_in_source() {
        // Source with no call expressions should return an empty edge list
        // immediately without hitting the DB.
        let conn = open_memory_db();
        seed_file(&conn, "src/main.rs");

        let source = "fn main() { let x = 42; }";
        seed_chunk(
            &conn,
            "src/main.rs",
            0,
            source.len() as i64,
            0,
            0,
            source,
            "main",
        );

        let edges = generate_ts_call_edges(&conn, "src/main.rs", source, rust_language()).unwrap();

        assert!(
            edges.is_empty(),
            "expected no edges for source without call expressions"
        );
    }

    #[test]
    fn test_write_ts_edges_preserves_lsp_edges() {
        let conn = open_memory_db();

        seed_file(&conn, "src/main.rs");
        seed_file(&conn, "src/lib.rs");

        // Insert an LSP symbol and an LSP edge.
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/main.rs:main', 'main', 12, 'src/main.rs', 0, 0, 5, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/lib.rs:init', 'init', 12, 'src/lib.rs', 0, 0, 3, 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges, source)
             VALUES ('lsp:src/main.rs:main', 'lsp:src/lib.rs:init', 'src/main.rs', 'src/lib.rs', '[]', 'lsp')",
            [],
        )
        .unwrap();

        // Now write tree-sitter edges for the same caller file.
        let ts_edges: Vec<CallEdge> = Vec::new();
        write_ts_edges(&conn, "src/main.rs", &ts_edges).unwrap();

        // The LSP edge should still be there.
        let lsp_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_call_edges WHERE source = 'lsp' AND caller_file = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp_count, 1, "LSP edge should be preserved");
    }
}
