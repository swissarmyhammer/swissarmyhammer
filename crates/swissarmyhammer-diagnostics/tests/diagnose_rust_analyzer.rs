//! End-to-end integration: [`swissarmyhammer_diagnostics::diagnose`] against a
//! real `rust-analyzer` over a populated code-context index.
//!
//! This is the heavy e2e half of the `diagnose` story (the broken-vs-clean
//! selection logic is unit-covered in `src/diagnose.rs` with a stub resolver).
//! Here we prove the whole chain works against the real machinery:
//!
//! - A compilable temp cargo workspace: `src/a.rs` defines a fn, `src/b.rs`
//!   calls it (a REAL cross-file call edge A←B), `src/c.rs` is independent and
//!   clean.
//! - The code-context SQLite index is populated via the tree-sitter call-edge
//!   pipeline (`ensure_ts_symbols` → `generate_ts_call_edges` → `write_ts_edges`)
//!   so `get_blastradius("src/a.rs", 1)` returns `src/b.rs` — the same
//!   `lsp_call_edges`/`lsp_symbols` tables the production
//!   [`swissarmyhammer_diagnostics::BlastRadiusDependents`] reads.
//! - A real `rust-analyzer` (via [`LspDaemon`]) analyzes the workspace. We break
//!   A — change `alpha`'s return type (so B's `let x: i32 = alpha()` becomes a
//!   type mismatch) and give A its own type-mismatch error — and pull so the
//!   server re-reports.
//! - `diagnose(["src/a.rs"])` must then carry A's error AND fold in B as a broken
//!   dependent, and must NOT carry the clean file C.
//!
//! Path-space coverage (what unit tests cannot reach): the workspace root is
//! **canonicalized** (on macOS the tempdir lives under `/var`, a symlink to
//! `/private/var`, and `rust-analyzer` publishes diagnostics under the resolved
//! `/private/var` uri). `diagnose` is fed canonical **absolute** paths while the
//! code-context index is keyed in **repo-relative** space, so this test bridges
//! the two exactly as the `diagnostics` MCP tool does (relativize for the
//! blast-radius lookup, absolutize the returned dependents). If the repo root
//! were not canonicalized, the tool's `repo.join(rel)` absolute paths would not
//! match the server's canonicalized uris and the diagnostics would be missed —
//! the bug the tool's `repo_root()` canonicalization guards against.
//!
//! Gated at runtime on `rust-analyzer` being on `PATH`: a no-op green skip when
//! it is absent, mirroring the other LSP-server-gated integration tests. It is
//! serialized against the other live-`rust-analyzer` tests in this crate so two
//! real analyzers do not contend on the shared cargo `target/`, and every wait
//! is bounded so a stalled server fails the test rather than hanging the suite.

use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

use rusqlite::Connection;
use tree_sitter::Language;

use swissarmyhammer_code_context::{
    ensure_ts_symbols, generate_ts_call_edges, get_blastradius, write_ts_edges, BlastRadiusOptions,
    CodeContextWorkspace,
};
use swissarmyhammer_diagnostics::{
    diagnose, refresh_file, BlastRadiusDependents, Dependents, DiagnosticsConfig,
    PrecomputedDependents, TokioTimer,
};
use swissarmyhammer_lsp::{file_uri_from_path, LspDaemon, OwnedLspServerSpec};
use tokio::sync::Mutex;

/// Upper bound on how long to wait for rust-analyzer to load the workspace and
/// publish diagnostics for the broken files. Deliberately generous so the test
/// stays robust on a cold, slow CI machine; a warm local run resolves in a few
/// seconds. Bounded so a stalled server fails the test rather than hanging the
/// whole `cargo test` run.
const RA_DIAGNOSTIC_DEADLINE: Duration = Duration::from_secs(60);

/// Interval between pull-model refresh polls while waiting for rust-analyzer to
/// report diagnostics for a freshly-broken file.
const DIAGNOSTIC_POLL_INTERVAL_MS: u64 = 500;

/// Pause after opening the fixture files so rust-analyzer has time to load the
/// crate before we break a file and pull.
const RA_WORKSPACE_LOAD_DELAY_SECS: u64 = 3;

/// Settle hard-timeout for `diagnose`'s quiescence wait. Deliberately generous
/// so a cold machine's re-flow has room; the settle still seeds from the warm
/// cache, so a warm run resolves quickly.
const CONFIG_SETTLE_HARD_TIMEOUT_SECS: u64 = 20;

