//! File system watcher for keeping code-context DB in sync with filesystem.
//!
//! Uses `async-watcher` (debounced `notify`) to watch the workspace for file
//! changes.  When files change:
//! 1. The `FanoutWatcher` marks affected rows dirty in `indexed_files`.
//! 2. A re-indexing pass processes the newly dirty files.
//!
//! The core batch-processing logic is extracted into [`process_file_events`]
//! so it can be tested with a real database without needing filesystem events.

use std::path::{Path, PathBuf};
use std::time::Duration;

use async_watcher::{notify::RecursiveMode, AsyncDebouncer};
use rusqlite::Connection;
use swissarmyhammer_code_context::{FanoutWatcher, FileEvent, SharedDb};

/// Source file extensions worth tracking for code context indexing.
const SOURCE_EXTENSIONS: &[&str] = &[
    "rs", "py", "js", "ts", "tsx", "jsx", "go", "java", "c", "cpp", "h", "hpp", "rb", "swift",
    "kt", "cs", "lua", "zig", "hs", "ml", "ex", "exs", "erl", "clj", "scala", "r", "jl", "toml",
    "yaml", "yml", "json", "sh", "bash", "zsh",
];

/// Check if a path is a source file we should index.
pub(crate) fn is_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Convert a `notify` event kind into our `FileEvent`, if relevant.
pub(crate) fn to_file_event(
    kind: &async_watcher::notify::EventKind,
    path: PathBuf,
    workspace_root: &Path,
) -> Option<FileEvent> {
    use async_watcher::notify::event::{CreateKind, ModifyKind, RemoveKind};
    use async_watcher::notify::EventKind;

    // Make path relative to workspace root for DB consistency
    let rel = path
        .strip_prefix(workspace_root)
        .unwrap_or(&path)
        .to_path_buf();

    match kind {
        EventKind::Create(CreateKind::File) | EventKind::Create(CreateKind::Any) => {
            Some(FileEvent::Created(rel))
        }
        EventKind::Modify(ModifyKind::Data(_))
        | EventKind::Modify(ModifyKind::Any)
        | EventKind::Modify(ModifyKind::Name(_)) => Some(FileEvent::Modified(rel)),
        EventKind::Remove(RemoveKind::File) | EventKind::Remove(RemoveKind::Any) => {
            Some(FileEvent::Deleted(rel))
        }
        _ => None,
    }
}

/// Result of processing a batch of file events.
#[derive(Debug, Default)]
pub(crate) struct ProcessResult {
    /// Number of files marked dirty (ts_indexed/lsp_indexed set to 0).
    pub dirty_count: usize,
    /// Number of files deleted from the index.
    pub deleted_count: usize,
    /// Number of events that failed to update the DB.
    pub error_count: usize,
}

/// Process a batch of file events: mark files dirty or delete them in the DB.
///
/// This is the core logic extracted from the watcher loop so it can be
/// tested with a real database connection without needing filesystem events.
///
/// Returns counts of dirty-marked, deleted, and errored files.
pub(crate) fn process_file_events(
    conn: &Connection,
    watcher: &FanoutWatcher,
    events: &[FileEvent],
) -> ProcessResult {
    let mut result = ProcessResult::default();
    for event in events {
        match watcher.notify(conn, event) {
            Ok(rows) => match event {
                FileEvent::Deleted(_) => result.deleted_count += rows,
                FileEvent::Created(_) | FileEvent::Modified(_) => result.dirty_count += rows,
            },
            Err(e) => {
                tracing::warn!(
                    "code-context watcher: failed to process {:?}: {}",
                    event.path(),
                    e
                );
                result.error_count += 1;
            }
        }
    }
    result
}

/// Start watching the workspace for file changes.
///
/// Uses the leader's shared write connection for all DB operations.
/// Spawns a background tokio task that:
/// 1. Watches `workspace_root` recursively with a 1-second debounce
/// 2. Converts notify events to `FileEvent`s
/// 3. Calls [`process_file_events`] to mark DB rows dirty
/// 4. Triggers re-indexing of dirty files
///
/// Returns the `JoinHandle` for the watcher task.
pub fn start_code_context_watcher(
    workspace_root: PathBuf,
    db: SharedDb,
) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        if let Err(e) = run_watcher(&workspace_root, &db).await {
            tracing::error!("code-context watcher failed: {}", e);
        }
    })
}

