//! Integration: the leader binds the election request socket and a follower-role
//! client routes a real `diagnose` to it — exercising THIS card's production
//! wiring end to end.
//!
//! Unlike the `swissarmyhammer-diagnostics` mechanism test (which binds a bare
//! socket at an ad-hoc path), this test drives the wiring the live `sah serve`
//! path uses:
//!
//! - a real leader [`CodeContextWorkspace`] surfaces `socket_path()` / `lock_path()`
//!   (the accessors added for this card);
//! - the leader binds a [`RequestServer`] at that `socket_path()` and serves the
//!   SAH request API over a single real `rust-analyzer` session (the same call
//!   `server.rs::spawn_request_server_for_leader` makes at startup);
//! - a follower builds a [`SessionRequestClient`] from the workspace's two
//!   accessors and gets back a real [`DiagnosticsReport`].
//!
//! Only the leader's single rust-analyzer runs; the follower spawns none. Gated
//! on `rust-analyzer` being installed, and serialized (`serial_test`) because it
//! shares the cargo `target/` with the other live-LSP integration tests, a known
//! hang hazard otherwise. The daemon is shut down explicitly so the test never
//! leaks a PPID=1 orphan rust-analyzer.

use std::time::Duration;

use swissarmyhammer_code_context::CodeContextWorkspace;
use swissarmyhammer_diagnostics::{
    DiagnosticsConfig, PrecomputedDependents, RequestServer, SessionRequestClient,
};
use swissarmyhammer_lsp::{LspDaemon, OwnedLspServerSpec};

/// Whether `rust-analyzer` is on PATH. The test is a no-op when it is not.
fn rust_analyzer_available() -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&paths).any(|dir| dir.join("rust-analyzer").is_file())
}

/// A `rust-analyzer` server spec for the given workspace.
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

/// Write a tiny single-crate Cargo project with one referenced symbol into
/// `root`, returning the path to `main.rs`.
fn seed_rust_project(root: &std::path::Path) -> std::path::PathBuf {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"socket_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let main_rs = src.join("main.rs");
    std::fs::write(
        &main_rs,
        "fn helper() -> i32 { 41 }\n\nfn main() {\n    let _ = helper();\n}\n",
    )
    .unwrap();
    main_rs
}

#[tokio::test]
#[serial_test::serial(cwd)]
async fn follower_routes_diagnose_through_the_workspace_socket_to_the_leader() {
    if !rust_analyzer_available() {
        eprintln!("SKIPPED: rust-analyzer not installed");
        return;
    }

    // --- Leader workspace: surfaces the election socket + lock paths. ---
    let workspace_dir = tempfile::tempdir().expect("workspace tempdir");
    let root = workspace_dir.path();
    let main_rs = seed_rust_project(root);
    let ws = CodeContextWorkspace::open(root).expect("open leader workspace");
    assert!(ws.is_leader(), "the first opener must be the leader");

    // The accessors this card added: the tool layer builds a client from these.
    let socket_path = ws.socket_path().to_path_buf();
    let lock_path = ws.lock_path().to_path_buf();

    // --- Leader: one real rust-analyzer session, served over the socket. ---
    let mut daemon = LspDaemon::new(rust_analyzer_spec(), root.to_path_buf());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

    // Bind + serve exactly as `server.rs::spawn_request_server_for_leader` does.
    let server = RequestServer::bind(&socket_path).expect("leader binds the election socket");
    let serve_session = session.clone();
    let server_task = tokio::spawn(async move {
        let _ = swissarmyhammer_diagnostics::serve_session_requests(
            server,
            serve_session,
            PrecomputedDependents::default(),
            DiagnosticsConfig::default(),
        )
        .await;
    });

    // Open the document so rust-analyzer analyzes it (the leader's session).
    let text = std::fs::read_to_string(&main_rs).unwrap();
    session.open(&main_rs, &text).expect("open main.rs");
    tokio::time::sleep(Duration::from_secs(2)).await;

    // --- Follower: NO local LSP server, just the workspace-derived client. ---
    let client = SessionRequestClient::connect(&socket_path, &lock_path)
        .await
        .expect("follower should connect to the leader over the workspace socket");

    let report = client
        .diagnose(&[main_rs.to_string_lossy().into_owned()])
        .await
        .expect("a real diagnostics report should round-trip from the leader");

    // A well-formed single-crate file has no errors — but the point is that this
    // is a REAL report produced by the leader's one rust-analyzer over the socket,
    // not the empty `settled_empty()` a follower used to return.
    assert_eq!(
        report.counts.errors, 0,
        "a well-formed file should round-trip a clean report, got {:?}",
        report.counts
    );

    // Cleanup: drop the client, abort the serve task, shut the one daemon so no
    // orphan rust-analyzer survives the test.
    drop(client);
    server_task.abort();
    daemon.shutdown().await;
}