/// Outer bound on the whole `diagnose` operation, so a stalled server fails the
/// test rather than hanging the suite.
const DIAGNOSE_OPERATION_TIMEOUT_SECS: u64 = 30;

/// Serialize the rust-analyzer-backed tests so two real analyzers indexing fresh
/// crates concurrently do not contend on the shared cargo `target/` and starve
/// each other (see the project's cargo-target-concurrency note). Held across the
/// test's `.await` points, so an async-aware mutex.
static SERIAL: Mutex<()> = Mutex::const_new(());

/// The tree-sitter Rust language handle.
fn rust_language() -> Language {
    tree_sitter_rust::LANGUAGE.into()
}

/// A `rust-analyzer` server spec for the daemon.
fn rust_analyzer_spec() -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![],
        command: "rust-analyzer".to_string(),
        args: vec![],
        language_ids: vec!["rust".to_string()],
        file_extensions: vec!["rs".to_string()],
        startup_timeout_secs: 60,
        health_check_interval_secs: 60,
        install_hint: "rustup component add rust-analyzer".to_string(),
        icon: None,
    }
}

/// The fixture's source files, keyed by repo-relative path. `b.rs` calls
/// `a::alpha` — the real cross-file call edge A←B — binding its `i32` result, and
/// `c.rs` is independent.
const A_RS: &str = "pub fn alpha() -> i32 {\n    7\n}\n";
const B_RS: &str =
    "use crate::a::alpha;\n\npub fn beta() -> i32 {\n    let x: i32 = alpha();\n    x + 1\n}\n";
const C_RS: &str = "pub fn gamma() -> i32 {\n    3\n}\n";
const LIB_RS: &str = "pub mod a;\npub mod b;\npub mod c;\n";

/// Break A by changing `alpha`'s return type from `i32` to `&'static str` and
/// giving A its own type-mismatch error. The call edge A←B is preserved (B still
/// calls `alpha`), but B's `let x: i32 = alpha()` is now a native type mismatch —
/// a real downstream breakage. Both A's and B's errors are native rust-analyzer
/// type-inference diagnostics (not flycheck-only lints), so the server surfaces
/// them via the pull without a cargo build.
const A_RS_BROKEN: &str =
    "pub fn alpha() -> &'static str {\n    let _x: u32 = \"not a number\";\n    \"broken\"\n}\n";

/// Seed a compilable single-crate cargo workspace under `root` with the four
/// source files. Returns nothing — paths are derived from `root`.
fn seed_fixture(root: &Path) {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"diagnose_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    std::fs::write(src.join("a.rs"), A_RS).unwrap();
    std::fs::write(src.join("b.rs"), B_RS).unwrap();
    std::fs::write(src.join("c.rs"), C_RS).unwrap();
    std::fs::write(src.join("lib.rs"), LIB_RS).unwrap();
}

/// Populate the code-context index so the cross-file call edge A←B is persisted
/// in `lsp_call_edges`/`lsp_symbols`. Uses the tree-sitter call-edge pipeline —
/// the same one the production index uses when no LSP edges are available, and
/// the one `get_blastradius` reads. The chunks are inserted directly (mirroring
/// the code-context integration tests) so the edge resolution has symbol paths
/// to match against.
fn populate_index(conn: &Connection) {
    let files = [("src/a.rs", A_RS), ("src/b.rs", B_RS), ("src/c.rs", C_RS)];

    // One top-level fn per file → one chunk per file, with its symbol path.
    for (rel_path, source) in &files {
        let symbol = top_level_fn_name(source);
        conn.execute(
            "INSERT INTO ts_chunks (file_path, start_byte, end_byte, start_line, end_line, text, symbol_path)
             VALUES (?1, 0, ?2, 0, ?3, ?4, ?5)",
            rusqlite::params![
                rel_path,
                source.len() as i64,
                source.lines().count() as i64,
                source,
                symbol,
            ],
        )
        .unwrap();
        ensure_ts_symbols(conn, rel_path).unwrap();
    }

    // Generate + persist call edges. B's `alpha()` call resolves to the `alpha`
    // symbol in src/a.rs, producing the edge B→A (caller B, callee A).
    for (rel_path, source) in &files {
        let edges = generate_ts_call_edges(conn, rel_path, source, rust_language()).unwrap();
        write_ts_edges(conn, rel_path, &edges).unwrap();
    }

    conn.execute(
        "UPDATE indexed_files SET ts_indexed = 1 WHERE file_path LIKE 'src/%.rs'",
        [],
    )
    .unwrap();
}

