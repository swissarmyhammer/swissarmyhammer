//! Code-context Doctor -- Diagnostic checks for code-context setup and configuration.
//!
//! Checks:
//! - Git repository (warning if not found)
//! - code-context binary in PATH
//! - `.code-context/` index directory existence
//! - LSP server availability per detected project type
//! - Semantic search smoke (does the canary query return a match?)

use std::env;
use std::path::{Path, PathBuf};

use model_embedding::TextEmbedder;
use rusqlite::{Connection, OpenFlags};
use swissarmyhammer_code_context::{search_code, SearchCodeOptions};
use swissarmyhammer_doctor::{Check, CheckStatus, DoctorRunner};
use swissarmyhammer_embedding::Embedder;
use swissarmyhammer_tools::mcp::tools::code_context::doctor as cc_doctor;

/// Canary query for the semantic-search smoke check.
///
/// Picked to be generic enough that any real source tree should produce at
/// least one nearby chunk. The semantic claim being verified is "embeddings
/// loaded from the DB rank against query embeddings", not "this specific
/// chunk wins" — so any non-empty match set passes.
const SEMANTIC_SEARCH_CANARY_QUERY: &str = "function that handles errors";

/// Name used in the [`Check`] this module emits for the semantic-search probe.
const SEMANTIC_SEARCH_CHECK_NAME: &str = "Semantic Search";

/// Code-context diagnostic runner.
///
/// Accumulates [`Check`]s for code-context setup: git repo detection,
/// binary availability, index directory presence, and LSP server status.
pub struct CodeContextDoctor {
    checks: Vec<Check>,
}

impl DoctorRunner for CodeContextDoctor {
    /// Returns immutable reference to accumulated checks.
    fn checks(&self) -> &[Check] {
        &self.checks
    }

    /// Returns mutable reference to accumulated checks.
    fn checks_mut(&mut self) -> &mut Vec<Check> {
        &mut self.checks
    }
}

impl CodeContextDoctor {
    /// Create a new CodeContextDoctor with no checks.
    pub fn new() -> Self {
        Self { checks: Vec::new() }
    }

    /// Run all code-context diagnostic checks.
    ///
    /// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
    ///
    /// Async because the semantic-search smoke probe loads an embedding model
    /// via [`Embedder::default`], which is async.
    pub async fn run_diagnostics(&mut self) -> i32 {
        self.check_git_repository();
        self.check_code_context_in_path();
        self.check_index_directory();
        self.check_lsp_status();
        self.check_semantic_search_status().await;

        self.get_exit_code()
    }

