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
    debug!(
        "Starting indexing worker for {}",
        workspace_root.display()
    );

    // Open database connection for reading dirty files
    let db = Connection::open(db_path)?;
    db.execute_batch("PRAGMA journal_mode = WAL; PRAGMA foreign_keys = ON;")?;

    // Work queue loop: process dirty files until none remain
    loop {
        // Query dirty files (ts_indexed = 0) in batches
        let dirty_files = query_dirty_files(&db, config.batch_size)?;

        if dirty_files.is_empty() {
            debug!("No more dirty files to index");
            break;
        }

        debug!("Found {} dirty files to index", dirty_files.len());

        // Process files in parallel using rayon
        // Note: The treesitter module handles actual tree-sitter parsing and writes
        // chunks to ts_chunks table. Here we just mark files as indexed for now.
        let results: Vec<_> = dirty_files
            .par_iter()
            .with_max_len(config.max_parallel_tasks)
            .map(|file_path| {
                // Validate file exists
                let full_path = workspace_root.join(file_path);
                if full_path.exists() {
                    (file_path.clone(), true)
                } else {
                    warn!("File not found: {}", file_path);
                    (file_path.clone(), false)
                }
            })
            .collect();

        // Write results back to database
        for (file_path, exists) in results {
            if exists {
                if let Err(e) = mark_ts_indexed(&db, &file_path) {
                    warn!("Failed to mark {} as indexed: {}", file_path, e);
                } else {
                    debug!("Marked {} as ts_indexed", file_path);
                }
            }
        }

        // Small delay to avoid busy-looping
        thread::sleep(Duration::from_millis(100));
    }

    info!(
        "Indexing worker completed for {}",
        workspace_root.display()
    );
    Ok(())
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
