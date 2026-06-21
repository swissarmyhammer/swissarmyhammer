//! Integration test for [`LspSession`] against a real `rust-analyzer`.
//!
//! Gated at runtime on `rust-analyzer` being present on `PATH`: when it is
//! absent (e.g. minimal CI images) the test logs a skip and returns green, so
//! it never blocks the model-free unit suite. When present, it proves the
//! session keeps one document open across two `textDocument/documentSymbol`
//! requests with no re-open — the single-session, persistent-open invariant.

use std::path::PathBuf;

use swissarmyhammer_lsp::{LspDaemon, OwnedLspServerSpec};
use swissarmyhammer_project_detection::ProjectType;

/// Build a minimal rust-analyzer spec for the daemon.
fn rust_analyzer_spec() -> OwnedLspServerSpec {
    OwnedLspServerSpec {
        project_types: vec![ProjectType::Rust],
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

#[tokio::test]
async fn document_stays_open_across_two_document_symbol_requests() {
    if which::which("rust-analyzer").is_err() {
        eprintln!("rust-analyzer not found on PATH; skipping session integration test");
        return;
    }

    // A self-contained cargo workspace so rust-analyzer has a project to load.
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let root = workspace.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write Cargo.toml");
    let src_dir = root.join("src");
    std::fs::create_dir_all(&src_dir).expect("create src");
    let lib_path = src_dir.join("lib.rs");
    let source = "pub struct Fixture;\n\npub fn fixture() -> Fixture {\n    Fixture\n}\n";
    std::fs::write(&lib_path, source).expect("write lib.rs");

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), root.to_path_buf());
    daemon
        .start()
        .await
        .expect("rust-analyzer should start and complete the handshake");

    let session = daemon.session();
    let lib_path: PathBuf = lib_path;
    let source = source.to_string();

    // Drive the synchronous, blocking session ops off the async runtime.
    let result = tokio::task::spawn_blocking(move || {
        session.open(&lib_path, &source).expect("open document");

        let uri = format!("file://{}", lib_path.to_string_lossy());
        let params = serde_json::json!({ "textDocument": { "uri": uri } });

        // Two documentSymbol requests with NO re-open in between.
        let first = session.request("textDocument/documentSymbol", params.clone());
        let second = session.request("textDocument/documentSymbol", params);

        let still_open = session.is_open(&lib_path);
        (first, second, still_open)
    })
    .await
    .expect("blocking session task should not panic");

    let (first, second, still_open) = result;
    assert!(
        first.is_ok(),
        "first documentSymbol request should succeed: {first:?}"
    );
    assert!(
        second.is_ok(),
        "second documentSymbol request should succeed against the persistent session: {second:?}"
    );
    assert!(
        still_open,
        "document must remain open across both requests (no open/close churn)"
    );

    daemon.shutdown().await;
}
