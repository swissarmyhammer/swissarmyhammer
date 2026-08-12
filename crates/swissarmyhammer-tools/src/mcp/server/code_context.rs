//! Code-context startup: which process indexes the workspace, and what it runs.
//!
//! Exactly one process per workspace may drive the index and the stdio LSP
//! servers, so startup begins with a leadership election. The leader spawns the
//! LSP supervisor, the tree-sitter indexing workers, and the diagnostics
//! fan-out; a follower spawns none of them and instead polls for promotion
//! while subscribing to the leader's diagnostics broadcast.

use super::McpServer;
use std::sync::Arc;

use swissarmyhammer_common::utils::find_git_repository_root_from;

impl McpServer {
    /// How often followers retry promotion to leader.
    const REELECTION_POLL_INTERVAL: std::time::Duration = std::time::Duration::from_secs(5);

    /// How often the leader refreshes its leadership lease. Mirrors
    /// [`swissarmyhammer_leader_election::HEARTBEAT_INTERVAL`] so the in-process
    /// loop cadence matches the lease-freshness math (TTL is 3x this).
    const LEADER_HEARTBEAT_INTERVAL: std::time::Duration =
        swissarmyhammer_code_context::LEASE_HEARTBEAT_INTERVAL;

    /// How often the LSP supervisor is polled for daemon health.
    const LSP_HEALTH_CHECK_INTERVAL: std::time::Duration = std::time::Duration::from_secs(60);

    /// Initialize code-context workspace and start indexing at MCP startup.
    ///
    /// Finds the git repository root from the working directory, opens a
    /// CodeContextWorkspace (which triggers file discovery and background indexing),
    /// then runs full tree-sitter indexing with symbols and call edges.
    ///
    /// Uses `std::sync::Once` to ensure this runs exactly once, even when
    /// multiple MCP connections call it concurrently (Claude Code opens ~3).
    ///
    /// # Panics
    ///
    /// Panics if called outside a Tokio runtime (internally uses `tokio::spawn`).
    pub(super) fn initialize_code_context(work_dir: &std::path::Path) {
        static INIT: std::sync::Once = std::sync::Once::new();
        let work_dir = work_dir.to_path_buf();
        INIT.call_once(move || Self::do_initialize_code_context(&work_dir));
    }

