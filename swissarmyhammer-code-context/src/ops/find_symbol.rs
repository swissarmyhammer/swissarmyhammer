//! Exact symbol location lookup.
//!
//! Returns definition locations (file, line, char) for a symbol by name.
//! Does NOT return source text -- just the location coordinates.
//!
//! Queries both `lsp_symbols` (precise char-level positions) and
//! `ts_chunks` (line-level positions) and deduplicates by
//! `(file_path, start_line)`, preferring the LSP source.

use rusqlite::Connection;
use std::collections::HashMap;

use crate::error::CodeContextError;

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// A symbol's definition location.
#[derive(Debug, Clone, serde::Serialize)]
pub struct SymbolLocation {
    /// The symbol's short name (leaf segment).
    pub name: String,
    /// Fully qualified path (e.g. `MyStruct::new`).
    pub qualified_path: String,
    /// Symbol kind (e.g. "function", "struct"), if known.
    pub kind: Option<String>,
    /// File containing the symbol.
    pub file_path: String,
    /// Start line (0-based from LSP, 1-based from tree-sitter).
    pub start_line: u32,
    /// Start character (0-based from LSP, 0 from tree-sitter).
    pub start_char: u32,
    /// End line.
    pub end_line: u32,
    /// End character.
    pub end_char: u32,
    /// Optional detail string from the LSP server.
    pub detail: Option<String>,
    /// Which index produced this result: `"lsp"` or `"treesitter"`.
    pub source: String,
}

// ---------------------------------------------------------------------------
// Symbol kind mapping
// ---------------------------------------------------------------------------

/// Map an LSP `SymbolKind` integer to a human-readable name.
///
/// Covers the most common kinds; returns `None` for unknown values.
pub fn symbol_kind_name(kind: i32) -> Option<&'static str> {
    match kind {
        1 => Some("file"),
        2 => Some("module"),
        3 => Some("namespace"),
        5 => Some("class"),
        6 => Some("method"),
        8 => Some("field"),
        9 => Some("constructor"),
        10 => Some("enum"),
        11 => Some("interface"),
        12 => Some("function"),
        13 => Some("variable"),
        14 => Some("constant"),
        22 => Some("enum_member"),
        23 => Some("struct"),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// Find the definition location of a symbol by exact or suffix name match.
///
/// Queries both `lsp_symbols` and `ts_chunks` (where `symbol_path IS NOT NULL`).
/// Matches by exact name or suffix (`::name` at end of qualified path).
/// Deduplicates by `(file_path, start_line)`, preferring LSP when both exist.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn find_symbol(
    conn: &Connection,
    name: &str,
) -> Result<Vec<SymbolLocation>, CodeContextError> {
    // Key: (file_path, start_line) -> SymbolLocation
    // We insert LSP results first, then only add tree-sitter results
    // for locations not already covered by LSP.
    let mut seen: HashMap<(String, u32), SymbolLocation> = HashMap::new();

    // --- LSP symbols ---
    load_lsp_matches(conn, name, &mut seen)?;

    // --- Tree-sitter symbols ---
    load_ts_matches(conn, name, &mut seen)?;

    let mut results: Vec<SymbolLocation> = seen.into_values().collect();
    results.sort_by(|a, b| a.file_path.cmp(&b.file_path).then(a.start_line.cmp(&b.start_line)));
    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the leaf name from a qualified path (e.g. `MyStruct::new` -> `new`).
fn leaf_name(path: &str) -> &str {
    path.rsplit("::").next().unwrap_or(path)
}

/// Extract the qualified path from an `lsp_symbols.id` field.
///
/// The ID format is `lsp:{file_path}:{qualified_path}`. We strip the
/// `lsp:` prefix and then the `{file_path}:` prefix to get the qualified path.
fn qualified_path_from_id(id: &str, file_path: &str) -> String {
    // id = "lsp:{file_path}:{qualified_path}"
    let prefix = format!("lsp:{}:", file_path);
    if let Some(qpath) = id.strip_prefix(&prefix) {
        qpath.to_string()
    } else {
        // Fallback: just use the name
        id.to_string()
    }
}

/// Returns true if the symbol matches by exact name or suffix.
fn matches_name(qualified_path: &str, query: &str) -> bool {
    // Exact match on the full qualified path
    if qualified_path == query {
        return true;
    }
    // Leaf name matches exactly
    if leaf_name(qualified_path) == query {
        return true;
    }
    // Suffix match: qualified_path ends with `::<query>`
    let suffix = format!("::{}", query);
    if qualified_path.ends_with(&suffix) {
        return true;
    }
    false
}

/// Load matching LSP symbols into the dedup map.
fn load_lsp_matches(
    conn: &Connection,
    name: &str,
    seen: &mut HashMap<(String, u32), SymbolLocation>,
) -> Result<(), CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT id, name, kind, file_path, start_line, start_char, end_line, end_char, detail \
         FROM lsp_symbols",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,  // id
            row.get::<_, String>(1)?,  // name
            row.get::<_, i32>(2)?,     // kind
            row.get::<_, String>(3)?,  // file_path
            row.get::<_, u32>(4)?,     // start_line
            row.get::<_, u32>(5)?,     // start_char
            row.get::<_, u32>(6)?,     // end_line
            row.get::<_, u32>(7)?,     // end_char
            row.get::<_, Option<String>>(8)?, // detail
        ))
    })?;

    for row in rows {
        let (id, sym_name, kind, file_path, start_line, start_char, end_line, end_char, detail) =
            row?;
        let qpath = qualified_path_from_id(&id, &file_path);

        if !matches_name(&qpath, name) {
            continue;
        }

        let key = (file_path.clone(), start_line);
        // LSP always wins -- overwrite any existing entry
        seen.insert(
            key,
            SymbolLocation {
                name: sym_name,
                qualified_path: qpath,
                kind: symbol_kind_name(kind).map(|s| s.to_string()),
                file_path,
                start_line,
                start_char,
                end_line,
                end_char,
                detail,
                source: "lsp".to_string(),
            },
        );
    }

    Ok(())
}

