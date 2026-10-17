//! Code-context startup: which process indexes the workspace, and what it runs.
//!
//! Exactly one process per workspace may drive the index and the stdio LSP
//! servers, so startup begins with a leadership election. The leader spawns the
//! LSP supervisor, the tree-sitter indexing workers, and the diagnostics
//! fan-out; a follower spawns none of them and instead polls for promotion
//! while subscribing to the leader's diagnostics broadcast.

use super::McpServer;
use std::sync::Arc;

use crate::mcp::tools::code_context::{
    install_lsp_supervisor, remove_lsp_supervisor, shutdown_lsp_supervisor, track_lsp_task,
    SharedLspSupervisor,
};
use swissarmyhammer_code_context::{SharedDb, SharedLspSession, ShutdownFlag};
use swissarmyhammer_common::utils::find_git_repository_root_from;

/// The `(server_name, session)` pairs of the running LSP servers.
type LspClients = Vec<(String, SharedLspSession)>;

/// The result of one LSP supervisor start.
pub(super) struct LspStartup {
    /// The supervisor that the start put in the process slot.
    supervisor: SharedLspSupervisor,
    /// The sessions of the servers that are running after the start.
    clients: LspClients,
}

/// What the LSP workers of one leader tenure need.
struct LspWorkerSetup<'a> {
    /// The workspace root.
    workspace_root: &'a std::path::Path,
    /// The read-write index database of the leader.
    shared_db: &'a SharedDb,
    /// The supervisor that owns the sessions. Each spawned task is tied to
    /// it, so the task stops when the supervisor leaves the slot.
    supervisor: &'a SharedLspSupervisor,
    /// Text added to the start logs (for example `" (after promotion)"`).
    log_suffix: &'a str,
    /// The bus frontend of the leader, for the cross-process fan-out.
    bus_frontend: Option<&'a str>,
    /// The election request socket that the leader serves.
    socket_path: &'a std::path::Path,
    /// The shutdown flag of this leader tenure.
    shutdown: ShutdownFlag,
}

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

        // Only a leader has the shutdown flag of a leader tenure.
        let leader_shutdown = ws
            .lock()
            .expect("workspace mutex poisoned")
            .leader_shutdown_flag();

        // Single gate: only the leader gets a supervisor handle; a follower gets
        // `None` and spawns no LSP server at all.
        match Self::spawn_lsp_supervisor_if_leader(leader_shutdown.clone(), &workspace_root) {
            Some(lsp_handle) => {
                // Each worker of this tenure gets the flag that startup read,
                // so it stops with this tenure.
                let shutdown = leader_shutdown.expect("leader has a shutdown flag");
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
                Self::start_workers_if_leader(&ws, &workspace_root, Arc::clone(&shutdown));
                // The leader runs no follower diagnostics subscriber, so there is
                // nothing to cancel on (a no-op) re-election.
                Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone(), None);
                // The leader heartbeats its lease; if preempted (a live session
                // took over a stale lease) it steps down to follower and starts
                // polling for re-promotion — preserving the single-writer
                // invariant (no two indexers).
                Self::spawn_leader_heartbeat_loop(Arc::clone(&ws), workspace_root.clone());
                Self::spawn_lsp_health_loop(lsp_handle, ws, workspace_root, shutdown);
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
    /// `leader_shutdown` is the shutdown flag of the leader tenure, and only a
    /// leader has one. Returns the supervisor task handle when it is `Some`
    /// (the leader spawns the one stdio LSP child for the workspace), and
    /// `None` when it is `None` (a follower spawns nothing). Keeping this the
    /// only place the startup path spawns the supervisor ensures a follower can
    /// never launch its own rust-analyzer over the shared tree.
    fn spawn_lsp_supervisor_if_leader(
        leader_shutdown: Option<ShutdownFlag>,
        workspace_root: &std::path::Path,
    ) -> Option<tokio::task::JoinHandle<LspStartup>> {
        leader_shutdown
            .map(|shutdown| Self::spawn_lsp_supervisor(workspace_root.to_path_buf(), shutdown))
    }

    /// If the workspace is already leader, start TS indexing + watcher workers.
    ///
    /// `shutdown` is the shutdown flag of the leader tenure that startup read.
    /// The workers get this flag, so they stop with that tenure.
    fn start_workers_if_leader(
        ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: &std::path::Path,
        shutdown: ShutdownFlag,
    ) {
        let shared_db = ws.lock().expect("workspace mutex poisoned").shared_db();
        if let Some(shared_db) = shared_db {
            Self::start_indexing_workers(workspace_root.to_path_buf(), shared_db, shutdown);
        }
    }

    /// Spawn the LSP supervisor task.
    ///
    /// The task puts a new supervisor in the process slot (this shuts down a
    /// supervisor that was there before), starts every configured LSP daemon,
    /// and gives the supervisor and its running `(server_name, session)`
    /// pairs through the join handle.
    ///
    /// The task locks the supervisor before it puts it in the slot, and holds
    /// the lock until the start ends. Thus a step-down that takes the
    /// supervisor from the slot during the start waits for the start, and
    /// then shuts down each server that the start spawned.
    ///
    /// `shutdown` is the shutdown flag of the leader tenure. A step-down that
    /// ends before the install finds an empty slot, so the task reads the flag
    /// after the install (see [`start_unless_stepped_down`]).
    fn spawn_lsp_supervisor(
        workspace_root: std::path::PathBuf,
        shutdown: ShutdownFlag,
    ) -> tokio::task::JoinHandle<LspStartup> {
        tokio::spawn(async move {
            let supervisor: SharedLspSupervisor = Arc::new(tokio::sync::Mutex::new(
                build_lsp_supervisor(&workspace_root).await,
            ));
            let mut start_guard = Arc::clone(&supervisor).lock_owned().await;
            install_lsp_supervisor(Arc::clone(&supervisor)).await;
            let clients = start_unless_stepped_down(&supervisor, &mut start_guard, &shutdown).await;
            drop(start_guard);
            tracing::info!(
                "code-context: {} LSP clients available for indexing: {:?}",
                clients.len(),
                clients.iter().map(|(n, _)| n.as_str()).collect::<Vec<_>>()
            );
            LspStartup {
                supervisor,
                clients,
            }
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
                step_down_and_stop_lsp(&ws).await;
                // Now a follower again — poll for re-promotion and stop this loop.
                Self::spawn_reelection_loop(Arc::clone(&ws), workspace_root.clone(), None);
                break;
            }
        });
    }

    /// Waits for the LSP supervisor to finish, starts LSP indexing workers if
    /// we're the leader, then runs the 60s LSP health-check loop until the
    /// leader tenure ends.
    ///
    /// `shutdown` is the shutdown flag of the tenure that started the
    /// supervisor. The LSP workers get this same flag, not the flag that the
    /// workspace holds when they start: a step-down and a new promotion can
    /// come first, and the workers of the old tenure must stop with it.
    fn spawn_lsp_health_loop(
        lsp_handle: tokio::task::JoinHandle<LspStartup>,
        ws: Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
        workspace_root: std::path::PathBuf,
        shutdown: ShutdownFlag,
    ) {
        let worker_shutdown = Arc::clone(&shutdown);
        Self::spawn_drain_supervisor_and_health_loop(
            lsp_handle,
            shutdown,
            move |supervisor, clients| {
                start_lsp_workers_if_leader(
                    &ws,
                    &workspace_root,
                    supervisor,
                    clients,
                    "",
                    worker_shutdown,
                );
            },
        );
    }

    /// Await the LSP supervisor's startup task, hand its supervisor and
    /// running clients to `start_workers` (skipped when no client runs), then
    /// run the LSP health-check loop until the leader tenure ends.
    ///
    /// `shutdown` is the shutdown flag of the leader tenure. When it is set
    /// before the start ends (a step-down came first), no worker starts and
    /// no health loop runs.
    ///
    /// Shared by initial-leader startup and the post-promotion cold re-spawn;
    /// the two differ only in how they start workers (the `start_workers`
    /// closure), so the await/drain/health-loop shell lives here once.
    fn spawn_drain_supervisor_and_health_loop<F>(
        lsp_handle: tokio::task::JoinHandle<LspStartup>,
        shutdown: ShutdownFlag,
        start_workers: F,
    ) where
        F: FnOnce(&SharedLspSupervisor, &[(String, SharedLspSession)]) + Send + 'static,
    {
        tokio::spawn(async move {
            let startup = match lsp_handle.await {
                Ok(startup) => startup,
                Err(e) => {
                    tracing::error!("code-context: LSP supervisor task failed: {e}");
                    return;
                }
            };
            if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
                tracing::info!(
                    "code-context: the leader stepped down before the LSP start ended; starting no LSP worker"
                );
                return;
            }
            if startup.clients.is_empty() {
                tracing::info!("code-context: no LSP clients available, skipping LSP indexing");
            } else {
                start_workers(&startup.supervisor, &startup.clients);
            }
            run_lsp_health_check_loop(&startup.supervisor, &shutdown).await;
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
            let filter = workspace_ignore_filter(&watcher_root).await;
            let _watcher_handle =
                start_code_context_watcher(watcher_root, watcher_db, filter, watcher_shutdown);
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
        let lsp_handle =
            Self::spawn_lsp_supervisor(workspace_root.clone(), std::sync::Arc::clone(&shutdown));
        let worker_shutdown = std::sync::Arc::clone(&shutdown);
        Self::spawn_drain_supervisor_and_health_loop(
            lsp_handle,
            shutdown,
            move |supervisor, clients| {
                let setup = LspWorkerSetup {
                    workspace_root: &workspace_root,
                    shared_db: &lsp_db,
                    supervisor,
                    log_suffix: " (after promotion)",
                    bus_frontend: bus_frontend.as_deref(),
                    socket_path: &socket_path,
                    shutdown: worker_shutdown,
                };
                spawn_lsp_workers_for_clients(&setup, clients);
            },
        );
    }
}