/// Extract the single top-level `fn <name>` from a fixture source. The fixtures
/// each define exactly one such function.
fn top_level_fn_name(source: &str) -> String {
    source
        .split_whitespace()
        .skip_while(|t| *t != "fn")
        .nth(1)
        .map(|t| t.trim_end_matches("()").to_string())
        .expect("fixture has a top-level fn")
}

/// Drive a pull-model refresh of `path` until the server reports a diagnostic
/// for it, or the deadline passes. Returns whether diagnostics arrived.
///
/// The daemon's read loop does NOT pump push-model `publishDiagnostics` into the
/// session cache — `refresh_file` (sync + `textDocument/diagnostic` pull) is what
/// makes a change observable (see the watcher's `refresh_file` doc). rust-analyzer
/// returns an empty report until it finishes analyzing the freshly-broken crate,
/// so we pull on a bounded poll. A bounded wait, never a hang.
async fn wait_for_diagnostics<C: swissarmyhammer_lsp::client::LspTransport>(
    session: &swissarmyhammer_lsp::LspSession<C>,
    path: &Path,
    uri: &str,
    deadline: Duration,
) -> bool {
    let start = std::time::Instant::now();
    loop {
        // Pull drives a `textDocument/diagnostic` request and stores the result
        // in the session cache + fan-out.
        refresh_file(session, path);
        if !session.diagnostics_for(uri).is_empty() {
            return true;
        }
        if start.elapsed() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(DIAGNOSTIC_POLL_INTERVAL_MS)).await;
    }
}

/// Express an absolute path relative to `repo` (the code-context index's key
/// space), matching the `diagnostics` tool's `relativize`.
fn relativize(path: &str, repo: &Path) -> String {
    Path::new(path)
        .strip_prefix(repo)
        .map(|p| p.to_string_lossy().into_owned())
        .unwrap_or_else(|_| path.to_string())
}

/// Join a repo-relative path onto `repo` (the absolute LSP key space), matching
/// the `diagnostics` tool's `absolutize`.
fn absolutize(rel: &str, repo: &Path) -> String {
    repo.join(rel).to_string_lossy().into_owned()
}

