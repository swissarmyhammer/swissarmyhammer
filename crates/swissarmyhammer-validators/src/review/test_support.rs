//! Shared test fixtures for the review pipeline's test modules.
//!
//! The pipeline's stages are tested against the same three real seams — a
//! throwaway libgit2 repository, a schema-applied in-memory code_context
//! index, and a deterministically injected validator loader — so the fixtures
//! live here exactly once and the test modules in `scope.rs`, `drive.rs`, and
//! `probes.rs` import them instead of carrying their own copies. The
//! agent-facing test modules (`fleet.rs`, `verify.rs`, and the pool tests in
//! `validators/pool.rs`) share the [`new_notifier`] channel fixture the same
//! way.

use std::path::{Path, PathBuf};

use rusqlite::Connection;
use tempfile::TempDir;

use swissarmyhammer_code_context::db::{configure_connection, create_schema};
use swissarmyhammer_code_context::serialize_embedding;

use crate::validators::types::{RuleSet, RuleSetManifest, RuleSetMetadata, ValidatorMatch};
use crate::validators::{Rule, Severity, ValidatorLoader, ValidatorSource};

/// Embedding dimension shared by the seeded index and the mock embedder.
pub(crate) const DIM: usize = 4;

/// A fresh notification channel for pool-backed tests. The 64-slot buffer
/// comfortably exceeds any test's notification volume so the broadcast
/// subscription never lags mid-assertion.
pub(crate) fn new_notifier() -> std::sync::Arc<claude_agent::NotificationSender> {
    let (notifier, _) = claude_agent::NotificationSender::new(64);
    std::sync::Arc::new(notifier)
}

/// The LSP `SymbolKind` code for a function — what every [`seed_symbol`] row is.
const LSP_SYMBOL_KIND_FUNCTION: i64 = 12;

/// A deterministic embedding two chunks can share so they register as
/// duplicates. The length derives from [`DIM`] so the seeded index and the
/// mock embedder can never drift apart.
pub(crate) fn dup_emb() -> Vec<f32> {
    let mut v = vec![0.0; DIM];
    v[0] = 1.0;
    v
}

// ---- git repo fixture -------------------------------------------------

/// A throwaway git repo backed by a [`TempDir`], driven via libgit2 so the
/// pipeline's real `swissarmyhammer-git` reads see real refs/working-tree.
pub(crate) struct TestRepo {
    dir: TempDir,
    repo: git2::Repository,
}

impl TestRepo {
    pub(crate) fn new() -> Self {
        let dir = TempDir::new().unwrap();
        let repo = git2::Repository::init(dir.path()).unwrap();
        {
            let mut cfg = repo.config().unwrap();
            cfg.set_str("user.name", "Test").unwrap();
            cfg.set_str("user.email", "test@example.com").unwrap();
        }
        Self { dir, repo }
    }

    pub(crate) fn path(&self) -> &Path {
        self.dir.path()
    }

    /// Write a file to the working tree (no staging).
    pub(crate) fn write(&self, rel: &str, content: &str) {
        let full = self.dir.path().join(rel);
        if let Some(parent) = full.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(full, content).unwrap();
    }

    /// Stage everything and commit, returning the commit sha.
    pub(crate) fn commit(&self, message: &str) -> String {
        let mut index = self.repo.index().unwrap();
        index
            .add_all(["*"].iter(), git2::IndexAddOption::DEFAULT, None)
            .unwrap();
        index.write().unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = self.repo.find_tree(tree_id).unwrap();
        let sig = git2::Signature::now("Test", "test@example.com").unwrap();
        let parent = self.repo.head().ok().and_then(|h| h.peel_to_commit().ok());
        let parents: Vec<&git2::Commit> = parent.iter().collect();
        let oid = self
            .repo
            .commit(Some("HEAD"), &sig, &sig, message, &tree, &parents)
            .unwrap();
        oid.to_string()
    }
}

// ---- code_context index fixture --------------------------------------

/// Open a real, schema-applied, in-memory code_context index (same shape the
/// probe runner uses in production), seeded deterministically.
pub(crate) fn index_conn() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    configure_connection(&conn).unwrap();
    create_schema(&conn).unwrap();
    conn
}

/// Register a file in `indexed_files`. `ts_chunks` / `lsp_symbols` carry a
/// foreign key onto this table (and `configure_connection` enforces it), so
/// every seeded chunk/symbol needs its file registered first.
pub(crate) fn seed_file(conn: &Connection, file_path: &str) {
    conn.execute(
        "INSERT OR IGNORE INTO indexed_files (file_path, content_hash, file_size, last_seen_at, ts_indexed, lsp_indexed, embedded)
         VALUES (?1, X'DEADBEEF', 1024, 1000, 1, 1, 1)",
        rusqlite::params![file_path],
    )
    .unwrap();
}