/// The one ignore rule set of the workspace of this process.
///
/// The LSP session path filter, the code-context watcher and the diagnostics
/// watcher each hold a clone. When a watcher sees a change to an ignore file,
/// it loads the rules again in this one place, and every consumer then obeys
/// the new rules. One process serves one workspace (code-context
/// initialization runs one time), so the first root loads the rules.
static WORKSPACE_IGNORE_FILTER: tokio::sync::OnceCell<
    swissarmyhammer_code_context::SharedIgnoreFilter,
> = tokio::sync::OnceCell::const_new();

/// Give the shared ignore rule set of `workspace_root`, and load it on the
/// first call.
///
/// This always gives a rule set, so each consumer always starts: the LSP path
/// filter, the code-context watcher and the diagnostics watcher. When the
/// rules cannot be loaded, the rule set has the hard-skip rules only (see
/// [`ignore_filter_or_hard_skips`]). The next change to an ignore file loads
/// the full rules again.
async fn workspace_ignore_filter(
    workspace_root: &std::path::Path,
) -> swissarmyhammer_code_context::SharedIgnoreFilter {
    WORKSPACE_IGNORE_FILTER
        .get_or_init(|| async {
            let loaded = swissarmyhammer_code_context::IgnoreFilter::load(workspace_root).await;
            swissarmyhammer_code_context::SharedIgnoreFilter::new(ignore_filter_or_hard_skips(
                workspace_root,
                loaded,
            ))
        })
        .await
        .clone()
}

