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

/// Debounce window for coalescing filesystem events before processing a batch.
///
/// `async-watcher` buffers raw `notify` events for this duration and emits them
/// as a single debounced batch, so a burst of writes to the same file (e.g. an
/// editor save) is collapsed into one re-index pass.
const DEBOUNCE_TIMEOUT: Duration = Duration::from_secs(1);

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
/// 1. Watches `workspace_root` recursively with a [`DEBOUNCE_TIMEOUT`] debounce
/// 2. Converts notify events to `FileEvent`s
/// 3. Calls [`process_file_events`] to mark DB rows dirty
/// 4. Triggers re-indexing of dirty files
///
/// Returns the `JoinHandle` for the watcher task.
pub fn start_code_context_watcher(
    workspace_root: impl AsRef<Path>,
    db: SharedDb,
) -> tokio::task::JoinHandle<()> {
    let workspace_root = workspace_root.as_ref().to_path_buf();
    tokio::spawn(async move {
        if let Err(e) = run_watcher(&workspace_root, &db).await {
            tracing::error!("code-context watcher failed: {}", e);
        }
    })
}

async fn run_watcher(
    workspace_root: impl AsRef<Path>,
    db: &SharedDb,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let workspace_root = workspace_root.as_ref();
    let (mut debouncer, mut event_rx) =
        AsyncDebouncer::new_with_channel(DEBOUNCE_TIMEOUT, None).await?;

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
                process_ok_events(db, &ws_root, &fanout, &debounced_events).await;
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

/// Handle one debounced batch of successful filesystem events.
///
/// Filters the batch to source-file [`FileEvent`]s, marks the affected DB rows
/// dirty via [`process_file_events`], then drains the dirty set by re-indexing.
/// A batch with no relevant source-file changes is a no-op.
async fn process_ok_events(
    db: &SharedDb,
    ws_root: &Path,
    fanout: &FanoutWatcher,
    debounced_events: &[async_watcher::DebouncedEvent],
) {
    // Collect file events from the batch
    let mut file_events = Vec::new();
    for debounced in debounced_events {
        for path in &debounced.event.paths {
            if !is_source_file(path) {
                continue;
            }
            if let Some(event) = to_file_event(&debounced.event.kind, path.clone(), ws_root) {
                file_events.push(event);
            }
        }
    }

    if file_events.is_empty() {
        return;
    }

    tracing::info!(
        "code-context: {} file change(s) detected, marking dirty",
        file_events.len()
    );

    // Lock DB and process events
    {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        let result = process_file_events(&conn, fanout, &file_events);
        tracing::info!(
            "code-context watcher: {} dirty, {} deleted, {} errors",
            result.dirty_count,
            result.deleted_count,
            result.error_count,
        );
    }

    // Re-index dirty files using the shared connection. The watcher path has
    // no progress channel of its own — pass the no-op reporter. Surfacing
    // watcher progress to MCP clients is a separate piece of work (see the
    // rebuild-index roadmap).
    super::index_discovered_files_async(
        ws_root,
        std::sync::Arc::clone(db),
        swissarmyhammer_code_context::noop_reporter(),
    )
    .await;
}

/// How often the leader re-walks the filesystem to reconcile the index.
///
/// The live watcher's per-event path is the low-latency fast path, but it has
/// no correctness floor: any missed `notify` event (event storms, editors that
/// write via rename/replace, files materialized before the watcher attached,
/// bulk regeneration) leaves rows the event path never revisits, and a leader
/// alive for hours or days drifts permanently. This timer gives the leader a
/// periodic FS-walk reconcile so it always converges on disk truth without a
/// restart or a hand-run `rebuild index`.
///
/// Five minutes balances staleness against cost. The reconcile diffs by
/// content-hash/mtime and only marks genuinely-changed rows dirty, so a
/// steady-state pass over an unchanged tree is a bounded hash-and-compare with
/// no re-indexing -- cheap enough to run often, infrequent enough not to compete
/// with the event fast path that already handles the common edit case in ~1s.
const RECONCILE_INTERVAL: Duration = Duration::from_secs(5 * 60);

/// Periodic correctness floor for the leader's index.
///
/// Runs the [`startup_cleanup`](swissarmyhammer_code_context::startup_cleanup)
/// FS-walk reconcile under the shared write lock, then drains the resulting
/// dirty set (`WHERE ts_indexed = 0`) via the same indexer the watcher uses.
/// This is the entrypoint the leader's [`run_periodic_reconcile`] timer calls,
/// and the one a unit test drives directly with `embedder = None` to skip the
/// model load.
///
/// `embedder` is threaded through to
/// [`index_discovered_files_with_embedder`](super::index_discovered_files_with_embedder):
/// `Some` embeds chunks, `None` writes chunks without embeddings (the test path,
/// and the soft fallback when the model is unavailable).
pub(crate) async fn reconcile_workspace_with_embedder(
    workspace_root: &Path,
    db: &SharedDb,
    embedder: Option<std::sync::Arc<dyn model_embedding::TextEmbedder>>,
    reporter: std::sync::Arc<dyn swissarmyhammer_code_context::ProgressReporter>,
) {
    // FS-walk reconcile under the write lock: deletes vanished files, marks
    // hash-changed files dirty, and inserts new files with ts_indexed = 0.
    {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        match swissarmyhammer_code_context::startup_cleanup(&conn, workspace_root) {
            Ok(stats) => tracing::info!(
                "code-context periodic reconcile: {} added, {} removed, {} dirty, {} unchanged",
                stats.files_added,
                stats.files_removed,
                stats.files_dirty,
                stats.files_unchanged,
            ),
            Err(e) => {
                tracing::warn!("code-context periodic reconcile: FS walk failed: {}", e);
                return;
            }
        }
    }

    // Drain the dirty set the reconcile just produced.
    super::index_discovered_files_with_embedder(
        workspace_root,
        std::sync::Arc::clone(db),
        embedder,
        reporter,
    )
    .await;
}

/// Run [`reconcile_workspace_with_embedder`] with the production embedder.
///
/// Builds the default chunk embedder (or `None` on load failure / opt-out) and
/// uses the no-op progress reporter -- the periodic reconcile has no JSON-RPC
/// progress channel, mirroring the watcher's own re-index path.
async fn reconcile_workspace(workspace_root: &Path, db: &SharedDb) {
    let embedder = super::build_default_embedder().await;
    reconcile_workspace_with_embedder(
        workspace_root,
        db,
        embedder,
        swissarmyhammer_code_context::noop_reporter(),
    )
    .await;
}

/// Drive [`reconcile_workspace`] forever on a [`RECONCILE_INTERVAL`] timer.
///
/// This is the leader's self-heal loop. It MUST be spawned only on the leader --
/// it holds and writes through the leader's shared write connection. Followers
/// route to the leader and never run it. The caller (the leader-gated worker
/// spawn in the MCP server) is responsible for that gating, exactly as it is for
/// the watcher itself.
pub fn run_periodic_reconcile(
    workspace_root: impl AsRef<Path>,
    db: SharedDb,
) -> tokio::task::JoinHandle<()> {
    let workspace_root = workspace_root.as_ref().to_path_buf();
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(RECONCILE_INTERVAL);
        // Skip the immediate first tick: startup already ran startup_cleanup, so
        // the first periodic reconcile fires one interval later.
        ticker.tick().await;
        loop {
            ticker.tick().await;
            reconcile_workspace(&workspace_root, &db).await;
        }
    })
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
    fn test_create_event_on_unknown_file_inserts_dirty_row() {
        let conn = test_db();

        // Created event for a file not yet in the DB — the watcher upserts a
        // dirty row so the file enters the `ts_indexed = 0` dirty set.
        let watcher = FanoutWatcher::new();
        let events = vec![FileEvent::Created(PathBuf::from("src/new_file.rs"))];
        let result = process_file_events(&conn, &watcher, &events);

        // INSERT ... ON CONFLICT affects 1 row for a previously unknown file.
        assert_eq!(result.dirty_count, 1);
        assert_eq!(result.error_count, 0);

        // The row exists and is marked dirty.
        assert!(file_exists(&conn, "src/new_file.rs"));
        assert_eq!(get_ts_indexed(&conn, "src/new_file.rs"), Some(0));
        assert_eq!(get_lsp_indexed(&conn, "src/new_file.rs"), Some(0));
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

    /// Regression: a file created AFTER the startup scan must get indexed.
    ///
    /// Drives the same dirty-set indexing path the live watcher runs — a
    /// `Created` event through [`process_file_events`], then the dirty-set
    /// indexer — and asserts the brand-new file (which has no pre-existing
    /// `indexed_files` row) is inserted, tree-sitter indexed, and produces
    /// chunks.
    ///
    /// The one deliberate divergence from production: production
    /// [`process_ok_events`] calls [`super::index_discovered_files_async`],
    /// which first builds the default embedder and then delegates to
    /// [`super::super::index_discovered_files_with_embedder`]. This test calls
    /// `index_discovered_files_with_embedder` directly with `embedder = None`,
    /// skipping the model load so the test is deterministic and fast (no model
    /// download/inference, no flakiness). Embedding is orthogonal to the bug
    /// under test — the indexing/insert path exercised is identical.
    ///
    /// Before the fix, `Created` did UPDATE-only SQL that matched 0 rows for a
    /// row-less file, so it never entered the dirty set and was never indexed.
    #[tokio::test]
    async fn watcher_indexes_file_created_after_startup() {
        use std::sync::{Arc, Mutex};
        use swissarmyhammer_code_context::{FanoutWatcher, FileEvent};

        // --- workspace with ONE file already indexed at "startup" ---
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/existing.rs"), "pub fn a() {}").unwrap();

        let conn = Connection::open(dir.path().join("index.db")).unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        // existing.rs is already in the index (as startup_cleanup would have left it)
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES ('src/existing.rs', X'00', 13, 1000, 1, 1)",
            [],
        )
        .unwrap();
        let db: swissarmyhammer_code_context::SharedDb = Arc::new(Mutex::new(conn));

        // --- a NEW file is created AFTER startup (what /finish does) ---
        std::fs::write(
            dir.path().join("src/created_after_start.rs"),
            "pub fn brand_new() -> i32 { 42 }",
        )
        .unwrap();

        // --- exactly what run_watcher does on the resulting notify event ---
        let fanout = FanoutWatcher::new();
        let events = vec![FileEvent::Created(std::path::PathBuf::from(
            "src/created_after_start.rs",
        ))];
        {
            let conn = db.lock().unwrap();
            super::process_file_events(&conn, &fanout, &events);
        }
        super::super::index_discovered_files_with_embedder(
            dir.path(),
            Arc::clone(&db),
            None,
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        // --- ASSERT the new file is indexed ---
        let conn = db.lock().unwrap();
        assert!(
            file_exists(&conn, "src/created_after_start.rs"),
            "new file created after startup must be inserted into indexed_files"
        );
        assert_eq!(
            get_ts_indexed(&conn, "src/created_after_start.rs"),
            Some(1),
            "new file must be tree-sitter indexed"
        );
        let chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'src/created_after_start.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(chunks > 0, "new file must produce ts_chunks");
    }

    /// Regression: a long-lived leader must self-heal when filesystem changes
    /// arrive with NO watcher event at all.
    ///
    /// This is the correctness floor underneath the watcher's event fast-path.
    /// It mutates the filesystem the way a missed/never-delivered notify event
    /// would leave it -- a brand-new file AND a delete+recreate of an existing
    /// file, neither announced to the watcher -- then invokes the periodic
    /// reconcile entrypoint directly (the function the leader's timer calls) and
    /// asserts both files converge to the indexed state.
    ///
    /// As in [`watcher_indexes_file_created_after_startup`], the embedder is
    /// `None` so no model loads -- the bug under test is the FS-walk reconcile,
    /// orthogonal to embedding, which keeps the unit test deterministic and well
    /// under the 10s budget.
    ///
    /// Before the periodic reconcile existed, nothing re-walked the filesystem
    /// after startup, so the new and recreated files never re-entered the dirty
    /// set and stayed absent/stale forever.
    #[tokio::test]
    async fn periodic_reconcile_indexes_files_changed_without_a_watcher_event() {
        use std::sync::{Arc, Mutex};

        // --- workspace indexed as the leader would leave it at startup ---
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/existing.rs"), "pub fn a() {}").unwrap();

        let conn = Connection::open(dir.path().join("index.db")).unwrap();
        configure_connection(&conn).unwrap();
        create_schema(&conn).unwrap();
        let db: swissarmyhammer_code_context::SharedDb = Arc::new(Mutex::new(conn));

        // Initial reconcile populates indexed_files and drives the dirty set,
        // exactly as the leader's startup pass does.
        super::reconcile_workspace_with_embedder(
            dir.path(),
            &db,
            None,
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;
        {
            let conn = db.lock().unwrap();
            assert_eq!(
                get_ts_indexed(&conn, "src/existing.rs"),
                Some(1),
                "existing file should be indexed after the initial reconcile"
            );
        }

        // --- mutate the FS with NO watcher event delivered ---
        // 1. a brand-new file that was never announced
        std::fs::write(
            dir.path().join("src/never_announced.rs"),
            "pub fn brand_new() -> i32 { 42 }",
        )
        .unwrap();
        // 2. delete+recreate an existing file with different contents (the
        //    rename/replace pattern editors use, whose event can be missed)
        std::fs::remove_file(dir.path().join("src/existing.rs")).unwrap();
        std::fs::write(
            dir.path().join("src/existing.rs"),
            "pub fn a() {}\npub fn b() {}",
        )
        .unwrap();

        // --- invoke the periodic reconcile entrypoint directly ---
        super::reconcile_workspace_with_embedder(
            dir.path(),
            &db,
            None,
            swissarmyhammer_code_context::noop_reporter(),
        )
        .await;

        // --- ASSERT both files converged to indexed (ts_indexed 0 -> indexed) ---
        let conn = db.lock().unwrap();

        assert!(
            file_exists(&conn, "src/never_announced.rs"),
            "new file with no watcher event must be reconciled into indexed_files"
        );
        assert_eq!(
            get_ts_indexed(&conn, "src/never_announced.rs"),
            Some(1),
            "new file must be tree-sitter indexed by the periodic reconcile"
        );
        let new_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'src/never_announced.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(new_chunks > 0, "new file must produce ts_chunks");

        assert_eq!(
            get_ts_indexed(&conn, "src/existing.rs"),
            Some(1),
            "recreated file must be re-indexed by the periodic reconcile"
        );
        let recreated_chunks: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM ts_chunks WHERE file_path = 'src/existing.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert!(
            recreated_chunks > 0,
            "recreated file must produce ts_chunks"
        );
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
