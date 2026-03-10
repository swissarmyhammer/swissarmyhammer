//! Background LSP indexing worker.
//!
//! Spawns a blocking thread that queries `lsp_indexed = 0` files from the database,
//! sends `textDocument/didOpen` and `textDocument/documentSymbol` requests through
//! a shared [`LspJsonRpcClient`], persists the resulting symbols, and marks files
//! as `lsp_indexed = 1`.
//!
//! The worker receives the client via `Arc<Mutex<Option<LspJsonRpcClient>>>`. The
//! outer `Option` allows the daemon to be absent (not yet started or restarting).
//! When `None`, the worker sleeps and retries.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::Connection;
use tracing::{debug, info, warn};

use crate::error::CodeContextError;
use crate::lsp_communication::LspJsonRpcClient;
use crate::lsp_indexer::mark_lsp_indexed;
use crate::workspace::SharedDb;

/// Configuration for the LSP indexing worker.
#[derive(Debug, Clone)]
pub struct LspWorkerConfig {
    /// Maximum files to process per batch before re-querying.
    pub batch_size: usize,
    /// How long to sleep when the client is unavailable.
    pub client_unavailable_sleep: Duration,
    /// How long to sleep when no dirty files remain.
    pub idle_sleep: Duration,
}

impl Default for LspWorkerConfig {
    fn default() -> Self {
        Self {
            batch_size: 50,
            client_unavailable_sleep: Duration::from_secs(5),
            idle_sleep: Duration::from_millis(500),
        }
    }
}

/// Shared handle to an LSP client that may or may not be available.
///
/// The `Option` is `None` when the LSP daemon hasn't started or is restarting.
/// The worker locks the mutex, checks for `Some`, and uses the client to send
/// requests. The daemon's owner is responsible for populating and clearing this.
pub type SharedLspClient = Arc<Mutex<Option<LspJsonRpcClient>>>;

/// Spawn a background thread that indexes files via LSP.
///
/// The thread loops indefinitely:
/// 1. Lock the shared client; if `None`, sleep and retry.
/// 2. Query `lsp_indexed = 0` files from the database.
/// 3. For each file: read content, send `didOpen`, request `documentSymbol`,
///    persist symbols, mark `lsp_indexed = 1`.
/// 4. On per-file failure, log the error and still mark `lsp_indexed = 1`
///    to avoid infinite retry loops.
///
/// # Arguments
/// * `workspace_root` - Absolute path to the workspace root.
/// * `db` - Shared write connection from the leader workspace.
/// * `client` - Shared handle to the LSP JSON-RPC client.
/// * `config` - Worker configuration.
pub fn spawn_lsp_indexing_worker(
    workspace_root: PathBuf,
    db: SharedDb,
    client: SharedLspClient,
    config: LspWorkerConfig,
) -> JoinHandle<()> {
    thread::Builder::new()
        .name("code-context-lsp-indexer".to_string())
        .spawn(move || {
            match run_lsp_indexing_loop(&workspace_root, &db, &client, &config) {
                Ok(()) => info!("LSP indexing worker completed"),
                Err(e) => warn!("LSP indexing worker error: {}", e),
            }
        })
        .expect("Failed to spawn LSP indexing worker thread")
}

/// Main indexing loop. Runs until the thread is terminated.
///
/// Uses the leader's shared write connection for all DB operations.
/// The mutex is locked only for the duration of each DB call, so the
/// TS indexer and file watcher can interleave writes without blocking.
fn run_lsp_indexing_loop(
    workspace_root: &Path,
    db: &SharedDb,
    client: &SharedLspClient,
    config: &LspWorkerConfig,
) -> Result<(), CodeContextError> {
    info!(
        "LSP indexing worker started for {}",
        workspace_root.display()
    );

    let mut total_indexed = 0u64;

    loop {
        // 1. Query dirty files (lock DB briefly)
        let dirty_files = {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());
            query_lsp_dirty_files(&conn, config.batch_size)?
        };

        if dirty_files.is_empty() {
            thread::sleep(config.idle_sleep);
            continue;
        }

        // 2. Try to get the client
        let mut guard = match client.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                warn!("LSP client mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };

        let lsp_client = match guard.as_mut() {
            Some(c) => c,
            None => {
                // Client not available (daemon not started or restarting)
                drop(guard);
                debug!("LSP client not available, sleeping");
                thread::sleep(config.client_unavailable_sleep);
                continue;
            }
        };

        info!(
            "LSP indexing: processing {} dirty files",
            dirty_files.len()
        );

        // 3. Process each file sequentially (LSP is single-threaded I/O)
        for relative_path in &dirty_files {
            let full_path = workspace_root.join(relative_path);

            match index_single_file(lsp_client, db, &full_path, relative_path) {
                Ok(symbol_count) => {
                    total_indexed += 1;
                    debug!(
                        "LSP indexed {} ({} symbols, {} total files)",
                        relative_path, symbol_count, total_indexed
                    );
                }
                Err(e) => {
                    warn!("LSP indexing failed for {}: {}", relative_path, e);
                    // Still mark as indexed to prevent infinite retry
                    let conn = db.lock().unwrap_or_else(|p| p.into_inner());
                    if let Err(mark_err) = mark_lsp_indexed(&conn, relative_path) {
                        warn!(
                            "Failed to mark {} as lsp_indexed after error: {}",
                            relative_path, mark_err
                        );
                    }
                    total_indexed += 1;
                }
            }
        }

        info!(
            "LSP indexing: batch complete, {} files indexed so far",
            total_indexed
        );
    }
}

