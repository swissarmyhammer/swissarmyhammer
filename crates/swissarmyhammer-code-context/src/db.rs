//! Unified SQLite schema for the code context index

use rusqlite::Connection;

/// Create all tables in the unified schema.
///
/// Tables: `indexed_files`, `ts_chunks`, `lsp_symbols`, `lsp_call_edges`.
/// Safe to call multiple times (uses IF NOT EXISTS).
///
/// After `CREATE TABLE`, runs any column-level migrations that bring
/// pre-existing databases up to the current schema. The migrations are
/// each individually idempotent.
pub fn create_schema(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS indexed_files (
            file_path     TEXT PRIMARY KEY,
            content_hash  BLOB NOT NULL,
            file_size     INTEGER NOT NULL,
            last_seen_at  INTEGER NOT NULL,
            ts_indexed    INTEGER NOT NULL DEFAULT 0,
            lsp_indexed   INTEGER NOT NULL DEFAULT 0,
            embedded      INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS ts_chunks (
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
    )?;

    migrate_indexed_files_add_embedded(conn)?;

    Ok(())
}

/// Bring an existing `indexed_files` table up to the current schema.
///
/// Runs every additive column migration in order. Each step is independently
/// idempotent (it checks `PRAGMA table_info` before issuing the ALTER), so it
/// is safe to invoke this on any database — fresh, partially-migrated, or
/// already current.
///
/// Workspace open paths call this both for leaders (after `create_schema`)
/// and for followers (through a brief write connection) so that a legacy
/// on-disk schema is brought current regardless of which process opens the
/// DB first. The bare `Connection` parameter lets `workspace.rs` migrate
/// without needing to know the full schema-creation surface.
pub(crate) fn migrate_indexed_files(conn: &Connection) -> rusqlite::Result<()> {
    migrate_indexed_files_add_embedded(conn)
}

/// Add the `embedded` column to `indexed_files` if it is missing.
///
/// Databases created before this column existed need to be migrated in
/// place. SQLite supports `ALTER TABLE ... ADD COLUMN`, so we just check
/// `PRAGMA table_info` for the column and run the ALTER when absent.
///
/// Idempotent: a no-op when the column is already present (the
/// `CREATE TABLE` above declares it for fresh databases).
fn migrate_indexed_files_add_embedded(conn: &Connection) -> rusqlite::Result<()> {
    let mut stmt = conn.prepare("PRAGMA table_info(indexed_files)")?;
    let columns: Vec<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<Result<_, _>>()?;
    drop(stmt);

    if !columns.iter().any(|c| c == "embedded") {
        conn.execute(
            "ALTER TABLE indexed_files ADD COLUMN embedded INTEGER NOT NULL DEFAULT 0",
            [],
        )?;
    }
    Ok(())
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

    /// `PRAGMA table_info(indexed_files)` returns one row per column.
    /// Column layout: cid, name, type, notnull, dflt_value, pk.
    fn indexed_files_column_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn.prepare("PRAGMA table_info(indexed_files)").unwrap();
        let rows: Vec<String> = stmt
            .query_map([], |row| row.get::<_, String>(1))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        rows
    }

    #[test]
    fn test_fresh_db_has_embedded_column() {
        let conn = open_memory_db();
        let cols = indexed_files_column_names(&conn);
        assert!(
            cols.iter().any(|c| c == "embedded"),
            "expected `embedded` column on fresh db, got: {:?}",
            cols
        );
    }

    #[test]
    fn test_migration_adds_embedded_column_to_legacy_db() {
        // Create a DB with the pre-migration schema (no `embedded` column).
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        conn.execute_batch(
            "CREATE TABLE indexed_files (
                file_path     TEXT PRIMARY KEY,
                content_hash  BLOB NOT NULL,
                file_size     INTEGER NOT NULL,
                last_seen_at  INTEGER NOT NULL,
                ts_indexed    INTEGER NOT NULL DEFAULT 0,
                lsp_indexed   INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();

        // Insert a row in the legacy schema.
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES ('legacy.rs', X'00', 1, 1, 1, 1)",
            [],
        )
        .unwrap();

        assert!(
            !indexed_files_column_names(&conn)
                .iter()
                .any(|c| c == "embedded"),
            "precondition: legacy schema should not have `embedded`"
        );

        // Running create_schema must trigger the migration.
        create_schema(&conn).unwrap();

        let cols = indexed_files_column_names(&conn);
        assert!(
            cols.iter().any(|c| c == "embedded"),
            "expected migration to add `embedded` column, got: {:?}",
            cols
        );

        // Existing rows default to embedded = 0.
        let embedded: i64 = conn
            .query_row(
                "SELECT embedded FROM indexed_files WHERE file_path = 'legacy.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(embedded, 0, "legacy rows must default to embedded=0");
    }

    #[test]
    fn test_migration_is_idempotent() {
        // Apply schema twice — second call must not fail even though
        // the `embedded` column already exists.
        let conn = open_memory_db();
        create_schema(&conn).unwrap();
        create_schema(&conn).unwrap();

        let cols = indexed_files_column_names(&conn);
        let n = cols.iter().filter(|c| *c == "embedded").count();
        assert_eq!(n, 1, "`embedded` column should appear exactly once");
    }
}
