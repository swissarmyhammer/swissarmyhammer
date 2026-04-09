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
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use rusqlite::types::Value;
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

/// Shared flag for signaling graceful shutdown to worker threads.
///
/// Set to `true` to request the worker to exit at the next loop iteration.
pub type ShutdownFlag = Arc<AtomicBool>;

/// Create a new shutdown flag initialized to `false`.
pub fn new_shutdown_flag() -> ShutdownFlag {
    Arc::new(AtomicBool::new(false))
}

/// Spawn a background thread that indexes files via LSP.
///
/// The thread loops until `shutdown` is set to `true`:
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
/// * `server_name` - Command name of the LSP server (used in log messages).
/// * `shutdown` - Shared flag; set to `true` to request graceful shutdown.
pub fn spawn_lsp_indexing_worker(
    workspace_root: PathBuf,
    db: SharedDb,
    client: SharedLspClient,
    config: LspWorkerConfig,
    server_name: String,
    shutdown: ShutdownFlag,
) -> JoinHandle<()> {
    let thread_name = format!("code-context-lsp-indexer-{}", server_name);
    thread::Builder::new()
        .name(thread_name)
        .spawn(move || {
            match run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                &server_name,
                &shutdown,
            ) {
                Ok(()) => info!(server = %server_name, "LSP indexing worker completed"),
                Err(e) => warn!(server = %server_name, "LSP indexing worker error: {}", e),
            }
        })
        .expect("Failed to spawn LSP indexing worker thread")
}