    fn do_initialize_code_context(work_dir: &std::path::Path) {
        let Some(workspace_root) = resolve_workspace_root(work_dir) else {
            return;
        };
        tracing::info!(
            "code-context: initializing for workspace {}",
            workspace_root.display()
        );

        // Create `.reviewignore` at server start rather than waiting for the
        // first review, so it's on disk (and editable — e.g. to exclude
        // upstream files in a fork) the moment `sah serve` comes up. Runs for
        // both leader and follower, before the leadership/LSP work below; the
        // enclosing `Once` in `initialize_code_context` already makes this
        // once-per-process, and `ensure_reviewignore` itself is idempotent.
        Self::ensure_workspace_review_files(&workspace_root);

        // Decide leadership FIRST. The LSP server (rust-analyzer, sourcekit-lsp,
        // clangd) speaks stdio only — one client, no listener — so only the
        // elected leader may spawn it. Spawning the supervisor before leadership
        // is decided made every `sah serve` (and every stdio-MCP subagent) launch
        // its own rust-analyzer over the same tree, thrashing the shared cargo
        // `target/`. Gating the spawn on `ws.is_leader()` keys LSP-session
        // ownership on the same code-context election (same workspace root), so
        // LSP and index leadership coincide.
        let Some(ws) = open_workspace(&workspace_root) else {
            return;
        };

        let is_leader = ws.lock().expect("workspace mutex poisoned").is_leader();

        // Single gate: only the leader gets a supervisor handle; a follower gets
        // `None` and spawns no LSP server at all.
        match Self::spawn_lsp_supervisor_if_leader(is_leader, &workspace_root) {
            Some(lsp_handle) => {
                // Prove which build elected itself leader: this is the
                // process actually indexing the workspace, so record its
                // baked-in git SHA in its own log.
                tracing::info!(
                    git_sha = swissarmyhammer_common::build_info::GIT_SHA,
                    workspace = %workspace_root.display(),
                    "code-context: elected leader"
                );
                // Leader: supervisor spawned. Start TS indexing + file watcher
                // and run the LSP health/indexing loop.
                Self::start_workers_if_leader(&ws, &workspace_root);
                // The leader runs no follower diagnostics subscriber, so there is
                // nothing to cancel on (a no-op) re-election.
                Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone(), None);
                // The leader heartbeats its lease; if preempted (a live session
                // took over a stale lease) it steps down to follower and starts
                // polling for re-promotion — preserving the single-writer
                // invariant (no two indexers).
                Self::spawn_leader_heartbeat_loop(Arc::clone(&ws), workspace_root.clone());
                Self::spawn_lsp_health_loop(lsp_handle, ws, workspace_root);
            }
            None => {
                // Follower: spawned NOTHING (no rust-analyzer, no indexing).
                // Only poll for promotion. On a successful promotion (the leader
                // exited), the re-election loop performs a cold re-spawn of the
                // supervisor and workers via
                // `start_indexing_workers_after_promotion`.
                tracing::info!(
                    "code-context: follower for {} — not spawning LSP server; polling for promotion",
                    workspace_root.display()
                );
                // A follower owns no LSP session, so it subscribes to the
                // leader's diagnostics broadcast to receive per-uri updates the
                // leader publishes (the cross-process fan-out's receive half).
                // The returned cancel handle is threaded into the re-election
                // loop so the subscriber is stopped on promotion, BEFORE this
                // process becomes the publisher (otherwise the orphaned
                // subscriber reconnects to its own proxy and self-logs).
                let sub_cancel = Self::spawn_follower_diagnostics_subscriber(&ws);
                Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone(), sub_cancel);
            }
        }
    }

    /// Ensure `<workspace_root>/.reviewignore` exists at server start, rather
    /// than waiting for the first review to create it lazily.
    ///
    /// Runs once per process for every `sah serve` (leader or follower alike),
    /// immediately after the workspace root resolves and before any
    /// leadership/LSP work. [`ensure_reviewignore`](swissarmyhammer_validators::review::ignore::ensure_reviewignore)
    /// is idempotent — it never overwrites an existing file — so concurrent
    /// sibling servers racing at startup at worst both write the identical
    /// default template.
    ///
    /// A failure to write the file (e.g. a read-only workspace root) is
    /// downgraded to a warning: server startup must never fail over an
    /// unwritable ignore file.
    fn ensure_workspace_review_files(workspace_root: &std::path::Path) {
        if let Err(e) =
            swissarmyhammer_validators::review::ignore::ensure_reviewignore(workspace_root)
        {
            tracing::warn!(
                "code-context: failed to create .reviewignore in {}: {}",
                workspace_root.display(),
                e
            );
        }
    }

    /// Leader-gated LSP supervisor spawn — the single decision point that keys
    /// LSP-session ownership on the code-context leadership election.
    ///
    /// Returns the supervisor task handle when `is_leader` is `true` (the leader
    /// spawns the one stdio LSP child for the workspace), and `None` when
    /// `is_leader` is `false` (a follower spawns nothing). Keeping this the only
    /// place the startup path spawns the supervisor ensures a follower can never
    /// launch its own rust-analyzer over the shared tree.
    fn spawn_lsp_supervisor_if_leader(
        is_leader: bool,
        workspace_root: &std::path::Path,
    ) -> Option<
        tokio::task::JoinHandle<Vec<(String, swissarmyhammer_code_context::SharedLspSession)>>,
    > {
        is_leader.then(|| Self::spawn_lsp_supervisor(workspace_root.to_path_buf()))
    }

    /// If the workspace is already leader, start TS indexing + watcher workers.
    fn start_workers_if_leader(
        ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: &std::path::Path,
    ) {
        let ws_lock = ws.lock().expect("workspace mutex poisoned");
        if let Some(shared_db) = ws_lock.shared_db() {
            let shutdown = ws_lock
                .leader_shutdown_flag()
                .expect("leader has a shutdown flag");
            Self::start_indexing_workers(workspace_root.to_path_buf(), shared_db, shutdown);
        }
    }

    /// Spawn the LSP supervisor task. Starts every configured LSP daemon,
    /// installs the supervisor into `LSP_SUPERVISOR`, and returns the list of
    /// successfully-running `(server_name, session)` pairs via the task's join
    /// handle.
    fn spawn_lsp_supervisor(
        workspace_root: std::path::PathBuf,
    ) -> tokio::task::JoinHandle<Vec<(String, swissarmyhammer_code_context::SharedLspSession)>>
    {
        tokio::spawn(async move {
            // Build the LSP-server stderr-noise filter from code-context's
            // stacked config and inject it into the supervisor. The filter
            // source lives in code-context; `swissarmyhammer-lsp` only exposes
            // the injection seam, so it carries no config dependency.
            let mut supervisor = swissarmyhammer_lsp::LspSupervisorManager::new(workspace_root);
            if let Ok(compiled) = swissarmyhammer_code_context::CompiledCodeContextConfig::compile(
                &swissarmyhammer_code_context::load_code_context_config(),
            ) {
                let compiled = std::sync::Arc::new(compiled);
                supervisor =
                    supervisor.with_stderr_filter(std::sync::Arc::new(move |line: &str| {
                        swissarmyhammer_code_context::should_filter_stderr(line, &compiled)
                    }));
            }
            let results = supervisor.start().await;
            let ok_count = results.iter().filter(|r| r.is_ok()).count();
            let err_count = results.iter().filter(|r| r.is_err()).count();
            tracing::info!(
                "code-context: LSP supervisor started — {} servers ok, {} failed",
                ok_count,
                err_count
            );
            for r in &results {
                if let Err(e) = r {
                    tracing::warn!("code-context: LSP start error: {}", e);
                }
            }

            let clients = collect_running_lsp_clients(&supervisor);
            tracing::info!(
                "code-context: {} LSP clients available for indexing: {:?}",
                clients.len(),
                clients.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
            );

            use crate::mcp::tools::code_context::LSP_SUPERVISOR;
            let _ = LSP_SUPERVISOR.set(Arc::new(tokio::sync::Mutex::new(supervisor)));

            clients
        })
    }

    /// Subscribe a follower to the leader's diagnostics broadcast.
    ///
    /// A follower spawns no LSP server, so it cannot observe diagnostics
    /// in-process; instead it rides the leader's existing pub/sub proxy (the
    /// same one the leader re-publishes onto) via the public `Subscriber::open`
    /// seam and receives each per-uri [`DiagnosticsBusMessage`]. The subscriber's
    /// `recv` blocks, so it runs on a blocking task. The received updates are
    /// traced today; folding them into a follower-served diagnostics view is the
    /// documented application seam (`on_update`).
    ///
    /// Returns the cooperative cancel handle for the spawned subscriber loop
    /// (`None` when no bus is available, so nothing was spawned). The re-election
    /// loop sets this flag on promotion so the orphaned follower subscriber stops
    /// before this process becomes the leader/publisher — a `spawn_blocking` task
    /// cannot be force-aborted mid-`recv`, so the subscriber must observe the flag
    /// on its next ≤500ms wake (see `subscribe_diagnostics_over_bus`).
    fn spawn_follower_diagnostics_subscriber(
        ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    ) -> Option<Arc<std::sync::atomic::AtomicBool>> {
        let backend = ws
            .lock()
            .expect("workspace mutex poisoned")
            .bus_addresses()
            .map(|a| a.backend);
        let backend = backend?;
        let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let loop_cancel = Arc::clone(&cancel);
        tokio::task::spawn_blocking(move || {
            let result = swissarmyhammer_diagnostics::subscribe_diagnostics_over_bus(
                &backend,
                &loop_cancel,
                |msg| {
                    // Application seam: a follower-served diagnostics cache would
                    // fold `msg` in here, keyed by `msg.uri`. For now the receipt
                    // is traced so the cross-process fan-out is observable.
                    tracing::debug!(
                        uri = %msg.uri,
                        count = msg.diagnostics.len(),
                        "diagnostics: follower received bus update"
                    );
                },
            );
            if let Err(e) = result {
                tracing::warn!(
                    error = %e,
                    "diagnostics: follower could not subscribe to the diagnostics bus"
                );
            }
        });
        tracing::info!("diagnostics: follower subscribed to the diagnostics bus");
        Some(cancel)
    }

    /// Poll for promotion. `follower_subscriber_cancel`, when present, is the
    /// cancel handle for this follower's diagnostics-bus subscriber; on a
    /// successful promotion it is signaled before the leader-side publish path
    /// starts (see `handle_promotion_result`). The leader path passes `None` (it
    /// never started a follower subscriber).
    ///
    /// A follower retries every [`REELECTION_POLL_INTERVAL`](Self::REELECTION_POLL_INTERVAL).
    /// Once it is promoted — or if it was the leader already — the loop exits
    /// permanently. Promotion is one-shot: leadership lost afterwards is not
    /// recovered automatically, but the `LeaderGuard` is held for the lifetime of
    /// the process by the `Arc` that `spawn_lsp_health_loop` keeps.
    fn spawn_reelection_loop(
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
        follower_subscriber_cancel: Option<Arc<std::sync::atomic::AtomicBool>>,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Self::REELECTION_POLL_INTERVAL).await;
                let promoted = try_promote_workspace(&ws);
                if handle_promotion_result(
                    promoted,
                    &ws,
                    &workspace_root,
                    follower_subscriber_cancel.as_ref(),
                ) {
                    break;
                }
            }
        });
    }

    /// Spawn the leader's lease-heartbeat loop.
    ///
    /// Only the leader runs this. Every `LEADER_HEARTBEAT_INTERVAL` it refreshes
    /// its leadership lease via `ws.heartbeat_lease()`. A `true` return means we
    /// are still the leader. A `false` return means we were PREEMPTED — a live
    /// session took over our stale lease (e.g. this process was wedged long
    /// enough for the lease to expire). On preemption the leader MUST stop being
    /// a writer: it steps down to follower (releasing the flock and the
    /// read-write DB connection), then starts the normal re-election poll so it
    /// can reclaim leadership later. This is the single-writer invariant — a
    /// preempted leader never keeps indexing alongside the new one.
    fn spawn_leader_heartbeat_loop(
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
    ) {
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(Self::LEADER_HEARTBEAT_INTERVAL).await;
                let still_leader = ws
                    .lock()
                    .expect("workspace mutex poisoned")
                    .heartbeat_lease();
                if still_leader {
                    continue;
                }
                tracing::warn!(
                    workspace = %workspace_root.display(),
                    "code-context: lost leadership lease, stepping down"
                );
                ws.lock().expect("workspace mutex poisoned").step_down();
                // Now a follower again — poll for re-promotion and stop this loop.
                Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone(), None);
                break;
            }
        });
    }

    /// Waits for the LSP supervisor to finish, starts LSP indexing workers if
    /// we're the leader, then runs the 60s LSP health-check loop forever.
    fn spawn_lsp_health_loop(
        lsp_handle: tokio::task::JoinHandle<
            Vec<(String, swissarmyhammer_code_context::SharedLspSession)>,
        >,
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
    ) {
        Self::spawn_drain_supervisor_and_health_loop(lsp_handle, move |clients| {
            start_lsp_workers_if_leader(&ws, &workspace_root, clients, "");
        });
    }

    /// Await the LSP supervisor's startup task, hand its running clients to
    /// `start_workers` (skipped when none are available), then run the LSP
    /// health-check loop for the rest of the process's lifetime.
    ///
    /// Shared by initial-leader startup and the post-promotion cold re-spawn;
    /// the two differ only in how they start workers (the `start_workers`
    /// closure), so the await/drain/health-loop shell lives here once.
    fn spawn_drain_supervisor_and_health_loop<F>(
        lsp_handle: tokio::task::JoinHandle<
            Vec<(String, swissarmyhammer_code_context::SharedLspSession)>,
        >,
        start_workers: F,
    ) where
        F: FnOnce(&[(String, swissarmyhammer_code_context::SharedLspSession)]) + Send + 'static,
    {
        tokio::spawn(async move {
            let clients = match lsp_handle.await {
                Ok(clients) => clients,
                Err(e) => {
                    tracing::error!("code-context: LSP supervisor task failed: {e}");
                    Vec::new()
                }
            };
            if clients.is_empty() {
                tracing::info!("code-context: no LSP clients available, skipping LSP indexing");
            } else {
                start_workers(&clients);
            }
            run_lsp_health_check_loop().await;
        });
    }

    /// Spawn the tree-sitter indexing task and the file-watcher task.
    ///
    /// `log_suffix` is appended to the "starting" log message so callers can
    /// distinguish normal startup from post-promotion startup (e.g. pass
    /// `" (after promotion)"` or `""`).
    fn spawn_ts_and_watcher_workers(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
        shutdown: swissarmyhammer_code_context::ShutdownFlag,
        log_suffix: &'static str,
    ) {
        // Start tree-sitter indexing
        let ts_root = workspace_root.clone();
        let ts_db = std::sync::Arc::clone(&shared_db);
        let ts_shutdown = std::sync::Arc::clone(&shutdown);
        tokio::spawn(async move {
            use crate::mcp::tools::code_context::index_discovered_files_async;
            // A step-down between spawn and execution must abort the one-shot
            // index so the stepped-down leader does not write.
            if ts_shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                return;
            }
            tracing::info!(
                "code-context: starting tree-sitter indexing for {}{}",
                ts_root.display(),
                log_suffix,
            );
            // The MCP bootstrap pass has no JSON-RPC progress channel —
            // pass the no-op reporter. The follow-on rebuild-index card will
            // wire a `McpProgressReporter` into `execute_rebuild_index`.
            index_discovered_files_async(
                &ts_root,
                ts_db,
                swissarmyhammer_code_context::noop_reporter(),
                std::sync::Arc::clone(&ts_shutdown),
            )
            .await;
            tracing::info!(
                "code-context: tree-sitter indexing complete for {}",
                ts_root.display()
            );
        });

        // Start file watcher
        let watcher_root = workspace_root.clone();
        let watcher_db = std::sync::Arc::clone(&shared_db);
        let watcher_shutdown = std::sync::Arc::clone(&shutdown);
        tokio::spawn(async move {
            use crate::mcp::tools::code_context::watcher::start_code_context_watcher;
            let _watcher_handle =
                start_code_context_watcher(watcher_root, watcher_db, watcher_shutdown);
            std::future::pending::<()>().await;
        });

        // Start the leader's periodic FS-walk reconcile (the index correctness
        // floor beneath the watcher's event fast-path). This call site is only
        // ever reached on the leader -- it is shared by initial-leader startup
        // and the post-promotion cold re-spawn, both of which run after
        // leadership is established -- so the leader-only invariant the reconcile
        // loop requires holds by construction, exactly as it does for the
        // watcher spawned just above.
        let reconcile_root = workspace_root;
        let reconcile_db = std::sync::Arc::clone(&shared_db);
        let reconcile_shutdown = shutdown;
        tokio::spawn(async move {
            use crate::mcp::tools::code_context::watcher::run_periodic_reconcile;
            let _reconcile_handle =
                run_periodic_reconcile(reconcile_root, reconcile_db, reconcile_shutdown);
            std::future::pending::<()>().await;
        });
    }

    /// Start tree-sitter indexing and file watcher workers with an existing shared DB.
    /// LSP workers are started separately by the LSP task.
    fn start_indexing_workers(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
        shutdown: swissarmyhammer_code_context::ShutdownFlag,
    ) {
        Self::spawn_ts_and_watcher_workers(workspace_root, shared_db, shutdown, "");
    }

    /// Start indexing workers after a follower-to-leader promotion.
    ///
    /// A process that started as a follower spawned no LSP server, so promotion
    /// performs a **cold re-spawn**: it starts the supervisor (rust-analyzer et
    /// al.) for the first time in this process, then starts the LSP indexing
    /// workers off it and runs the health loop. This is the handoff path — only
    /// reached once the prior leader has exited and released its flock — so a
    /// cold start (re-spawn + re-index) is acceptable.
    fn start_indexing_workers_after_promotion(
        workspace_root: std::path::PathBuf,
        shared_db: swissarmyhammer_code_context::SharedDb,
        shutdown: swissarmyhammer_code_context::ShutdownFlag,
        bus_frontend: Option<String>,
        socket_path: std::path::PathBuf,
    ) {
        Self::spawn_ts_and_watcher_workers(
            workspace_root.clone(),
            std::sync::Arc::clone(&shared_db),
            std::sync::Arc::clone(&shutdown),
            " (after promotion)",
        );

        // LSP workers: cold-spawn the supervisor (followers never spawned one),
        // then start indexing workers off its running sessions and run the
        // health loop. Leadership is already established by the time we reach
        // here, so workers start directly (no further is_leader gate).
        let lsp_db = std::sync::Arc::clone(&shared_db);
        let lsp_handle = Self::spawn_lsp_supervisor(workspace_root.clone());
        Self::spawn_drain_supervisor_and_health_loop(lsp_handle, move |clients| {
            spawn_lsp_workers_for_clients(
                &workspace_root,
                &lsp_db,
                clients,
                " (after promotion)",
                bus_frontend.as_deref(),
                &socket_path,
                std::sync::Arc::clone(&shutdown),
            );
        });
    }
}

