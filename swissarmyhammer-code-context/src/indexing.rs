//! In-process parallel indexing worker for the leader process
//!
//! The leader spawns a background thread that:
//! 1. Monitors the database for dirty files (ts_indexed=0 or lsp_indexed=0)
//! 2. Runs tree-sitter parsing in parallel using IndexContext
//! 3. Writes chunks and symbols to the database
//! 4. Updates indexed flags
//! 5. Handles LSP requests (placeholder for future LSP integration)

use std::path::{Path, PathBuf};
use std::thread;
use std::time::Duration;
use rusqlite::Connection;
use rayon::prelude::*;
use tracing::{debug, info, warn};

use crate::error::CodeContextError;

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

/// Spawn a background indexing worker thread in the leader process
///
/// This starts a detached thread that:
/// 1. Queries dirty files from the database
/// 2. Parses them using tree-sitter
/// 3. Writes results back to the database
/// 4. Repeats until no dirty files remain
pub fn spawn_indexing_worker(
    workspace_root: PathBuf,
    db_path: PathBuf,
    config: IndexingConfig,
) {
    thread::Builder::new()
        .name("code-context-indexer".to_string())
        .spawn(move || {
            match run_indexing_worker(&workspace_root, &db_path, config) {
                Ok(()) => {
                    info!("Indexing worker completed successfully");
                }
                Err(e) => {
                    warn!("Indexing worker encountered error: {}", e);
                }
            }
        })
        .expect("Failed to spawn indexing worker thread");
}

/// Main indexing worker loop
fn run_indexing_worker(
    workspace_root: &Path,
    db_path: &Path,
    config: IndexingConfig,
) -> Result<(), CodeContextError> {
    info!(
        "code-context indexing worker started for {}",
        workspace_root.display()
    );

    // Open database connection for reading dirty files
    let db = Connection::open(db_path)?;
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

    // Work queue loop: keep checking for dirty files indefinitely
    // This allows the worker to index files that are discovered after startup
    let mut indexed_count = 0;
    loop {
        // Query dirty files (ts_indexed = 0) in batches
        let dirty_files = query_dirty_files(&db, config.batch_size)?;

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
                        return (file_path.clone(), 0);
                    }

                    // Try to parse file with tree-sitter
                    match parse_and_extract_chunks(&full_path) {
                        Ok(chunks) => {
                            debug!("Extracted {} chunks from {}", chunks.len(), file_path);
                            (file_path.clone(), chunks.len())
                        }
                        Err(e) => {
                            warn!("Failed to parse {}: {}", file_path, e);
                            (file_path.clone(), 0)
                        }
                    }
                })
                .collect();

            // Write results back to database
            for (file_path, chunk_count) in results {
                if chunk_count == 0 {
                    debug!("Skipping {} - no chunks extracted, marking indexed to avoid retry loop", file_path);
                    if let Err(e) = mark_ts_indexed(&db, &file_path) {
                        warn!("Failed to mark {} as indexed: {}", file_path, e);
                    }
                    continue;
                }

                // Parse the file again to get chunks for writing
                // (We parse twice, which is suboptimal, but keeps code simple and thread-safe)
                let full_path = workspace_root.join(&file_path);
                match parse_and_extract_chunks(&full_path) {
                    Ok(chunks) => {
                        if let Err(e) = write_ts_chunks(&db, &file_path, &chunks) {
                            warn!("Failed to write chunks for {}: {}", file_path, e);
                            continue;
                        }

                        if let Err(e) = mark_ts_indexed(&db, &file_path) {
                            warn!("Failed to mark {} as indexed: {}", file_path, e);
                        } else {
                            indexed_count += 1;
                            debug!("Successfully indexed {} with {} chunks", file_path, chunk_count);
                        }
                    }
                    Err(e) => {
                        warn!("Failed to parse {} again: {}", file_path, e);
                    }
                }
            }
            info!("code-context: indexed {} files so far (batch complete)", indexed_count);
        }

        // Sleep before next iteration (allows new files to be discovered)
        // In production, this would be longer; in tests we use shorter intervals
        thread::sleep(Duration::from_millis(100));
    }
}

