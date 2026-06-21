//! Integration: the leader-owned diagnostics file watcher re-reports on a
//! disk write it never made through the closed edit surface.
//!
//! A single leader process owns one real `rust-analyzer` session and runs the
//! diagnostics watcher over the workspace. The test writes a file with a type
//! error *directly to disk* (the way a follower's `files edit`, a formatter, or
//! `git checkout` would) and asserts the watcher debounces the change, issues
//! `didChange` into the session, and the session re-reports a diagnostic for the
//! file — without anyone syncing the analyzer by hand.
//!
//! Gated on `rust-analyzer`: a no-op skip when it is absent, mirroring the other
//! LSP-server-gated integration tests.

use std::path::Path;
use std::time::Duration;

use swissarmyhammer_diagnostics::{refresh_file, start_diagnostics_watcher, SessionRoute};
use swissarmyhammer_lsp::{file_uri_from_path, LspDaemon, OwnedLspServerSpec};
use tokio::sync::Mutex;

/// Upper bound on how long to wait for rust-analyzer to load the workspace and
/// re-report after a disk write. Deliberately generous so the test stays robust
/// on a cold, slow CI machine; a warm local run resolves in a few seconds.
const CI_DIAGNOSTIC_WAIT_DEADLINE_SECS: u64 = 45;

/// Serialize the rust-analyzer-backed tests in this binary. Two real
/// rust-analyzer daemons indexing fresh crates concurrently contend on the
/// shared cargo `target/` and starve each other past the per-test deadline (see
/// the project's cargo-target-concurrency note). Holding this for the duration
/// of each test keeps exactly one live analyzer at a time. An async-aware mutex
/// so the guard can be held across the test's `.await` points.
static SERIAL: Mutex<()> = Mutex::const_new(());

/// Whether `rust-analyzer` is on PATH. The test is a no-op when it is not.
fn rust_analyzer_available() -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join("rust-analyzer").is_file())
}

/// Build a `rust-analyzer` server spec for the given workspace.
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

/// Seed a tiny single-crate Cargo project with a clean `main.rs`.
fn seed_rust_project(root: &Path) -> std::path::PathBuf {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"watch_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let main_rs = src.join("main.rs");
    std::fs::write(&main_rs, "fn main() {}\n").unwrap();
    main_rs
}

/// Wait for the watcher to drive a non-empty re-report, nudging it with fresh
/// disk writes on a cadence until one lands (or the deadline passes).
///
/// The watcher re-diagnoses only on the filesystem changes it observes, and a
/// single write right after a cold start can land its one debounced pull before
/// rust-analyzer has finished indexing the fresh crate — after which nothing
/// re-pulls the static cache (push diagnostics are not wired into the daemon
/// read loop). Re-writing the erroneous file keeps the watcher firing — each
/// write is exactly the external-edit scenario the watcher exists to react to,
/// not a hand-sync of the analyzer. The cadence is kept above the watcher's
/// debounce so each write actually settles into a refresh instead of perpetually
/// resetting the debounce timer. Every write preserves the type error (only a
/// trailing marker comment varies), so any captured diagnostic still carries it.
async fn wait_for_watcher_rereport<C: swissarmyhammer_lsp::client::LspTransport>(
    session: &swissarmyhammer_lsp::LspSession<C>,
    main_rs: &Path,
    uri: &str,
    deadline: Duration,
) -> Vec<lsp_types::Diagnostic> {
    let start = std::time::Instant::now();
    let mut nudge = 0u32;
    loop {
        // Settle window: longer than DIAGNOSTICS_WATCH_DEBOUNCE (~1s) so the
        // prior write debounces into a refresh + pull before we re-check.
        tokio::time::sleep(Duration::from_secs(3)).await;
        let diags = session.diagnostics_for(uri);
        if !diags.is_empty() || start.elapsed() >= deadline {
            return diags;
        }
        // Still nothing — nudge the watcher with another on-disk edit that
        // keeps the type error in place.
        nudge += 1;
        std::fs::write(
            main_rs,
            format!("fn main() {{\n    let _x: u32 = \"not a number\";\n}}\n// nudge {nudge}\n"),
        )
        .unwrap();
    }
}