/// Walk up from `work_dir` to find the enclosing git repository root. Returns
/// `None` (and logs) if we're not inside a repo — callers use that signal to
/// skip code-context initialization.
fn resolve_workspace_root(work_dir: &std::path::Path) -> Option<std::path::PathBuf> {
    match find_git_repository_root_from(work_dir) {
        Some(root) => Some(root),
        None => {
            tracing::info!(
                "code-context: no git repository found from {}, skipping initialization",
                work_dir.display()
            );
            None
        }
    }
}

/// Open the code-context workspace and wrap it in an `Arc<Mutex>` for sharing
/// across the spawned background tasks. Logs and returns `None` on failure.
fn open_workspace(
    workspace_root: &std::path::Path,
) -> Option<Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>> {
    tracing::info!(
        "code-context: opening workspace for {}",
        workspace_root.display()
    );
    match swissarmyhammer_code_context::CodeContextWorkspace::open(workspace_root) {
        Ok(ws) => {
            tracing::info!(
                "code-context: workspace opened as {}",
                if ws.is_leader() { "leader" } else { "follower" }
            );
            Some(Arc::new(std::sync::Mutex::new(ws)))
        }
        Err(e) => {
            tracing::warn!("code-context: failed to open workspace: {}", e);
            None
        }
    }
}

