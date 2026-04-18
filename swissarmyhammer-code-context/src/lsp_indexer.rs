//! LSP symbol extraction and call edge writing.
//!
//! Provides pure functions for flattening nested `DocumentSymbol` trees into
//! flat records and writing them (along with call edges) to the unified DB.
//! This module does **not** own an LSP process -- it only transforms data.

use lsp_types::{DocumentSymbol, SymbolKind};
use rusqlite::Connection;
use serde_json;

use crate::error::CodeContextError;

/// Extract the underlying `i32` from a `SymbolKind` newtype via serde.
///
/// `SymbolKind` is `#[serde(transparent)]` over `i32` but the field is private,
/// so we roundtrip through JSON to get the numeric value.
pub(crate) fn symbol_kind_to_i32(kind: SymbolKind) -> i32 {
    serde_json::to_value(kind)
        .ok()
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32
}

/// A flattened symbol ready for DB insertion.
#[derive(Debug, Clone)]
pub struct FlatSymbol {
    /// Unique ID: `"lsp:{file_path}:{qualified_path}"`.
    pub id: String,
    /// The symbol's short name (e.g. `"new"`).
    pub name: String,
    /// LSP symbol kind (function, struct, etc.).
    pub kind: SymbolKind,
    /// File the symbol belongs to.
    pub file_path: String,
    /// Fully qualified path (e.g. `"MyStruct::new"`).
    pub qualified_path: String,
    /// Start line (0-based).
    pub start_line: u32,
    /// Start character (0-based).
    pub start_char: u32,
    /// End line (0-based).
    pub end_line: u32,
    /// End character (0-based).
    pub end_char: u32,
    /// Optional detail string from the LSP server.
    pub detail: Option<String>,
}

/// A call edge ready for DB insertion.
#[derive(Debug, Clone)]
pub struct CallEdge {
    /// ID of the calling symbol.
    pub caller_id: String,
    /// ID of the called symbol.
    pub callee_id: String,
    /// File containing the caller.
    pub caller_file: String,
    /// File containing the callee.
    pub callee_file: String,
    /// JSON array of source ranges where the call occurs.
    pub from_ranges: String,
    /// Origin of this edge (`"lsp"` or `"treesitter"`).
    pub source: String,
}

/// Build the qualified path for a symbol by joining parent names with `"::"`.
///
/// # Examples
///
/// ```
/// use swissarmyhammer_code_context::lsp_indexer::build_qualified_path;
///
/// assert_eq!(build_qualified_path(&[], "main"), "main");
/// assert_eq!(build_qualified_path(&["MyStruct"], "new"), "MyStruct::new");
/// ```
pub fn build_qualified_path(parents: &[&str], name: &str) -> String {
    if parents.is_empty() {
        name.to_string()
    } else {
        let mut path = parents.join("::");
        path.push_str("::");
        path.push_str(name);
        path
    }
}

/// Build the canonical symbol ID: `"lsp:{file_path}:{qualified_path}"`.
pub fn build_symbol_id(file_path: &str, qualified_path: &str) -> String {
    format!("lsp:{file_path}:{qualified_path}")
}

/// Flatten nested `DocumentSymbol` trees into a flat list with qualified paths.
///
/// Walks the tree recursively, building qualified paths from parent names
/// joined with `"::"`.
pub fn flatten_symbols(file_path: &str, symbols: &[DocumentSymbol]) -> Vec<FlatSymbol> {
    let mut out = Vec::new();
    flatten_recursive(file_path, &[], symbols, &mut out);
    out
}

/// Recursive helper that accumulates parent names while walking children.
fn flatten_recursive(
    file_path: &str,
    parents: &[&str],
    symbols: &[DocumentSymbol],
    out: &mut Vec<FlatSymbol>,
) {
    for sym in symbols {
        let qpath = build_qualified_path(parents, &sym.name);
        let id = build_symbol_id(file_path, &qpath);
        out.push(FlatSymbol {
            id,
            name: sym.name.clone(),
            kind: sym.kind,
            file_path: file_path.to_string(),
            qualified_path: qpath,
            start_line: sym.range.start.line,
            start_char: sym.range.start.character,
            end_line: sym.range.end.line,
            end_char: sym.range.end.character,
            detail: sym.detail.clone(),
        });
        if let Some(children) = &sym.children {
            let mut new_parents: Vec<&str> = parents.to_vec();
            new_parents.push(&sym.name);
            flatten_recursive(file_path, &new_parents, children, out);
        }
    }
}

