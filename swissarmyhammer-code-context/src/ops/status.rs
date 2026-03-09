//! Status operations for the code context index.
//!
//! - `get_status` -- health report with file counts, indexed percentages,
//!   chunk/edge counts. Always returns immediately, even mid-index.
//! - `build_status` -- marks files for re-indexing by resetting indexed flags.
//! - `clear_status` -- wipes all index data and returns stats about what was cleared.

use rusqlite::Connection;

use crate::error::CodeContextError;

// ---------------------------------------------------------------------------
// get_status
// ---------------------------------------------------------------------------

/// Health report for the code context index.
#[derive(Debug, Clone, serde::Serialize)]
pub struct StatusReport {
    /// Total number of tracked files.
    pub total_files: u64,
    /// Number of files with `ts_indexed = 1`.
    pub ts_indexed_files: u64,
    /// Number of files with `lsp_indexed = 1`.
    pub lsp_indexed_files: u64,
    /// Tree-sitter indexed percentage (0.0 to 100.0).
    pub ts_indexed_percent: f64,
    /// LSP indexed percentage (0.0 to 100.0).
    pub lsp_indexed_percent: f64,
    /// Total number of tree-sitter chunks.
    pub ts_chunk_count: u64,
    /// Number of files that actually have chunks in ts_chunks (honest metric).
    pub files_with_chunks: u64,
    /// Number of files that have symbols in lsp_symbols.
    pub files_with_symbols: u64,
    /// Total number of LSP symbols.
    pub lsp_symbol_count: u64,
    /// Total number of call edges (both LSP and tree-sitter sourced).
    pub call_edge_count: u64,
    /// Number of files still waiting for indexing (ts_indexed=0).
    pub dirty_files: u64,
    /// Suggested next step.
    pub hint: &'static str,
}

/// Returns a health report for the code context index.
///
/// Queries file counts, indexed percentages, chunk counts, and edge counts
/// from the SQLite database. Always returns immediately -- never blocks.
///
/// # Arguments
///
/// * `conn` - A reference to the SQLite connection.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn get_status(conn: &Connection) -> Result<StatusReport, CodeContextError> {
    let total_files: u64 =
        conn.query_row("SELECT COUNT(*) FROM indexed_files", [], |r| r.get(0))?;

    let ts_indexed_files: u64 = conn.query_row(
        "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 1",
        [],
        |r| r.get(0),
    )?;

    let lsp_indexed_files: u64 = conn.query_row(
        "SELECT COUNT(*) FROM indexed_files WHERE lsp_indexed = 1",
        [],
        |r| r.get(0),
    )?;

    let ts_indexed_percent = if total_files > 0 {
        (ts_indexed_files as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    let lsp_indexed_percent = if total_files > 0 {
        (lsp_indexed_files as f64 / total_files as f64) * 100.0
    } else {
        0.0
    };

    let ts_chunk_count: u64 =
        conn.query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))?;

    let files_with_chunks: u64 = conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM ts_chunks",
        [],
        |r| r.get(0),
    )?;

    let files_with_symbols: u64 = conn.query_row(
        "SELECT COUNT(DISTINCT file_path) FROM lsp_symbols",
        [],
        |r| r.get(0),
    )?;

    let lsp_symbol_count: u64 =
        conn.query_row("SELECT COUNT(*) FROM lsp_symbols", [], |r| r.get(0))?;

    let call_edge_count: u64 =
        conn.query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))?;

    let dirty_files: u64 = conn.query_row(
        "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 0",
        [],
        |r| r.get(0),
    )?;

    let hint = crate::hints::hint_for_operation("get_status");

    Ok(StatusReport {
        total_files,
        ts_indexed_files,
        lsp_indexed_files,
        ts_indexed_percent,
        lsp_indexed_percent,
        ts_chunk_count,
        files_with_chunks,
        files_with_symbols,
        lsp_symbol_count,
        call_edge_count,
        dirty_files,
        hint,
    })
}

// ---------------------------------------------------------------------------
// build_status
// ---------------------------------------------------------------------------

/// Result of a `build_status` operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct BuildStatusResult {
    /// Number of files marked for re-indexing.
    pub files_marked: u64,
    /// Which layer was reset.
    pub layer: String,
    /// Suggested next step.
    pub hint: &'static str,
}