/// Pick the ignore rules that the watchers start with.
///
/// `loaded` is the result of [`IgnoreFilter::load`]. When the load succeeded,
/// its rules are given. When it failed, a warning is logged and the rules are
/// [`IgnoreFilter::hard_skips_only`]: hidden paths and the hard-skip
/// directories, with no ignore file. An empty rule set is not correct here.
/// With no rules, the watchers would send events for `.build/`, `target/` and
/// `node_modules/` to the index and to the language server again.
///
/// [`IgnoreFilter::load`]: swissarmyhammer_code_context::IgnoreFilter::load
/// [`IgnoreFilter::hard_skips_only`]: swissarmyhammer_code_context::IgnoreFilter::hard_skips_only
fn ignore_filter_or_hard_skips(
    workspace_root: &std::path::Path,
    loaded: Result<swissarmyhammer_code_context::IgnoreFilter, tokio::task::JoinError>,
) -> swissarmyhammer_code_context::IgnoreFilter {
    loaded.unwrap_or_else(|e| {
        tracing::warn!(
            "code-context: cannot load the ignore rules, the watchers start with the hard-skip rules only: {}",
            e
        );
        swissarmyhammer_code_context::IgnoreFilter::hard_skips_only(workspace_root)
    })
}

/// Make a new LSP supervisor for `workspace_root` with the filters of this
/// process.
///
/// The path filter is the one shared ignore rule set of the workspace: the
/// LSP sessions skip each ignored path and each path outside the workspace
/// root. Thus no didOpen and no request goes to a language server for build
/// output such as `.build/checkouts/`, and a watcher that loads the rules
/// again after an ignore file change also changes what the sessions skip.
/// Each supervisor that a promotion makes gets the same filter.
///
/// The stderr-noise filter comes from the stacked code-context config. The
/// filter sources live in code-context; `swissarmyhammer-lsp` only gives the
/// injection seams, so it carries no config dependency.
async fn build_lsp_supervisor(
    workspace_root: &std::path::Path,
) -> swissarmyhammer_lsp::LspSupervisorManager {
    let supervisor = swissarmyhammer_lsp::LspSupervisorManager::new(workspace_root.to_path_buf())
        .with_path_filter(workspace_ignore_filter(workspace_root).await.path_filter());
    match swissarmyhammer_code_context::CompiledCodeContextConfig::compile(
        &swissarmyhammer_code_context::load_code_context_config(),
    ) {
        Ok(compiled) => {
            let compiled = Arc::new(compiled);
            supervisor.with_stderr_filter(Arc::new(move |line: &str| {
                swissarmyhammer_code_context::should_filter_stderr(line, &compiled)
            }))
        }
        Err(_) => supervisor,
    }
}

