//! List all symbols in a specific file.
//!
//! Returns every symbol defined in a given file, sorted by start line.
//! Draws from both `lsp_symbols` (preferred) and `ts_chunks`, deduplicating
//! by `start_line`.

use rusqlite::Connection;
use std::collections::HashMap;

use crate::error::CodeContextError;
use crate::ops::get_symbol::{symbol_kind_name, SymbolLocation};

// ---------------------------------------------------------------------------
// Core function
// ---------------------------------------------------------------------------

/// List all symbols in a specific file, sorted by start line.
///
/// Queries both `lsp_symbols` and `ts_chunks` for the given file path.
/// Deduplicates by `start_line`, preferring LSP when both exist at the
/// same line. Returns an empty vec if the file has no symbols or doesn't
/// exist.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn list_symbols(
    conn: &Connection,
    file_path: &str,
) -> Result<Vec<SymbolLocation>, CodeContextError> {
    // Key: start_line -> SymbolLocation
    let mut seen: HashMap<u32, SymbolLocation> = HashMap::new();

    // --- LSP symbols (preferred) ---
    {
        let mut stmt = conn.prepare(
            "SELECT id, name, kind, start_line, start_char, end_line, end_char, detail \
             FROM lsp_symbols WHERE file_path = ?1",
        )?;

        let rows = stmt.query_map([file_path], |row| {
            Ok((
                row.get::<_, String>(0)?,         // id
                row.get::<_, String>(1)?,         // name
                row.get::<_, i32>(2)?,            // kind
                row.get::<_, u32>(3)?,            // start_line
                row.get::<_, u32>(4)?,            // start_char
                row.get::<_, u32>(5)?,            // end_line
                row.get::<_, u32>(6)?,            // end_char
                row.get::<_, Option<String>>(7)?, // detail
            ))
        })?;

        for row in rows {
            let (id, name, kind, start_line, start_char, end_line, end_char, detail) = row?;
            let qpath = qualified_path_from_id(&id, file_path);

            // LSP always wins
            seen.insert(
                start_line,
                SymbolLocation {
                    name,
                    qualified_path: qpath,
                    kind: symbol_kind_name(kind).map(|s| s.to_string()),
                    file_path: file_path.to_string(),
                    start_line,
                    start_char,
                    end_line,
                    end_char,
                    detail,
                    source: "lsp".to_string(),
                },
            );
        }
    }

    // --- Tree-sitter symbols ---
    {
        let mut stmt = conn.prepare(
            "SELECT start_line, end_line, symbol_path \
             FROM ts_chunks WHERE file_path = ?1 AND symbol_path IS NOT NULL",
        )?;

        let rows = stmt.query_map([file_path], |row| {
            Ok((
                row.get::<_, u32>(0)?,    // start_line
                row.get::<_, u32>(1)?,    // end_line
                row.get::<_, String>(2)?, // symbol_path
            ))
        })?;

        for row in rows {
            let (start_line, end_line, symbol_path) = row?;

            // Only insert if LSP hasn't already provided this line
            seen.entry(start_line).or_insert_with(|| {
                let name = symbol_path
                    .rsplit("::")
                    .next()
                    .unwrap_or(&symbol_path)
                    .to_string();
                SymbolLocation {
                    name,
                    qualified_path: symbol_path,
                    kind: None,
                    file_path: file_path.to_string(),
                    start_line,
                    start_char: 0,
                    end_line,
                    end_char: 0,
                    detail: None,
                    source: "treesitter".to_string(),
                }
            });
        }
    }

    let mut results: Vec<SymbolLocation> = seen.into_values().collect();
    results.sort_by_key(|s| s.start_line);
    Ok(results)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Extract the qualified path from an `lsp_symbols.id` field.
fn qualified_path_from_id(id: &str, file_path: &str) -> String {
    let prefix = format!("lsp:{}:", file_path);
    if let Some(qpath) = id.strip_prefix(&prefix) {
        qpath.to_string()
    } else {
        id.to_string()
    }
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

    /// Insert an `indexed_files` row.
    fn insert_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES (?1, X'DEADBEEF', 1024, 1000)",
            [path],
        )
        .unwrap();
    }

    /// Insert an LSP symbol.
    #[allow(clippy::too_many_arguments)]
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

        // LSP symbols in src/lib.rs
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct",
            "MyStruct",
            23,
            "src/lib.rs",
            0,
            0,
            20,
            1,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::new",
            "new",
            12,
            "src/lib.rs",
            5,
            4,
            8,
            5,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/lib.rs:MyStruct::authenticate",
            "authenticate",
            6,
            "src/lib.rs",
            10,
            4,
            15,
            5,
        );

        // LSP symbols in src/auth.rs
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService",
            "AuthService",
            5,
            "src/auth.rs",
            0,
            0,
            30,
            1,
        );
        insert_lsp_symbol(
            conn,
            "lsp:src/auth.rs:AuthService::validate",
            "validate",
            6,
            "src/auth.rs",
            15,
            4,
            20,
            5,
        );

        // Tree-sitter chunks (overlapping with LSP)
        insert_ts_chunk(conn, "src/lib.rs", 0, 20, "MyStruct");
        insert_ts_chunk(conn, "src/lib.rs", 5, 8, "MyStruct::new");
        insert_ts_chunk(conn, "src/lib.rs", 10, 15, "MyStruct::authenticate");
        insert_ts_chunk(conn, "src/auth.rs", 0, 30, "AuthService");
        insert_ts_chunk(conn, "src/auth.rs", 15, 20, "AuthService::validate");
    }

    #[test]
    fn test_list_symbols() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = list_symbols(&conn, "src/lib.rs").unwrap();

        assert_eq!(results.len(), 3, "expected 3 symbols in src/lib.rs");

        // Should be sorted by start_line
        assert!(
            results
                .windows(2)
                .all(|w| w[0].start_line <= w[1].start_line),
            "results should be sorted by start_line"
        );

        // Check the symbols
        assert_eq!(results[0].qualified_path, "MyStruct");
        assert_eq!(results[0].kind, Some("struct".to_string()));
        assert_eq!(results[1].qualified_path, "MyStruct::new");
        assert_eq!(results[1].kind, Some("function".to_string()));
        assert_eq!(results[2].qualified_path, "MyStruct::authenticate");
        assert_eq!(results[2].kind, Some("method".to_string()));

        // All should be from LSP since both sources exist
        for r in &results {
            assert_eq!(r.source, "lsp");
        }
    }

    #[test]
    fn test_list_symbols_empty_file() {
        let conn = test_db();
        seed_fixtures(&conn);

        let results = list_symbols(&conn, "nonexistent.rs").unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_list_symbols_treesitter_only() {
        let conn = test_db();
        insert_file(&conn, "src/other.rs");
        insert_ts_chunk(&conn, "src/other.rs", 1, 5, "helper");
        insert_ts_chunk(&conn, "src/other.rs", 10, 15, "worker");

        let results = list_symbols(&conn, "src/other.rs").unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].qualified_path, "helper");
        assert_eq!(results[0].source, "treesitter");
        assert_eq!(results[1].qualified_path, "worker");
        assert_eq!(results[1].source, "treesitter");
    }
}