/// The indexing layer to reset for re-indexing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildLayer {
    /// Reset tree-sitter indexed flag.
    TreeSitter,
    /// Reset LSP indexed flag.
    Lsp,
    /// Reset both layers.
    Both,
}

/// Marks files for re-indexing by resetting the indexed flag for the
/// specified layer.
///
/// This does not actually perform indexing -- it marks files so the
/// leader process will re-index them on its next cycle.
///
/// # Arguments
///
/// * `conn` - A reference to the SQLite connection (must be read-write).
/// * `layer` - Which indexing layer to reset.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn build_status(
    conn: &Connection,
    layer: BuildLayer,
) -> Result<BuildStatusResult, CodeContextError> {
    let (sql, layer_name) = match layer {
        BuildLayer::TreeSitter => (
            "UPDATE indexed_files SET ts_indexed = 0",
            "treesitter",
        ),
        BuildLayer::Lsp => (
            "UPDATE indexed_files SET lsp_indexed = 0",
            "lsp",
        ),
        BuildLayer::Both => (
            "UPDATE indexed_files SET ts_indexed = 0, lsp_indexed = 0",
            "both",
        ),
    };

    let files_marked = conn.execute(sql, [])? as u64;

    Ok(BuildStatusResult {
        files_marked,
        layer: layer_name.to_string(),
        hint: crate::hints::hint_for_operation("build_status"),
    })
}

// ---------------------------------------------------------------------------
// clear_status
// ---------------------------------------------------------------------------

/// Stats about what was cleared during a `clear_status` operation.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClearStatusResult {
    /// Number of call edges deleted.
    pub edges_deleted: u64,
    /// Number of LSP symbols deleted.
    pub symbols_deleted: u64,
    /// Number of tree-sitter chunks deleted.
    pub chunks_deleted: u64,
    /// Number of indexed files deleted.
    pub files_deleted: u64,
    /// Suggested next step.
    pub hint: &'static str,
}