#[tokio::test(flavor = "multi_thread")]
async fn diagnose_reports_target_and_broken_dependent_not_clean() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("rust-analyzer not found on PATH; skipping diagnose e2e test");
        return;
    }
    let _serial = SERIAL.lock().await;

    // Canonicalize the workspace root: on macOS the tempdir is under `/var`, a
    // symlink to `/private/var`, and rust-analyzer publishes diagnostics under
    // the resolved `/private/var` uri. Feeding `diagnose` canonical absolute
    // paths keeps the watched uris matching the server's — the same
    // canonicalization the `diagnostics` tool's `repo_root()` must apply.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let repo = std::fs::canonicalize(workspace.path()).expect("canonicalize");
    seed_fixture(&repo);

    // Stand up the code-context index so the blast radius A←B is persisted.
    let ws = CodeContextWorkspace::open(&repo).expect("open code-context workspace");
    {
        let conn = ws.db();
        populate_index(&conn);

        // Sanity: the index really knows B depends on A before we go near the
        // server, so a later empty fold-in cannot be blamed on a missing edge.
        let radius = get_blastradius(
            &conn,
            &BlastRadiusOptions {
                file_path: "src/a.rs".to_string(),
                symbol: None,
                max_hops: 1,
            },
        )
        .expect("blast radius for src/a.rs");
        let affected: Vec<&str> = radius
            .hops
            .iter()
            .flat_map(|h| h.symbols.iter().map(|s| s.file_path.as_str()))
            .collect();
        assert!(
            affected.contains(&"src/b.rs"),
            "index must know B depends on A; blast radius was {affected:?}"
        );
    }

    // Start rust-analyzer over the canonical workspace. `daemon` is dropped at
    // the end of the test (kill-on-drop via its child handle) and explicitly
    // shut down, so a panic cannot leak the process.
    let mut daemon = LspDaemon::new(rust_analyzer_spec(), repo.clone());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

    let a_path = repo.join("src/a.rs");
    let b_path = repo.join("src/b.rs");
    let a_abs = a_path.to_string_lossy().into_owned();
    let b_abs = b_path.to_string_lossy().into_owned();

    // Open every file clean and let rust-analyzer load the crate.
    session.open(&a_path, A_RS).expect("open a.rs");
    session.open(&b_path, B_RS).expect("open b.rs");
    session
        .open(&repo.join("src/c.rs"), C_RS)
        .expect("open c.rs");
    tokio::time::sleep(Duration::from_secs(RA_WORKSPACE_LOAD_DELAY_SECS)).await;

    // Break A on disk: change alpha's return type i32 -> &'static str and add A's
    // own (u32 = &str) type-mismatch error. The A←B call edge is preserved, but
    // B's `let x: i32 = alpha()` is now a native type mismatch. `refresh_file`
    // (below) reads the broken content from disk, syncs it, and pulls.
    std::fs::write(&a_path, A_RS_BROKEN).unwrap();

    // Pre-warm via the pull model until the server reports the errors for both A
    // and B. `diagnose` seeds its settle from this now-warm cache, so it settles
    // within the window rather than racing the server's first analysis.
    let a_uri = file_uri_from_path(&a_abs);
    let b_uri = file_uri_from_path(&b_abs);
    assert!(
        wait_for_diagnostics(&session, &a_path, &a_uri, RA_DIAGNOSTIC_DEADLINE).await,
        "rust-analyzer should report an error for the broken A"
    );
    assert!(
        wait_for_diagnostics(&session, &b_path, &b_uri, RA_DIAGNOSTIC_DEADLINE).await,
        "rust-analyzer should report an error for B (binds A's now-changed return type)"
    );

    // Resolve A's one-hop dependents from the index exactly as the production
    // `diagnostics` tool does: relativize the absolute target for the lookup,
    // absolutize the returned dependents back. This bridges the absolute (LSP)
    // and repo-relative (index) path spaces — the path-space handling the e2e
    // exists to validate. The DB handle is dropped before the `.await`.
    let c_abs = repo.join("src/c.rs").to_string_lossy().into_owned();
    let dependents: PrecomputedDependents = {
        let conn = ws.db();
        let resolver = BlastRadiusDependents::new(&conn);
        let mut map = HashMap::new();
        let mut deps: Vec<String> = resolver
            .one_hop(&relativize(&a_abs, &repo))
            .iter()
            .map(|rel| absolutize(rel, &repo))
            .collect();
        assert!(
            deps.contains(&b_abs),
            "B (absolute) must be A's one-hop dependent; got {deps:?}"
        );
        // Make the clean file C a DECLARED dependent of A too, so `diagnose`
        // actually watches and settles C. That turns the C-exclusion assertion
        // into a real broken-vs-clean discriminator (C is watched but, being
        // clean, must be filtered out of the report) rather than a vacuous guard.
        deps.push(c_abs.clone());
        map.insert(a_abs.clone(), deps);
        PrecomputedDependents::new(map)
    };

    // A generous settle hard-timeout so a cold server's re-flow has room; the
    // settle still seeds from the now-warm cache, so it resolves quickly.
    let config = DiagnosticsConfig {
        settle_hard_timeout: Duration::from_secs(CONFIG_SETTLE_HARD_TIMEOUT_SECS),
        ..DiagnosticsConfig::default()
    };

    let report = tokio::time::timeout(
        Duration::from_secs(DIAGNOSE_OPERATION_TIMEOUT_SECS),
        diagnose(
            &session,
            std::slice::from_ref(&a_abs),
            &config,
            &dependents,
            &TokioTimer,
        ),
    )
    .await
    .expect("diagnose must not hang past its bounded timeout");

    let paths: Vec<&str> = report.diagnostics.iter().map(|r| r.path.as_str()).collect();

    // A (the target) must be present with its own error.
    assert!(
        paths.iter().any(|p| *p == a_abs),
        "report must include the target A ({a_abs}); paths were {paths:?}"
    );
    // B (the broken dependent) must be folded in.
    assert!(
        paths.iter().any(|p| *p == b_abs),
        "report must fold in the broken dependent B ({b_abs}); paths were {paths:?}"
    );
    // C is a watched dependent but clean, so it must NOT appear — the
    // broken-vs-clean filter must drop it.
    assert!(
        !paths.iter().any(|p| *p == c_abs),
        "clean dependent C must be excluded; paths were {paths:?}"
    );
    assert!(
        report.counts.errors >= 2,
        "expected at least A's and B's errors; counts were {:?}",
        report.counts
    );

    daemon.shutdown().await;
}
