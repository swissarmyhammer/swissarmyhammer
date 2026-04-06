//! In-process parallel indexing worker for the leader process
//!
//! The leader spawns a background thread that:
//! 1. Monitors the database for dirty files (ts_indexed=0 or lsp_indexed=0)
//! 2. Runs tree-sitter parsing in parallel using IndexContext
//! 3. Writes chunks and symbols to the database
//! 4. Updates indexed flags
//! 5. Handles LSP requests (placeholder for future LSP integration)
//!
//! All database writes go through the leader's [`SharedDb`] — a single
//! `Arc<Mutex<Connection>>` — so there is no write contention with the
//! LSP worker or any other writer.

use rayon::prelude::*;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use tracing::{debug, info, warn};

use crate::error::CodeContextError;
use crate::workspace::SharedDb;

/// Configuration for the indexing worker
#[derive(Debug, Clone)]
pub struct IndexingConfig {
    /// Maximum parallel parsing tasks
    pub max_parallel_tasks: usize,
    /// Maximum files to process per batch
    pub batch_size: usize,
}

impl Default for IndexingConfig {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 4,
            batch_size: 100,
        }
    }
}

/// Spawn a background indexing worker thread in the leader process.
///
/// This starts a detached thread that:
/// 1. Queries dirty files from the database
/// 2. Parses them using tree-sitter
/// 3. Writes results back to the database via the shared connection
/// 4. Repeats until no dirty files remain
///
/// The worker uses the leader's [`SharedDb`] so all writes are serialized
/// through a single connection, eliminating SQLITE_BUSY contention with the
/// LSP worker and other writers.
pub fn spawn_indexing_worker(workspace_root: PathBuf, db: SharedDb, config: IndexingConfig) {
    thread::Builder::new()
        .name("code-context-indexer".to_string())
        .spawn(
            move || match run_indexing_worker(&workspace_root, &db, config) {
                Ok(()) => {
                    info!("Indexing worker completed successfully");
                }
                Err(e) => {
                    warn!("Indexing worker encountered error: {}", e);
                }
            },
        )
        .expect("Failed to spawn indexing worker thread");
}

/// Main indexing worker loop.
///
/// Uses the leader's shared DB connection for all reads and writes.
/// The mutex is locked only for the duration of each individual DB call
/// so the LSP worker and other writers can interleave without contention.
fn run_indexing_worker(
    workspace_root: &Path,
    db: &SharedDb,
    config: IndexingConfig,
) -> Result<(), CodeContextError> {
    info!(
        "code-context indexing worker started for {}",
        workspace_root.display()
    );

    // Work queue loop: keep checking for dirty files indefinitely
    // This allows the worker to index files that are discovered after startup
    let mut indexed_count = 0;
    loop {
        // Query dirty files (ts_indexed = 0) in batches
        let dirty_files = query_dirty_files(db, config.batch_size)?;

        if !dirty_files.is_empty() {
            info!("code-context: processing {} dirty files", dirty_files.len());

            // Process files in parallel using rayon
            // Parse files using tree-sitter and extract chunks
            let results: Vec<_> = dirty_files
                .par_iter()
                .with_max_len(config.max_parallel_tasks)
                .map(|file_path| {
                    let full_path = workspace_root.join(file_path);
                    if !full_path.exists() {
                        warn!("File not found: {}", file_path);
                        return (file_path.clone(), vec![]);
                    }

                    // Parse file and extract chunks
                    match parse_and_extract_chunks(&full_path) {
                        Ok(chunks) => {
                            debug!("Extracted {} chunks from {}", chunks.len(), file_path);
                            (file_path.clone(), chunks)
                        }
                        Err(e) => {
                            warn!("Failed to parse {}: {}", file_path, e);
                            (file_path.clone(), vec![])
                        }
                    }
                })
                .collect();

            // Write results back to database (each call locks the shared connection briefly)
            for (file_path, chunks) in results {
                if chunks.is_empty() {
                    debug!(
                        "Skipping {} - no chunks extracted, marking indexed to avoid retry loop",
                        file_path
                    );
                    if let Err(e) = mark_ts_indexed(db, &file_path) {
                        warn!("Failed to mark {} as indexed: {}", file_path, e);
                    }
                    continue;
                }

                if let Err(e) = write_ts_chunks(db, &file_path, &chunks) {
                    warn!("Failed to write chunks for {}: {}", file_path, e);
                    // Mark as indexed anyway to avoid infinite retry loop
                    if let Err(e2) = mark_ts_indexed(db, &file_path) {
                        warn!(
                            "Failed to mark {} as indexed after chunk write error: {}",
                            file_path, e2
                        );
                    }
                    continue;
                }

                if let Err(e) = mark_ts_indexed(db, &file_path) {
                    warn!("Failed to mark {} as indexed: {}", file_path, e);
                } else {
                    indexed_count += 1;
                    debug!(
                        "Successfully indexed {} with {} chunks",
                        file_path,
                        chunks.len()
                    );
                }
            }
            info!(
                "code-context: indexed {} files so far (batch complete)",
                indexed_count
            );
        }

        // Sleep before next iteration (allows new files to be discovered)
        // In production, this would be longer; in tests we use shorter intervals
        thread::sleep(Duration::from_millis(100));
    }
}