/// Collect every running daemon's `(server_name, session)` pair from the
/// supervisor. Daemons that are not in the `Running` state are skipped.
///
/// The worker consumes the daemon-owned [`LspSession`](swissarmyhammer_lsp::LspSession),
/// so it shares the one open-document set with the query ops and the
/// diagnostics path rather than driving its own client lifecycle.
fn collect_running_lsp_clients(
    supervisor: &swissarmyhammer_lsp::LspSupervisorManager,
) -> Vec<(String, swissarmyhammer_code_context::SharedLspSession)> {
    supervisor
        .daemon_names()
        .into_iter()
        .filter_map(|name| lsp_client_if_running(supervisor, name))
        .collect()
}

/// Return `(name, session)` for the daemon if it's in the `Running` state.
fn lsp_client_if_running(
    supervisor: &swissarmyhammer_lsp::LspSupervisorManager,
    name: String,
) -> Option<(String, swissarmyhammer_code_context::SharedLspSession)> {
    let daemon = supervisor.get_daemon(&name)?;
    match daemon.state() {
        swissarmyhammer_lsp::LspDaemonState::Running { .. } => Some((name, daemon.session())),
        _ => None,
    }
}

/// Spawn an `spawn_lsp_indexing_worker` per running LSP session, plus the
/// leader-owned diagnostics file watcher and cross-process bus fan-out.
///
/// `log_suffix` is appended to the startup log so callers can distinguish fresh
/// startup from post-promotion startup (e.g. `" (after promotion)"` or `""`).
/// `bus_frontend` is the leader's bus frontend address (from
/// [`CodeContextWorkspace::bus_addresses`]); `None` skips the cross-process
/// fan-out (the in-process diagnostics fan-out is unaffected).
fn spawn_lsp_workers_for_clients(
    workspace_root: &std::path::Path,
    shared_db: &swissarmyhammer_code_context::SharedDb,
    clients: &[(String, swissarmyhammer_code_context::SharedLspSession)],
    log_suffix: &str,
    bus_frontend: Option<&str>,
    socket_path: &std::path::Path,
    shutdown: swissarmyhammer_code_context::ShutdownFlag,
) {
    if clients.is_empty() {
        return;
    }

    // The leader binds the election request socket and serves the SAH request
    // API over its single session, so followers (which spawn no LSP server) can
    // route diagnose/query calls to this one server. Done here — the single
    // chokepoint both initial-leader and post-promotion startup pass through —
    // right where the running sessions first become available.
    spawn_request_server_for_leader(socket_path, clients);
    use swissarmyhammer_code_context::{spawn_lsp_indexing_worker, LspWorkerConfig};
    for (server_name, session) in clients {
        let worker_db = std::sync::Arc::clone(shared_db);
        spawn_lsp_indexing_worker(
            workspace_root.to_path_buf(),
            worker_db,
            session.clone(),
            LspWorkerConfig::default(),
            server_name.clone(),
            shutdown.clone(),
        );
        tracing::info!(
            "code-context: LSP indexing worker started for {}{} (server: {})",
            workspace_root.display(),
            log_suffix,
            server_name,
        );

        // Each session has its own in-process diagnostics fan-out, so the
        // cross-process re-publisher is per session.
        spawn_diagnostics_fan_out(server_name, session, bus_frontend);

        // A third consumer of the same in-process fan-out: feed each per-uri
        // update into the subscribable diagnostics MCP resource so a host that
        // subscribed gets `notifications/resources/updated` without a tool call.
        // Runs regardless of `bus_frontend` — the resource lives in-process.
        spawn_diagnostics_resource_feed(server_name, session);
    }

    // Exactly ONE diagnostics file watcher per workdir (not per session): it
    // watches the tree once and routes each changed file to the session whose
    // server handles that extension, so a multi-language workspace does not run
    // N watchers and a `.py` edit is never fed into the rust session. The watch
    // root is canonicalized so the watcher's `didChange` uris match the
    // canonical paths the sessions open documents under (on macOS `/var`
    // resolves to `/private/var`); a mismatch would split a file into two
    // documents from the server's view.
    let routes = build_diagnostics_routes(clients);
    if !routes.is_empty() {
        let watch_root =
            std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
        // Best-effort watcher-push: tell a connected host (via a plain MCP
        // `notifications/message`) when a native edit is seen. The notifier is a
        // courtesy channel — a `None`/absent peer never gates the re-diagnose.
        let notifier: swissarmyhammer_diagnostics::WatcherNotifier =
            std::sync::Arc::new(|path: &std::path::Path| {
                crate::mcp::diagnostics_resource::watcher_push_log(path);
            });
        swissarmyhammer_diagnostics::start_diagnostics_watcher_with_notifier(
            watch_root,
            routes,
            Some(notifier),
        );
        tracing::info!(
            "diagnostics: file watcher started for {}{}",
            workspace_root.display(),
            log_suffix,
        );
    }
}

