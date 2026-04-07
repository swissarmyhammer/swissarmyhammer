//! Shared test helpers for the code-context crate.
//!
//! Provides canonical versions of the helper functions that were previously
//! duplicated across every test module. Import with `use crate::test_fixtures::*;`
//! from any `#[cfg(test)] mod tests` block.

use rusqlite::Connection;

use crate::db::{configure_connection, create_schema};

/// Create an in-memory test database with the full schema applied.
pub fn test_db() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    conn
}

/// Insert a row into `indexed_files`.
///
/// The simple form sets `ts_indexed` and `lsp_indexed` to 0 (unindexed).
/// Pass explicit values when tests need to control indexing state.
pub fn insert_file(conn: &Connection, path: &str, ts_indexed: i32, lsp_indexed: i32) {
    conn.execute(
        "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
         VALUES (?1, X'DEADBEEF', 1024, 1000, ?2, ?3)",
        rusqlite::params![path, ts_indexed, lsp_indexed],
    )
    .unwrap();
}

/// Insert a row into `indexed_files` with default indexing flags (both 0).
pub fn insert_file_simple(conn: &Connection, path: &str) {
    insert_file(conn, path, 0, 0);
}

/// Insert an LSP symbol into `lsp_symbols`.
#[allow(clippy::too_many_arguments)]
pub fn insert_lsp_symbol(
    conn: &Connection,
    id: &str,
    name: &str,
    kind: i32,
    file_path: &str,
    start_line: i32,
    start_char: i32,
    end_line: i32,
    end_char: i32,
    detail: Option<&str>,
) {
    conn.execute(
        "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        rusqlite::params![id, name, kind, file_path, start_line, start_char, end_line, end_char, detail],
    )
    .unwrap();
}

/// Insert a tree-sitter chunk into `ts_chunks`.
pub fn insert_ts_chunk(
    conn: &Connection,
    file_path: &str,
    start_line: i32,
    end_line: i32,
    text: &str,
    symbol_path: Option<&str>,
) {
    conn.execute(
        "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path)
         VALUES (?1, 0, ?5, ?2, ?3, ?4, ?6)",
        rusqlite::params![file_path, start_line, end_line, text, text.len() as i64, symbol_path],
    )
    .unwrap();
}

/// Insert a call edge into `lsp_call_edges`.
pub fn insert_call_edge(
    conn: &Connection,
    caller_id: &str,
    callee_id: &str,
    caller_file: &str,
    callee_file: &str,
    source: &str,
    from_ranges: &str,
) {
    conn.execute(
        "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        rusqlite::params![caller_id, callee_id, caller_file, callee_file, source, from_ranges],
    )
    .unwrap();
}