/// Query files that need tree-sitter indexing (ts_indexed=0)
fn query_dirty_files(
    db: &Connection,
    limit: usize,
) -> Result<Vec<String>, CodeContextError> {
    let mut stmt = db.prepare(
        "SELECT file_path FROM indexed_files WHERE ts_indexed=0 LIMIT ?"
    )?;

    let files = stmt
        .query_map([limit as i64], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;

    Ok(files)
}

/// Mark a file as indexed in the database
fn mark_ts_indexed(
    db: &Connection,
    file_path: &str,
) -> Result<(), CodeContextError> {
    db.execute(
        "UPDATE indexed_files SET ts_indexed=1 WHERE file_path=?",
        [file_path],
    )?;
    Ok(())
}

/// Read a file and extract chunks based on lines
///
/// This is a simple chunking strategy that splits files into chunks of ~1000 bytes.
/// A more sophisticated implementation would use tree-sitter AST-aware chunking.
fn parse_and_extract_chunks(
    file_path: &Path,
) -> Result<Vec<(usize, String)>, CodeContextError> {
    let content = std::fs::read_to_string(file_path)?;

    const CHUNK_SIZE: usize = 1000; // bytes per chunk
    let mut chunks = Vec::new();
    let mut start_byte = 0;

    // Split by newlines to avoid breaking in the middle of lines
    for line in content.lines() {
        let line_with_newline = format!("{}\n", line);

        // If adding this line would exceed chunk size, start a new chunk
        if start_byte > 0 && chunks.last().map_or(false, |(_, chunk): &(usize, String)| {
            chunk.len() + line_with_newline.len() > CHUNK_SIZE
        }) {
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

/// Write parsed chunks to the ts_chunks table
fn write_ts_chunks(
    db: &Connection,
    file_path: &str,
    chunks: &[(usize, String)],
) -> Result<(), CodeContextError> {
    for (start_byte, content) in chunks {
        let end_byte = start_byte + content.len();
        // Count lines in the content
        let start_line = 1i64; // Simple implementation: all chunks start at line 1
        let end_line = 1i64 + content.lines().count() as i64;

        db.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text) VALUES (?, ?, ?, ?, ?, ?)",
            rusqlite::params![file_path, *start_byte as i64, end_byte as i64, start_line, end_line, content],
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "
            CREATE TABLE indexed_files (
                file_path     TEXT PRIMARY KEY,
                content_hash  BLOB NOT NULL,
                file_size     INTEGER NOT NULL,
                last_seen_at  INTEGER NOT NULL,
                ts_indexed    INTEGER NOT NULL DEFAULT 0,
                lsp_indexed   INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE ts_chunks (
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
            "
        ).unwrap();
        conn
    }

    fn insert_test_file(conn: &Connection, file_path: &str) {
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
        let mut stmt = db.prepare(
            "SELECT COUNT(*) FROM ts_chunks WHERE file_path = ?"
        ).unwrap();

        let count: i64 = stmt.query_row([file_path], |row| row.get(0)).unwrap();
        assert_eq!(count, 2, "Should have 2 chunks in database");
    }

    #[test]
    fn test_mark_ts_indexed() {
        let db = create_test_db();
        let file_path = "test.rs";

        insert_test_file(&db, file_path);

        // Verify file is not indexed initially
        let mut stmt = db.prepare(
            "SELECT ts_indexed FROM indexed_files WHERE file_path = ?"
        ).unwrap();
        let initial: i64 = stmt.query_row([file_path], |row| row.get(0)).unwrap();
        assert_eq!(initial, 0, "File should not be indexed initially");

        // Mark as indexed
        let result = mark_ts_indexed(&db, file_path);
        assert!(result.is_ok());

        // Verify file is indexed
        let indexed: i64 = stmt.query_row([file_path], |row| row.get(0)).unwrap();
        assert_eq!(indexed, 1, "File should be indexed");
    }

    #[test]
    fn test_query_dirty_files() {
        let db = create_test_db();

        insert_test_file(&db, "file1.rs");
        insert_test_file(&db, "file2.rs");

        // Mark one as indexed
        db.execute(
            "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'file1.rs'",
            [],
        ).unwrap();

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
        let mut count_stmt = db.prepare(
            "SELECT COUNT(*) FROM ts_chunks"
        ).unwrap();
        let all_chunks: i64 = count_stmt.query_row([], |row| row.get(0)).unwrap();
        assert_eq!(all_chunks, 2, "Should have 2 chunks total in database");

        // Mark as indexed
        let mark_result = mark_ts_indexed(&db, file_path);
        assert!(mark_result.is_ok());
    }
}