    /// Check if we're in a Git repository.
    ///
    /// This is a warning (not error) since code-context can work outside git repos,
    /// but many features depend on repository context.
    fn check_git_repository(&mut self) {
        use swissarmyhammer_common::utils::find_git_repository_root;

        match find_git_repository_root() {
            Some(path) => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Detected at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "Git Repository".to_string(),
                    status: CheckStatus::Warning,
                    message: "Not in a Git repository".to_string(),
                    fix: Some("Run from within a Git repository or run `git init`".to_string()),
                });
            }
        }
    }

    /// Check if the code-context binary is in PATH.
    fn check_code_context_in_path(&mut self) {
        let path_var = env::var("PATH").unwrap_or_default();
        let paths: Vec<PathBuf> = env::split_paths(&path_var).collect();

        let exe_name = if cfg!(windows) {
            "code-context.exe"
        } else {
            "code-context"
        };

        let mut found_path = None;
        for path in paths {
            let exe_path = path.join(exe_name);
            if exe_path.exists() {
                found_path = Some(exe_path);
                break;
            }
        }

        match found_path {
            Some(path) => {
                self.add_check(Check {
                    name: "code-context in PATH".to_string(),
                    status: CheckStatus::Ok,
                    message: format!("Found at {}", path.display()),
                    fix: None,
                });
            }
            None => {
                self.add_check(Check {
                    name: "code-context in PATH".to_string(),
                    status: CheckStatus::Warning,
                    message: "code-context not found in PATH".to_string(),
                    fix: Some(
                        "Add code-context to your PATH or install with \
                         `cargo install --path code-context-cli`"
                            .to_string(),
                    ),
                });
            }
        }
    }

    /// Check if the `.code-context/` index directory exists in the current working directory.
    fn check_index_directory(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let index_dir = cwd.join(".code-context");

        if index_dir.is_dir() {
            self.add_check(Check {
                name: "Index Directory".to_string(),
                status: CheckStatus::Ok,
                message: format!("Found at {}", index_dir.display()),
                fix: None,
            });
        } else {
            self.add_check(Check {
                name: "Index Directory".to_string(),
                status: CheckStatus::Warning,
                message: "No .code-context/ directory found".to_string(),
                fix: Some(
                    "Run `code-context serve` to initialize the index, or start an MCP session"
                        .to_string(),
                ),
            });
        }
    }

    /// Check semantic-search smoke: open the workspace's index, count embedded
    /// chunks, and (if any exist) embed a canary query through the production
    /// embedder and assert [`search_code`] returns at least one match.
    ///
    /// Delegates to [`check_semantic_search`] so the same logic can be exercised
    /// in unit tests against a synthetic workspace root.
    async fn check_semantic_search_status(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let check = check_semantic_search(&cwd).await;
        self.add_check(check);
    }

    /// Check LSP availability for detected project types.
    ///
    /// Delegates to [`cc_doctor::run_doctor`] to detect project types and probe
    /// each LSP server. Creates one check per LSP server — Ok if installed,
    /// Warning with an install hint if not.
    fn check_lsp_status(&mut self) {
        let cwd = env::current_dir().unwrap_or_default();
        let report = cc_doctor::run_doctor(&cwd);

        for lsp in &report.lsp_servers {
            if lsp.installed {
                self.add_check(Check {
                    name: format!("LSP: {}", lsp.name),
                    status: CheckStatus::Ok,
                    message: match &lsp.path {
                        Some(p) => format!("Installed at {}", p),
                        None => "Installed".to_string(),
                    },
                    fix: None,
                });
            } else {
                let message = match &lsp.error {
                    Some(err) => format!("{} found but failed: {}", lsp.name, err),
                    None => format!("{} not found", lsp.name),
                };
                self.add_check(Check {
                    name: format!("LSP: {}", lsp.name),
                    status: CheckStatus::Warning,
                    message,
                    fix: lsp.install_hint.clone(),
                });
            }
        }
    }
}

impl Default for CodeContextDoctor {
    fn default() -> Self {
        Self::new()
    }
}

/// Filename of the SQLite database inside `.code-context/`.
const CONTEXT_DB_FILENAME: &str = "index.db";

/// Directory name of the code-context workspace inside a workspace root.
const CONTEXT_DIR_NAME: &str = ".code-context";

/// Construct the default [`Embedder`] and load its model weights.
///
/// On either failure, returns a `Warning` [`Check`] describing the problem in
/// terms the user can act on. We deliberately downgrade missing-model errors
/// to a warning (not an error) so a workspace that has embeddings but is
/// being inspected on a machine without the model weights still reports
/// "embeddings present, but I cannot probe them here" rather than "broken".
///
/// `embedded_count` is interpolated into the warning message to give the
/// user context about what state the DB is actually in.
async fn load_default_embedder(embedded_count: i64) -> Result<Embedder, Check> {
    let embedder = Embedder::default().await.map_err(|e| Check {
        name: SEMANTIC_SEARCH_CHECK_NAME.to_string(),
        status: CheckStatus::Warning,
        message: format!(
            "Could not construct embedder for canary query: {e}. \
             {embedded_count} chunks have embeddings in the DB."
        ),
        fix: Some(
            "Ensure the default embedding model is available; \
             run with --debug for more detail"
                .to_string(),
        ),
    })?;

    embedder.load().await.map_err(|e| Check {
        name: SEMANTIC_SEARCH_CHECK_NAME.to_string(),
        status: CheckStatus::Warning,
        message: format!(
            "Could not load embedding model for canary query: {e}. \
             {embedded_count} chunks have embeddings in the DB."
        ),
        fix: Some("Ensure the default embedding model weights are downloaded".to_string()),
    })?;

    Ok(embedder)
}