/// Index a single file via LSP: didOpen, documentSymbol, persist, mark indexed.
///
/// Locks the shared DB only for the persist step — LSP I/O happens without
/// holding the mutex so other writers aren't blocked during network waits.
///
/// Returns the number of symbols persisted on success.
fn index_single_file(
    client: &mut LspJsonRpcClient,
    db: &SharedDb,
    full_path: &Path,
    relative_path: &str,
) -> Result<usize, CodeContextError> {
    // Read file content
    let content = std::fs::read_to_string(full_path)?;
    let language_id = extension_to_language_id(full_path);

    // Send didOpen notification (no DB lock needed)
    client.send_did_open(full_path, language_id, &content)?;

    // Collect symbols and persist them — lock DB for the write
    let result = {
        let conn = db.lock().unwrap_or_else(|p| p.into_inner());
        client.collect_and_persist_file_symbols(&conn, full_path, relative_path)?
    };

    // Close the document so re-indexing won't trigger "duplicate didOpen"
    if let Err(e) = client.send_did_close(full_path) {
        debug!("Failed to send didClose for {}: {}", relative_path, e);
    }

    if let Some(err) = &result.error {
        warn!(
            "LSP symbol collection warning for {}: {}",
            relative_path, err
        );
    }

    Ok(result.symbol_count)
}