async fn run_watcher(
    workspace_root: &Path,
    db: &SharedDb,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let (mut debouncer, mut event_rx) =
        AsyncDebouncer::new_with_channel(Duration::from_secs(1), None).await?;

    debouncer
        .watcher()
        .watch(workspace_root, RecursiveMode::Recursive)?;

    tracing::info!(
        "code-context: file watcher started for {}",
        workspace_root.display()
    );

    let fanout = FanoutWatcher::new();
    let ws_root = workspace_root.to_path_buf();

    while let Some(events_result) = event_rx.recv().await {
        match events_result {
            Ok(debounced_events) => {
                // Collect file events from the batch
                let mut file_events = Vec::new();
                for debounced in &debounced_events {
                    for path in &debounced.event.paths {
                        if !is_source_file(path) {
                            continue;
                        }
                        if let Some(event) =
                            to_file_event(&debounced.event.kind, path.clone(), &ws_root)
                        {
                            file_events.push(event);
                        }
                    }
                }

                if file_events.is_empty() {
                    continue;
                }

                tracing::info!(
                    "code-context: {} file change(s) detected, marking dirty",
                    file_events.len()
                );

                // Lock DB and process events
                {
                    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                    let result = process_file_events(&conn, &fanout, &file_events);
                    tracing::info!(
                        "code-context watcher: {} dirty, {} deleted, {} errors",
                        result.dirty_count,
                        result.deleted_count,
                        result.error_count,
                    );
                }

                // Re-index dirty files using the shared connection
                super::index_discovered_files_async(&ws_root, std::sync::Arc::clone(db)).await;
            }
            Err(errors) => {
                for error in errors {
                    tracing::warn!("code-context watcher error: {}", error);
                }
            }
        }
    }

    tracing::info!("code-context: file watcher stopped");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use swissarmyhammer_code_context::db::{configure_connection, create_schema};

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Create an in-memory DB with the full code-context schema.
    fn test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        conn
    }

    /// Insert a fully-indexed file row into the DB.
    fn insert_indexed_file(conn: &Connection, path: &str) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES (?1, X'AABBCCDD', 1024, 1000, 1, 1)",
            [path],
        )
        .unwrap();
    }

    /// Insert a file with associated chunks and symbols to test cascade.
    fn insert_file_with_data(conn: &Connection, path: &str) {
        insert_indexed_file(conn, path);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text)
             VALUES (?1, 0, 50, 1, 5, 'fn example() {}')",
            [path],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char)
             VALUES (?1, 'example', 12, ?2, 1, 0, 5, 1)",
            [&format!("lsp:{path}:example"), path],
        )
        .unwrap();
    }

    /// Query the ts_indexed flag for a file.
    fn get_ts_indexed(conn: &Connection, path: &str) -> Option<i64> {
        conn.query_row(
            "SELECT ts_indexed FROM indexed_files WHERE file_path = ?1",
            [path],
            |r| r.get(0),
        )
        .ok()
    }

    /// Query the lsp_indexed flag for a file.
    fn get_lsp_indexed(conn: &Connection, path: &str) -> Option<i64> {
        conn.query_row(
            "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
            [path],
            |r| r.get(0),
        )
        .ok()
    }

    /// Count rows in a table.
    fn count_rows(conn: &Connection, table: &str) -> i64 {
        conn.query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    /// Check if a file exists in indexed_files.
    fn file_exists(conn: &Connection, path: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM indexed_files WHERE file_path = ?1",
            [path],
            |r| r.get::<_, i64>(0),
        )
        .unwrap()
            > 0
    }

    // -----------------------------------------------------------------------
    // Unit tests: pure functions
    // -----------------------------------------------------------------------

    #[test]
    fn test_is_source_file() {
        assert!(is_source_file(Path::new("src/main.rs")));
        assert!(is_source_file(Path::new("lib.py")));
        assert!(is_source_file(Path::new("deep/path/to/file.ts")));
        assert!(!is_source_file(Path::new("image.png")));
        assert!(!is_source_file(Path::new("binary")));
        assert!(!is_source_file(Path::new("Makefile")));
    }

    #[test]
    fn test_to_file_event_create() {
        use async_watcher::notify::event::CreateKind;
        use async_watcher::notify::EventKind;

        let root = PathBuf::from("/workspace");
        let path = PathBuf::from("/workspace/src/main.rs");
        let event = to_file_event(&EventKind::Create(CreateKind::File), path, &root);

        match event.unwrap() {
            FileEvent::Created(p) => assert_eq!(p, PathBuf::from("src/main.rs")),
            _ => panic!("expected Created"),
        }
    }

    #[test]
    fn test_to_file_event_modify() {
        use async_watcher::notify::event::{DataChange, ModifyKind};
        use async_watcher::notify::EventKind;

        let root = PathBuf::from("/workspace");
        let path = PathBuf::from("/workspace/src/lib.rs");
        let event = to_file_event(
            &EventKind::Modify(ModifyKind::Data(DataChange::Content)),
            path,
            &root,
        );

        match event.unwrap() {
            FileEvent::Modified(p) => assert_eq!(p, PathBuf::from("src/lib.rs")),
            _ => panic!("expected Modified"),
        }
    }

    #[test]
    fn test_to_file_event_delete() {
        use async_watcher::notify::event::RemoveKind;
        use async_watcher::notify::EventKind;

        let root = PathBuf::from("/workspace");
        let path = PathBuf::from("/workspace/src/old.rs");
        let event = to_file_event(&EventKind::Remove(RemoveKind::File), path, &root);

        match event.unwrap() {
            FileEvent::Deleted(p) => assert_eq!(p, PathBuf::from("src/old.rs")),
            _ => panic!("expected Deleted"),
        }
    }

    #[test]
    fn test_to_file_event_ignores_access() {
        use async_watcher::notify::event::AccessKind;
        use async_watcher::notify::EventKind;

        let root = PathBuf::from("/workspace");
        let path = PathBuf::from("/workspace/src/main.rs");
        let event = to_file_event(&EventKind::Access(AccessKind::Read), path, &root);
        assert!(event.is_none());
    }

    #[test]
    fn test_to_file_event_relative_path() {
        use async_watcher::notify::event::CreateKind;
        use async_watcher::notify::EventKind;

        let root = PathBuf::from("/deep/nested/workspace");
        let path = PathBuf::from("/deep/nested/workspace/src/handlers/auth.rs");
        let event = to_file_event(&EventKind::Create(CreateKind::File), path, &root);

        match event.unwrap() {
            FileEvent::Created(p) => assert_eq!(p, PathBuf::from("src/handlers/auth.rs")),
            _ => panic!("expected Created"),
        }
    }

    // -----------------------------------------------------------------------
    // Integration tests: process_file_events with real DB
    // -----------------------------------------------------------------------

    #[test]
    fn test_modify_event_marks_file_dirty() {
        let conn = test_db();
        insert_indexed_file(&conn, "src/main.rs");

        // File starts fully indexed
        assert_eq!(get_ts_indexed(&conn, "src/main.rs"), Some(1));
        assert_eq!(get_lsp_indexed(&conn, "src/main.rs"), Some(1));

        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Modified(PathBuf::from("src/main.rs"))];
        let result = process_file_events(&conn, &watcher, &events);

        assert_eq!(result.dirty_count, 1);
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.error_count, 0);

        // Both indexed flags should now be 0
        assert_eq!(get_ts_indexed(&conn, "src/main.rs"), Some(0));
        assert_eq!(get_lsp_indexed(&conn, "src/main.rs"), Some(0));
    }

    #[test]
    fn test_delete_event_removes_file_and_cascades() {
        let conn = test_db();
        insert_file_with_data(&conn, "src/main.rs");

        // Verify data exists before delete
        assert!(file_exists(&conn, "src/main.rs"));
        assert_eq!(count_rows(&conn, "ts_chunks"), 1);
        assert_eq!(count_rows(&conn, "lsp_symbols"), 1);

        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Deleted(PathBuf::from("src/main.rs"))];
        let result = process_file_events(&conn, &watcher, &events);

        assert_eq!(result.deleted_count, 1);
        assert_eq!(result.dirty_count, 0);

        // File, chunks, and symbols should all be gone (CASCADE)
        assert!(!file_exists(&conn, "src/main.rs"));
        assert_eq!(count_rows(&conn, "ts_chunks"), 0);
        assert_eq!(count_rows(&conn, "lsp_symbols"), 0);
    }

    #[test]
    fn test_create_event_on_unknown_file_does_not_error() {
        let conn = test_db();

        // Created event for a file not yet in the DB — should succeed with 0 rows
        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Created(PathBuf::from("src/new_file.rs"))];
        let result = process_file_events(&conn, &watcher, &events);

        // FanoutWatcher does UPDATE ... WHERE file_path = ?, which returns 0 rows
        // for a file not in the DB — that's not an error
        assert_eq!(result.dirty_count, 0);
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn test_batch_of_mixed_events() {
        let conn = test_db();
        insert_indexed_file(&conn, "src/a.rs");
        insert_indexed_file(&conn, "src/b.rs");
        insert_file_with_data(&conn, "src/c.rs");

        let watcher = FanoutWatcher::new();
        let events = vec![
            FileEvent::Modified(PathBuf::from("src/a.rs")),
            FileEvent::Modified(PathBuf::from("src/b.rs")),
            FileEvent::Deleted(PathBuf::from("src/c.rs")),
        ];
        let result = process_file_events(&conn, &watcher, &events);

        assert_eq!(result.dirty_count, 2);
        assert_eq!(result.deleted_count, 1);
        assert_eq!(result.error_count, 0);

        // a.rs and b.rs should be dirty
        assert_eq!(get_ts_indexed(&conn, "src/a.rs"), Some(0));
        assert_eq!(get_ts_indexed(&conn, "src/b.rs"), Some(0));

        // c.rs should be gone entirely
        assert!(!file_exists(&conn, "src/c.rs"));
    }

    #[test]
    fn test_modify_already_dirty_file_is_idempotent() {
        let conn = test_db();
        insert_indexed_file(&conn, "src/main.rs");

        let watcher = FanoutWatcher::new();

        // First modify
        let events = vec![FileEvent::Modified(PathBuf::from("src/main.rs"))];
        let r1 = process_file_events(&conn, &watcher, &events);
        assert_eq!(r1.dirty_count, 1);
        assert_eq!(get_ts_indexed(&conn, "src/main.rs"), Some(0));

        // Second modify — file is already dirty, UPDATE still runs but changes 0→0
        let r2 = process_file_events(&conn, &watcher, &events);
        assert_eq!(r2.dirty_count, 1); // UPDATE matched 1 row
        assert_eq!(r2.error_count, 0);
        assert_eq!(get_ts_indexed(&conn, "src/main.rs"), Some(0));
    }

    #[test]
    fn test_delete_preserves_other_files() {
        let conn = test_db();
        insert_file_with_data(&conn, "src/keep.rs");
        insert_file_with_data(&conn, "src/remove.rs");

        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Deleted(PathBuf::from("src/remove.rs"))];
        process_file_events(&conn, &watcher, &events);

        // keep.rs should be untouched
        assert!(file_exists(&conn, "src/keep.rs"));
        assert_eq!(get_ts_indexed(&conn, "src/keep.rs"), Some(1));

        // remove.rs should be gone
        assert!(!file_exists(&conn, "src/remove.rs"));
    }

    #[test]
    fn test_empty_event_batch() {
        let conn = test_db();
        insert_indexed_file(&conn, "src/main.rs");

        let watcher = FanoutWatcher::new();
        let result = process_file_events(&conn, &watcher, &[]);

        assert_eq!(result.dirty_count, 0);
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.error_count, 0);

        // Nothing should have changed
        assert_eq!(get_ts_indexed(&conn, "src/main.rs"), Some(1));
    }

    #[test]
    fn test_delete_nonexistent_file_is_not_error() {
        let conn = test_db();

        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Deleted(PathBuf::from("src/ghost.rs"))];
        let result = process_file_events(&conn, &watcher, &events);

        // DELETE WHERE file_path = 'ghost.rs' affects 0 rows — not an error
        assert_eq!(result.deleted_count, 0);
        assert_eq!(result.error_count, 0);
    }

    #[test]
    fn test_process_file_events_db_error_increments_error_count() {
        // Create a DB without the indexed_files schema to force a SQL error.
        let conn = Connection::open_in_memory().unwrap();
        // No schema: indexed_files table does not exist.

        let watcher = FanoutWatcher::new();
        let events = vec![
            FileEvent::Modified(PathBuf::from("src/main.rs")),
            FileEvent::Deleted(PathBuf::from("src/lib.rs")),
        ];
        let result = process_file_events(&conn, &watcher, &events);

        // Both events should fail since the table doesn't exist.
        assert_eq!(result.error_count, 2);
        assert_eq!(result.dirty_count, 0);
        assert_eq!(result.deleted_count, 0);
    }

    #[test]
    fn test_modify_clears_both_index_flags() {
        let conn = test_db();

        // Insert with only ts_indexed = 1 (lsp still pending)
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES ('src/partial.rs', X'AABB', 512, 1000, 1, 0)",
            [],
        )
        .unwrap();

        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Modified(PathBuf::from("src/partial.rs"))];
        process_file_events(&conn, &watcher, &events);

        assert_eq!(get_ts_indexed(&conn, "src/partial.rs"), Some(0));
        assert_eq!(get_lsp_indexed(&conn, "src/partial.rs"), Some(0));
    }
}