/// Probe semantic search end-to-end against the index at `root/.code-context/index.db`.
///
/// Three states are reported via the returned [`Check`]:
///
/// * `✓ Semantic search functional (canary returned N matches).`
///   — at least one chunk has an embedding **and** [`search_code`] returned a
///   non-empty result for [`SEMANTIC_SEARCH_CANARY_QUERY`] with a strictly
///   positive cosine similarity.
/// * `⚠ Semantic search index is empty — no chunks have embeddings.`
///   — the DB exists but `ts_chunks` has zero rows with a non-NULL
///   `embedding` blob. Treated as a warning, not an error: this is the
///   expected state on a fresh workspace before indexing has run.
/// * `❌ Semantic search returned no results for canary query — ...`
///   — embeddings are present but no chunk scored above
///   [`f32::EPSILON`] against the canary query. `cosine_similarity`
///   returns `0.0` for vectors of differing dimensions, so this is the
///   signal that the query embedding dimension does not match the stored
///   embeddings (i.e. the model used to index is different from the model
///   doctor just loaded). Reported as an error.
///
/// If `.code-context/` or `index.db` is missing, the check reports a warning
/// pointing the user at `code-context serve` to bootstrap the index.
///
/// The check opens its own read-only SQLite connection rather than going
/// through `CodeContextWorkspace::open`, because the latter contests the
/// leader lock and may attempt write operations. Doctor must be observation-
/// only.
pub async fn check_semantic_search(root: &Path) -> Check {
    let conn = match open_index_for_doctor(root) {
        Ok(c) => c,
        Err(check) => return check,
    };

    let embedded_count = count_embedded_chunks(&conn);
    if embedded_count == 0 {
        return semantic_search_check(
            CheckStatus::Warning,
            "Semantic search index is empty — no chunks have embeddings yet.",
            Some(
                "Wait for the indexing worker to embed chunks, or run \
                 `code-context build status` to trigger re-indexing",
            ),
        );
    }

    let embedder = match load_default_embedder(embedded_count).await {
        Ok(e) => e,
        Err(check) => return check,
    };

    run_canary_query(&conn, &embedder, embedded_count).await
}

/// Build a `Check` whose name is fixed to [`SEMANTIC_SEARCH_CHECK_NAME`].
fn semantic_search_check(
    status: CheckStatus,
    message: impl Into<String>,
    fix: Option<&str>,
) -> Check {
    Check {
        name: SEMANTIC_SEARCH_CHECK_NAME.to_string(),
        status,
        message: message.into(),
        fix: fix.map(str::to_string),
    }
}