/// Delete existing symbols for `file_path` and insert `symbols`.
///
/// Returns the number of symbols inserted.
pub fn write_symbols(
    conn: &Connection,
    file_path: &str,
    symbols: &[FlatSymbol],
) -> Result<usize, CodeContextError> {
    conn.execute("DELETE FROM lsp_symbols WHERE file_path = ?1", [file_path])?;

    let mut stmt = conn.prepare_cached(
        "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?;

    for sym in symbols {
        stmt.execute(rusqlite::params![
            sym.id,
            sym.name,
            symbol_kind_to_i32(sym.kind),
            sym.file_path,
            sym.start_line,
            sym.start_char,
            sym.end_line,
            sym.end_char,
            sym.detail,
        ])?;
    }

    Ok(symbols.len())
}

/// Delete existing outgoing call edges for `caller_file` and insert `edges`.
///
/// Returns the number of edges inserted.
pub fn write_edges(
    conn: &Connection,
    caller_file: &str,
    edges: &[CallEdge],
) -> Result<usize, CodeContextError> {
    conn.execute(
        "DELETE FROM lsp_call_edges WHERE caller_file = ?1",
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

/// Mark a file as LSP-indexed by setting `lsp_indexed = 1` in `indexed_files`.
pub fn mark_lsp_indexed(conn: &Connection, file_path: &str) -> Result<(), CodeContextError> {
    conn.execute(
        "UPDATE indexed_files SET lsp_indexed = 1 WHERE file_path = ?1",
        [file_path],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db;
    use lsp_types::{Position, Range};

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
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'00112233', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    // ── pure function tests ──────────────────────────────────────────

    #[test]
    fn test_build_qualified_path() {
        assert_eq!(build_qualified_path(&[], "main"), "main");
        assert_eq!(build_qualified_path(&["MyStruct"], "new"), "MyStruct::new");
        assert_eq!(
            build_qualified_path(&["auth", "AuthService"], "new"),
            "auth::AuthService::new"
        );
    }

    #[test]
    fn test_build_symbol_id() {
        assert_eq!(
            build_symbol_id("src/main.rs", "main"),
            "lsp:src/main.rs:main"
        );
        assert_eq!(
            build_symbol_id("src/auth.rs", "auth::AuthService::new"),
            "lsp:src/auth.rs:auth::AuthService::new"
        );
    }

    #[test]
    #[allow(deprecated)]
    fn test_flatten_symbols() {
        let symbols = vec![DocumentSymbol {
            name: "MyStruct".to_string(),
            detail: Some("struct".to_string()),
            kind: SymbolKind::STRUCT,
            tags: None,
            deprecated: None,
            range: Range::new(Position::new(0, 0), Position::new(10, 0)),
            selection_range: Range::new(Position::new(0, 0), Position::new(0, 8)),
            children: Some(vec![
                DocumentSymbol {
                    name: "new".to_string(),
                    detail: None,
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(Position::new(2, 4), Position::new(5, 5)),
                    selection_range: Range::new(Position::new(2, 4), Position::new(2, 7)),
                    children: None,
                },
                DocumentSymbol {
                    name: "run".to_string(),
                    detail: None,
                    kind: SymbolKind::METHOD,
                    tags: None,
                    deprecated: None,
                    range: Range::new(Position::new(6, 4), Position::new(9, 5)),
                    selection_range: Range::new(Position::new(6, 4), Position::new(6, 7)),
                    children: None,
                },
            ]),
        }];

        let flat = flatten_symbols("src/lib.rs", &symbols);
        assert_eq!(flat.len(), 3);

        assert_eq!(flat[0].name, "MyStruct");
        assert_eq!(flat[0].qualified_path, "MyStruct");
        assert_eq!(flat[0].id, "lsp:src/lib.rs:MyStruct");
        assert_eq!(flat[0].kind, SymbolKind::STRUCT);
        assert_eq!(flat[0].detail, Some("struct".to_string()));

        assert_eq!(flat[1].name, "new");
        assert_eq!(flat[1].qualified_path, "MyStruct::new");
        assert_eq!(flat[1].id, "lsp:src/lib.rs:MyStruct::new");

        assert_eq!(flat[2].name, "run");
        assert_eq!(flat[2].qualified_path, "MyStruct::run");
        assert_eq!(flat[2].id, "lsp:src/lib.rs:MyStruct::run");
    }

    // ── DB tests ─────────────────────────────────────────────────────

    #[test]
    fn test_write_symbols() {
        let conn = open_memory_db();
        seed_file(&conn, "src/main.rs");

        let symbols = vec![
            FlatSymbol {
                id: "lsp:src/main.rs:main".to_string(),
                name: "main".to_string(),
                kind: SymbolKind::FUNCTION,
                file_path: "src/main.rs".to_string(),
                qualified_path: "main".to_string(),
                start_line: 0,
                start_char: 0,
                end_line: 5,
                end_char: 1,
                detail: None,
            },
            FlatSymbol {
                id: "lsp:src/main.rs:helper".to_string(),
                name: "helper".to_string(),
                kind: SymbolKind::FUNCTION,
                file_path: "src/main.rs".to_string(),
                qualified_path: "helper".to_string(),
                start_line: 7,
                start_char: 0,
                end_line: 10,
                end_char: 1,
                detail: Some("fn()".to_string()),
            },
        ];

        let count = write_symbols(&conn, "src/main.rs", &symbols).unwrap();
        assert_eq!(count, 2);

        let row_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(row_count, 2);

        // Verify a specific row
        let (name, kind, detail): (String, i32, Option<String>) = conn
            .query_row(
                "SELECT name, kind, detail FROM lsp_symbols WHERE id = 'lsp:src/main.rs:helper'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(name, "helper");
        assert_eq!(kind, symbol_kind_to_i32(SymbolKind::FUNCTION));
        assert_eq!(detail, Some("fn()".to_string()));
    }

    #[test]
    fn test_write_symbols_replaces_old() {
        let conn = open_memory_db();
        seed_file(&conn, "src/main.rs");

        // First write
        let old = vec![FlatSymbol {
            id: "lsp:src/main.rs:old_fn".to_string(),
            name: "old_fn".to_string(),
            kind: SymbolKind::FUNCTION,
            file_path: "src/main.rs".to_string(),
            qualified_path: "old_fn".to_string(),
            start_line: 0,
            start_char: 0,
            end_line: 3,
            end_char: 1,
            detail: None,
        }];
        write_symbols(&conn, "src/main.rs", &old).unwrap();

        // Second write with different symbols
        let new = vec![FlatSymbol {
            id: "lsp:src/main.rs:new_fn".to_string(),
            name: "new_fn".to_string(),
            kind: SymbolKind::FUNCTION,
            file_path: "src/main.rs".to_string(),
            qualified_path: "new_fn".to_string(),
            start_line: 0,
            start_char: 0,
            end_line: 3,
            end_char: 1,
            detail: None,
        }];
        let count = write_symbols(&conn, "src/main.rs", &new).unwrap();
        assert_eq!(count, 1);

        // Old symbol should be gone
        let old_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE id = 'lsp:src/main.rs:old_fn'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(old_count, 0);

        // New symbol should exist
        let new_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE id = 'lsp:src/main.rs:new_fn'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(new_count, 1);
    }

    #[test]
    fn test_write_edges() {
        let conn = open_memory_db();
        seed_file(&conn, "src/main.rs");
        seed_file(&conn, "src/lib.rs");

        // Insert symbols that the edges reference
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

        let edges = vec![CallEdge {
            caller_id: "lsp:src/main.rs:main".to_string(),
            callee_id: "lsp:src/lib.rs:init".to_string(),
            caller_file: "src/main.rs".to_string(),
            callee_file: "src/lib.rs".to_string(),
            from_ranges: "[[2,4,2,10]]".to_string(),
            source: "lsp".to_string(),
        }];

        let count = write_edges(&conn, "src/main.rs", &edges).unwrap();
        assert_eq!(count, 1);

        let (caller_id, callee_id, source): (String, String, String) = conn
            .query_row(
                "SELECT caller_id, callee_id, source FROM lsp_call_edges WHERE caller_file = 'src/main.rs'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .unwrap();
        assert_eq!(caller_id, "lsp:src/main.rs:main");
        assert_eq!(callee_id, "lsp:src/lib.rs:init");
        assert_eq!(source, "lsp");
    }

    #[test]
    fn test_mark_lsp_indexed() {
        let conn = open_memory_db();

        // Insert a file with lsp_indexed = 0
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, lsp_indexed)
             VALUES ('src/main.rs', X'00112233', 1024, 1000, 0)",
            [],
        )
        .unwrap();

        mark_lsp_indexed(&conn, "src/main.rs").unwrap();

        let lsp: i64 = conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp, 1);
    }
}