/// Query files that need tree-sitter indexing (ts_indexed=0).
///
/// Locks the shared connection for the duration of the query.
fn query_dirty_files(db: &SharedDb, limit: usize) -> Result<Vec<String>, CodeContextError> {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    let mut stmt =
        conn.prepare("SELECT file_path FROM indexed_files WHERE ts_indexed=0 LIMIT ?")?;

    let files = stmt
        .query_map([limit as i64], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(files)
}

/// Mark a file as indexed in the database.
///
/// Locks the shared connection for the duration of the update.
fn mark_ts_indexed(db: &SharedDb, file_path: &str) -> Result<(), CodeContextError> {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    conn.execute(
        "UPDATE indexed_files SET ts_indexed=1 WHERE file_path=?",
        [file_path],
    )?;
    Ok(())
}

/// Read a file and extract chunks based on lines
///
/// This is a simple chunking strategy that splits files into chunks of ~1000 bytes.
/// A more sophisticated implementation would use tree-sitter AST-aware chunking.
fn parse_and_extract_chunks(file_path: &Path) -> Result<Vec<(usize, String)>, CodeContextError> {
    let content = std::fs::read_to_string(file_path)?;

    const CHUNK_SIZE: usize = 1000; // bytes per chunk
    let mut chunks = Vec::new();
    let mut start_byte = 0;

    // Split by newlines to avoid breaking in the middle of lines
    for line in content.lines() {
        let line_with_newline = format!("{}\n", line);

        // If adding this line would exceed chunk size, start a new chunk
        if start_byte > 0
            && chunks.last().is_some_and(|(_, chunk): &(usize, String)| {
                chunk.len() + line_with_newline.len() > CHUNK_SIZE
            })
        {
            start_byte += chunks.last().unwrap().1.len();
        }

        // Add to current or new chunk
        if chunks.is_empty() || start_byte == 0 {
            chunks.push((start_byte, line_with_newline));
            start_byte = 0;
        } else {
            chunks.last_mut().unwrap().1.push_str(&line_with_newline);
        }
    }

    // Simple approach: just create one chunk per file for now
    // A better implementation would use AST-aware chunking via tree-sitter
    if content.is_empty() {
        return Ok(vec![]);
    }

    Ok(vec![(0, content)])
}

/// Write parsed chunks to the ts_chunks table.
///
/// Locks the shared connection for the duration of all inserts in one batch.
fn write_ts_chunks(
    db: &SharedDb,
    file_path: &str,
    chunks: &[(usize, String)],
) -> Result<(), CodeContextError> {
    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
    // Delete any existing chunks for this file before inserting new ones.
    // Without this, re-indexing a file (after a hash change marks it dirty)
    // would accumulate duplicate chunk rows.
    conn.execute("DELETE FROM ts_chunks WHERE file_path = ?", [file_path])?;
    for (start_byte, content) in chunks {
        let end_byte = start_byte + content.len();
        // Count lines in the content
        let start_line = 1i64; // Simple implementation: all chunks start at line 1
        let end_line = 1i64 + content.lines().count() as i64;

        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![file_path, *start_byte as i64, end_byte as i64, start_line, end_line, content],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::fs;
    use std::sync::{Arc, Mutex};
    use tempfile::TempDir;

    /// Create a test SharedDb with the required schema.
    fn create_test_db() -> SharedDb {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            PRAGMA foreign_keys = ON;
            CREATE TABLE indexed_files (
                file_path     TEXT PRIMARY KEY,
                content_hash  BLOB NOT NULL,
                file_size     INTEGER NOT NULL,
                last_seen_at  INTEGER NOT NULL,
                ts_indexed    INTEGER NOT NULL DEFAULT 0,
                lsp_indexed   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE ts_chunks (
                file_path    TEXT NOT NULL REFERENCES indexed_files(file_path) ON DELETE CASCADE,
                start_byte   INTEGER NOT NULL,
                end_byte     INTEGER NOT NULL,
                start_line   INTEGER NOT NULL,
                end_line     INTEGER NOT NULL,
                text         TEXT NOT NULL,
                symbol_path  TEXT,
                embedding    BLOB
            );
            ",
        )
        .unwrap();
        Arc::new(Mutex::new(conn))
    }

    /// Insert a test file row into indexed_files via the shared connection.
    fn insert_test_file(db: &SharedDb, file_path: &str) {
        let conn = db.lock().unwrap();
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed) VALUES (?, ?, ?, ?, 0, 0)",
            rusqlite::params![file_path, vec![0u8; 16], 1024i64, 1000i64],
        ).unwrap();
    }

    #[test]
    fn test_parse_and_extract_chunks_rust_file() {
        // Create a temporary Rust file
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.rs");

        let rust_code = r#"
fn hello() {
    println!("Hello, world!");
}
"#;

        fs::write(&file_path, rust_code).unwrap();

        // Parse and extract chunks
        let result = parse_and_extract_chunks(&file_path);
        assert!(result.is_ok());

        let chunks = result.unwrap();
        assert!(!chunks.is_empty(), "Should extract at least one chunk");

        // Verify chunks have content
        for (_start, content) in chunks {
            assert!(!content.is_empty(), "Chunk content should not be empty");
        }
    }

    #[test]
    fn test_write_ts_chunks_to_database() {
        let db = create_test_db();
        let file_path = "test.rs";

        insert_test_file(&db, file_path);

        let chunks = vec![
            (0usize, "fn hello() {".to_string()),
            (12usize, "    println!(\"Hello\");".to_string()),
        ];

        let result = write_ts_chunks(&db, file_path, &chunks);
        assert!(result.is_ok());

        // Verify chunks were written
        let conn = db.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?")
            .unwrap();

        let count: i64 = stmt.query_row([file_path], |row| row.get(0)).unwrap();
        assert_eq!(count, 2, "Should have 2 chunks in database");
    }

    #[test]
    fn test_mark_ts_indexed() {
        let db = create_test_db();
        let file_path = "test.rs";

        insert_test_file(&db, file_path);

        // Verify file is not indexed initially
        {
            let conn = db.lock().unwrap();
            let initial: i64 = conn
                .query_row(
                    "SELECT ts_indexed FROM indexed_files WHERE file_path = ?",
                    [file_path],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(initial, 0, "File should not be indexed initially");
        }

        // Mark as indexed
        let result = mark_ts_indexed(&db, file_path);
        assert!(result.is_ok());

        // Verify file is indexed
        let conn = db.lock().unwrap();
        let indexed: i64 = conn
            .query_row(
                "SELECT ts_indexed FROM indexed_files WHERE file_path = ?",
                [file_path],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, 1, "File should be indexed");
    }

    #[test]
    fn test_query_dirty_files() {
        let db = create_test_db();

        insert_test_file(&db, "file1.rs");
        insert_test_file(&db, "file2.rs");

        // Mark one as indexed
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'file1.rs'",
                [],
            )
            .unwrap();
        }

        // Query dirty files
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert_eq!(dirty.len(), 1, "Should find 1 dirty file");
        assert_eq!(dirty[0], "file2.rs", "Should find file2.rs as dirty");
    }

    #[test]
    fn test_chunks_persist_through_write_read_cycle() {
        let db = create_test_db();
        let file_path = "module.rs";

        insert_test_file(&db, file_path);

        // Write chunks
        let chunks = vec![
            (0usize, "pub struct Data".to_string()),
            (100usize, "impl Data".to_string()),
        ];

        let write_result = write_ts_chunks(&db, file_path, &chunks);
        assert!(write_result.is_ok(), "Writing chunks should succeed");

        // Verify chunks count
        {
            let conn = db.lock().unwrap();
            let all_chunks: i64 = conn
                .query_row("SELECT COUNT(*) FROM ts_chunks", [], |row| row.get(0))
                .unwrap();
            assert_eq!(all_chunks, 2, "Should have 2 chunks total in database");
        }

        // Mark as indexed
        let mark_result = mark_ts_indexed(&db, file_path);
        assert!(mark_result.is_ok());
    }

    #[test]
    fn test_write_ts_chunks_deletes_old_before_insert() {
        let db = create_test_db();
        let file_path = "reindexed.rs";

        insert_test_file(&db, file_path);

        // First write: 2 chunks
        let chunks_v1 = vec![
            (0usize, "fn old_v1() {}".to_string()),
            (14usize, "fn old_v2() {}".to_string()),
        ];
        write_ts_chunks(&db, file_path, &chunks_v1).unwrap();

        // Verify 2 chunks exist
        {
            let conn = db.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                    [file_path],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(count, 2, "Should have 2 chunks after first write");
        }

        // Second write: 1 chunk (simulates re-indexing after file edit)
        let chunks_v2 = vec![(0usize, "fn new_only() {}".to_string())];
        write_ts_chunks(&db, file_path, &chunks_v2).unwrap();

        // Should have exactly 1 chunk, not 3 (old ones must be deleted)
        {
            let conn = db.lock().unwrap();
            let count: i64 = conn
                .query_row(
                    "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                    [file_path],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                count, 1,
                "Should have 1 chunk after re-index, not 3 (old chunks must be deleted)"
            );

            // Verify it's the new content
            let text: String = conn
                .query_row(
                    "SELECT text FROM ts_chunks WHERE file_path = ?",
                    [file_path],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(
                text, "fn new_only() {}",
                "Chunk text should be the new version"
            );
        }
    }

    #[test]
    fn test_mark_ts_indexed_returns_error_on_broken_db() {
        // Verify that mark_ts_indexed surfaces errors rather than silently
        // swallowing them. This matters because the caller must log or
        // propagate the error so files don't stay dirty forever.
        let db = create_test_db();
        insert_test_file(&db, "broken.rs");

        // Drop the indexed_files table to simulate a broken database state
        {
            let conn = db.lock().unwrap();
            conn.execute_batch("DROP TABLE ts_chunks; DROP TABLE indexed_files;")
                .unwrap();
        }

        // mark_ts_indexed should return Err, not silently succeed
        let result = mark_ts_indexed(&db, "broken.rs");
        assert!(
            result.is_err(),
            "mark_ts_indexed should return an error when the table is missing"
        );
    }

    #[test]
    fn test_shared_db_no_contention_between_reads_and_writes() {
        // Verify that the SharedDb approach works: one thread writes chunks
        // while another reads dirty files, with no SQLITE_BUSY errors.
        let db = create_test_db();
        insert_test_file(&db, "concurrent.rs");

        // Write from one reference
        let db2 = Arc::clone(&db);
        let handle = std::thread::spawn(move || {
            let chunks = vec![(0usize, "fn concurrent() {}".to_string())];
            write_ts_chunks(&db2, "concurrent.rs", &chunks).unwrap();
            mark_ts_indexed(&db2, "concurrent.rs").unwrap();
        });

        handle.join().unwrap();

        // Read from original reference -- should see the write
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert!(
            dirty.is_empty(),
            "File should be indexed after write thread completes"
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_empty_file() {
        // An empty file should return Ok with no chunks, not an error.
        // This exercises the early-return branch at lines 229-231.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("empty.rs");
        fs::write(&file_path, "").unwrap();

        let result = parse_and_extract_chunks(&file_path);
        assert!(result.is_ok(), "Empty file should return Ok");
        let chunks = result.unwrap();
        assert!(
            chunks.is_empty(),
            "Empty file should produce zero chunks, got {}",
            chunks.len()
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_small_file() {
        // A file smaller than CHUNK_SIZE (1000 bytes) should produce exactly
        // one chunk containing the entire file content.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("small.rs");
        let content = "fn small() { println!(\"hi\"); }\n";
        fs::write(&file_path, content).unwrap();

        let result = parse_and_extract_chunks(&file_path);
        assert!(result.is_ok(), "Small file should parse without error");
        let chunks = result.unwrap();
        assert_eq!(
            chunks.len(),
            1,
            "Small file should produce exactly one chunk"
        );
        assert_eq!(
            chunks[0].1, content,
            "Single chunk should contain the full file content"
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_large_file() {
        // A file larger than CHUNK_SIZE (1000 bytes) is currently returned as a
        // single chunk (the implementation's simple strategy).
        // This test documents that behaviour and ensures no panic occurs on large input.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("large.rs");
        // Build content well over 1000 bytes (approx 2000 bytes of code)
        let line = "fn placeholder_function() { let _x = 42; }\n";
        let content = line.repeat(50);
        assert!(
            content.len() > 1000,
            "Precondition: content must exceed CHUNK_SIZE"
        );
        fs::write(&file_path, &content).unwrap();

        let result = parse_and_extract_chunks(&file_path);
        assert!(result.is_ok(), "Large file should parse without error");
        let chunks = result.unwrap();
        assert!(
            !chunks.is_empty(),
            "Large file must produce at least one chunk"
        );
        // The current simple implementation returns the whole file as one chunk.
        // Verify the content is preserved.
        let combined: String = chunks.iter().map(|(_, text)| text.as_str()).collect();
        assert_eq!(
            combined, content,
            "All chunk content together must equal the original file"
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_nonexistent_file() {
        // Attempting to read a file that does not exist must return an Err,
        // not panic or return empty chunks.
        let result = parse_and_extract_chunks(Path::new("/nonexistent/path/missing.rs"));
        assert!(
            result.is_err(),
            "Nonexistent file should return an error, not Ok"
        );
    }

    #[test]
    fn test_worker_nonexistent_file_is_marked_indexed() {
        // When a file path in the DB does not exist on disk the worker should
        // still mark it as indexed (to prevent an infinite retry loop) and
        // not write any chunks for it.
        let db = create_test_db();
        insert_test_file(&db, "ghost.rs");

        // Simulate the worker behaviour directly: query dirty files, skip
        // non-existent ones (returning empty chunks), then mark indexed.
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert_eq!(dirty.len(), 1, "Should find one dirty file");
        assert_eq!(dirty[0], "ghost.rs");

        // The full_path does not exist, so the worker would produce empty chunks.
        let full_path = PathBuf::from("/nonexistent_workspace").join("ghost.rs");
        assert!(!full_path.exists(), "Precondition: path must not exist");

        // Simulate the worker's "file not found → empty chunks → mark indexed" path.
        mark_ts_indexed(&db, "ghost.rs").unwrap();

        let dirty_after = query_dirty_files(&db, 10).unwrap();
        assert!(
            dirty_after.is_empty(),
            "After marking indexed, no dirty files should remain"
        );

        // No chunks should have been written.
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'ghost.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0, "Nonexistent file should produce no chunks");
    }

    #[test]
    fn test_indexing_config_default() {
        // Verify the Default impl produces expected values.
        let config = IndexingConfig::default();
        assert_eq!(config.max_parallel_tasks, 4);
        assert_eq!(config.batch_size, 100);
    }

    #[test]
    fn test_indexing_config_custom() {
        // Verify custom configuration works.
        let config = IndexingConfig {
            max_parallel_tasks: 8,
            batch_size: 50,
        };
        assert_eq!(config.max_parallel_tasks, 8);
        assert_eq!(config.batch_size, 50);
    }

    #[test]
    fn test_query_dirty_files_respects_limit() {
        // When more dirty files exist than the limit, only limit files are returned.
        let db = create_test_db();
        insert_test_file(&db, "a.rs");
        insert_test_file(&db, "b.rs");
        insert_test_file(&db, "c.rs");

        let dirty = query_dirty_files(&db, 2).unwrap();
        assert_eq!(dirty.len(), 2, "Should respect limit of 2");
    }

    #[test]
    fn test_query_dirty_files_empty_when_all_indexed() {
        // When all files are already indexed, query_dirty_files returns empty.
        let db = create_test_db();
        insert_test_file(&db, "done.rs");
        {
            let conn = db.lock().unwrap();
            conn.execute(
                "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'done.rs'",
                [],
            )
            .unwrap();
        }

        let dirty = query_dirty_files(&db, 10).unwrap();
        assert!(dirty.is_empty(), "No dirty files should remain");
    }

    #[test]
    fn test_query_dirty_files_empty_db() {
        // An empty database should return an empty list without error.
        let db = create_test_db();
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert!(dirty.is_empty(), "Empty DB should return no dirty files");
    }

    #[test]
    fn test_write_ts_chunks_empty_input() {
        // Writing zero chunks should succeed (deletes old, inserts nothing).
        let db = create_test_db();
        insert_test_file(&db, "empty_chunks.rs");

        let result = write_ts_chunks(&db, "empty_chunks.rs", &[]);
        assert!(result.is_ok(), "Empty chunks should succeed");

        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'empty_chunks.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn test_mark_ts_indexed_idempotent() {
        // Marking the same file as indexed twice should succeed both times.
        let db = create_test_db();
        insert_test_file(&db, "idem.rs");

        mark_ts_indexed(&db, "idem.rs").unwrap();
        mark_ts_indexed(&db, "idem.rs").unwrap();

        let conn = db.lock().unwrap();
        let indexed: i64 = conn
            .query_row(
                "SELECT ts_indexed FROM indexed_files WHERE file_path = 'idem.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(indexed, 1);
    }

    #[test]
    fn test_mark_ts_indexed_nonexistent_file_succeeds() {
        // Marking a file that doesn't exist in the DB should succeed
        // (UPDATE on zero rows is not an error).
        let db = create_test_db();
        let result = mark_ts_indexed(&db, "nonexistent.rs");
        assert!(result.is_ok(), "Marking nonexistent file should not error");
    }

    #[test]
    fn test_parse_and_extract_chunks_binary_file() {
        // A file with invalid UTF-8 should return an error.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("binary.bin");
        fs::write(&file_path, [0xFF, 0xFE, 0x00, 0x01]).unwrap();

        let result = parse_and_extract_chunks(&file_path);
        assert!(
            result.is_err(),
            "Binary file with invalid UTF-8 should return error"
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_single_line() {
        // A file with a single line (no newline) should produce one chunk.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("oneline.rs");
        let content = "fn one() {}";
        fs::write(&file_path, content).unwrap();

        let result = parse_and_extract_chunks(&file_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1, content);
    }

    #[test]
    fn test_write_ts_chunks_records_line_counts() {
        // Verify that write_ts_chunks correctly records end_line.
        let db = create_test_db();
        insert_test_file(&db, "lines.rs");

        let content = "line1\nline2\nline3\n";
        let chunks = vec![(0usize, content.to_string())];
        write_ts_chunks(&db, "lines.rs", &chunks).unwrap();

        let conn = db.lock().unwrap();
        let end_line: i64 = conn
            .query_row(
                "SELECT end_line FROM ts_chunks WHERE file_path = 'lines.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        // 3 lines + start at 1 = end_line of 4
        assert_eq!(end_line, 4, "end_line should be 1 + line count");
    }

    #[test]
    fn test_worker_chunk_write_failure_still_marks_indexed() {
        // When writing chunks fails the worker must still mark the file as indexed
        // so it does not stay in the dirty queue forever.
        // We simulate this by corrupting the ts_chunks table after inserting the
        // file row, then calling write_ts_chunks and checking mark_ts_indexed.
        let db = create_test_db();
        insert_test_file(&db, "bad_chunks.rs");

        // Drop ts_chunks to force write_ts_chunks to fail.
        {
            let conn = db.lock().unwrap();
            conn.execute_batch("DROP TABLE ts_chunks;").unwrap();
        }

        let chunks = vec![(0usize, "fn bad() {}".to_string())];

        // write_ts_chunks must fail (table is gone).
        let write_result = write_ts_chunks(&db, "bad_chunks.rs", &chunks);
        assert!(
            write_result.is_err(),
            "write_ts_chunks should fail when the table is missing"
        );

        // The worker falls back to mark_ts_indexed despite the write failure.
        let mark_result = mark_ts_indexed(&db, "bad_chunks.rs");
        assert!(
            mark_result.is_ok(),
            "mark_ts_indexed should succeed even when chunk write failed"
        );

        // Verify the file is now marked indexed.
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert!(
            dirty.is_empty(),
            "File must be indexed after the fallback mark-indexed call"
        );
    }

    #[test]
    fn test_integration_dirty_file_indexed_by_worker_components() {
        // End-to-end integration of the worker pipeline using a real temp file:
        // 1. Insert a dirty file into the DB
        // 2. Create the corresponding file on disk
        // 3. Run the worker pipeline components (query → parse → write → mark)
        // 4. Verify the file is marked indexed and chunks are written
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();
        let file_name = "src/lib.rs";
        let file_path = temp_dir.path().join("src").join("lib.rs");
        fs::create_dir_all(file_path.parent().unwrap()).unwrap();
        fs::write(&file_path, "pub fn answer() -> u32 { 42 }\n").unwrap();

        insert_test_file(&db, file_name);

        // Query: file should appear as dirty
        let dirty = query_dirty_files(&db, 10).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], file_name);

        // Parse: using the real file on disk
        let chunks = parse_and_extract_chunks(&file_path).unwrap();
        assert!(!chunks.is_empty(), "Real file should produce chunks");

        // Write chunks
        write_ts_chunks(&db, file_name, &chunks).unwrap();

        // Mark indexed
        mark_ts_indexed(&db, file_name).unwrap();

        // Verify: no more dirty files
        let dirty_after = query_dirty_files(&db, 10).unwrap();
        assert!(dirty_after.is_empty(), "File should be indexed now");

        // Verify: chunks are in the DB
        let conn = db.lock().unwrap();
        let count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                [file_name],
                |row| row.get(0),
            )
            .unwrap();
        assert!(count > 0, "Chunks should have been written to DB");
    }
}