/// Drive [`refresh_file`] repeatedly until it populates diagnostics or the
/// deadline passes, returning the captured set (possibly empty on timeout).
///
/// A single pull right after a write races rust-analyzer's async indexing of a
/// fresh crate: the analyzer answers the first `textDocument/diagnostic` before
/// it has computed anything, so one immediate `refresh_file` reliably observes
/// an empty report and the cache then never changes (push is not wired into the
/// daemon read loop). A real watcher re-fires `refresh_file` on every event;
/// this mirrors that by re-pulling on a cadence so the test observes the
/// eventual re-report instead of only the empty first pull.
async fn refresh_until_diagnostics<C: swissarmyhammer_lsp::client::LspTransport>(
    session: &swissarmyhammer_lsp::LspSession<C>,
    path: &Path,
    uri: &str,
    deadline: Duration,
) -> Vec<lsp_types::Diagnostic> {
    let start = std::time::Instant::now();
    loop {
        refresh_file(session, path);
        let diags = session.diagnostics_for(uri);
        if !diags.is_empty() || start.elapsed() >= deadline {
            return diags;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn watcher_redreport_on_direct_disk_write() {
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let _serial = SERIAL.lock().await;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    // Canonicalize the workspace root: on macOS the tempdir is under `/var`,
    // which is a symlink to `/private/var`, and the OS file watcher reports the
    // resolved `/private/var` path. Opening documents under the same canonical
    // root keeps the watcher's `didChange` uris matching the ones the session
    // already has open — otherwise rust-analyzer treats them as two documents.
    let workspace_root = std::fs::canonicalize(workspace.path()).expect("canonicalize");
    let main_rs = seed_rust_project(&workspace_root);

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), workspace_root.clone());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

    // Open the clean file and let rust-analyzer load the workspace.
    let text = std::fs::read_to_string(&main_rs).unwrap();
    session.open(&main_rs, &text).expect("open main.rs");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Start the one leader-owned watcher over the workspace. Give the async
    // debouncer a moment to actually begin watching before we write, so the
    // write is not missed in the watcher's startup window.
    let watcher = start_diagnostics_watcher(
        workspace_root.clone(),
        vec![SessionRoute::new(vec!["rs".to_string()], session.clone())],
    );
    tokio::time::sleep(Duration::from_secs(1)).await;

    // Write a type error DIRECTLY to disk — the path the closed edit surface
    // can't see (a follower's direct write / a formatter / git checkout).
    std::fs::write(
        &main_rs,
        "fn main() {\n    let _x: u32 = \"not a number\";\n}\n",
    )
    .unwrap();

    let uri = file_uri_from_path(&main_rs.to_string_lossy());

    // The watcher debounces (~1s) then re-diagnoses. Keep nudging it with fresh
    // disk writes on a cadence so a cold rust-analyzer that misses the first
    // pull still gets re-driven before the generous deadline.
    let diags = wait_for_watcher_rereport(
        &session,
        &main_rs,
        &uri,
        Duration::from_secs(CI_DIAGNOSTIC_WAIT_DEADLINE_SECS),
    )
    .await;

    assert!(
        !diags.is_empty(),
        "watcher should have driven a re-report with the type error; got none"
    );
    assert!(
        diags
            .iter()
            .any(|d| d.severity == Some(lsp_types::DiagnosticSeverity::ERROR)),
        "the type mismatch should surface as an error diagnostic: {diags:?}"
    );

    watcher.abort();
    daemon.shutdown().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn refresh_file_direct_reports_error_against_real_server() {
    // A tighter, watcher-free check that the per-file action itself drives a
    // re-report against a real server (no debounce/notify timing in play).
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }
    let _serial = SERIAL.lock().await;

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let workspace_root = std::fs::canonicalize(workspace.path()).expect("canonicalize");
    let main_rs = seed_rust_project(&workspace_root);

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), workspace_root.clone());
    daemon.start().await.expect("handshake");
    let session = daemon.session();

    let text = std::fs::read_to_string(&main_rs).unwrap();
    session.open(&main_rs, &text).expect("open");
    tokio::time::sleep(Duration::from_secs(3)).await;

    // Direct disk write with an error, then drive the per-file refresh.
    std::fs::write(&main_rs, "fn main() {\n    let _x: u32 = \"nope\";\n}\n").unwrap();
    assert!(refresh_file(&session, &main_rs), "rs file is diagnosable");

    // Re-pull on a cadence: the first pull races rust-analyzer's indexing of the
    // fresh crate and answers empty, so we keep driving `refresh_file` until the
    // re-report lands (or the deadline passes).
    let uri = file_uri_from_path(&main_rs.to_string_lossy());
    let diags = refresh_until_diagnostics(
        &session,
        &main_rs,
        &uri,
        Duration::from_secs(CI_DIAGNOSTIC_WAIT_DEADLINE_SECS),
    )
    .await;
    assert!(
        !diags.is_empty(),
        "refresh_file should have populated diagnostics for the file"
    );

    daemon.shutdown().await;
}