/// Bind the leader-election request socket and serve the SAH request API onto
/// the leader's single LSP session, so out-of-process followers can route a
/// `diagnose` (or LSP query) to the one server the leader owns.
///
/// `socket_path` is the election socket surfaced on
/// [`CodeContextWorkspace::socket_path`](swissarmyhammer_code_context::CodeContextWorkspace::socket_path);
/// the leader binds a [`RequestServer`](swissarmyhammer_leader_election::request_ipc::RequestServer)
/// there. The request API is served over the first running session in `clients`
/// (the diagnose/lsp_request methods carry no language selector, so the
/// multiplexer serves one session — the common single-language case; multi-server
/// follower routing is deferred). The serve task runs for the process lifetime.
///
/// Best-effort: a bind failure (e.g. a live socket from another leader) is logged
/// and skipped — it must not take down the leader's indexing/diagnostics workers.
/// A follower that then cannot connect surfaces the typed not-leader error on its
/// own side.
fn spawn_request_server_for_leader(
    socket_path: &std::path::Path,
    clients: &[(String, swissarmyhammer_code_context::SharedLspSession)],
) {
    let Some((server_name, session)) = clients.first() else {
        return;
    };
    let server = match swissarmyhammer_diagnostics::RequestServer::bind(socket_path) {
        Ok(server) => server,
        Err(e) => {
            tracing::warn!(
                socket = %socket_path.display(),
                error = %e,
                "diagnostics: leader could not bind the request socket; followers cannot route to it",
            );
            return;
        }
    };
    tracing::info!(
        socket = %socket_path.display(),
        server = %server_name,
        "diagnostics: leader serving the request socket for follower diagnose/query",
    );
    let session = session.clone();
    tokio::spawn(async move {
        let result = swissarmyhammer_diagnostics::serve_session_requests(
            server,
            session,
            swissarmyhammer_diagnostics::PrecomputedDependents::default(),
            swissarmyhammer_diagnostics::DiagnosticsConfig::default(),
        )
        .await;
        if let Err(e) = result {
            tracing::warn!(error = %e, "diagnostics: request socket serve loop ended with error");
        }
    });
}