/// Start the LSP servers of `supervisor`, unless the leader tenure ended.
///
/// The caller has put `supervisor` in the process slot, and holds its lock as
/// `start_guard`. A step-down that ended before the install found an empty
/// slot, so it could not stop `supervisor`. Thus this reads `shutdown` after
/// the install. When the flag is set, `supervisor` leaves the slot, it is shut
/// down, and no LSP server starts. A step-down after this read waits for
/// `start_guard`, and then stops each server that the start spawned.
///
/// Returns the `(server_name, session)` pairs of the running servers.
async fn start_unless_stepped_down(
    supervisor: &SharedLspSupervisor,
    start_guard: &mut swissarmyhammer_lsp::LspSupervisorManager,
    shutdown: &ShutdownFlag,
) -> LspClients {
    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        tracing::info!(
            "code-context: the leader stepped down before the LSP start; starting no LSP server"
        );
        remove_lsp_supervisor(supervisor);
        start_guard.shutdown().await;
        return LspClients::new();
    }
    log_lsp_start_results(&start_guard.start().await);
    collect_running_lsp_clients(start_guard)
}

/// Log the outcome of one supervisor start: the counts, and each error.
fn log_lsp_start_results(results: &[Result<(), swissarmyhammer_lsp::LspError>]) {
    let ok_count = results.iter().filter(|r| r.is_ok()).count();
    let err_count = results.len() - ok_count;
    tracing::info!(
        "code-context: LSP supervisor started — {} servers ok, {} failed",
        ok_count,
        err_count
    );
    for e in results.iter().filter_map(|r| r.as_ref().err()) {
        tracing::warn!("code-context: LSP start error: {}", e);
    }
}