/// Open the index DB read-only for doctor inspection.
///
/// Returns a populated `Check` on missing-file or open failure so the caller
/// can early-return. Doctor must never contest the leader write lock — hence
/// `SQLITE_OPEN_READ_ONLY | SQLITE_OPEN_NO_MUTEX`.
fn open_index_for_doctor(root: &Path) -> Result<Connection, Check> {
    let db_path = root.join(CONTEXT_DIR_NAME).join(CONTEXT_DB_FILENAME);

    if !db_path.exists() {
        return Err(semantic_search_check(
            CheckStatus::Warning,
            format!(
                "No .code-context/{CONTEXT_DB_FILENAME} found — semantic search index has not been built."
            ),
            Some("Run `code-context serve` or start an MCP session to initialize the index"),
        ));
    }

    Connection::open_with_flags(
        &db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .map_err(|e| {
        semantic_search_check(
            CheckStatus::Error,
            format!("Failed to open index DB at {}: {e}", db_path.display()),
            None,
        )
    })
}

/// Count chunks with a non-NULL `embedding` blob. Missing table → 0.
fn count_embedded_chunks(conn: &Connection) -> i64 {
    conn.query_row(
        "SELECT COUNT(*) FROM ts_chunks WHERE embedding IS NOT NULL",
        [],
        |row| row.get(0),
    )
    .unwrap_or_default()
}

/// Embed the canary query, run `search_code`, classify the result.
///
/// Returns `Ok` when at least one match scores above `f32::EPSILON`,
/// `Error` when zero matches come back (signals dimension mismatch — see
/// the function-level doc on [`check_semantic_search`] for why
/// `min_similarity` must be strictly positive).
async fn run_canary_query(conn: &Connection, embedder: &Embedder, embedded_count: i64) -> Check {
    let query_embedding = match embedder.embed_text(SEMANTIC_SEARCH_CANARY_QUERY).await {
        Ok(r) => r.embedding().to_vec(),
        Err(e) => {
            return semantic_search_check(
                CheckStatus::Error,
                format!(
                    "Embedder failed to embed canary query \"{SEMANTIC_SEARCH_CANARY_QUERY}\": {e}"
                ),
                None,
            );
        }
    };

    let options = SearchCodeOptions {
        top_k: 1,
        min_similarity: f32::EPSILON,
        language: None,
        file_pattern: None,
    };
    match search_code(conn, &query_embedding, &options) {
        Ok(r) => classify_canary_matches(r.matches.len(), embedded_count),
        Err(e) => semantic_search_check(
            CheckStatus::Error,
            format!("search_code() failed against the canary query: {e}"),
            None,
        ),
    }
}

/// Turn a canary `search_code` match count into a `Check`.
fn classify_canary_matches(match_count: usize, embedded_count: i64) -> Check {
    if match_count == 0 {
        return semantic_search_check(
            CheckStatus::Error,
            format!(
                "Semantic search returned no results for canary query \"{SEMANTIC_SEARCH_CANARY_QUERY}\" \
                 (searched {embedded_count} embedded chunks above min_similarity=f32::EPSILON). \
                 The query embedding dimension likely does not match the stored embeddings — \
                 `cosine_similarity` returns 0.0 for mismatched vector lengths."
            ),
            Some(
                "Re-index the workspace with the current embedding model: \
                 run `code-context clear status` then `code-context build status`",
            ),
        );
    }

    semantic_search_check(
        CheckStatus::Ok,
        format!(
            "Semantic search functional (canary returned {match_count} match{} from {embedded_count} embedded chunks).",
            if match_count == 1 { "" } else { "es" },
        ),
        None,
    )
}

/// Run the doctor command and display results.
///
/// Returns an exit code: 0 for success, 1 for warnings, 2 for errors.
///
/// Async because the semantic-search smoke probe loads an embedding model
/// via [`Embedder::default`], which is async.
pub async fn run_doctor(verbose: bool) -> i32 {
    let mut doctor = CodeContextDoctor::new();
    let exit_code = doctor.run_diagnostics().await;
    doctor.print_table(verbose);
    exit_code
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::params;
    use swissarmyhammer_code_context::{db, serialize_embedding};
    use tempfile::TempDir;

    /// Build a minimal `.code-context/` workspace inside `root` with a freshly
    /// created (but otherwise empty) `index.db`. Returns the read-write
    /// connection so the caller can populate it.
    ///
    /// The connection is closed when dropped; subsequent reads in tests
    /// reopen the DB with read-only flags, matching the production path
    /// taken by [`check_semantic_search`].
    fn make_empty_index(root: &Path) -> Connection {
        let context_dir = root.join(CONTEXT_DIR_NAME);
        std::fs::create_dir_all(&context_dir).expect("create .code-context");
        let db_path = context_dir.join(CONTEXT_DB_FILENAME);
        let conn = Connection::open(&db_path).expect("open db");
        db::create_schema(&conn).expect("create schema");
        conn
    }

    /// Insert an embedded chunk row into `ts_chunks` so the canary path has
    /// something to score against. Uses the same encoding [`search_code`]
    /// expects: little-endian f32 blobs via [`serialize_embedding`].
    fn insert_embedded_chunk(conn: &Connection, file_path: &str, text: &str, embedding: &[f32]) {
        // First insert into indexed_files so the FK / progress logic is consistent.
        // All NOT NULL columns must be populated; we use stub values since this
        // is a test fixture.
        let stub_hash: Vec<u8> = vec![0u8; 16];
        conn.execute(
            "INSERT OR IGNORE INTO indexed_files \
             (file_path, content_hash, file_size, last_seen_at, embedded) \
             VALUES (?1, ?2, ?3, ?4, 1)",
            params![file_path, stub_hash, text.len() as i64, 0i64],
        )
        .expect("insert indexed_files");
        let blob = serialize_embedding(embedding);
        conn.execute(
            "INSERT INTO ts_chunks \
             (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding) \
             VALUES (?1, 0, ?2, 1, 1, ?3, ?4, ?5)",
            params![file_path, text.len() as i64, "canary_symbol", text, blob],
        )
        .expect("insert ts_chunks");
    }

    #[test]
    fn test_new() {
        let doctor = CodeContextDoctor::new();
        assert!(doctor.checks().is_empty());
    }

    #[tokio::test]
    async fn test_run_diagnostics() {
        let mut doctor = CodeContextDoctor::new();
        let exit_code = doctor.run_diagnostics().await;

        // Should have at least 4 checks: git, path, index directory, semantic search
        // (LSP checks depend on detected project types, so the minimum is 4)
        assert!(
            doctor.checks().len() >= 4,
            "expected >= 4 checks, got {}",
            doctor.checks().len()
        );

        // Exit code should be 0, 1, or 2
        assert!(exit_code <= 2, "exit code was {}", exit_code);
    }

    #[test]
    fn test_check_git_repository() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_git_repository();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Git Repository");
        // Status depends on whether we're actually in a git repo
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_code_context_in_path() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_code_context_in_path();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "code-context in PATH");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_index_directory() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_index_directory();

        // Should produce exactly one check
        assert_eq!(doctor.checks().len(), 1);

        let check = &doctor.checks()[0];
        assert_eq!(check.name, "Index Directory");
        assert!(check.status == CheckStatus::Ok || check.status == CheckStatus::Warning);
    }

    #[test]
    fn test_check_lsp_status() {
        let mut doctor = CodeContextDoctor::new();
        doctor.check_lsp_status();

        // The number of checks depends on detected project types in the working
        // directory. Every check that is produced must be structurally valid:
        // non-empty name, non-empty message, and a recognized status.
        for check in doctor.checks() {
            assert!(!check.name.is_empty(), "check name must not be empty");
            assert!(!check.message.is_empty(), "check message must not be empty");
            assert!(
                check.status == CheckStatus::Ok || check.status == CheckStatus::Warning,
                "LSP check status should be Ok or Warning, got {:?}",
                check.status
            );
            // All LSP checks are prefixed with "LSP: "
            assert!(
                check.name.starts_with("LSP: "),
                "expected LSP check name to start with 'LSP: ', got {:?}",
                check.name
            );
        }
    }

    #[test]
    fn test_default() {
        let doctor = CodeContextDoctor::default();
        assert!(doctor.checks().is_empty());
    }

    #[tokio::test]
    async fn test_run_doctor() {
        let exit_code = run_doctor(false).await;
        assert!(exit_code <= 2);
    }

    #[tokio::test]
    async fn test_run_doctor_verbose() {
        let exit_code = run_doctor(true).await;
        assert!(exit_code <= 2);
    }

    /// When the workspace has no `.code-context/` directory at all,
    /// the check must report a warning that points at how to create one,
    /// not crash and not return an error severity.
    #[tokio::test]
    async fn check_semantic_search_missing_index_warns() {
        let tmp = TempDir::new().expect("tempdir");
        let check = check_semantic_search(tmp.path()).await;
        assert_eq!(check.name, SEMANTIC_SEARCH_CHECK_NAME);
        assert_eq!(
            check.status,
            CheckStatus::Warning,
            "missing index must be a warning, not an error: {check:?}"
        );
        assert!(
            check.message.contains("not been built") || check.message.contains("No .code-context"),
            "message should mention missing index, got {:?}",
            check.message
        );
        assert!(
            check.fix.is_some(),
            "missing-index warning should suggest a fix"
        );
    }

    /// When `.code-context/index.db` exists but `ts_chunks` has no embedded
    /// rows, the check must report `Warning` with an "empty index" message.
    /// It must NOT attempt to load the embedder (that would be a slow false
    /// positive on a freshly-cloned workspace).
    #[tokio::test]
    async fn check_semantic_search_empty_index_warns() {
        let tmp = TempDir::new().expect("tempdir");
        // Drop the rw connection before reopening the DB read-only inside the
        // check. SQLite tolerates concurrent handles, but the convention in
        // this codebase is "one writer at a time" — match it.
        let conn = make_empty_index(tmp.path());
        drop(conn);

        let check = check_semantic_search(tmp.path()).await;
        assert_eq!(check.name, SEMANTIC_SEARCH_CHECK_NAME);
        assert_eq!(
            check.status,
            CheckStatus::Warning,
            "empty index must be a warning, got: {check:?}"
        );
        assert!(
            check.message.contains("empty") || check.message.contains("no chunks have embeddings"),
            "message should mention emptiness, got {:?}",
            check.message
        );
    }

    /// When the stored embedding's dimension does not match what the default
    /// embedder produces, `cosine_similarity` returns `0.0` and the
    /// `min_similarity: f32::EPSILON` filter in [`check_semantic_search`]
    /// drops the row. The check must report `Error`, not `Ok`.
    ///
    /// This is the regression guard for the review finding: with
    /// `min_similarity: 0.0`, a dimension-mismatched chunk slipped through
    /// the filter as a "match" and the doctor falsely reported `Ok`.
    ///
    /// The embedding model is gated: if `Embedder::default()` or `load()`
    /// fails (e.g., model weights not downloaded in this environment), the
    /// check short-circuits to `Warning` before reaching the search step,
    /// which is an acceptable outcome — the test only asserts `Ok` is not
    /// produced.
    #[tokio::test]
    async fn check_semantic_search_dimension_mismatch_is_not_ok() {
        let tmp = TempDir::new().expect("tempdir");
        let conn = make_empty_index(tmp.path());
        // A unit-vector with a dimension (3) that cannot match the default
        // embedder's output dimension. cosine_similarity returns 0.0 for
        // mismatched lengths, so this row must be filtered out.
        insert_embedded_chunk(
            &conn,
            "src/canary.rs",
            "fn handle_error() {}",
            &[1.0, 0.0, 0.0],
        );
        drop(conn);

        let check = check_semantic_search(tmp.path()).await;
        assert_eq!(check.name, SEMANTIC_SEARCH_CHECK_NAME);
        assert_ne!(
            check.status,
            CheckStatus::Ok,
            "dimension-mismatched chunks must not produce false-positive Ok: {check:?}"
        );
        // Acceptable outcomes:
        //  * Error  — embedder loaded, search returned no matches.
        //  * Warning — embedder unavailable on this machine.
        assert!(
            check.status == CheckStatus::Error || check.status == CheckStatus::Warning,
            "status should be Error or Warning, got {:?}",
            check.status
        );
    }

    /// When the stored embedding matches the embedder's output dimension,
    /// the canary should produce at least one match above `f32::EPSILON`
    /// and the check should report `Ok`.
    ///
    /// We learn the embedder's actual output dimension at runtime by
    /// embedding a probe string. If the embedder is unavailable on this
    /// machine, the test is skipped — the contract being tested is "no
    /// false-negative Error when dimensions agree".
    #[tokio::test]
    async fn check_semantic_search_with_matching_dimension_is_functional() {
        // Probe the embedder up front so we can size the stored embedding
        // correctly. If the model isn't available, skip the test.
        let Ok(embedder) = Embedder::default().await else {
            eprintln!("skipping: default embedder unavailable");
            return;
        };
        if embedder.load().await.is_err() {
            eprintln!("skipping: default embedder failed to load");
            return;
        }
        let probe = match embedder.embed_text("probe").await {
            Ok(r) => r.embedding().to_vec(),
            Err(_) => {
                eprintln!("skipping: default embedder failed to embed probe text");
                return;
            }
        };

        let tmp = TempDir::new().expect("tempdir");
        let conn = make_empty_index(tmp.path());
        insert_embedded_chunk(&conn, "src/canary.rs", "fn handle_error() {}", &probe);
        drop(conn);

        let check = check_semantic_search(tmp.path()).await;
        assert_eq!(check.name, SEMANTIC_SEARCH_CHECK_NAME);
        assert_eq!(
            check.status,
            CheckStatus::Ok,
            "matching-dimension chunks should produce Ok: {check:?}"
        );
    }
}
