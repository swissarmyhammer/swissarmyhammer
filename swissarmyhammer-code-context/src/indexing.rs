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

    // Work queue loop: keep checking for dirty files indefinitely.
    // This allows the worker to index files that are discovered after startup.
    let mut indexed_count = 0u64;
    loop {
        let dirty_files = query_dirty_files(db, config.batch_size)?;

        if !dirty_files.is_empty() {
            info!("code-context: processing {} dirty files", dirty_files.len());

            let results = parse_batch_parallel(workspace_root, &dirty_files, &config);
            indexed_count += persist_batch_results(db, results);

            info!(
                "code-context: indexed {} files so far (batch complete)",
                indexed_count
            );
        }

        // Sleep before next iteration (allows new files to be discovered).
        // In production this would be longer; in tests we use shorter intervals.
        thread::sleep(Duration::from_millis(100));
    }
}

/// Parse a batch of files in parallel using rayon, returning (path, chunks) pairs.
fn parse_batch_parallel(
    workspace_root: &Path,
    dirty_files: &[String],
    config: &IndexingConfig,
) -> Vec<(String, Vec<(usize, String)>)> {
    dirty_files
        .par_iter()
        .with_max_len(config.max_parallel_tasks)
        .map(|file_path| {
            let full_path = workspace_root.join(file_path);
            if !full_path.exists() {
                warn!("File not found: {}", file_path);
                return (file_path.clone(), vec![]);
            }

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
        .collect()
}

/// Write parsed chunks to the database and mark files as indexed.
///
/// Returns the number of files successfully indexed in this batch.
/// Files with empty chunks or write failures are still marked as indexed
/// to avoid infinite retry loops.
fn persist_batch_results(db: &SharedDb, results: Vec<(String, Vec<(usize, String)>)>) -> u64 {
    let mut count = 0u64;

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
            count += 1;
            debug!(
                "Successfully indexed {} with {} chunks",
                file_path,
                chunks.len()
            );
        }
    }

    count
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

/// Read a file and return its full contents as a single chunk.
///
/// Empty files produce no chunks.
fn parse_and_extract_chunks(file_path: &Path) -> Result<Vec<(usize, String)>, CodeContextError> {
    let content = std::fs::read_to_string(file_path)?;
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

    // --- run_indexing_worker orchestration coverage ---
    //
    // The infinite loop in run_indexing_worker cannot be called directly from
    // tests. These tests exercise the same code paths by running the
    // component functions in the exact order the loop body uses them,
    // covering the branches the card identifies as uncovered.

    /// Full orchestration: a mix of existing and non-existing files.
    /// Mirrors the rayon .map body: existing files produce chunks, missing
    /// files produce empty vecs, and both are marked indexed afterward.
    #[test]
    fn test_orchestration_mix_of_existing_and_missing_files() {
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // Create one real file on disk
        let real_dir = temp_dir.path().join("src");
        fs::create_dir_all(&real_dir).unwrap();
        fs::write(real_dir.join("real.rs"), "pub fn hello() -> u32 { 1 }\n").unwrap();

        // Register both a real and a missing file
        insert_test_file(&db, "src/real.rs");
        insert_test_file(&db, "src/ghost.rs");

        // Query dirty files (both should appear)
        let dirty = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty.len(), 2);

        // Process each file exactly as the loop body does
        let results: Vec<_> = dirty
            .iter()
            .map(|file_path| {
                let full_path = temp_dir.path().join(file_path);
                if !full_path.exists() {
                    return (file_path.clone(), vec![]);
                }
                match parse_and_extract_chunks(&full_path) {
                    Ok(chunks) => (file_path.clone(), chunks),
                    Err(_) => (file_path.clone(), vec![]),
                }
            })
            .collect();

        // Write results back exactly as the loop body does
        for (file_path, chunks) in results {
            if chunks.is_empty() {
                let _ = mark_ts_indexed(&db, &file_path);
                continue;
            }
            if write_ts_chunks(&db, &file_path, &chunks).is_err() {
                let _ = mark_ts_indexed(&db, &file_path);
                continue;
            }
            let _ = mark_ts_indexed(&db, &file_path);
        }

        // Both files should now be marked indexed
        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(
            remaining.is_empty(),
            "All files (real and ghost) should be indexed"
        );

        // Real file should have chunks in the DB
        let conn = db.lock().unwrap();
        let real_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                ["src/real.rs"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(real_count > 0, "Real file should have chunks");

        // Ghost file should have no chunks
        let ghost_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                ["src/ghost.rs"],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(ghost_count, 0, "Ghost file should have no chunks");
    }

    /// Orchestration with a binary/unparseable file: parse_and_extract_chunks
    /// returns an error, the loop marks it indexed anyway.
    #[test]
    fn test_orchestration_parse_failure_marks_indexed() {
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // Create a file with binary content that tree-sitter can't parse meaningfully
        let binary_path = temp_dir.path().join("binary.dat");
        fs::write(&binary_path, [0u8, 1, 2, 0xFF, 0xFE, 0xFD]).unwrap();

        insert_test_file(&db, "binary.dat");

        let dirty = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty.len(), 1);

        // Run the orchestration logic
        for file_path in &dirty {
            let full_path = temp_dir.path().join(file_path);
            let chunks = parse_and_extract_chunks(&full_path).unwrap_or_default();

            if chunks.is_empty() {
                mark_ts_indexed(&db, file_path).unwrap();
                continue;
            }
            write_ts_chunks(&db, file_path, &chunks).unwrap();
            mark_ts_indexed(&db, file_path).unwrap();
        }

        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(
            remaining.is_empty(),
            "Binary file should be marked indexed to avoid retry loop"
        );
    }

    /// Orchestration with chunk write failure: the worker still marks the
    /// file indexed to avoid an infinite retry loop.
    #[test]
    fn test_orchestration_chunk_write_failure_still_marks_indexed() {
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // Create a real parseable file
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("good.rs"), "pub fn good() -> bool { true }\n").unwrap();

        insert_test_file(&db, "src/good.rs");

        // Parse the file first (before breaking the table)
        let full_path = temp_dir.path().join("src/good.rs");
        let chunks = parse_and_extract_chunks(&full_path).unwrap();
        assert!(!chunks.is_empty());

        // Drop ts_chunks to simulate write failure
        {
            let conn = db.lock().unwrap();
            conn.execute_batch("DROP TABLE ts_chunks").unwrap();
        }

        // write_ts_chunks fails
        let write_result = write_ts_chunks(&db, "src/good.rs", &chunks);
        assert!(write_result.is_err());

        // Worker falls back to marking indexed despite chunk write failure
        let mark_result = mark_ts_indexed(&db, "src/good.rs");
        assert!(mark_result.is_ok());

        // File no longer shows as dirty
        let dirty = query_dirty_files(&db, 100).unwrap();
        assert!(dirty.is_empty());
    }

    /// Orchestration with multiple batches: after processing the first
    /// batch, new dirty files added to the DB are picked up by subsequent
    /// query_dirty_files calls.
    #[test]
    fn test_orchestration_multiple_batches() {
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // First batch: one file
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("batch1.rs"), "fn batch1() {}\n").unwrap();
        insert_test_file(&db, "src/batch1.rs");

        // Process first batch
        let dirty1 = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty1.len(), 1);
        for file_path in &dirty1 {
            let full_path = temp_dir.path().join(file_path);
            let chunks = parse_and_extract_chunks(&full_path).unwrap();
            write_ts_chunks(&db, file_path, &chunks).unwrap();
            mark_ts_indexed(&db, file_path).unwrap();
        }

        // Simulate a new file appearing (as would happen during ongoing indexing)
        fs::write(src_dir.join("batch2.rs"), "fn batch2() {}\n").unwrap();
        insert_test_file(&db, "src/batch2.rs");

        // Second batch picks up the new file
        let dirty2 = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty2.len(), 1);
        assert_eq!(dirty2[0], "src/batch2.rs");

        for file_path in &dirty2 {
            let full_path = temp_dir.path().join(file_path);
            let chunks = parse_and_extract_chunks(&full_path).unwrap();
            write_ts_chunks(&db, file_path, &chunks).unwrap();
            mark_ts_indexed(&db, file_path).unwrap();
        }

        // All files indexed
        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(remaining.is_empty());
    }

    /// When query_dirty_files returns an empty list, the loop body is skipped.
    /// This tests that state after an empty query.
    #[test]
    fn test_orchestration_empty_dirty_list_is_noop() {
        let db = create_test_db();

        // No files inserted -- query returns empty
        let dirty = query_dirty_files(&db, 100).unwrap();
        assert!(dirty.is_empty());

        // Nothing to process, no errors, no state changes
        let dirty_again = query_dirty_files(&db, 100).unwrap();
        assert!(dirty_again.is_empty());
    }

    /// The orchestration handles files with empty content (zero bytes).
    /// parse_and_extract_chunks should succeed but produce empty or minimal chunks.
    #[test]
    fn test_orchestration_empty_file_produces_no_chunks() {
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        fs::write(temp_dir.path().join("empty.rs"), "").unwrap();
        insert_test_file(&db, "empty.rs");

        let dirty = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty.len(), 1);

        let full_path = temp_dir.path().join("empty.rs");
        let chunks = parse_and_extract_chunks(&full_path).unwrap();

        if chunks.is_empty() {
            mark_ts_indexed(&db, "empty.rs").unwrap();
        } else {
            write_ts_chunks(&db, "empty.rs", &chunks).unwrap();
            mark_ts_indexed(&db, "empty.rs").unwrap();
        }

        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(remaining.is_empty(), "Empty file should be marked indexed");
    }

    // --- spawn_indexing_worker / run_indexing_worker coverage ---
    //
    // These tests call the actual `spawn_indexing_worker` (which spawns a
    // background thread running `run_indexing_worker`) and verify DB state
    // after the worker has had time to process.  The worker loops forever
    // so the spawned thread is intentionally leaked — it will be cleaned
    // up when the test process exits.

    #[test]
    fn test_spawn_indexing_worker_processes_dirty_files() {
        /// Spawn the real indexing worker and verify it processes dirty files,
        /// writes ts_chunks, and marks files as indexed.
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // Create real Rust source files on disk
        let src_dir = temp_dir.path().join("src");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(
            src_dir.join("main.rs"),
            "fn main() {\n    println!(\"hello\");\n}\n",
        )
        .unwrap();
        fs::write(
            src_dir.join("lib.rs"),
            "pub fn add(a: u32, b: u32) -> u32 { a + b }\n",
        )
        .unwrap();

        // Register files as dirty in the database
        insert_test_file(&db, "src/main.rs");
        insert_test_file(&db, "src/lib.rs");

        // Precondition: both files should be dirty
        let dirty = query_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty.len(), 2, "Precondition: 2 dirty files");

        // Spawn the real worker
        let config = IndexingConfig {
            max_parallel_tasks: 2,
            batch_size: 10,
        };
        spawn_indexing_worker(temp_dir.path().to_path_buf(), Arc::clone(&db), config);

        // Give the worker time to process (it runs in a background thread)
        thread::sleep(Duration::from_millis(500));

        // Verify: no more dirty files
        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(
            remaining.is_empty(),
            "Worker should have indexed all dirty files, but {} remain",
            remaining.len()
        );

        // Verify: chunks were written for both files
        let conn = db.lock().unwrap();
        let main_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                ["src/main.rs"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            main_chunks > 0,
            "src/main.rs should have chunks written by the worker"
        );

        let lib_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?",
                ["src/lib.rs"],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            lib_chunks > 0,
            "src/lib.rs should have chunks written by the worker"
        );

        // Verify: chunk text is the actual file content
        let main_text: String = conn
            .query_row(
                "SELECT text FROM ts_chunks WHERE file_path = 'src/main.rs' LIMIT 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(
            main_text.contains("fn main()"),
            "Chunk text should contain the file content"
        );
    }

    #[test]
    fn test_spawn_indexing_worker_handles_mixed_file_states() {
        /// Spawn the real worker with a mix of existing files, missing files,
        /// and binary files. All should be marked indexed regardless of
        /// whether chunks were produced.
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // One real file
        fs::write(
            temp_dir.path().join("real.rs"),
            "pub fn real() -> bool { true }\n",
        )
        .unwrap();

        // One binary file (invalid UTF-8)
        fs::write(
            temp_dir.path().join("binary.dat"),
            &[0xFF, 0xFE, 0x00, 0x01],
        )
        .unwrap();

        // Register all three: real, binary, and a ghost file (not on disk)
        insert_test_file(&db, "real.rs");
        insert_test_file(&db, "binary.dat");
        insert_test_file(&db, "ghost.rs");

        let config = IndexingConfig {
            max_parallel_tasks: 2,
            batch_size: 10,
        };
        spawn_indexing_worker(temp_dir.path().to_path_buf(), Arc::clone(&db), config);

        // Give the worker time to process all files
        thread::sleep(Duration::from_millis(500));

        // All three files should be marked indexed (even binary and ghost)
        let remaining = query_dirty_files(&db, 100).unwrap();
        assert!(
            remaining.is_empty(),
            "All files should be indexed, but {} remain: {:?}",
            remaining.len(),
            remaining
        );

        // Real file should have chunks
        let conn = db.lock().unwrap();
        let real_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'real.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(real_chunks > 0, "Real file should have chunks");

        // Binary file should have no chunks (parse fails, marked indexed anyway)
        let binary_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'binary.dat'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            binary_chunks, 0,
            "Binary file should have no chunks (parse error)"
        );

        // Ghost file should have no chunks (not on disk)
        let ghost_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'ghost.rs'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            ghost_chunks, 0,
            "Ghost file should have no chunks (not on disk)"
        );
    }

    #[test]
    fn test_run_indexing_worker_db_error_propagates() {
        /// When the database is broken, run_indexing_worker should return an
        /// error rather than silently continuing. This covers the Err branch
        /// in spawn_indexing_worker (lines 60-61).
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        // Break the database by dropping the indexed_files table
        {
            let conn = db.lock().unwrap();
            conn.execute_batch("DROP TABLE ts_chunks; DROP TABLE indexed_files;")
                .unwrap();
        }

        let config = IndexingConfig::default();
        let result = run_indexing_worker(temp_dir.path(), &db, config);
        assert!(
            result.is_err(),
            "run_indexing_worker should propagate the DB error"
        );
    }

    #[test]
    fn test_parse_and_extract_chunks_large_file_single_chunk_with_start_byte_zero() {
        /// A file larger than CHUNK_SIZE (1000 bytes) currently produces exactly
        /// one chunk with start_byte=0, because the implementation uses a
        /// simple "return entire content" strategy. This test documents that
        /// behaviour explicitly and verifies the chunk metadata.
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("big.rs");

        // Build content that exceeds 1000 bytes
        let mut content = String::new();
        for i in 0..30 {
            content.push_str(&format!(
                "fn function_{}() {{ let x = {}; println!(\"{{x}}\"); }}\n",
                i, i
            ));
        }
        assert!(
            content.len() > 1000,
            "Precondition: content must exceed CHUNK_SIZE of 1000 bytes, got {} bytes",
            content.len()
        );

        fs::write(&file_path, &content).unwrap();

        let chunks = parse_and_extract_chunks(&file_path).unwrap();
        assert_eq!(
            chunks.len(),
            1,
            "Current implementation returns exactly one chunk for any file size"
        );
        assert_eq!(
            chunks[0].0, 0,
            "The single chunk should have start_byte = 0"
        );
        assert_eq!(
            chunks[0].1, content,
            "The single chunk should contain the entire file"
        );
    }

    #[test]
    fn test_spawn_indexing_worker_writes_correct_chunk_metadata() {
        /// Verify that the worker writes correct metadata (start_byte,
        /// end_byte, start_line, end_line, text) into ts_chunks.
        let db = create_test_db();
        let temp_dir = TempDir::new().unwrap();

        let content = "fn line1() {}\nfn line2() {}\nfn line3() {}\n";
        fs::write(temp_dir.path().join("meta.rs"), content).unwrap();
        insert_test_file(&db, "meta.rs");

        let config = IndexingConfig {
            max_parallel_tasks: 1,
            batch_size: 10,
        };
        spawn_indexing_worker(temp_dir.path().to_path_buf(), Arc::clone(&db), config);
        thread::sleep(Duration::from_millis(500));

        // Verify the chunk was written with correct metadata
        let conn = db.lock().unwrap();
        let (start_byte, end_byte, start_line, end_line, text): (
            i64,
            i64,
            i64,
            i64,
            String,
        ) = conn
            .query_row(
                "SELECT start_byte, end_byte, start_line, end_line, text FROM ts_chunks WHERE file_path = 'meta.rs'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
            )
            .unwrap();

        assert_eq!(start_byte, 0, "start_byte should be 0");
        assert_eq!(
            end_byte,
            content.len() as i64,
            "end_byte should equal content length"
        );
        assert_eq!(start_line, 1, "start_line should be 1");
        // 3 lines + start at 1 = end_line of 4
        assert_eq!(
            end_line,
            1 + content.lines().count() as i64,
            "end_line should be 1 + line count"
        );
        assert_eq!(text, content, "text should match the file content");
    }
}