/// Seed a `ts_chunks` row with an embedding so `find_duplicates` /
/// `search_code` (which filter on `embedding IS NOT NULL`) can see it.
pub(crate) fn seed_chunk(
    conn: &Connection,
    file_path: &str,
    symbol_path: &str,
    text: &str,
    embedding: &[f32],
) {
    seed_file(conn, file_path);
    let blob = serialize_embedding(embedding);
    conn.execute(
        "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, symbol_path, text, embedding)
         VALUES (?1, 0, ?2, 1, 10, ?3, ?4, ?5)",
        rusqlite::params![file_path, text.len() as i64, symbol_path, text, blob],
    )
    .unwrap();
}

/// Seed an `lsp_symbols` row (a function) so the `callers` probe can resolve a
/// symbol.
pub(crate) fn seed_symbol(conn: &Connection, id: &str, name: &str, file_path: &str) {
    seed_file(conn, file_path);
    conn.execute(
        "INSERT INTO lsp_symbols (id, name, kind, file_path, start_line, start_char, end_line, end_char, detail)
         VALUES (?1, ?2, ?3, ?4, 1, 0, 5, 0, NULL)",
        rusqlite::params![id, name, LSP_SYMBOL_KIND_FUNCTION, file_path],
    )
    .unwrap();
}

/// Seed an `lsp_call_edges` row (caller -> callee) for the `callers` probe.
pub(crate) fn seed_call_edge(
    conn: &Connection,
    caller_id: &str,
    callee_id: &str,
    caller_file: &str,
    callee_file: &str,
) {
    conn.execute(
        "INSERT INTO lsp_call_edges (caller_id, callee_id, caller_file, callee_file, source, from_ranges)
         VALUES (?1, ?2, ?3, ?4, 'lsp', '[]')",
        rusqlite::params![caller_id, callee_id, caller_file, callee_file],
    )
    .unwrap();
}

// ---- validator loader fixture ----------------------------------------

/// A loader carrying one RuleSet named `name` that matches `file_glob` and
/// declares `probes` at `severity`. `add_builtin_ruleset` is the deterministic
/// injection seam (no on-disk validators, so tests don't depend on the
/// machine).
pub(crate) fn loader_with(
    name: &str,
    file_glob: &str,
    probes: &[&str],
    severity: Severity,
) -> ValidatorLoader {
    let mut loader = ValidatorLoader::new();
    loader.add_builtin_ruleset(ruleset(name, file_glob, probes, severity));
    loader
}

/// A single-rule RuleSet matching `file_glob` and declaring `probes`.
pub(crate) fn ruleset(name: &str, file_glob: &str, probes: &[&str], severity: Severity) -> RuleSet {
    RuleSet {
        manifest: RuleSetManifest {
            name: name.to_string(),
            description: format!("{name} test ruleset"),
            metadata: RuleSetMetadata {
                version: "1.0.0".to_string(),
            },
            match_criteria: Some(ValidatorMatch {
                tools: vec![],
                files: vec![file_glob.to_string()],
            }),
            trigger_matcher: None,
            tags: vec![],
            probes: probes.iter().map(|p| p.to_string()).collect(),
            severity,
            timeout: 30,
            once: false,
        },
        rules: vec![Rule {
            name: format!("{name}-rule"),
            description: "rule".to_string(),
            body: "body".to_string(),
            severity: None,
            timeout: None,
        }],
        source: ValidatorSource::Builtin,
        base_path: PathBuf::from("/test"),
    }
}

// ---- composed pipeline fixture ----------------------------------------

/// The seeded-duplicate starting point the drive tests share: a repo whose
/// `src/lib.rs` gains an uncommitted `compute` function that duplicates an
/// indexed `old_compute` chunk, plus the schema-applied index seeded with both
/// chunks and a [`MockEmbedder`] over the same [`DIM`].
///
/// Composing it here keeps the seeds (file paths, symbol names, embedding) in
/// one place — a drift in any copy would silently desynchronize the tests.
/// Each test adds only its scenario-specific extras (e.g. a second working
/// file for a second validator).
pub(crate) fn seeded_dup_repo() -> (TestRepo, Connection, model_embedding::mock::MockEmbedder) {
    let repo = TestRepo::new();
    repo.write("src/lib.rs", "fn placeholder() {}\n");
    repo.commit("initial");
    let dup = body("compute");
    repo.write("src/lib.rs", &format!("fn placeholder() {{}}\n\n{dup}\n"));

    let conn = index_conn();
    let emb = dup_emb();
    seed_chunk(&conn, "src/lib.rs", "compute", &dup, &emb);
    seed_chunk(&conn, "src/existing.rs", "old_compute", &dup, &emb);

    (repo, conn, model_embedding::mock::MockEmbedder::new(DIM))
}

/// A function body long enough to clear the default `min_chunk_bytes` (100).
pub(crate) fn body(label: &str) -> String {
    format!(
        "pub fn {label}(input: &[f64]) -> f64 {{\n    let mut total = 0.0;\n    for value in input {{\n        total += value * value;\n    }}\n    total / input.len() as f64\n}}"
    )
}