/// Build the diagnostics watcher's per-server routing table from the running
/// clients, resolving each server's file extensions from the supervisor.
///
/// A server whose extensions cannot be resolved (supervisor lock contended, or
/// daemon gone) is dropped from the table — its files simply will not be
/// re-diagnosed by the watcher until the next startup, which is safe.
fn build_diagnostics_routes(
    clients: &[(String, swissarmyhammer_code_context::SharedLspSession)],
) -> Vec<swissarmyhammer_diagnostics::SessionRoute> {
    use crate::mcp::tools::code_context::LSP_SUPERVISOR;

    let extensions_for = |server_name: &str| -> Option<Vec<String>> {
        let sup = LSP_SUPERVISOR.get()?;
        let guard = sup.try_lock().ok()?;
        let daemon = guard.get_daemon(server_name)?;
        Some(daemon.file_extensions().to_vec())
    };

    clients
        .iter()
        .filter_map(|(server_name, session)| {
            let extensions = extensions_for(server_name)?;
            Some(swissarmyhammer_diagnostics::SessionRoute::new(
                extensions,
                session.clone(),
            ))
        })
        .collect()
}

/// Tee one session's in-process diagnostics fan-out onto the leader's existing
/// pub/sub proxy.
///
/// Builds a typed [`Publisher`](swissarmyhammer_leader_election::Publisher) with
/// the public `open` seam over the leader's own bus frontend — reusing the one
/// proxy, not starting a second — and runs
/// [`fan_out_to_bus`](swissarmyhammer_diagnostics::fan_out_to_bus). A follower
/// (no `bus_frontend`) or a publisher that fails to connect is skipped: the
/// in-process fan-out still works, only the cross-process mirror is absent.
fn spawn_diagnostics_fan_out(
    server_name: &str,
    session: &swissarmyhammer_code_context::SharedLspSession,
    bus_frontend: Option<&str>,
) {
    let Some(frontend) = bus_frontend else {
        return;
    };
    let frontend = frontend.to_string();
    let rx = session.subscribe();
    let server = server_name.to_string();
    tokio::spawn(async move {
        if let Err(e) = swissarmyhammer_diagnostics::fan_out_over_bus(&frontend, rx).await {
            tracing::warn!(
                server = %server,
                error = %e,
                "diagnostics: could not open bus publisher; cross-process fan-out absent"
            );
        }
    });
    tracing::info!("diagnostics: cross-process fan-out started (server: {server_name})");
}

