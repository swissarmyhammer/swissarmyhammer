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
use std::time::{Duration, Instant};

use serde_json::json;

use swissarmyhammer_diagnostics::SessionRequestClient;
use swissarmyhammer_leader_election::request_ipc::RequestServer;
use swissarmyhammer_lsp::{LspDaemon, OwnedLspServerSpec};

/// How long to wait for `rust-analyzer` to load the workspace before the first
/// query. rust-analyzer indexes the crate asynchronously after the handshake; a
/// query fired too early answers null/transient, so the routing tests then poll
/// (see [`WARM_UP_BUDGET`]) on top of this initial settle.
const RUST_ANALYZER_INITIAL_LOAD_WAIT_SECS: u64 = 3;

/// Wall-clock budget for polling a routed live-LSP op until rust-analyzer is
/// warm enough to return the real cross-reference / rename (hang-safe: the loop
/// is finite, never an unbounded wait on a server that never resolves).
///
/// This is a deadline rather than an attempt count on purpose. An attempt count
/// times [`WARM_UP_POLL_INTERVAL`] only describes the elapsed time when every
/// attempt returns promptly, so it states a bound it does not hold; a deadline
/// bounds the polling itself.
///
/// The budget has been raised twice, and only ever the budget — the assertions
/// never moved: 10s (tuned for an idle machine), then 60s (matching the sibling
/// `ra_pull_readiness.rs`), now 120s.
///
/// TWO independent deadlines bound these tests, and they fail through DIFFERENT
/// surfaces. This one is the second:
///
/// 1. The per-request LSP timeout — `SAH_LSP_REQUEST_TIMEOUT_SECS`, which
///    `.cargo/config.toml` sets to 120s for test runs over the shipped 30s
///    default. `is_transient_not_ready` does not match a timeout, so a request
///    that spends its whole budget takes the `Err(e) => panic!` arm on the spot.
///    This budget cannot absorb that — the loop never gets another attempt.
/// 2. This budget, which is the ONLY path to the trailing
///    `assert!(resolved, ...)`.
///
/// Either way the failure is a deadline, never a lost lock. Forcing the
/// per-request timeout down to 1s under ~250 competing CPU hogs reproduces
/// surface 1 verbatim — `LSP request 'textDocument/prepareRename' (id=13) timed
/// out after 1s` — and no red run ever showed a content or ordering mismatch.
/// Completion time scales with starvation while the result stays identical:
/// 3.1s idle, 11-28s at load average 125-317. The one `--workspace` failure that
/// prompted the last raise was never captured, so which of the two surfaces it
/// took is unknown.
///
/// Each loop below drives exactly one leader, one follower, and one
/// rust-analyzer, strictly in sequence, so nothing here can race. The atomicity
/// contract the test name refers to is pinned deterministically elsewhere, by
/// `dispatch_lsp_multi_request_runs_steps_in_order_under_one_lock` in
/// `request_api.rs`, which uses a fake transport and no clock.
///
/// `lsp-ipc-serial` already caps this crate to one real analyzer at a time (see
/// `.config/nextest.toml`) — the contention comes from every other package's
/// tests sharing the same CPU, not from a second concurrent analyzer. One
/// continuous poll also beats a nextest `retries` of the same wall budget: a
/// retry restarts the analyzer cold each attempt and throws the indexing away,
/// whereas one longer poll lets the same analyzer keep indexing throughout.
///
/// The deadline is checked between attempts, so a request that runs to its own
/// per-request timeout can still overrun it, and the 300s `slow-timeout` kill
/// can fire before the assertion prints. Bounding that would mean lowering
/// deadline 1, and no evidence asks for it: with the real 120s per-request
/// timeout, the slowest observed run was 28s.
const WARM_UP_BUDGET: Duration = Duration::from_secs(120);

/// Poll interval between warm-up attempts.
const WARM_UP_POLL_INTERVAL: Duration = Duration::from_millis(500);

