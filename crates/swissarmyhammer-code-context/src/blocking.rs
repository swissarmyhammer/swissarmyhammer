//! Blocking-readiness checks for the code context index.
//!
//! Provides a function to check whether indexing is complete by comparing
//! indexed counts vs total file counts. This is a query, not an actual
//! blocking wait -- callers decide what to do with the result.

use rusqlite::Connection;

use crate::error::CodeContextError;

/// Whether the index is ready for queries on a given layer.
#[derive(Debug, Clone, serde::Serialize)]
pub enum BlockingStatus {
    /// The layer is fully indexed and ready for queries.
    Ready,
    /// The layer is still being indexed.
    NotReady {
        /// Total number of tracked files.
        total_files: u64,
        /// Number of files indexed for this layer.
        indexed_files: u64,
        /// Progress as a percentage (0.0 to 100.0).
        progress_percent: f64,
    },
}

/// The indexing layer to check readiness for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub enum IndexLayer {
    /// Tree-sitter chunking layer (`ts_indexed` column).
    TreeSitter,
    /// LSP symbol/edge layer (`lsp_indexed` column).
    Lsp,
    /// Per-file embedding completeness layer (`embedded` column).
    ///
    /// `embedded = 1` means every chunk for that file has had an embedding
    /// blob populated. This is independent of `ts_indexed`, which only
    /// signals that chunks have been written.
    Embedding,
}

/// Check whether indexing is complete for the specified layer.
///
/// Compares the count of files where the layer's indexed flag is set
/// against the total number of tracked files. Returns `Ready` when all
/// files are indexed, `NotReady` with progress info otherwise.
///
/// Always returns immediately -- never blocks.
///
/// # Arguments
///
/// * `conn` - A reference to the SQLite connection.
/// * `layer` - Which indexing layer to check.
///
/// # Errors
///
/// Returns [`CodeContextError::Database`] on SQLite failures.
pub fn check_blocking_status(
    conn: &Connection,
    layer: IndexLayer,
) -> Result<BlockingStatus, CodeContextError> {
    let total_files: i64 =
        conn.query_row("SELECT COUNT(*) FROM indexed_files", [], |r| r.get(0))?;

    if total_files == 0 {
        return Ok(BlockingStatus::Ready);
    }

    // Each layer maps to a hardcoded full SQL string. We deliberately do
    // not interpolate the column name into a SQL fragment — the match
    // returns a complete, audited literal so there is no path for
    // untrusted data to influence the query.
    let sql = match layer {
        IndexLayer::TreeSitter => "SELECT COUNT(*) FROM indexed_files WHERE ts_indexed = 1",
        IndexLayer::Lsp => "SELECT COUNT(*) FROM indexed_files WHERE lsp_indexed = 1",
        IndexLayer::Embedding => "SELECT COUNT(*) FROM indexed_files WHERE embedded = 1",
    };

    let indexed_files: i64 = conn.query_row(sql, [], |r| r.get(0))?;

    if indexed_files >= total_files {
        Ok(BlockingStatus::Ready)
    } else {
        let progress_percent = (indexed_files as f64 / total_files as f64) * 100.0;
        Ok(BlockingStatus::NotReady {
            total_files: total_files as u64,
            indexed_files: indexed_files as u64,
            progress_percent,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_fixtures::{insert_file, test_db};

    #[test]
    fn test_empty_db_is_ready() {
        let conn = test_db();
        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        assert!(matches!(status, BlockingStatus::Ready));
    }

    #[test]
    fn test_all_indexed_is_ready() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 1);

        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        assert!(matches!(status, BlockingStatus::Ready));

        let status = check_blocking_status(&conn, IndexLayer::Lsp).unwrap();
        assert!(matches!(status, BlockingStatus::Ready));
    }

    #[test]
    fn test_partial_indexed_is_not_ready() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 0);
        insert_file(&conn, "b.rs", 0, 0);

        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        match status {
            BlockingStatus::NotReady {
                total_files,
                indexed_files,
                progress_percent,
            } => {
                assert_eq!(total_files, 2);
                assert_eq!(indexed_files, 1);
                assert!((progress_percent - 50.0).abs() < 0.01);
            }
            BlockingStatus::Ready => panic!("expected NotReady"),
        }
    }

    #[test]
    fn test_embedding_layer_ready_when_all_embedded() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 1);
        conn.execute("UPDATE indexed_files SET embedded = 1", [])
            .unwrap();

        let status = check_blocking_status(&conn, IndexLayer::Embedding).unwrap();
        assert!(matches!(status, BlockingStatus::Ready));
    }

    #[test]
    fn test_embedding_layer_not_ready_when_partial() {
        let conn = test_db();
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 1);
        insert_file(&conn, "c.rs", 1, 1);
        // Only one file has been embedded.
        conn.execute(
            "UPDATE indexed_files SET embedded = 1 WHERE file_path = 'a.rs'",
            [],
        )
        .unwrap();

        let status = check_blocking_status(&conn, IndexLayer::Embedding).unwrap();
        match status {
            BlockingStatus::NotReady {
                total_files,
                indexed_files,
                progress_percent,
            } => {
                assert_eq!(total_files, 3);
                assert_eq!(indexed_files, 1);
                assert!((progress_percent - (100.0 / 3.0)).abs() < 0.01);
            }
            BlockingStatus::Ready => panic!("expected NotReady for embedding layer"),
        }
    }

    #[test]
    fn test_embedding_layer_not_ready_when_none_embedded() {
        let conn = test_db();
        // ts_indexed = 1 but embedded defaults to 0 — this is exactly the
        // "indexed but not yet embedded" state we want to detect.
        insert_file(&conn, "a.rs", 1, 1);
        insert_file(&conn, "b.rs", 1, 1);

        let status = check_blocking_status(&conn, IndexLayer::Embedding).unwrap();
        match status {
            BlockingStatus::NotReady {
                total_files,
                indexed_files,
                progress_percent,
            } => {
                assert_eq!(total_files, 2);
                assert_eq!(indexed_files, 0);
                assert!(progress_percent.abs() < 0.01);
            }
            BlockingStatus::Ready => panic!("expected NotReady when no files are embedded"),
        }
    }

    #[test]
    fn test_blocking_waits_until_ts_indexed_matches_total() {
        let conn = test_db();
        // Start with 3 files, none indexed
        insert_file(&conn, "a.rs", 0, 0);
        insert_file(&conn, "b.rs", 0, 0);
        insert_file(&conn, "c.rs", 0, 0);

        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        assert!(matches!(status, BlockingStatus::NotReady { .. }));

        // Simulate indexing progress
        conn.execute(
            "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'a.rs'",
            [],
        )
        .unwrap();
        conn.execute(
            "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'b.rs'",
            [],
        )
        .unwrap();

        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        assert!(matches!(status, BlockingStatus::NotReady { .. }));

        // Index the last file
        conn.execute(
            "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path = 'c.rs'",
            [],
        )
        .unwrap();

        let status = check_blocking_status(&conn, IndexLayer::TreeSitter).unwrap();
        assert!(matches!(status, BlockingStatus::Ready));
    }
}