/// Step the leader down to follower and stop its LSP servers.
///
/// `step_down` sets the shutdown flag of the tenure, so the indexing workers
/// and the health loop stop. Then the supervisor leaves the process slot: the
/// tasks that use its sessions (the diagnostics watcher, the request server,
/// the diagnostics feeds) are aborted, and each LSP server is shut down. The
/// shutdown waits for a start in progress, so no LSP child of this tenure
/// stays alive.
async fn step_down_and_stop_lsp(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
) {
    ws.lock().expect("workspace mutex poisoned").step_down();
    shutdown_lsp_supervisor().await;
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
///
/// Each task this spawns (except the indexing worker, which obeys the
/// shutdown flag) is tied to `setup.supervisor`, so it stops when that
/// supervisor leaves the process slot at a step-down or at exit.
fn spawn_lsp_workers_for_clients(
    setup: &LspWorkerSetup<'_>,
    clients: &[(String, SharedLspSession)],
) {
    if clients.is_empty() {
        return;
    }

    // The leader binds the election request socket and serves the SAH request
    // API over its single session, so followers (which spawn no LSP server) can
    // route diagnose/query calls to this one server. Done here — the single
    // chokepoint both initial-leader and post-promotion startup pass through —
    // right where the running sessions first become available.
    if let Some(task) = spawn_request_server_for_leader(setup.socket_path, clients) {
        track_lsp_task(setup.supervisor, task.abort_handle());
    }
    for (server_name, session) in clients {
        spawn_session_workers(setup, server_name, session);
    }
    spawn_diagnostics_watcher(setup, clients);
}

/// Spawn the workers of one LSP session: the indexing worker, the
/// cross-process diagnostics fan-out, and the diagnostics resource feed.
fn spawn_session_workers(
    setup: &LspWorkerSetup<'_>,
    server_name: &str,
    session: &SharedLspSession,
) {
    use swissarmyhammer_code_context::{spawn_lsp_indexing_worker, LspWorkerConfig};
    spawn_lsp_indexing_worker(
        setup.workspace_root.to_path_buf(),
        Arc::clone(setup.shared_db),
        session.clone(),
        LspWorkerConfig::default(),
        server_name.to_string(),
        setup.shutdown.clone(),
    );
    tracing::info!(
        "code-context: LSP indexing worker started for {}{} (server: {})",
        setup.workspace_root.display(),
        setup.log_suffix,
        server_name,
    );

    // Each session has its own in-process diagnostics fan-out, so the
    // cross-process re-publisher is per session.
    if let Some(task) = spawn_diagnostics_fan_out(server_name, session, setup.bus_frontend) {
        track_lsp_task(setup.supervisor, task.abort_handle());
    }

    // A third consumer of the same in-process fan-out: feed each per-uri
    // update into the subscribable diagnostics MCP resource so a host that
    // subscribed gets `notifications/resources/updated` without a tool call.
    // Runs regardless of `bus_frontend` — the resource lives in-process.
    let feed = spawn_diagnostics_resource_feed(server_name, session);
    track_lsp_task(setup.supervisor, feed.abort_handle());
}

/// Spawn the one diagnostics file watcher of the workspace.
///
/// Exactly ONE diagnostics file watcher per workdir (not per session): it
/// watches the tree once and routes each changed file to the session whose
/// server handles that extension, so a multi-language workspace does not run
/// N watchers and a `.py` edit is never fed into the rust session. The watch
/// root is canonicalized so the watcher's `didChange` uris match the
/// canonical paths the sessions open documents under (on macOS `/var`
/// resolves to `/private/var`); a mismatch would split a file into two
/// documents from the server's view.
///
/// The watcher task and the task that starts it are both tied to the
/// supervisor, so a step-down stops the watcher.
fn spawn_diagnostics_watcher(setup: &LspWorkerSetup<'_>, clients: &[(String, SharedLspSession)]) {
    let routes = build_diagnostics_routes(setup.supervisor, clients);
    if routes.is_empty() {
        return;
    }
    let workspace_root = setup.workspace_root;
    let watch_root =
        std::fs::canonicalize(workspace_root).unwrap_or_else(|_| workspace_root.to_path_buf());
    // Best-effort watcher-push: tell a connected host (via a plain MCP
    // `notifications/message`) when a native edit is seen. The notifier is a
    // courtesy channel — a `None`/absent peer never gates the re-diagnose.
    let notifier: swissarmyhammer_diagnostics::WatcherNotifier =
        Arc::new(|path: &std::path::Path| {
            crate::mcp::diagnostics_resource::watcher_push_log(path);
        });
    // The watcher shares the one ignore rule set of the workspace. The
    // rules have the workspace root; the watcher gives each path in the
    // canonical form of `watch_root`.
    let filter_root = workspace_root.to_path_buf();
    let owner = Arc::clone(setup.supervisor);
    let starter = tokio::spawn(async move {
        let filter = workspace_ignore_filter(&filter_root).await;
        let watcher = swissarmyhammer_diagnostics::start_diagnostics_watcher_with_notifier(
            watch_root,
            routes,
            filter,
            Some(notifier),
        );
        track_lsp_task(&owner, watcher.abort_handle());
    });
    track_lsp_task(setup.supervisor, starter.abort_handle());
    tracing::info!(
        "diagnostics: file watcher started for {}{}",
        workspace_root.display(),
        setup.log_suffix,
    );
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
///
/// Returns the serve task, or `None` when no task started.
fn spawn_request_server_for_leader(
    socket_path: &std::path::Path,
    clients: &[(String, SharedLspSession)],
) -> Option<tokio::task::JoinHandle<()>> {
    let (server_name, session) = clients.first()?;
    let server = match swissarmyhammer_diagnostics::RequestServer::bind(socket_path) {
        Ok(server) => server,
        Err(e) => {
            tracing::warn!(
                socket = %socket_path.display(),
                error = %e,
                "diagnostics: leader could not bind the request socket; followers cannot route to it",
            );
            return None;
        }
    };
    tracing::info!(
        socket = %socket_path.display(),
        server = %server_name,
        "diagnostics: leader serving the request socket for follower diagnose/query",
    );
    let session = session.clone();
    Some(tokio::spawn(async move {
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
    }))
}

/// Build the diagnostics watcher's per-server routing table from the running
/// clients, resolving each server's file extensions from `supervisor`.
///
/// A server whose extensions cannot be resolved (supervisor lock contended, or
/// daemon gone) is dropped from the table — its files simply will not be
/// re-diagnosed by the watcher until the next startup, which is safe.
fn build_diagnostics_routes(
    supervisor: &SharedLspSupervisor,
    clients: &[(String, SharedLspSession)],
) -> Vec<swissarmyhammer_diagnostics::SessionRoute> {
    let extensions_for = |server_name: &str| -> Option<Vec<String>> {
        let guard = supervisor.try_lock().ok()?;
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
///
/// Returns the fan-out task, or `None` when there is no bus frontend.
fn spawn_diagnostics_fan_out(
    server_name: &str,
    session: &SharedLspSession,
    bus_frontend: Option<&str>,
) -> Option<tokio::task::JoinHandle<()>> {
    let frontend = bus_frontend?.to_string();
    let rx = session.subscribe();
    let server = server_name.to_string();
    let task = tokio::spawn(async move {
        if let Err(e) = swissarmyhammer_diagnostics::fan_out_over_bus(&frontend, rx).await {
            tracing::warn!(
                server = %server,
                error = %e,
                "diagnostics: could not open bus publisher; cross-process fan-out absent"
            );
        }
    });
    tracing::info!("diagnostics: cross-process fan-out started (server: {server_name})");
    Some(task)
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
///
/// Returns the feed task.
fn spawn_diagnostics_resource_feed(
    server_name: &str,
    session: &SharedLspSession,
) -> tokio::task::JoinHandle<()> {
    use tokio::sync::broadcast::error::RecvError;

    let mut rx = session.subscribe();
    let server = server_name.to_string();
    let task = tokio::spawn(async move {
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
    task
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

/// Spawn the LSP workers of one leader tenure for the supplied sessions.
///
/// `shutdown` is the shutdown flag of the tenure that started `supervisor`.
/// The workers get this flag, so they stop with that tenure, together with
/// the TS, watcher and reconcile workers of the same tenure. When the flag is
/// set (a step-down came first), no worker starts. This reads the flag under
/// the workspace lock. A step-down needs that lock, so the tenure cannot end
/// while this reads the shared DB of the workspace. No-op if the workspace
/// has no shared DB.
fn start_lsp_workers_if_leader(
    ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    workspace_root: &std::path::Path,
    supervisor: &SharedLspSupervisor,
    clients: &[(String, SharedLspSession)],
    log_suffix: &str,
    shutdown: ShutdownFlag,
) {
    let ws_lock = ws.lock().expect("workspace mutex poisoned");
    if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
        tracing::info!(
            "code-context: the leader tenure ended before the LSP workers started; starting no LSP worker"
        );
        return;
    }
    let Some(shared_db) = ws_lock.shared_db() else {
        return;
    };
    // The leader's bus frontend, for re-publishing diagnostics across processes.
    let bus_frontend = ws_lock.bus_addresses().map(|a| a.frontend);
    // The election request socket the leader binds so followers can route to it.
    let socket_path = ws_lock.socket_path().to_path_buf();
    drop(ws_lock);
    let setup = LspWorkerSetup {
        workspace_root,
        shared_db: &shared_db,
        supervisor,
        log_suffix,
        bus_frontend: bus_frontend.as_deref(),
        socket_path: &socket_path,
        shutdown,
    };
    spawn_lsp_workers_for_clients(&setup, clients);
}

/// Run the health-check loop of `supervisor` on the
/// `McpServer::LSP_HEALTH_CHECK_INTERVAL` cadence, until `shutdown` is set.
///
/// `shutdown` is the flag of one leader tenure. A step-down sets it, so the
/// loop of that tenure ends, and a later promotion runs a new loop over its
/// new supervisor. Thus a process runs at most one health loop.
async fn run_lsp_health_check_loop(supervisor: &SharedLspSupervisor, shutdown: &ShutdownFlag) {
    loop {
        tokio::time::sleep(McpServer::LSP_HEALTH_CHECK_INTERVAL).await;
        if shutdown.load(std::sync::atomic::Ordering::Relaxed) {
            tracing::info!("code-context: the leader stepped down; the LSP health loop stops");
            return;
        }
        supervisor.lock().await.health_check_all().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mcp::tools::code_context::supervisor_slot::slot_holds;

    /// Make the [`tokio::task::JoinError`] of a task that the runtime
    /// cancelled, the same error a failed [`IgnoreFilter::load`] gives.
    ///
    /// [`IgnoreFilter::load`]: swissarmyhammer_code_context::IgnoreFilter::load
    async fn cancelled_join_error() -> tokio::task::JoinError {
        let task = tokio::spawn(std::future::pending::<()>());
        task.abort();
        task.await.unwrap_err()
    }

    /// When the ignore rules cannot load, the watchers start with the
    /// hard-skip rules. Build output stays out of the index and out of the
    /// language server, and a source file is kept.
    #[tokio::test]
    async fn ignore_filter_or_hard_skips_gives_the_hard_skips_when_the_load_fails() {
        let dir = tempfile::tempdir().unwrap();

        let filter = ignore_filter_or_hard_skips(dir.path(), Err(cancelled_join_error().await));

        assert!(filter.is_ignored(&dir.path().join(".build/x.swift")));
        assert!(!filter.is_ignored(&dir.path().join("src/a.rs")));
    }

    /// When the ignore rules load, the watchers use them.
    #[tokio::test]
    async fn ignore_filter_or_hard_skips_keeps_the_loaded_rules() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "gen/\n").unwrap();
        let loaded = swissarmyhammer_code_context::IgnoreFilter::load(dir.path()).await;

        let filter = ignore_filter_or_hard_skips(dir.path(), loaded);

        assert!(filter.is_ignored(&dir.path().join("gen/a.rs")));
    }

    /// The leader-gated supervisor spawn is the single decision point that
    /// stops a follower from launching its own rust-analyzer. A follower
    /// (`is_leader == false`) must get no supervisor handle; a leader must get
    /// one. This guards the exact regression this task fixed: spawning the
    /// supervisor regardless of leadership.
    #[tokio::test]
    async fn test_lsp_supervisor_spawn_is_leader_gated() {
        let tmp = tempfile::tempdir().unwrap();

        // Follower: no handle, spawns nothing.
        let follower_handle = McpServer::spawn_lsp_supervisor_if_leader(None, tmp.path());
        assert!(
            follower_handle.is_none(),
            "a follower must not spawn the LSP supervisor"
        );

        // Leader: a supervisor task handle is returned.
        let leader_shutdown = ShutdownFlag::default();
        let leader_handle =
            McpServer::spawn_lsp_supervisor_if_leader(Some(leader_shutdown), tmp.path());
        assert!(
            leader_handle.is_some(),
            "the leader must spawn the LSP supervisor"
        );
        // Drive the spawned task to completion so it does not outlive the test.
        let _ = leader_handle.unwrap().await;
    }

    /// A step-down stops the supervisor of the old tenure and empties the
    /// slot. A later promotion puts a new, live supervisor in the slot; the
    /// slot does not drop it (the old `OnceLock` did, and `kill_on_drop` then
    /// killed the new LSP).
    #[tokio::test]
    async fn step_down_then_promotion_leaves_a_live_supervisor_in_the_slot() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().to_path_buf();
        let ws = Arc::new(std::sync::Mutex::new(
            swissarmyhammer_code_context::CodeContextWorkspace::open(&root).unwrap(),
        ));
        assert!(ws.lock().unwrap().is_leader());
        let first = McpServer::spawn_lsp_supervisor(root.clone(), tenure_shutdown_flag(&ws))
            .await
            .unwrap();
        assert!(slot_holds(&first.supervisor));

        step_down_and_stop_lsp(&ws).await;

        assert!(crate::mcp::tools::code_context::lsp_supervisor().is_none());
        assert!(first.supervisor.lock().await.is_shut_down());
        let promoted = ws.lock().unwrap().try_promote().unwrap();
        assert!(promoted.is_some(), "the stepped-down process must promote");
        let second = McpServer::spawn_lsp_supervisor(root, tenure_shutdown_flag(&ws))
            .await
            .unwrap();
        assert!(!Arc::ptr_eq(&first.supervisor, &second.supervisor));
        assert!(slot_holds(&second.supervisor));
        assert!(!second.supervisor.lock().await.is_shut_down());
        shutdown_lsp_supervisor().await;
    }

    /// The shutdown flag of the current leader tenure of `ws`.
    fn tenure_shutdown_flag(
        ws: &Arc<std::sync::Mutex<swissarmyhammer_code_context::CodeContextWorkspace>>,
    ) -> ShutdownFlag {
        ws.lock()
            .unwrap()
            .leader_shutdown_flag()
            .expect("the workspace is the leader")
    }

    /// A step-down that ends before the start task installs its supervisor
    /// finds an empty slot, so it cannot stop that supervisor. The start task
    /// must see the shutdown flag of the tenure after the install, shut the
    /// supervisor down, and start no LSP server. The slot is empty after the
    /// start, as after each step-down.
    #[tokio::test]
    async fn a_step_down_before_the_install_starts_no_lsp_server() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Arc::new(std::sync::Mutex::new(
            swissarmyhammer_code_context::CodeContextWorkspace::open(dir.path()).unwrap(),
        ));
        let shutdown = tenure_shutdown_flag(&ws);
        step_down_and_stop_lsp(&ws).await;

        let startup = McpServer::spawn_lsp_supervisor(dir.path().to_path_buf(), shutdown)
            .await
            .unwrap();

        assert!(
            startup.supervisor.lock().await.is_shut_down(),
            "the supervisor of a stepped-down tenure must be shut down"
        );
        assert!(
            crate::mcp::tools::code_context::lsp_supervisor().is_none(),
            "the slot of a stepped-down process must be empty"
        );
        assert!(startup.clients.is_empty(), "no LSP server may start");
    }

    /// A session that has no LSP process behind it.
    fn session_without_a_server() -> SharedLspSession {
        SharedLspSession::new(Arc::new(std::sync::Mutex::new(None)), "rust")
    }

    /// A step-down and a new promotion can come between the start of a
    /// supervisor and the start of its LSP workers. The workers get the
    /// shutdown flag of the old tenure, which the step-down set, so no worker
    /// starts. The request server is one of the workers, and it binds the
    /// election socket when it starts, so the socket must not be on disk.
    #[tokio::test]
    async fn the_lsp_workers_of_an_old_tenure_do_not_start() {
        let dir = tempfile::tempdir().unwrap();
        let ws = Arc::new(std::sync::Mutex::new(
            swissarmyhammer_code_context::CodeContextWorkspace::open(dir.path()).unwrap(),
        ));
        let old_tenure = tenure_shutdown_flag(&ws);
        step_down_and_stop_lsp(&ws).await;
        let promoted = ws.lock().unwrap().try_promote().unwrap();
        assert!(promoted.is_some(), "the stepped-down process must promote");
        let socket_path = ws.lock().unwrap().socket_path().to_path_buf();
        assert!(!socket_path.exists(), "no worker bound the socket yet");
        let supervisor: SharedLspSupervisor = Arc::new(tokio::sync::Mutex::new(
            swissarmyhammer_lsp::LspSupervisorManager::new(dir.path().to_path_buf()),
        ));
        let clients = vec![("rust-analyzer".to_string(), session_without_a_server())];

        start_lsp_workers_if_leader(&ws, dir.path(), &supervisor, &clients, "", old_tenure);

        assert!(
            !socket_path.exists(),
            "the workers of an old tenure must not start"
        );
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