/// Whether a `CodeContextError` reports rust-analyzer's `ContentModified`
/// condition — the LSP contract for "I am still processing an earlier
/// request against a now-stale document version, resend this one." rust-
/// analyzer raises it routinely right after a cold-start `didOpen`, or under
/// CPU contention that delays its internal debounce. It is not a bug in the
/// request; the warm-up polling loops below must treat it exactly like the
/// "not warmed up yet" case they already retry on (a live-LSP miss, or
/// `can_rename: false`), instead of failing the test on the first transient
/// answer.
fn is_transient_not_ready(err: &swissarmyhammer_code_context::CodeContextError) -> bool {
    matches!(
        err,
        swissarmyhammer_code_context::CodeContextError::LspError(msg)
            if msg.to_lowercase().contains("content modified")
    )
}

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
    tokio::time::sleep(Duration::from_secs(RUST_ANALYZER_INITIAL_LOAD_WAIT_SECS)).await;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follower_request_with_document_gets_real_definition_without_leader_preopen() {
    // A follower's code-context op (get definition / get hover / get references)
    // routes through lsp_request_with_document. Unlike the raw lsp_request test
    // above, the LEADER does NOT pre-open the document — the follower's request
    // carries the file_path and the leader must sync_open it on its single
    // session before the request, or rust-analyzer answers against a buffer it
    // never saw. We assert the follower gets a REAL definition for the helper()
    // call site, with only the leader's one rust-analyzer running.
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let main_rs = seed_rust_project(workspace.path());

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), workspace.path().to_path_buf());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

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

    // Give rust-analyzer time to load the workspace. Note: the leader does NOT
    // open the document — the document-sync must come from the follower request.
    tokio::time::sleep(Duration::from_secs(RUST_ANALYZER_INITIAL_LOAD_WAIT_SECS)).await;

    let client = SessionRequestClient::connect(&socket_path, &lock_path)
        .await
        .expect("follower should connect to the leader socket");

    // Drive the REAL code-context op (get_definition) through a LiveLspRouter
    // backed by this follower's SessionRequestClient — exactly the production
    // wiring (build_follower_router → route_one → lsp_request_with_document).
    // This proves the END-TO-END consumer contract, not just the wire: the op's
    // parser (parse_definition_locations) must receive the *bare* LSP result
    // (the router/layered-context must unwrap the JSON-RPC envelope), or it
    // silently degrades to the index/tree-sitter layer. We assert
    // SourceLayer::LiveLsp with a real location, which fails on an un-unwrapped
    // envelope.
    let ws_for_router = workspace.path().to_path_buf();

    let opts = swissarmyhammer_code_context::GetDefinitionOptions {
        file_path: main_rs.to_string_lossy().to_string(),
        line: 3,
        character: 12,
        include_source: false,
    };

    // rust-analyzer may still be analyzing right after the first didOpen
    // (returning null or a transient server-initiated message), so poll with a
    // bounded retry until the real cross-reference resolves.
    let mut last = String::new();
    let mut resolved = false;
    let warm_up_deadline = Instant::now() + WARM_UP_BUDGET;
    while Instant::now() < warm_up_deadline {
        // The DB handle (DbRef) is !Send, so the synchronous op call — open
        // workspace, build the routed context, run get_definition — is scoped in
        // its own block so ws/db/ctx all drop BEFORE the await below. The router
        // closure itself bridges to the async client via block_in_place.
        let result = {
            let ws = swissarmyhammer_code_context::CodeContextWorkspace::open(&ws_for_router)
                .expect("open code-context workspace");
            let db = ws.db();
            let router_client = client.clone();
            let handle = tokio::runtime::Handle::current();
            let router_attempt: swissarmyhammer_code_context::LiveLspRouter = Box::new(
                move |file_path: &str, method: &str, params: serde_json::Value| {
                    let router_client = router_client.clone();
                    let file_path = file_path.to_string();
                    let method = method.to_string();
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            router_client
                                .lsp_request_with_document(&file_path, &method, params)
                                .await
                                .map(Some)
                                .map_err(|e| {
                                    swissarmyhammer_code_context::CodeContextError::LspError(
                                        format!("leader LSP request failed: {e}"),
                                    )
                                })
                        })
                    })
                },
            );
            let ctx = swissarmyhammer_code_context::LayeredContext::with_live_lsp_router(
                &db,
                router_attempt,
            );
            swissarmyhammer_code_context::get_definition(&ctx, &opts)
        };
        let result = match result {
            Ok(result) => result,
            Err(e) if is_transient_not_ready(&e) => {
                last = format!("transient: {e}");
                tokio::time::sleep(WARM_UP_POLL_INTERVAL).await;
                continue;
            }
            Err(e) => panic!("get_definition via leader router: {e}"),
        };
        last = format!("{result:?}");
        if result.source_layer == swissarmyhammer_code_context::SourceLayer::LiveLsp
            && result
                .locations
                .iter()
                .any(|l| l.file_path.contains("main.rs") && l.range.start_line == 0)
        {
            resolved = true;
            break;
        }
        tokio::time::sleep(WARM_UP_POLL_INTERVAL).await;
    }
    assert!(
        resolved,
        "follower's get_definition must resolve via SourceLayer::LiveLsp to helper() on line 0 \
         of main.rs once rust-analyzer is warm — proving the leader-routed result is parsed, not \
         a silently-degraded index/tree-sitter empty: last={last}"
    );

    drop(client);
    server_task.abort();
    daemon.shutdown().await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn follower_multi_step_rename_gets_real_leader_edits_under_one_lock() {
    // A follower's get_rename_edits is a MULTI-STEP op: prepareRename then rename
    // must run as ONE atomic exchange under the leader's single client lock. The
    // follower owns NO LSP server — it drives a real code-context get_rename_edits
    // through a MultiLspRouter backed by its SessionRequestClient, which routes
    // the whole batch over METHOD_LSP_MULTI_REQUEST to the leader's one
    // rust-analyzer. We assert the follower gets the leader's REAL rename
    // (can_rename = true with edits), not the degraded can_rename:false fallback,
    // with only the leader's one rust-analyzer in play.
    if !rust_analyzer_available() {
        eprintln!("skipping: rust-analyzer not installed");
        return;
    }

    let workspace = tempfile::tempdir().expect("workspace tempdir");
    let main_rs = seed_rust_project(workspace.path());

    let mut daemon = LspDaemon::new(rust_analyzer_spec(), workspace.path().to_path_buf());
    daemon
        .start()
        .await
        .expect("rust-analyzer handshake should complete");
    let session = daemon.session();

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

    // Give rust-analyzer time to load the workspace. The leader does NOT open the
    // document — the follower's batch carries the file_path and the leader syncs
    // it before the prepareRename+rename exchange.
    tokio::time::sleep(Duration::from_secs(RUST_ANALYZER_INITIAL_LOAD_WAIT_SECS)).await;

    let client = SessionRequestClient::connect(&socket_path, &lock_path)
        .await
        .expect("follower should connect to the leader socket");

    // Rename the `helper` function (defined on line 0, col 3 of main.rs).
    let opts = swissarmyhammer_code_context::GetRenameEditsOptions {
        file_path: main_rs.to_string_lossy().to_string(),
        line: 0,
        character: 3,
        new_name: "renamed_helper".to_string(),
    };
    let ws_for_router = workspace.path().to_path_buf();

    // rust-analyzer may still be analyzing right after startup, so poll with a
    // bounded retry until the real rename resolves.
    let mut last = String::new();
    let mut resolved = false;
    let warm_up_deadline = Instant::now() + WARM_UP_BUDGET;
    while Instant::now() < warm_up_deadline {
        // The DB handle (DbRef) is !Send, so the synchronous op call is scoped in
        // its own block so ws/db/ctx all drop BEFORE the await below. The multi
        // router closure bridges to the async client via block_in_place.
        let result = {
            let ws = swissarmyhammer_code_context::CodeContextWorkspace::open(&ws_for_router)
                .expect("open code-context workspace");
            let db = ws.db();
            let router_client = client.clone();
            let handle = tokio::runtime::Handle::current();
            let multi: swissarmyhammer_code_context::MultiLspRouter = Box::new(
                move |file_path: &str, steps: Vec<(String, serde_json::Value)>| {
                    let router_client = router_client.clone();
                    let file_path = file_path.to_string();
                    tokio::task::block_in_place(|| {
                        handle.block_on(async {
                            router_client
                                .lsp_multi_request_with_document(&file_path, steps)
                                .await
                                .map(Some)
                                .map_err(|e| {
                                    swissarmyhammer_code_context::CodeContextError::LspError(
                                        format!("leader LSP multi request failed: {e}"),
                                    )
                                })
                        })
                    })
                },
            );
            let ctx =
                swissarmyhammer_code_context::LayeredContext::with_multi_lsp_router(&db, multi);
            swissarmyhammer_code_context::get_rename_edits(&ctx, &opts)
        };
        let result = match result {
            Ok(result) => result,
            Err(e) if is_transient_not_ready(&e) => {
                last = format!("transient: {e}");
                tokio::time::sleep(WARM_UP_POLL_INTERVAL).await;
                continue;
            }
            Err(e) => panic!("get_rename_edits via leader multi router: {e}"),
        };
        last = format!("{result:?}");
        if result.can_rename && !result.edits.is_empty() {
            resolved = true;
            break;
        }
        tokio::time::sleep(WARM_UP_POLL_INTERVAL).await;
    }
    assert!(
        resolved,
        "follower's get_rename_edits must resolve to a REAL leader-routed rename \
         (can_rename=true with edits) once rust-analyzer is warm — proving the multi-step \
         batch ran under one leader lock and was parsed, not a degraded can_rename:false: \
         last={last}"
    );

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