/// Load matching tree-sitter symbols into the dedup map (only if not already
/// covered by LSP).
fn load_ts_matches(
    conn: &Connection,
    name: &str,
    seen: &mut HashMap<(String, u32), SymbolLocation>,
) -> Result<(), CodeContextError> {
    let mut stmt = conn.prepare(
        "SELECT file_path, start_line, end_line, symbol_path \
         FROM ts_chunks WHERE symbol_path IS NOT NULL",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?, // file_path
            row.get::<_, u32>(1)?,    // start_line
            row.get::<_, u32>(2)?,    // end_line
            row.get::<_, String>(3)?, // symbol_path
        ))
    })?;

    for row in rows {
        let (file_path, start_line, end_line, symbol_path) = row?;

        if !matches_name(&symbol_path, name) {
            continue;
        }

        let key = (file_path.clone(), start_line);
        // Only insert if LSP hasn't already provided this location
        seen.entry(key).or_insert_with(|| SymbolLocation {
            name: leaf_name(&symbol_path).to_string(),
            qualified_path: symbol_path,
            kind: None,
            file_path,
            start_line,
            start_char: 0,
            end_line,
            end_char: 0,
            detail: None,
            source: "treesitter".to_string(),
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{configure_connection, create_schema};

    /// Create an in-memory database with the schema applied.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    /// Insert an `indexed_files` row (required by foreign key constraint).
    fn insert_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'DEADBEEF', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Insert an LSP symbol.
    fn insert_lsp_symbol(
        conn: &Connection,
        id: &str,
        name: &str,
        kind: i32,
        file_path: &str,
        start_line: u32,
        start_char: u32,
        end_line: u32,
        end_char: u32,
    ) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
            rusqlite::params![id, name, kind, file_path, start_line, start_char, end_line, end_char],
        )
        .unwrap();
    }

    /// Insert a ts_chunks row with a symbol_path.
    fn insert_ts_chunk(
        conn: &Connection,
        file_path: &str,
        start_line: u32,
        end_line: u32,
        symbol_path: &str,
    ) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text)
             VALUES (?1, 0, 100, ?2, ?3, ?4, 'source text')",
            rusqlite::params![file_path, start_line, end_line, symbol_path],
        )
        .unwrap();
    }

    /// Seed the database with standard test fixtures.
    fn seed_fixtures(conn: &Connection) {
        insert_file(conn, "src/lib.rs");
        insert_file(conn, "src/auth.rs");

        // LSP symbols
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct",
            "MyStruct",
            23, // struct
            "src/lib.rs",
            0, 0, 20, 1,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::new",
            "new",
            12, // function
            "src/lib.rs",
            5, 4, 8, 5,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::authenticate",
            "authenticate",
            6, // method
            "src/lib.rs",
            10, 4, 15, 5,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService",
            "AuthService",
            5, // class
            "src/auth.rs",
            0, 0, 30, 1,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService::validate",
            "validate",
            6, // method
            "src/auth.rs",
            15, 4, 20, 5,
        );

        // Tree-sitter chunks (some overlap with LSP, some unique)
        insert_ts_chunk(conn, "src/lib.rs", 0, 20, "MyStruct");
        insert_ts_chunk(conn, "src/lib.rs", 5, 8, "MyStruct::new");
        insert_ts_chunk(conn, "src/lib.rs", 10, 15, "MyStruct::authenticate");
        insert_ts_chunk(conn, "src/auth.rs", 0, 30, "AuthService");
        insert_ts_chunk(conn, "src/auth.rs", 15, 20, "AuthService::validate");
    }

    #[test]
    fn test_find_symbol_exact() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = find_symbol(&conn, "MyStruct").unwrap();

        assert!(!results.is_empty(), "expected at least one result");
        let r = results.iter().find(|s| s.qualified_path == "MyStruct").unwrap();
        assert_eq!(r.file_path, "src/lib.rs");
        assert_eq!(r.source, "lsp");
        assert_eq!(r.kind, Some("struct".to_string()));
    }

    #[test]
    fn test_find_symbol_suffix() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = find_symbol(&conn, "new").unwrap();

        assert_eq!(results.len(), 1, "expected exactly MyStruct::new");
        assert_eq!(results[0].qualified_path, "MyStruct::new");
        assert_eq!(results[0].source, "lsp");
    }

    #[test]
    fn test_find_symbol_not_found() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = find_symbol(&conn, "nonexistent").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_find_symbol_dedup_prefers_lsp() {
        let conn = test_db();
        seed_fixtures(&conn);

        // MyStruct exists in both LSP and tree-sitter at the same (file, start_line)
        let results = find_symbol(&conn, "MyStruct").unwrap();

        let r = results.iter().find(|s| s.qualified_path == "MyStruct").unwrap();
        assert_eq!(r.source, "lsp", "LSP should be preferred over treesitter");
        assert_eq!(r.start_char, 0); // LSP has char-level precision
    }

    #[test]
    fn test_find_symbol_treesitter_only() {
        let conn = test_db();
        insert_file(&conn, "src/other.rs");

        // Only tree-sitter data, no LSP
        insert_ts_chunk(&conn, "src/other.rs", 1, 5, "helper_fn");

        let results = find_symbol(&conn, "helper_fn").unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].source, "treesitter");
        assert_eq!(results[0].start_char, 0);
        assert_eq!(results[0].end_char, 0);
    }
}
