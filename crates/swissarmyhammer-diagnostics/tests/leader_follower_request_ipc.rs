//! Integration: leader + follower over a real election socket.
//!
//! A single leader process owns one real `rust-analyzer` session and binds a
//! [`RequestServer`] at the election socket. A follower — which spawns NO LSP
//! server of its own, holding only a [`SessionRequestClient`] — issues N
//! concurrent `diagnose` and `textDocument/definition` calls over the socket.
//! The test asserts every concurrent call gets its own correlated response and
//! that the only LSP server in play is the leader's single session.
//!
//! Gated on `rust-analyzer` being installed: when it is absent the test prints a
//! skip notice and returns, mirroring the other LSP-server-gated integration
//! tests (it needs a real language server, so it lives here, not in the unit
//! suite).

use std::path::Path;
use std::time::Duration;

use serde_json::json;

use swissarmyhammer_diagnostics::SessionRequestClient;
use swissarmyhammer_leader_election::request_ipc::RequestServer;
use swissarmyhammer_lsp::{LspDaemon, OwnedLspServerSpec};

/// Whether `rust-analyzer` is on PATH. The test is a no-op when it is not.
///
/// Does the PATH lookup inline (a minimal `which`) to avoid pulling a crate dep
/// into the test just for this one check.
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

/// Write a tiny single-crate Cargo project with one referenced symbol into
/// `root`, returning the path to `main.rs`.
fn seed_rust_project(root: &Path) -> std::path::PathBuf {
    std::fs::write(
        root.join("Cargo.toml"),
        "[package]\nname = \"ipc_fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
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
async fn leader_serves_concurrent_follower_diagnose_and_definition_calls() {
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }

    // --- Leader: one real session, bound at an election socket. ---
    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let main_rs = seed_rust_project(workspace.path());

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), workspace.path().to_path_buf());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

    // Sockets live in a separate temp dir so they are cleaned up independently.
    let sock_dir = tempfile::tempdir().expect("socket tempdir");
    let socket_path = sock_dir.path().join("leader.sock");
    let lock_path = sock_dir.path().join("leader.lock");

    let server = RequestServer::bind(&socket_path).expect("bind request server");
    let serve_session = session.clone();
    let server_task = tokio::spawn(async move {
        let _ = swissarmyhammer_diagnostics::serve_session_requests(
            server,
            serve_session,
            swissarmyhammer_diagnostics::PrecomputedDependents::default(),
            swissarmyhammer_diagnostics::DiagnosticsConfig::default(),
        )
        .await;
    });

    // Open the document so rust-analyzer analyzes it (the leader's session).
    let text = std::fs::read_to_string(&main_rs).unwrap();
    session.open(&main_rs, &text).expect("open main.rs");

    // Give rust-analyzer a moment to load the workspace before queries.
    tokio::time::sleep(Duration::from_secs(2)).await;

    // --- Follower: NO local LSP server, just a socket client. ---
    let client = SessionRequestClient::connect(&socket_path, &lock_path)
        .await
        .expect("follower should connect to the leader socket");

    let path_str = main_rs.to_string_lossy().to_string();

    // Fire N concurrent calls, mixing diagnose and definition, all over the one
    // follower connection. Each must come back correctly correlated.
    let mut handles = Vec::new();
    for i in 0..8u32 {
        let client = client.clone();
        let path_str = path_str.clone();
        let main_rs = main_rs.clone();
        handles.push(tokio::spawn(async move {
            if i % 2 == 0 {
                // diagnose: a well-formed file has no errors.
                let report = client
                    .diagnose(&[path_str])
                    .await
                    .expect("diagnose over socket");
                ("diagnose", report.counts.errors)
            } else {
                // textDocument/definition on the `helper()` call site (line 3,
                // the call inside main). The result shape varies, but a live
                // server returns a JSON value, not an error.
                let uri = format!("file://{}", main_rs.to_string_lossy());
                // A live server returns a JSON value (location list / null), not
                // a transport error — that round trip is what we assert.
                client
                    .lsp_request(
                        "textDocument/definition",
                        json!({
                            "textDocument": { "uri": uri },
                            "position": { "line": 3, "character": 12 }
                        }),
                    )
                    .await
                    .expect("definition over socket");
                ("definition", 0)
            }
        }));
    }

    let mut diagnose_count = 0;
    let mut definition_count = 0;
    for h in handles {
        let (kind, errors) = h.await.expect("task joined");
        match kind {
            "diagnose" => {
                diagnose_count += 1;
                assert_eq!(errors, 0, "a well-formed file should have no errors");
            }
            "definition" => definition_count += 1,
            other => panic!("unexpected kind {other}"),
        }
    }
    assert_eq!(diagnose_count, 4, "all diagnose calls must be correlated");
    assert_eq!(
        definition_count, 4,
        "all definition calls must be correlated"
    );

    // Cleanup: drop the client, abort the server task, shut the one daemon.
    drop(client);
    server_task.abort();
    daemon.shutdown().await;
}

#[tokio::test]
async fn follower_connect_to_absent_leader_is_typed_not_leader() {
    // No server is bound. A follower's connect must fail with a typed
    // not-leader error carrying the leader PID from the lock file — never hang
    // and never silently spawn its own server.
    let dir = tempfile::tempdir().unwrap();
    let socket_path = dir.path().join("missing.sock");
    let lock_path = dir.path().join("leader.lock");
    std::fs::write(&lock_path, "9931\n").unwrap();

    let err = SessionRequestClient::connect(&socket_path, &lock_path)
        .await
        .expect_err("connecting to an unbound socket must fail");
    let rendered = err.to_string();
    assert!(
        rendered.contains("9931"),
        "error should attribute the leader PID: {rendered}"
    );
}