/// Feed one session's in-process diagnostics fan-out into the subscribable
/// diagnostics MCP resource.
///
/// Subscribes to the same `session.subscribe()` broadcast the cross-process bus
/// tee consumes (a third consumer, not a second mechanism) and forwards each
/// per-uri [`DiagnosticUpdate`](swissarmyhammer_lsp::diagnostics::DiagnosticUpdate)
/// into the process-wide diagnostics resource via
/// [`publish_diagnostics_update`](crate::mcp::diagnostics_resource::publish_diagnostics_update),
/// which folds the view and pushes `notifications/resources/updated` to a
/// subscribing host (best-effort). A `Lagged` receiver skips ahead — diagnostics
/// are full per-uri replacements, so missing an intermediate update only delays
/// the host one tick.
fn spawn_diagnostics_resource_feed(
    server_name: &str,
    session: &swissarmyhammer_code_context::SharedLspSession,
) {
    use tokio::sync::broadcast::error::RecvError;

    let mut rx = session.subscribe();
    let server = server_name.to_string();
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(update) => {
                    crate::mcp::diagnostics_resource::publish_diagnostics_update(
                        &update.uri,
                        update.diagnostics,
                    );
                }
                Err(RecvError::Lagged(skipped)) => {
                    tracing::debug!(
                        server = %server,
                        skipped,
                        "diagnostics resource feed lagged; continuing"
                    );
                }
                Err(RecvError::Closed) => break,
            }
        }
    });
    tracing::info!("diagnostics: resource feed started (server: {server_name})");
}

/// Try to promote the workspace to leader. Returns `Ok(Some(db))` on success,
/// `Ok(None)` if the lock is still held elsewhere, `Err` on a real failure.
/// Returns `Ok(None)` (and signals "stop looping") via the caller if the
/// workspace is already the leader.
enum PromotionState {
    AlreadyLeader,
    Outcome(
        std::result::Result<
            Option<swissarmyhammer_code_context::SharedDb>,
            swissarmyhammer_code_context::CodeContextError,
        >,
    ),
}

fn try_promote_workspace(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
) -> PromotionState {
    let mut ws_lock = ws.lock().expect("workspace mutex poisoned");
    if ws_lock.is_leader() {
        return PromotionState::AlreadyLeader;
    }
    PromotionState::Outcome(ws_lock.try_promote())
}

/// Handle the outcome of `try_promote_workspace`. Returns `true` when the
/// re-election loop should stop (either because we're already leader or the
/// promotion succeeded).
fn handle_promotion_result(
    state: PromotionState,
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    workspace_root: &std::path::Path,
    follower_subscriber_cancel: Option<&Arc<std::sync::atomic::AtomicBool>>,
) -> bool {
    match state {
        PromotionState::AlreadyLeader => true,
        PromotionState::Outcome(Ok(Some(shared_db))) => {
            tracing::info!(
                "code-context: promoted to leader for {}, starting indexing workers",
                workspace_root.display()
            );
            // This process is now the leader and is about to start the
            // leader-side publish path. Stop the orphaned follower diagnostics
            // subscriber FIRST: the bus address is deterministic by workspace
            // hash, so a still-running subscriber would reconnect to this
            // process's own proxy and self-log the leader's diagnostics. The
            // subscriber loop observes this flag on its next ≤500ms wake.
            if let Some(cancel) = follower_subscriber_cancel {
                cancel.store(true, std::sync::atomic::Ordering::Relaxed);
                tracing::debug!("diagnostics: signaled follower subscriber to stop on promotion");
            }
            // After promotion this process owns the proxy; surface its bus
            // frontend so the cold re-spawn also wires the cross-process fan-out,
            // and its election request socket so the new leader serves followers.
            let (bus_frontend, socket_path, shutdown) = {
                let ws_lock = ws.lock().expect("workspace mutex poisoned");
                (
                    ws_lock.bus_addresses().map(|a| a.frontend),
                    ws_lock.socket_path().to_path_buf(),
                    ws_lock
                        .leader_shutdown_flag()
                        .expect("promoted leader has a shutdown flag"),
                )
            };
            McpServer::start_indexing_workers_after_promotion(
                workspace_root.to_path_buf(),
                shared_db,
                shutdown,
                bus_frontend,
                socket_path,
            );
            true
        }
        PromotionState::Outcome(Ok(None)) => false,
        PromotionState::Outcome(Err(e)) => {
            tracing::warn!("code-context: re-election error: {}", e);
            false
        }
    }
}