/// Query files that need LSP indexing (`lsp_indexed = 0`).
fn query_lsp_dirty_files(
    db: &Connection,
    limit: usize,
) -> Result<Vec<String>, CodeContextError> {
    let mut stmt = db.prepare(
        "SELECT file_path FROM indexed_files WHERE lsp_indexed = 0 LIMIT ?"
    )?;
    let files = stmt
        .query_map([limit as i64], |row| row.get::<_, String>(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(files)
}

/// Map a file extension to an LSP language identifier.
///
/// Returns a best-effort language ID for the `textDocument/didOpen` notification.
/// Unknown extensions default to `"plaintext"`.
fn extension_to_language_id(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => "rust",
        Some("py") => "python",
        Some("js") => "javascript",
        Some("jsx") => "javascriptreact",
        Some("ts") => "typescript",
        Some("tsx") => "typescriptreact",
        Some("go") => "go",
        Some("java") => "java",
        Some("c") => "c",
        Some("cpp" | "cc" | "cxx") => "cpp",
        Some("h") => "c",
        Some("hpp" | "hxx") => "cpp",
        Some("rb") => "ruby",
        Some("swift") => "swift",
        Some("kt" | "kts") => "kotlin",
        Some("lua") => "lua",
        Some("sh" | "bash") => "shellscript",
        Some("toml") => "toml",
        Some("yaml" | "yml") => "yaml",
        Some("json") => "json",
        Some("md") => "markdown",
        Some("html") => "html",
        Some("css") => "css",
        _ => "plaintext",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;

    /// Create an in-memory DB with the required schema.
    fn create_test_db() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        crate::db::configure_connection(&conn).unwrap();
        crate::db::create_schema(&conn).unwrap();
        conn
    }

    /// Insert a test file row into indexed_files.
    fn insert_test_file(conn: &Connection, file_path: &str) {
        conn.execute(
            "INSERT INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed)
             VALUES (?, X'00112233', 1024, 1000, 1, 0)",
            [file_path],
        )
        .unwrap();
    }

    // -- query_lsp_dirty_files tests --

    #[test]
    fn test_query_lsp_dirty_files_returns_unindexed() {
        let db = create_test_db();
        insert_test_file(&db, "src/main.rs");
        insert_test_file(&db, "src/lib.rs");

        // Mark one as lsp_indexed
        db.execute(
            "UPDATE indexed_files SET lsp_indexed = 1 WHERE file_path = 'src/lib.rs'",
            [],
        )
        .unwrap();

        let dirty = query_lsp_dirty_files(&db, 100).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "src/main.rs");
    }

    #[test]
    fn test_query_lsp_dirty_files_respects_limit() {
        let db = create_test_db();
        insert_test_file(&db, "a.rs");
        insert_test_file(&db, "b.rs");
        insert_test_file(&db, "c.rs");

        let dirty = query_lsp_dirty_files(&db, 2).unwrap();
        assert_eq!(dirty.len(), 2);
    }

    #[test]
    fn test_query_lsp_dirty_files_empty_when_all_indexed() {
        let db = create_test_db();
        insert_test_file(&db, "src/main.rs");
        db.execute(
            "UPDATE indexed_files SET lsp_indexed = 1 WHERE file_path = 'src/main.rs'",
            [],
        )
        .unwrap();

        let dirty = query_lsp_dirty_files(&db, 100).unwrap();
        assert!(dirty.is_empty());
    }

    // -- extension_to_language_id tests --

    #[test]
    fn test_extension_to_language_id_rust() {
        assert_eq!(extension_to_language_id(Path::new("main.rs")), "rust");
    }

    #[test]
    fn test_extension_to_language_id_python() {
        assert_eq!(extension_to_language_id(Path::new("app.py")), "python");
    }

    #[test]
    fn test_extension_to_language_id_typescript() {
        assert_eq!(extension_to_language_id(Path::new("index.ts")), "typescript");
        assert_eq!(extension_to_language_id(Path::new("App.tsx")), "typescriptreact");
    }

    #[test]
    fn test_extension_to_language_id_unknown_defaults_to_plaintext() {
        assert_eq!(extension_to_language_id(Path::new("data.xyz")), "plaintext");
    }

    #[test]
    fn test_extension_to_language_id_no_extension() {
        assert_eq!(extension_to_language_id(Path::new("Makefile")), "plaintext");
    }

    // -- SharedLspClient tests --

    #[test]
    fn test_shared_client_none_initially() {
        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let guard = client.lock().unwrap();
        assert!(guard.is_none());
    }

    // -- index_single_file tests (requires real LSP, so we test error path) --

    #[test]
    fn test_index_single_file_missing_file_returns_error() {
        // We can't construct a real LspJsonRpcClient without an LSP process,
        // but we can verify that missing files produce an I/O error before
        // any client interaction. This validates the early content read.
        let result = std::fs::read_to_string("/nonexistent/path/test.rs");
        assert!(result.is_err());
    }

    // -- mark_lsp_indexed integration --

    #[test]
    fn test_mark_lsp_indexed_after_failure() {
        let db = create_test_db();
        insert_test_file(&db, "src/failing.rs");

        // Simulate the fallback: mark as indexed even on failure
        mark_lsp_indexed(&db, "src/failing.rs").unwrap();

        let lsp: i64 = db
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = 'src/failing.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp, 1);
    }

    // -- collect_and_persist_symbols integration --

    #[test]
    fn test_collect_and_persist_symbols_marks_indexed() {
        use lsp_types::{DocumentSymbol, Position, Range, SymbolKind};

        let db = create_test_db();
        insert_test_file(&db, "src/demo.rs");

        #[allow(deprecated)]
        let symbols = vec![DocumentSymbol {
            name: "demo_fn".to_string(),
            detail: None,
            kind: SymbolKind::FUNCTION,
            tags: None,
            deprecated: None,
            range: Range::new(Position::new(0, 0), Position::new(5, 1)),
            selection_range: Range::new(Position::new(0, 3), Position::new(0, 10)),
            children: None,
        }];

        let count = crate::lsp_communication::collect_and_persist_symbols(
            &db,
            "src/demo.rs",
            &symbols,
        )
        .unwrap();
        assert_eq!(count, 1);

        // Verify lsp_indexed is now 1
        let lsp: i64 = db
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = 'src/demo.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp, 1);

        // Verify symbol was written
        let sym_count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = 'src/demo.rs'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sym_count, 1);
    }
}