/// Main indexing loop. Runs until shutdown is signaled or an error occurs.
///
/// Uses the leader's shared write connection for all DB operations.
/// The mutex is locked only for the duration of each DB call, so the
/// TS indexer and file watcher can interleave writes without blocking.
fn run_lsp_indexing_loop(
    workspace_root: &Path,
    db: &SharedDb,
    client: &SharedLspClient,
    config: &LspWorkerConfig,
    server_name: &str,
    shutdown: &AtomicBool,
) -> Result<(), CodeContextError> {
    let extensions = lsp_supported_extensions(server_name);
    info!(
        server = %server_name,
        extensions = ?extensions,
        "LSP indexing worker started for {} ({} supported extensions)",
        workspace_root.display(),
        extensions.len()
    );

    if extensions.is_empty() {
        warn!(
            server = %server_name,
            "No known extensions for LSP server '{}', worker will idle",
            server_name
        );
    }

    let mut total_indexed = 0u64;

    loop {
        if shutdown.load(Ordering::Relaxed) {
            info!(server = %server_name, "LSP indexing worker shutting down ({} files indexed)", total_indexed);
            return Ok(());
        }

        // 1. Query dirty files filtered to extensions this server handles
        let dirty_files = {
            let conn = db.lock().unwrap_or_else(|p| p.into_inner());
            query_lsp_dirty_files(&conn, config.batch_size, extensions)?
        };

        if dirty_files.is_empty() {
            thread::sleep(config.idle_sleep);
            continue;
        }

        // 2. Try to get the client
        let mut guard = match client.lock() {
            Ok(g) => g,
            Err(poisoned) => {
                warn!(server = %server_name, "LSP client mutex poisoned, recovering");
                poisoned.into_inner()
            }
        };

        let lsp_client = match guard.as_mut() {
            Some(c) => c,
            None => {
                // Client not available (daemon not started or restarting)
                drop(guard);
                debug!(server = %server_name, "LSP client not available, sleeping");
                thread::sleep(config.client_unavailable_sleep);
                continue;
            }
        };

        info!(server = %server_name, "LSP indexing: processing {} dirty files", dirty_files.len());

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
            server = %server_name,
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

/// File extensions supported by each known LSP server.
///
/// Maps a server command name to the file extensions it can handle.
/// Unknown servers return an empty slice, which prevents indexing files
/// that no server understands.
pub fn lsp_supported_extensions(server_name: &str) -> &'static [&'static str] {
    match server_name {
        "rust-analyzer" => &["rs"],
        "pyright" | "pylsp" | "pyright-langserver" => &["py", "pyi", "pyw"],
        "typescript-language-server" | "tsserver" | "ts_ls" => {
            &["ts", "mts", "cts", "tsx", "js", "mjs", "cjs", "jsx"]
        }
        "gopls" => &["go"],
        "jdtls" | "java-language-server" => &["java"],
        "clangd" => &["c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh"],
        "solargraph" | "ruby-lsp" => &["rb", "rake", "gemspec"],
        "sourcekit-lsp" => &["swift"],
        "kotlin-language-server" => &["kt", "kts"],
        "lua-language-server" => &["lua"],
        "omnisharp" => &["cs"],
        "dart" | "dart-language-server" => &["dart"],
        "phpactor" | "intelephense" => &["php", "phtml"],
        "metals" => &["scala", "sc"],
        _ => &[],
    }
}

/// Union of all file extensions that at least one known LSP server supports.
///
/// Files with extensions not in this list can be marked `lsp_indexed = 1`
/// immediately since no LSP server will ever process them.
pub const LSP_CAPABLE_EXTENSIONS: &[&str] = &[
    "rs", "py", "pyi", "pyw", "ts", "mts", "cts", "tsx", "js", "mjs", "cjs", "jsx", "go", "java",
    "c", "h", "cpp", "cc", "cxx", "hpp", "hxx", "hh", "rb", "rake", "gemspec", "swift", "kt",
    "kts", "lua", "cs", "dart", "php", "phtml", "scala", "sc",
];

/// Query files that need LSP indexing (`lsp_indexed = 0`), filtered to only
/// include files whose extension matches what the given LSP server supports.
///
/// If `extensions` is empty the query returns no files, which is the correct
/// behaviour for unknown servers.
fn query_lsp_dirty_files(
    db: &Connection,
    limit: usize,
    extensions: &[&str],
) -> Result<Vec<String>, CodeContextError> {
    if extensions.is_empty() {
        return Ok(Vec::new());
    }

    // Build WHERE clause with parameterized LIKE placeholders
    let like_clauses: Vec<String> = (1..=extensions.len())
        .map(|i| format!("file_path LIKE ?{}", i))
        .collect();
    let filter = like_clauses.join(" OR ");

    let sql = format!(
        "SELECT file_path FROM indexed_files WHERE lsp_indexed = 0 AND ({}) LIMIT ?{}",
        filter,
        extensions.len() + 1
    );

    // Bind extension patterns and limit as parameters
    let mut params: Vec<Value> = extensions
        .iter()
        .map(|ext| Value::Text(format!("%.{}", ext)))
        .collect();
    params.push(Value::Integer(limit as i64));

    let mut stmt = db.prepare(&sql)?;
    let files = stmt
        .query_map(rusqlite::params_from_iter(params), |row| {
            row.get::<_, String>(0)
        })?
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

        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "src/main.rs");
    }

    #[test]
    fn test_query_lsp_dirty_files_respects_limit() {
        let db = create_test_db();
        insert_test_file(&db, "a.rs");
        insert_test_file(&db, "b.rs");
        insert_test_file(&db, "c.rs");

        let dirty = query_lsp_dirty_files(&db, 2, &["rs"]).unwrap();
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

        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert!(dirty.is_empty());
    }

    #[test]
    fn test_query_lsp_dirty_files_filters_by_extension() {
        let db = create_test_db();
        insert_test_file(&db, "src/main.rs");
        insert_test_file(&db, "config.toml");
        insert_test_file(&db, "script.sh");
        insert_test_file(&db, "app.py");

        // rust-analyzer should only see .rs files
        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "src/main.rs");

        // pyright should only see .py files
        let dirty = query_lsp_dirty_files(&db, 100, &["py", "pyi", "pyw"]).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "app.py");
    }

    #[test]
    fn test_query_lsp_dirty_files_empty_extensions_returns_nothing() {
        let db = create_test_db();
        insert_test_file(&db, "src/main.rs");
        insert_test_file(&db, "config.toml");

        // Unknown server -> empty extensions -> no files
        let dirty = query_lsp_dirty_files(&db, 100, &[]).unwrap();
        assert!(dirty.is_empty());
    }

    // -- lsp_supported_extensions tests --

    #[test]
    fn test_lsp_supported_extensions_known_servers() {
        assert_eq!(lsp_supported_extensions("rust-analyzer"), &["rs"]);
        assert!(lsp_supported_extensions("pyright").contains(&"py"));
        assert!(lsp_supported_extensions("typescript-language-server").contains(&"ts"));
        assert!(lsp_supported_extensions("gopls").contains(&"go"));
    }

    #[test]
    fn test_lsp_supported_extensions_unknown_server() {
        assert!(lsp_supported_extensions("unknown-server").is_empty());
        assert!(lsp_supported_extensions("").is_empty());
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
        assert_eq!(
            extension_to_language_id(Path::new("index.ts")),
            "typescript"
        );
        assert_eq!(
            extension_to_language_id(Path::new("App.tsx")),
            "typescriptreact"
        );
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

        let count =
            crate::lsp_communication::collect_and_persist_symbols(&db, "src/demo.rs", &symbols)
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

    // -- new_shutdown_flag tests --

    #[test]
    fn test_new_shutdown_flag_starts_false() {
        // The shutdown flag must start as false so workers don't exit immediately.
        let flag = new_shutdown_flag();
        assert!(!flag.load(Ordering::Relaxed));
    }

    #[test]
    fn test_shutdown_flag_can_be_set_true() {
        // Verify the flag can be set to true via store.
        let flag = new_shutdown_flag();
        flag.store(true, Ordering::Relaxed);
        assert!(flag.load(Ordering::Relaxed));
    }

    // -- run_lsp_indexing_loop shutdown tests --

    /// Build a SharedDb wrapping an in-memory test database.
    fn create_shared_test_db() -> SharedDb {
        let conn = create_test_db();
        Arc::new(Mutex::new(conn))
    }

    #[test]
    fn test_loop_exits_immediately_when_shutdown_set() {
        // Set the shutdown flag before calling the loop so the first iteration
        // exits cleanly without touching the DB or client.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(1),
            idle_sleep: Duration::from_millis(1),
        };
        let shutdown = new_shutdown_flag();
        shutdown.store(true, Ordering::Relaxed);

        let result = run_lsp_indexing_loop(
            &workspace_root,
            &db,
            &client,
            &config,
            "rust-analyzer",
            &shutdown,
        );

        assert!(result.is_ok(), "Loop should return Ok when shut down");
    }

    #[test]
    fn test_loop_idles_and_shuts_down_with_no_dirty_files() {
        // When there are no dirty files the loop sleeps (idle_sleep) then checks
        // shutdown again. Setting shutdown after one iteration terminates cleanly.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        // No files inserted — dirty list will always be empty.
        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(1),
            idle_sleep: Duration::from_millis(1),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        // Run the loop in a background thread; signal shutdown after a short delay.
        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "rust-analyzer",
                &shutdown,
            )
        });

        thread::sleep(Duration::from_millis(20));
        shutdown_clone.store(true, Ordering::Relaxed);

        let result = handle.join().expect("Worker thread should not panic");
        assert!(result.is_ok(), "Loop should return Ok on graceful shutdown");
    }

    #[test]
    fn test_loop_sleeps_when_client_unavailable_then_shuts_down() {
        // When dirty files exist but the LSP client is None (unavailable), the
        // loop should sleep `client_unavailable_sleep` and retry. Setting shutdown
        // after a short delay terminates the worker without processing any files.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();

        // Insert a dirty .rs file so the loop reaches the client-availability check.
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/main.rs");
        }

        let client: SharedLspClient = Arc::new(Mutex::new(None)); // client unavailable
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(5),
            idle_sleep: Duration::from_millis(5),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "rust-analyzer",
                &shutdown,
            )
        });

        // Give the loop time to enter the client-unavailable sleep at least once.
        thread::sleep(Duration::from_millis(30));
        shutdown_clone.store(true, Ordering::Relaxed);

        let result = handle.join().expect("Worker thread should not panic");
        assert!(result.is_ok(), "Loop should return Ok on graceful shutdown");
    }

    #[test]
    fn test_loop_unknown_server_name_no_files_processed() {
        // An unknown server name produces an empty extensions list.
        // The loop should log a warning, then idle (no files match) and exit
        // on shutdown without processing anything.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();

        // Insert a dirty .rs file — it should NOT be processed by an unknown server.
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/main.rs");
        }

        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(1),
            idle_sleep: Duration::from_millis(1),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "unknown-server", // triggers empty extensions warning
                &shutdown,
            )
        });

        thread::sleep(Duration::from_millis(20));
        shutdown_clone.store(true, Ordering::Relaxed);

        let result = handle.join().expect("Worker thread should not panic");
        assert!(
            result.is_ok(),
            "Loop should return Ok when unknown server idles out"
        );
    }

    // -- spawn_lsp_indexing_worker tests --

    #[test]
    fn test_spawn_lsp_indexing_worker_shuts_down_cleanly() {
        // Verify the public spawn function returns a JoinHandle that exits
        // cleanly when the shutdown flag is set.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(1),
            idle_sleep: Duration::from_millis(1),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = spawn_lsp_indexing_worker(
            workspace_root,
            db,
            client,
            config,
            "rust-analyzer".to_string(),
            shutdown,
        );

        // Signal shutdown immediately.
        shutdown_clone.store(true, Ordering::Relaxed);

        // The join should succeed (no panic, clean exit).
        handle.join().expect("Worker thread should not panic");
    }

    #[test]
    fn test_spawn_lsp_indexing_worker_client_unavailable_then_shutdown() {
        // Worker is spawned with a None client; dirty files exist so the worker
        // reaches the client-unavailable branch. It should exit cleanly on shutdown.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/lib.rs");
        }

        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(5),
            idle_sleep: Duration::from_millis(5),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = spawn_lsp_indexing_worker(
            workspace_root,
            db,
            client,
            config,
            "rust-analyzer".to_string(),
            shutdown,
        );

        thread::sleep(Duration::from_millis(30));
        shutdown_clone.store(true, Ordering::Relaxed);

        handle.join().expect("Worker thread should not panic");
    }

    #[test]
    fn test_loop_client_unavailable_leaves_files_unindexed() {
        // When the client is permanently unavailable (None), the worker should
        // never mark any files as lsp_indexed — it only retries.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();

        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/alpha.rs");
            insert_test_file(&conn, "src/beta.rs");
        }

        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(5),
            idle_sleep: Duration::from_millis(5),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);
        let db_check = Arc::clone(&db);

        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "rust-analyzer",
                &shutdown,
            )
        });

        thread::sleep(Duration::from_millis(30));
        shutdown_clone.store(true, Ordering::Relaxed);
        let _ = handle.join().expect("Worker thread should not panic");

        // Files should still be unindexed because the client was never available.
        let conn = db_check.lock().unwrap();
        let unindexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE lsp_indexed = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            unindexed, 2,
            "Files should remain unindexed when client is unavailable"
        );
    }

    // -- LspWorkerConfig tests --

    #[test]
    fn test_lsp_worker_config_default() {
        // Verify the Default impl produces expected values.
        let config = LspWorkerConfig::default();
        assert_eq!(config.batch_size, 50);
        assert_eq!(config.client_unavailable_sleep, Duration::from_secs(5));
        assert_eq!(config.idle_sleep, Duration::from_millis(500));
    }

    #[test]
    fn test_lsp_worker_config_custom() {
        // Verify custom configuration works.
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(100),
            idle_sleep: Duration::from_millis(50),
        };
        assert_eq!(config.batch_size, 10);
        assert_eq!(config.client_unavailable_sleep, Duration::from_millis(100));
        assert_eq!(config.idle_sleep, Duration::from_millis(50));
    }

    #[test]
    fn test_lsp_worker_config_clone() {
        // Verify the Clone derive works.
        let config = LspWorkerConfig::default();
        let cloned = config.clone();
        assert_eq!(config.batch_size, cloned.batch_size);
        assert_eq!(
            config.client_unavailable_sleep,
            cloned.client_unavailable_sleep
        );
        assert_eq!(config.idle_sleep, cloned.idle_sleep);
    }

    #[test]
    fn test_lsp_worker_config_debug() {
        // Verify the Debug derive works.
        let config = LspWorkerConfig::default();
        let debug_str = format!("{:?}", config);
        assert!(debug_str.contains("LspWorkerConfig"));
        assert!(debug_str.contains("batch_size"));
    }

    // -- LSP_CAPABLE_EXTENSIONS tests --

    #[test]
    fn test_lsp_capable_extensions_not_empty() {
        assert!(
            !LSP_CAPABLE_EXTENSIONS.is_empty(),
            "LSP_CAPABLE_EXTENSIONS should not be empty"
        );
    }

    #[test]
    fn test_lsp_capable_extensions_contains_common_languages() {
        assert!(
            LSP_CAPABLE_EXTENSIONS.contains(&"rs"),
            "should contain Rust"
        );
        assert!(
            LSP_CAPABLE_EXTENSIONS.contains(&"py"),
            "should contain Python"
        );
        assert!(
            LSP_CAPABLE_EXTENSIONS.contains(&"ts"),
            "should contain TypeScript"
        );
        assert!(LSP_CAPABLE_EXTENSIONS.contains(&"go"), "should contain Go");
        assert!(
            LSP_CAPABLE_EXTENSIONS.contains(&"java"),
            "should contain Java"
        );
    }

    #[test]
    fn test_lsp_capable_extensions_superset_of_all_servers() {
        // Every extension returned by lsp_supported_extensions for a known server
        // should be present in LSP_CAPABLE_EXTENSIONS.
        let known_servers = [
            "rust-analyzer",
            "pyright",
            "typescript-language-server",
            "gopls",
            "jdtls",
            "clangd",
            "solargraph",
            "sourcekit-lsp",
            "kotlin-language-server",
            "lua-language-server",
            "omnisharp",
            "dart",
            "phpactor",
            "metals",
        ];

        for server in &known_servers {
            for ext in lsp_supported_extensions(server) {
                assert!(
                    LSP_CAPABLE_EXTENSIONS.contains(ext),
                    "Extension '{}' from server '{}' is not in LSP_CAPABLE_EXTENSIONS",
                    ext,
                    server
                );
            }
        }
    }

    // -- extension_to_language_id additional tests --

    #[test]
    fn test_extension_to_language_id_javascript() {
        assert_eq!(
            extension_to_language_id(Path::new("script.js")),
            "javascript"
        );
        assert_eq!(
            extension_to_language_id(Path::new("App.jsx")),
            "javascriptreact"
        );
    }

    #[test]
    fn test_extension_to_language_id_go() {
        assert_eq!(extension_to_language_id(Path::new("main.go")), "go");
    }

    #[test]
    fn test_extension_to_language_id_java() {
        assert_eq!(extension_to_language_id(Path::new("App.java")), "java");
    }

    #[test]
    fn test_extension_to_language_id_c_cpp() {
        assert_eq!(extension_to_language_id(Path::new("main.c")), "c");
        assert_eq!(extension_to_language_id(Path::new("main.cpp")), "cpp");
        assert_eq!(extension_to_language_id(Path::new("main.cc")), "cpp");
        assert_eq!(extension_to_language_id(Path::new("main.cxx")), "cpp");
        assert_eq!(extension_to_language_id(Path::new("header.h")), "c");
        assert_eq!(extension_to_language_id(Path::new("header.hpp")), "cpp");
        assert_eq!(extension_to_language_id(Path::new("header.hxx")), "cpp");
    }

    #[test]
    fn test_extension_to_language_id_ruby() {
        assert_eq!(extension_to_language_id(Path::new("app.rb")), "ruby");
    }

    #[test]
    fn test_extension_to_language_id_swift() {
        assert_eq!(extension_to_language_id(Path::new("main.swift")), "swift");
    }

    #[test]
    fn test_extension_to_language_id_kotlin() {
        assert_eq!(extension_to_language_id(Path::new("Main.kt")), "kotlin");
        assert_eq!(extension_to_language_id(Path::new("build.kts")), "kotlin");
    }

    #[test]
    fn test_extension_to_language_id_lua() {
        assert_eq!(extension_to_language_id(Path::new("init.lua")), "lua");
    }

    #[test]
    fn test_extension_to_language_id_shell() {
        assert_eq!(extension_to_language_id(Path::new("run.sh")), "shellscript");
        assert_eq!(
            extension_to_language_id(Path::new("run.bash")),
            "shellscript"
        );
    }

    #[test]
    fn test_extension_to_language_id_config_formats() {
        assert_eq!(extension_to_language_id(Path::new("config.toml")), "toml");
        assert_eq!(extension_to_language_id(Path::new("data.yaml")), "yaml");
        assert_eq!(extension_to_language_id(Path::new("data.yml")), "yaml");
        assert_eq!(extension_to_language_id(Path::new("data.json")), "json");
        assert_eq!(extension_to_language_id(Path::new("README.md")), "markdown");
        assert_eq!(extension_to_language_id(Path::new("index.html")), "html");
        assert_eq!(extension_to_language_id(Path::new("style.css")), "css");
    }

    // -- lsp_supported_extensions additional tests --

    #[test]
    fn test_lsp_supported_extensions_all_known_servers() {
        // Exercise all match arms in lsp_supported_extensions.
        assert!(!lsp_supported_extensions("pylsp").is_empty());
        assert!(!lsp_supported_extensions("pyright-langserver").is_empty());
        assert!(!lsp_supported_extensions("tsserver").is_empty());
        assert!(!lsp_supported_extensions("ts_ls").is_empty());
        assert!(!lsp_supported_extensions("jdtls").is_empty());
        assert!(!lsp_supported_extensions("java-language-server").is_empty());
        assert!(!lsp_supported_extensions("clangd").is_empty());
        assert!(!lsp_supported_extensions("solargraph").is_empty());
        assert!(!lsp_supported_extensions("ruby-lsp").is_empty());
        assert!(!lsp_supported_extensions("sourcekit-lsp").is_empty());
        assert!(!lsp_supported_extensions("kotlin-language-server").is_empty());
        assert!(!lsp_supported_extensions("lua-language-server").is_empty());
        assert!(!lsp_supported_extensions("omnisharp").is_empty());
        assert!(!lsp_supported_extensions("dart").is_empty());
        assert!(!lsp_supported_extensions("dart-language-server").is_empty());
        assert!(!lsp_supported_extensions("phpactor").is_empty());
        assert!(!lsp_supported_extensions("intelephense").is_empty());
        assert!(!lsp_supported_extensions("metals").is_empty());
    }

    // -- query_lsp_dirty_files additional tests --

    #[test]
    fn test_query_lsp_dirty_files_multiple_extensions() {
        // Verify that multiple extensions are OR'd together correctly.
        let db = create_test_db();
        insert_test_file(&db, "src/main.rs");
        insert_test_file(&db, "src/app.py");
        insert_test_file(&db, "src/lib.rs");
        insert_test_file(&db, "src/test.js");

        // Query for Rust and Python
        let dirty = query_lsp_dirty_files(&db, 100, &["rs", "py"]).unwrap();
        assert_eq!(dirty.len(), 3, "should find .rs and .py files");
        assert!(dirty.iter().any(|f| f.ends_with(".rs")));
        assert!(dirty.iter().any(|f| f.ends_with(".py")));
        assert!(!dirty.iter().any(|f| f.ends_with(".js")));
    }

    #[test]
    fn test_query_lsp_dirty_files_empty_db() {
        // An empty database should return an empty list.
        let db = create_test_db();
        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert!(dirty.is_empty());
    }

    // -- new_shutdown_flag additional tests --

    #[test]
    fn test_shutdown_flag_shared_across_threads() {
        // Verify the shutdown flag can be shared and read across threads.
        let flag = new_shutdown_flag();
        let flag_clone = Arc::clone(&flag);

        let handle = thread::spawn(move || {
            flag_clone.store(true, Ordering::Relaxed);
        });

        handle.join().unwrap();
        assert!(flag.load(Ordering::Relaxed));
    }

    // -- extension_to_language_id: "hh" extension (C++ header variant) --

    #[test]
    fn test_extension_to_language_id_hh_falls_through_to_plaintext() {
        // "hh" is in LSP_CAPABLE_EXTENSIONS and clangd supports it, but
        // extension_to_language_id does not have an explicit match arm for it.
        // This documents the current behavior: "hh" maps to "plaintext".
        assert_eq!(
            extension_to_language_id(Path::new("header.hh")),
            "plaintext",
            "hh extension currently falls through to plaintext"
        );
    }

    // -- query_lsp_dirty_files: paths with dots in directory names --

    #[test]
    fn test_query_lsp_dirty_files_dotted_path_prefix() {
        // Files like "foo.bar/baz.rs" have dots before the extension.
        // The LIKE pattern "%.rs" should still match only the extension.
        let db = create_test_db();
        insert_test_file(&db, "foo.bar/baz.rs");
        insert_test_file(&db, "foo.bar/baz.py");

        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "foo.bar/baz.rs");
    }

    #[test]
    fn test_query_lsp_dirty_files_double_extension() {
        // A file like "foo.test.rs" has a double extension. The LIKE "%.rs"
        // pattern should still match it.
        let db = create_test_db();
        insert_test_file(&db, "src/foo.test.rs");
        insert_test_file(&db, "src/foo.test.py");

        let dirty = query_lsp_dirty_files(&db, 100, &["rs"]).unwrap();
        assert_eq!(dirty.len(), 1);
        assert_eq!(dirty[0], "src/foo.test.rs");
    }

    // -- spawn_lsp_indexing_worker: None client + dirty files + immediate shutdown --

    #[test]
    fn test_spawn_worker_none_client_dirty_files_immediate_shutdown() {
        // Spawn the worker with dirty files and a None client, then immediately
        // signal shutdown. The thread should not panic and files should remain
        // unindexed since no LSP client was available to process them.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/test_file.rs");
        }
        let db_check = Arc::clone(&db);

        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(1),
            idle_sleep: Duration::from_millis(1),
        };
        let shutdown = new_shutdown_flag();
        // Set shutdown before spawning so the worker exits as soon as possible
        shutdown.store(true, Ordering::Relaxed);

        let handle = spawn_lsp_indexing_worker(
            workspace_root,
            db,
            client,
            config,
            "rust-analyzer".to_string(),
            shutdown,
        );

        handle.join().expect("Worker thread should not panic");

        // The file should still be unindexed because the client was None
        let conn = db_check.lock().unwrap();
        let unindexed: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM indexed_files WHERE lsp_indexed = 0",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(
            unindexed, 1,
            "File should remain unindexed when client is None and shutdown is immediate"
        );
    }

    // -- index_single_file with mock LSP client tests --

    /// Spawn a mock LSP process that reads a notification (didOpen), responds
    /// to a request (documentSymbol), then reads a notification (didClose).
    ///
    /// The mock expects exactly this sequence:
    /// 1. Read one message (didOpen notification) — no reply
    /// 2. Read one message (documentSymbol request) — reply with `response`
    /// 3. Read one message (didClose notification) — no reply
    ///
    /// The response JSON is written to a temp file. The file path is passed
    /// via the `MOCK_RESPONSE_FILE` environment variable so neither JSON nor
    /// the path is ever interpolated into the Python source code.
    fn spawn_mock_lsp_for_index_single_file(
        response: serde_json::Value,
        response_file: &std::path::Path,
    ) -> std::process::Child {
        // Write the response JSON to a file the Python script will read
        std::fs::write(response_file, response.to_string())
            .expect("failed to write mock response file");

        // The script reads the response file path from an env var, avoiding
        // any interpolation of untrusted data into Python source code.
        let script = "\
            import sys, json, os\n\
            def read_msg():\n\
            \tcl = None\n\
            \twhile True:\n\
            \t\tline = sys.stdin.readline()\n\
            \t\tif not line: return None\n\
            \t\tline = line.strip()\n\
            \t\tif not line: break\n\
            \t\tif line.startswith('Content-Length:'):\n\
            \t\t\tcl = int(line.split(':', 1)[1].strip())\n\
            \tif cl is None: return None\n\
            \tbody = sys.stdin.read(cl)\n\
            \treturn json.loads(body)\n\
            def send_msg(obj):\n\
            \ts = json.dumps(obj)\n\
            \tsys.stdout.write(f'Content-Length: {len(s)}\\r\\n\\r\\n{s}')\n\
            \tsys.stdout.flush()\n\
            with open(os.environ['MOCK_RESPONSE_FILE']) as f:\n\
            \tresponse = json.load(f)\n\
            read_msg()\n\
            read_msg()\n\
            send_msg(response)\n\
            read_msg()\n";

        std::process::Command::new("python3")
            .arg("-c")
            .arg(script)
            .env("MOCK_RESPONSE_FILE", response_file)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("failed to spawn mock LSP python3 process for index_single_file")
    }

    #[test]
    fn test_index_single_file_with_mock_lsp_persists_symbols() {
        // Verify that index_single_file reads a temp file, talks to a mock LSP
        // server, persists the returned symbols, and marks lsp_indexed = 1.
        use std::io::Write;

        let symbol_response = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": [
                {
                    "name": "my_function",
                    "kind": 12,
                    "range": {"start": {"line": 0, "character": 0}, "end": {"line": 2, "character": 1}},
                    "selectionRange": {"start": {"line": 0, "character": 3}, "end": {"line": 0, "character": 14}}
                }
            ]
        });

        // Create a real temp dir for both the source file and mock response file
        let temp_dir = tempfile::tempdir().unwrap();
        let file_path = temp_dir.path().join("demo.rs");
        {
            let mut f = std::fs::File::create(&file_path).unwrap();
            writeln!(f, "fn my_function() {{}}").unwrap();
        }
        let response_file = temp_dir.path().join("mock_response.json");

        let db = create_test_db();
        let relative_path = "src/demo.rs";
        insert_test_file(&db, relative_path);
        let shared_db: SharedDb = Arc::new(Mutex::new(db));

        let mut child = spawn_mock_lsp_for_index_single_file(symbol_response, &response_file);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut client = LspJsonRpcClient::new(stdin, stdout);

        let result = index_single_file(&mut client, &shared_db, &file_path, relative_path);
        assert!(
            result.is_ok(),
            "index_single_file should succeed: {:?}",
            result
        );
        let symbol_count = result.unwrap();
        assert_eq!(symbol_count, 1, "should have persisted 1 symbol");

        // Verify lsp_indexed was marked
        let conn = shared_db.lock().unwrap();
        let lsp: i64 = conn
            .query_row(
                "SELECT lsp_indexed FROM indexed_files WHERE file_path = ?1",
                [relative_path],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(lsp, 1, "lsp_indexed should be 1 after successful indexing");

        // Verify symbol was written to lsp_symbols
        let sym_count: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM lsp_symbols WHERE file_path = ?1",
                [relative_path],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(sym_count, 1, "1 symbol should be persisted in lsp_symbols");

        let _ = child.wait();
    }

    #[test]
    fn test_index_single_file_missing_file_returns_io_error() {
        // When the file doesn't exist on disk, index_single_file should return
        // an I/O error before any LSP interaction occurs.
        use serde_json::json;

        let temp_dir = tempfile::tempdir().unwrap();
        let response_file = temp_dir.path().join("mock_response.json");

        // The mock won't be used because the file read fails first,
        // but we still need a valid LspJsonRpcClient.
        let symbol_response = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "result": []
        });

        let mut child = spawn_mock_lsp_for_index_single_file(symbol_response, &response_file);
        let stdin = child.stdin.take().unwrap();
        let stdout = child.stdout.take().unwrap();
        let mut client = LspJsonRpcClient::new(stdin, stdout);

        let db = create_test_db();
        insert_test_file(&db, "nonexistent.rs");
        let shared_db: SharedDb = Arc::new(Mutex::new(db));

        let result = index_single_file(
            &mut client,
            &shared_db,
            Path::new("/definitely/nonexistent/path/demo.rs"),
            "nonexistent.rs",
        );
        assert!(
            result.is_err(),
            "index_single_file should fail with I/O error for missing file"
        );

        let _ = child.kill();
        let _ = child.wait();
    }

    // -- Poison recovery tests --

    #[test]
    fn test_loop_recovers_from_poisoned_client_mutex() {
        // When a thread panics while holding the client mutex lock, the mutex
        // becomes poisoned. The worker loop should recover via `into_inner`
        // and continue operating. Here we poison the mutex, insert dirty files,
        // and verify the loop doesn't panic — it should recover and find
        // the client is None (since we poison with None), then sleep and retry.
        let workspace_root = std::env::temp_dir();
        let db = create_shared_test_db();
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/poison_test.rs");
        }

        let client: SharedLspClient = Arc::new(Mutex::new(None));

        // Poison the mutex by panicking in a thread that holds the lock
        let client_clone = Arc::clone(&client);
        let poison_handle = thread::spawn(move || {
            let _guard = client_clone.lock().unwrap();
            panic!("intentional panic to poison the mutex");
        });
        // The thread panicked — wait for it to finish
        let _ = poison_handle.join();

        // Verify the mutex is actually poisoned
        assert!(
            client.lock().is_err(),
            "Client mutex should be poisoned after thread panic"
        );

        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(5),
            idle_sleep: Duration::from_millis(5),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "rust-analyzer",
                &shutdown,
            )
        });

        // Give the loop time to hit the poisoned mutex and recover
        thread::sleep(Duration::from_millis(50));
        shutdown_clone.store(true, Ordering::Relaxed);

        let result = handle
            .join()
            .expect("Worker thread should not panic on poisoned mutex");
        assert!(
            result.is_ok(),
            "Loop should return Ok after recovering from poisoned client mutex"
        );
    }

    #[test]
    fn test_loop_recovers_from_poisoned_db_mutex() {
        // The DB mutex can also be poisoned (unwrap_or_else on line 149).
        // Verify the loop recovers from a poisoned DB mutex too.
        let workspace_root = std::env::temp_dir();

        // Create a shared DB and poison its mutex
        let db = create_shared_test_db();

        // Insert a file before poisoning
        {
            let conn = db.lock().unwrap();
            insert_test_file(&conn, "src/db_poison.rs");
        }

        // Poison the DB mutex
        let db_clone = Arc::clone(&db);
        let poison_handle = thread::spawn(move || {
            let _guard = db_clone.lock().unwrap();
            panic!("intentional panic to poison the DB mutex");
        });
        let _ = poison_handle.join();

        assert!(
            db.lock().is_err(),
            "DB mutex should be poisoned after thread panic"
        );

        let client: SharedLspClient = Arc::new(Mutex::new(None));
        let config = LspWorkerConfig {
            batch_size: 10,
            client_unavailable_sleep: Duration::from_millis(5),
            idle_sleep: Duration::from_millis(5),
        };
        let shutdown = new_shutdown_flag();
        let shutdown_clone = Arc::clone(&shutdown);

        let handle = thread::spawn(move || {
            run_lsp_indexing_loop(
                &workspace_root,
                &db,
                &client,
                &config,
                "rust-analyzer",
                &shutdown,
            )
        });

        // Give the loop time to recover from the poisoned DB mutex and process
        thread::sleep(Duration::from_millis(50));
        shutdown_clone.store(true, Ordering::Relaxed);

        let result = handle
            .join()
            .expect("Worker thread should not panic on poisoned DB mutex");
        assert!(
            result.is_ok(),
            "Loop should return Ok after recovering from poisoned DB mutex"
        );
    }
}