/// If the workspace is currently leader, spawn LSP indexing workers for the
/// supplied sessions. No-op if the workspace has no shared DB.
fn start_lsp_workers_if_leader(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    workspace_root: &std::path::Path,
    clients: &[(String, swissarmyhammer_code_context::SharedLspSession)],
    log_suffix: &str,
) {
    let ws_lock = ws.lock().expect("workspace mutex poisoned");
    let Some(shared_db) = ws_lock.shared_db() else {
        return;
    };
    // The leader's bus frontend, for re-publishing diagnostics across processes.
    let bus_frontend = ws_lock.bus_addresses().map(|a| a.frontend);
    // The election request socket the leader binds so followers can route to it.
    let socket_path = ws_lock.socket_path().to_path_buf();
    // The same shutdown flag the workspace owns for this tenure, so these LSP
    // workers stop on step-down alongside the TS/watcher/reconcile workers.
    let shutdown = ws_lock
        .leader_shutdown_flag()
        .expect("leader has a shutdown flag");
    drop(ws_lock);
    spawn_lsp_workers_for_clients(
        workspace_root,
        &shared_db,
        clients,
        log_suffix,
        bus_frontend.as_deref(),
        &socket_path,
        shutdown,
    );
}

/// Run the LSP supervisor health-check loop forever, polling on the
/// `McpServer::LSP_HEALTH_CHECK_INTERVAL` cadence.
async fn run_lsp_health_check_loop() -> ! {
    loop {
        tokio::time::sleep(McpServer::LSP_HEALTH_CHECK_INTERVAL).await;
        use crate::mcp::tools::code_context::LSP_SUPERVISOR;
        if let Some(sup) = LSP_SUPERVISOR.get() {
            sup.lock().await.health_check_all().await;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The leader-gated supervisor spawn is the single decision point that
    /// stops a follower from launching its own rust-analyzer. A follower
    /// (`is_leader == false`) must get no supervisor handle; a leader must get
    /// one. This guards the exact regression this task fixed: spawning the
    /// supervisor regardless of leadership.
    #[tokio::test]
    async fn test_lsp_supervisor_spawn_is_leader_gated() {
        let tmp = tempfile::tempdir().unwrap();

        // Follower: no handle, spawns nothing.
        let follower_handle = McpServer::spawn_lsp_supervisor_if_leader(false, tmp.path());
        assert!(
            follower_handle.is_none(),
            "a follower must not spawn the LSP supervisor"
        );

        // Leader: a supervisor task handle is returned.
        let leader_handle = McpServer::spawn_lsp_supervisor_if_leader(true, tmp.path());
        assert!(
            leader_handle.is_some(),
            "the leader must spawn the LSP supervisor"
        );
        // Drive the spawned task to completion so it does not outlive the test.
        let _ = leader_handle.unwrap().await;
    }

    /// Server start must leave `.reviewignore` on disk before any review runs,
    /// so it can be edited immediately (e.g. to exclude upstream files in a
    /// fork) rather than waiting for the first review's lazy creation.
    #[test]
    fn test_ensure_workspace_review_files_creates_default_when_absent() {
        let tmp = tempfile::tempdir().unwrap();

        McpServer::ensure_workspace_review_files(tmp.path());

        let path = tmp.path().join(".reviewignore");
        assert!(path.exists(), "server start must create .reviewignore");
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(
            content.contains(".kanban/"),
            "the default must ignore the kanban board directory, got:\n{content}"
        );
    }

    /// A pre-existing `.reviewignore` (user edits) is authoritative — server
    /// start must never clobber it.
    #[test]
    fn test_ensure_workspace_review_files_preserves_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let edited = "# my rules\ntarget/\n!target/keep.rs\n";
        std::fs::write(tmp.path().join(".reviewignore"), edited).unwrap();

        McpServer::ensure_workspace_review_files(tmp.path());

        let content = std::fs::read_to_string(tmp.path().join(".reviewignore")).unwrap();
        assert_eq!(
            content, edited,
            "an existing .reviewignore must be preserved byte-for-byte"
        );
    }

    /// A write failure (e.g. an unwritable workspace root) must never panic —
    /// server startup proceeds regardless, only logging a warning.
    #[test]
    fn test_ensure_workspace_review_files_does_not_panic_on_unwritable_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let unwritable = tmp.path().join("no-such-parent").join("nested");

        // Does not exist and its parent is never created, so the write inside
        // `ensure_reviewignore` fails with an I/O error; this call must swallow
        // it (as a warning) rather than propagate or panic.
        McpServer::ensure_workspace_review_files(&unwritable);

        assert!(
            !unwritable.join(".reviewignore").exists(),
            "no file should exist when the target directory itself is absent"
        );
    }
}