/// Wipes all index data from all tables and returns stats about what was cleared.
///
/// Deletes in dependency order: edges, symbols, chunks, files.
///
/// # Arguments
///
/// * `conn` - A reference to the SQLite connection (must be read-write).
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn clear_status(conn: &Connection) -> Result<ClearStatusResult, CodeContextError> {
    // Delete in dependency order to respect foreign keys
    let edges_deleted = conn.execute("DELETE FROM lsp_call_edges", [])? as u64;
    let symbols_deleted = conn.execute("DELETE FROM lsp_symbols", [])? as u64;
    let chunks_deleted = conn.execute("DELETE FROM ts_chunks", [])? as u64;
    let files_deleted = conn.execute("DELETE FROM indexed_files", [])? as u64;

    Ok(ClearStatusResult {
        edges_deleted,
        symbols_deleted,
        chunks_deleted,
        files_deleted,
        hint: crate::hints::hint_for_operation("clear_status"),
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::{configure_connection, create_schema};

    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    fn insert_file(conn: &Connection, path: &str, ts_indexed: i32, lsp_indexed: i32) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES (?1, X'DEADBEEF', 1024, 1000, ?2, ?3)",
            rusqlite::params![path, ts_indexed, lsp_indexed],
        )
        .unwrap();
    }

    fn insert_chunk(conn: &Connection, file_path: &str) {
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES (?1, 0, 100, 1, 10, 'fn main() {}')",
            [file_path],
        )
        .unwrap();
    }

    fn insert_symbol(conn: &Connection, id: &str, file_path: &str) {
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, 'sym', 12, ?2, 1, 0, 10, 1)",
            rusqlite::params![id, file_path],
        )
        .unwrap();
    }

    // -- get_status tests --

    #[test]
    fn test_get_status_fresh_db_shows_zero_percent() {
        let conn = test_db();
        let report = get_status(&conn).unwrap();

        assert_eq!(report.total_files, 0);
        assert_eq!(report.ts_indexed_files, 0);
        assert_eq!(report.lsp_indexed_files, 0);
        assert_eq!(report.ts_indexed_percent, 0.0);
        assert_eq!(report.lsp_indexed_percent, 0.0);
        assert_eq!(report.ts_chunk_count, 0);
        assert_eq!(report.files_with_chunks, 0);
        assert_eq!(report.files_with_symbols, 0);
        assert_eq!(report.lsp_symbol_count, 0);
        assert_eq!(report.call_edge_count, 0);
        assert_eq!(report.dirty_files, 0);
        assert!(!report.hint.is_empty());
    }

    #[test]
    fn test_get_status_with_indexed_files() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 0);
        insert_file(&conn, "c.rs", 0, 0);
        insert_chunk(&conn, "a.rs");
        insert_chunk(&conn, "b.rs");
        insert_symbol(&conn, "lsp:a.rs:main", "a.rs");

        let report = get_status(&conn).unwrap();

        assert_eq!(report.total_files, 3);
        assert_eq!(report.ts_indexed_files, 2);
        assert_eq!(report.lsp_indexed_files, 1);
        assert!((report.ts_indexed_percent - 66.666).abs() < 1.0);
        assert!((report.lsp_indexed_percent - 33.333).abs() < 1.0);
        assert_eq!(report.ts_chunk_count, 2);
        assert_eq!(report.files_with_chunks, 2);
        assert_eq!(report.files_with_symbols, 1);
        assert_eq!(report.lsp_symbol_count, 1);
        assert_eq!(report.call_edge_count, 0);
        assert_eq!(report.dirty_files, 1);
    }

    #[test]
    fn test_get_status_shows_lsp_server_states() {
        // "LSP server states" at the library layer means: how many files
        // have lsp_indexed=1 (Running/indexed) vs lsp_indexed=0 (not yet).
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1); // LSP indexed (Running equivalent)
        insert_file(&conn, "b.rs", 1, 0); // Not LSP indexed (Failed/NotFound equivalent)

        let report = get_status(&conn).unwrap();

        assert_eq!(report.lsp_indexed_files, 1, "1 file with LSP data (Running)");
        assert_eq!(
            report.total_files - report.lsp_indexed_files,
            1,
            "1 file without LSP data (Failed/NotFound)"
        );
        assert!((report.lsp_indexed_percent - 50.0).abs() < 0.01);
    }

    // -- build_status tests --

    #[test]
    fn test_build_status_resets_ts_layer() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 0);

        let result = build_status(&conn, BuildLayer::TreeSitter).unwrap();

        assert_eq!(result.files_marked, 2);
        assert_eq!(result.layer, "treesitter");
        assert!(!result.hint.is_empty());

        // Verify flags were reset
        let ts_count: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(ts_count, 0);

        // LSP should be untouched
        let lsp_count: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE lsp_indexed = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp_count, 1);
    }

    #[test]
    fn test_build_status_resets_both_layers() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 1);

        let result = build_status(&conn, BuildLayer::Both).unwrap();

        assert_eq!(result.files_marked, 2);
        assert_eq!(result.layer, "both");

        let indexed: u64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 1 OR lsp_indexed = 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(indexed, 0);
    }

    // -- clear_status tests --

    #[test]
    fn test_clear_status_wipes_everything() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 0);
        insert_chunk(&conn, "a.rs");
        insert_chunk(&conn, "b.rs");
        insert_symbol(&conn, "lsp:a.rs:main", "a.rs");

        let result = clear_status(&conn).unwrap();

        assert_eq!(result.files_deleted, 2);
        assert_eq!(result.chunks_deleted, 2);
        assert_eq!(result.symbols_deleted, 1);
        assert_eq!(result.edges_deleted, 0);
        assert!(!result.hint.is_empty());

        // Verify everything is gone
        let report = get_status(&conn).unwrap();
        assert_eq!(report.total_files, 0);
        assert_eq!(report.ts_chunk_count, 0);
        assert_eq!(report.lsp_symbol_count, 0);
    }

    #[test]
    fn test_clear_status_on_empty_db() {
        let conn = test_db();

        let result = clear_status(&conn).unwrap();

        assert_eq!(result.files_deleted, 0);
        assert_eq!(result.chunks_deleted, 0);
        assert_eq!(result.symbols_deleted, 0);
        assert_eq!(result.edges_deleted, 0);
    }
}
