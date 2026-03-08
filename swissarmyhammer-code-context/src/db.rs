//! Unified SQLite schema for the code context index

use rusqlite::Connection;

/// Create all tables in the unified schema.
///
/// Tables: `indexed_files`, `ts_chunks`, `lsp_symbols`, `lsp_call_edges`.
/// Safe to call multiple times (uses IF NOT EXISTS).
pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS indexed_files (
            file_path     TEXT PRIMARY KEY,
            content_hash  BLOB NOT NULL,
            file_size     INTEGER NOT NULL,
            last_seen_at  INTEGER NOT NULL,
            ts_indexed    INTEGER NOT NULL DEFAULT 0,
            lsp_indexed   INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS ts_chunks (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
            start_byte   INTEGER NOT NULL,
            end_byte     INTEGER NOT NULL,
            start_line   INTEGER NOT NULL,
            end_line     INTEGER NOT NULL,
            text         TEXT NOT NULL,
            symbol_path  TEXT,
            embedding    BLOB
        );
        CREATE INDEX IF NOT EXISTS idx_ts_chunks_file ON ts_chunks(file_path);

        CREATE TABLE IF NOT EXISTS lsp_symbols (
            id           TEXT PRIMARY KEY,
            name         TEXT NOT NULL,
            kind         INTEGER NOT NULL,
            file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
            start_line   INTEGER NOT NULL,
            start_char   INTEGER NOT NULL,
            end_line     INTEGER NOT NULL,
            end_char     INTEGER NOT NULL,
            detail       TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_lsp_symbols_file ON lsp_symbols(file_path);

        CREATE TABLE IF NOT EXISTS lsp_call_edges (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            caller_id    TEXT NOT NULL REFERENCES lsp_symbols(id) ON DELETE CASCADE,
            callee_id    TEXT NOT NULL REFERENCES lsp_symbols(id) ON DELETE CASCADE,
            caller_file  TEXT NOT NULL,
            callee_file  TEXT NOT NULL,
            from_ranges  TEXT NOT NULL,
            source       TEXT NOT NULL DEFAULT 'lsp'
        );
        CREATE INDEX IF NOT EXISTS idx_edges_caller_file ON lsp_call_edges(caller_file);
        CREATE INDEX IF NOT EXISTS idx_edges_callee_file ON lsp_call_edges(callee_file);
        ",
    )
}

/// Enable WAL mode and foreign keys on a connection.
pub fn configure_connection(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        PRAGMA journal_mode = WAL;
        PRAGMA foreign_keys = ON;
        ",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open_memory_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    #[test]
    fn test_schema_creation() {
        let conn = open_memory_db();

        let tables: Vec<String> = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |row| row.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();

        assert!(tables.contains(&"indexed_files".to_string()));
        assert!(tables.contains(&"ts_chunks".to_string()));
        assert!(tables.contains(&"lsp_symbols".to_string()));
        assert!(tables.contains(&"lsp_call_edges".to_string()));
    }

    #[test]
    fn test_schema_idempotent() {
        let conn = open_memory_db();
        // Second call should not fail
        create_schema(&conn).unwrap();
    }

    #[test]
    fn test_cascade_delete() {
        let conn = open_memory_db();

        // Insert a file
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('src/main.rs', X'00112233', 1024, 1000)",
            [],
        )
        .unwrap();

        // Insert a chunk
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES ('src/main.rs', 0, 100, 1, 10, 'fn main() {}')",
            [],
        )
        .unwrap();

        // Insert a symbol
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/main.rs:main', 'main', 12, 'src/main.rs', 1, 0, 10, 1)",
            [],
        )
        .unwrap();

        // Insert another file + symbol for the edge target
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at)
             VALUES ('src/lib.rs', X'44556677', 512, 1000)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES ('lsp:src/lib.rs:init', 'init', 12, 'src/lib.rs', 1, 0, 5, 1)",
            [],
        )
        .unwrap();

        // Insert a call edge
        conn.execute(
            "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, from_ranges)
             VALUES ('lsp:src/main.rs:main', 'lsp:src/lib.rs:init', 'src/main.rs', 'src/lib.rs', '[]')",
            [],
        )
        .unwrap();

        // Verify data exists
        let chunk_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunk_count, 1);

        let edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edge_count, 1);

        // Delete the file — CASCADE should remove chunks and symbols
        conn.execute(
            "DELETE FROM indexed_files WHERE file_path = 'src/main.rs'",
            [],
        )
        .unwrap();

        // Chunks should be gone
        let chunk_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM ts_chunks", [], |r| r.get(0))
            .unwrap();
        assert_eq!(chunk_count, 0);

        // Symbol for main.rs should be gone
        let sym_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/main.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sym_count, 0);

        // Edge should be gone (CASCADE from caller symbol deletion)
        let edge_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM lsp_call_edges", [], |r| r.get(0))
            .unwrap();
        assert_eq!(edge_count, 0);

        // lib.rs symbol should still exist
        let lib_sym: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/lib.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lib_sym, 1);
    }

    #[test]
    fn test_foreign_keys_enabled() {
        let conn = open_memory_db();

        // Inserting a chunk with a non-existent file_path should fail
        let result = conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES ('nonexistent.rs', 0, 10, 1, 1, 'test')",
            [],
        );
        assert!(result.is_err());
    }
}
