//! The three pipeline ops of the `review` tool: `review file/working/sha`.
//!
//! These ops are a thin dispatch shim. Each maps its op + args onto a
//! [`Scope`](swissarmyhammer_validators::review::Scope), resolves the engine's
//! inputs from the MCP session/work-dir — the repo root (CWD), the full validator
//! loader, the code_context index connection, the embedder, and a live ACP agent
//! — and calls the engine's
//! [`run_review_over_agent`](swissarmyhammer_validators::review::run_review_over_agent)
//! driver, returning the [`ReviewReport`]. No pipeline logic lives here.

use std::future::Future;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::sync::Arc;

use agent_client_protocol::schema::SessionNotification;
use agent_client_protocol::{Client, DynConnectTo};
use rmcp::model::{ProgressNotificationParam, ProgressToken};
use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::{broadcast, OnceCell, Semaphore};

use swissarmyhammer_validators::review::{
    run_review_over_agent, FleetConfig, ReviewProgressEvent, ReviewProgressSender, ReviewReport,
    Scope,
};
use swissarmyhammer_validators::{load_rules, PoolConfig};

use crate::mcp::progress::{spawn_drain_task, spawn_in_process_drain_task};
use crate::mcp::tool_registry::ToolContext;

/// The two halves of a ready-to-drive ACP agent handle: its
/// [`DynConnectTo<Client>`] component and the broadcast receiver of its streamed
/// `session/update` notifications. This is exactly the shape of
/// `swissarmyhammer_agent::AcpAgentHandle`, supplied to the tool so this crate
/// (which `swissarmyhammer-agent` depends on) never constructs an agent itself.
pub struct AgentHandle {
    /// The agent component the driver runs as the ACP server side.
    pub agent: DynConnectTo<Client>,
    /// The receiver of the agent's streamed notifications.
    pub notification_rx: broadcast::Receiver<SessionNotification>,
}

/// A factory that mints a fresh [`AgentHandle`] for one review run.
///
/// The review tool resolves its agent through this seam rather than constructing
/// one inline: the production server injects a factory that builds the configured
/// backend (Claude / Llama) from the session's `ModelConfig`, while tests inject a
/// scripted ACP agent. The factory is async and fallible — a backend that fails
/// to start surfaces as a tool error.
pub type AgentFactory = Arc<
    dyn Fn() -> Pin<Box<dyn Future<Output = Result<AgentHandle, String>> + Send>> + Send + Sync,
>;

/// A factory that resolves the [`TextEmbedder`](model_embedding::TextEmbedder)
/// the probe runner uses to embed query bodies and changed blocks.
///
/// Injected for the same reason as [`AgentFactory`]: the production server
/// resolves the loaded platform embedder, while tests inject a deterministic mock
/// so the pipeline runs without a 600 MB model load.
pub type EmbedderFactory = Arc<
    dyn Fn() -> Pin<
            Box<dyn Future<Output = Result<Arc<dyn model_embedding::TextEmbedder>, String>> + Send>,
        > + Send
        + Sync,
>;

/// Process-global cache of the loaded default embedder.
///
/// The default embedder is the platform `qwen-embedding` model — a
/// multi-hundred-MB-to-GB load. Building a fresh one per review run wastes that
/// load and, before the [`REVIEW_PIPELINE_GATE`] cap, multiplied the model's
/// resident footprint across concurrent runs. Caching it here loads it once and
/// shares one `Arc` across every default-factory run. Sharing is safe because
/// review pipelines are serialized by the gate and a run embeds sequentially, so
/// the shared model is never driven concurrently.
static DEFAULT_EMBEDDER: OnceCell<Arc<dyn model_embedding::TextEmbedder>> = OnceCell::const_new();

/// Return the cached embedder from `cell`, initializing it once via `init`.
///
/// A thin wrapper over [`OnceCell::get_or_try_init`] that hands back an owned
/// `Arc` clone (the cache keeps its own). A failed `init` is *not* stored, so a
/// later call retries the load rather than caching the failure. Factored out so
/// the share-once contract is unit-testable without loading the real model.
async fn shared_embedder<F, Fut>(
    cell: &OnceCell<Arc<dyn model_embedding::TextEmbedder>>,
    init: F,
) -> Result<Arc<dyn model_embedding::TextEmbedder>, String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Arc<dyn model_embedding::TextEmbedder>, String>>,
{
    let embedder = cell.get_or_try_init(init).await?;
    Ok(Arc::clone(embedder))
}

/// The default embedder factory: load the platform embedder once, share it.
///
/// `swissarmyhammer_embedding::Embedder::default()` resolves the default model;
/// the probe runner needs it *loaded*, so this awaits the load before handing it
/// back. The loaded model is cached in [`DEFAULT_EMBEDDER`] and reused across
/// review runs rather than reloaded per run. Tests inject their own
/// [`EmbedderFactory`] (a mock), which never touches this cache.
pub fn default_embedder_factory() -> EmbedderFactory {
    Arc::new(|| {
        Box::pin(shared_embedder(&DEFAULT_EMBEDDER, || async {
            use model_embedding::TextEmbedder as _;
            let embedder = swissarmyhammer_embedding::Embedder::default()
                .await
                .map_err(|e| format!("failed to resolve embedder: {e}"))?;
            embedder
                .load()
                .await
                .map_err(|e| format!("failed to load embedder: {e}"))?;
            Ok(Arc::new(embedder) as Arc<dyn model_embedding::TextEmbedder>)
        }))
    })
}

/// Resolve the pool's concurrency policy from the coarse `backend` choice and an
/// optional pinned `review.concurrency` override.
///
/// `local` → a single in-process worker (one model/GPU); `session` (or absent) →
/// the remote/Claude-API default. When `concurrency` is `Some(n)`, the worker
/// count is pinned to `n` (and AIMD disabled) regardless of the backend — this is
/// the `review.concurrency` override the server applies at the wiring layer.
fn pool_config_for(backend: Option<&str>, concurrency: Option<usize>) -> PoolConfig {
    let base = match backend {
        Some(b) if b.eq_ignore_ascii_case("local") => PoolConfig::local(),
        _ => PoolConfig::remote(DEFAULT_REMOTE_WORKERS),
    };
    match concurrency {
        Some(workers) => base.with_concurrency(workers),
        None => base,
    }
}

/// Default remote worker count when `backend` is `session`/absent and no
/// `review.concurrency` override is supplied.
const DEFAULT_REMOTE_WORKERS: usize = 4;

/// Process-global cap on concurrent review pipelines.
///
/// A single review already fans out internally across its
/// [`AgentPool`](swissarmyhammer_validators::AgentPool); running many review
/// *pipelines* at once instead multiplies the per-run footprint — each loads its
/// own embedding corpus, its own embedder model, and its own agent — which OOMed
/// large repos under a full parallel review (e.g. a `review file`-per-file
/// fan-out minting dozens of pipelines, each holding a multi-hundred-MB corpus +
/// model). One permit serializes pipelines so only one such resource set is
/// resident at a time; throughput is preserved by the in-run fan-out, which this
/// does not touch.
static REVIEW_PIPELINE_GATE: Semaphore = Semaphore::const_new(1);

/// Directory holding the code_context index, relative to the workspace root.
const CONTEXT_DIR: &str = ".code-context";
/// The code_context index database filename.
const DB_NAME: &str = "index.db";

/// A run-review request resolved from one of the three `review` ops.
pub struct ReviewRequest {
    /// The resolved scope (working / sha / file / glob).
    pub scope: Scope,
    /// The `backend` modifier (`session` | `local`), if supplied.
    pub backend: Option<String>,
    /// The optional validator-subset modifier. When non-empty, the fan-out is
    /// scoped to just these validators (via `retain_rulesets`); empty means
    /// every matching validator.
    pub validators: Vec<String>,
    /// The pinned pool worker count from `review.concurrency`, applied by the
    /// server at the wiring layer. `None` defers to the coarse `backend` policy.
    pub concurrency: Option<usize>,
    /// The content-budgeted batch size in BYTES, from the `batch_size` modifier.
    /// `None` defers to [`FleetConfig`]'s default (256 KiB). Applies to every scope.
    pub batch_size: Option<usize>,
}

/// Run a resolved review request end to end and return the report.
///
/// Resolves the engine inputs from `repo_path` (the session work-dir): the full
/// validator loader, an owned read-only code_context index connection, the
/// embedder, and a live agent from `agent_factory`. Delegates the whole pipeline
/// to [`run_review_over_agent`].
///
/// The pipeline holds a `&`[`Connection`] (which is `!Sync`) and drives an ACP
/// connection across `await`s, so it runs on a dedicated current-thread runtime
/// on a blocking thread — the same pattern `swissarmyhammer_agent::execute_prompt`
/// uses. This keeps the non-`Send` futures off the shared async-trait executor.
///
/// # Errors
///
/// Returns a message on loader failure, a missing/locked index, embedder load
/// failure, agent-construction failure, or a pipeline error.
pub async fn run_review_request(
    request: ReviewRequest,
    repo_path: PathBuf,
    embedder_factory: EmbedderFactory,
    agent_factory: AgentFactory,
    now: String,
    progress: Option<ReviewProgressSender>,
) -> Result<ReviewReport, String> {
    // Carry the current span across the thread boundary so the engine's
    // observability lines stay correlated with the originating `tool_call{...}`
    // request span. The *subscriber* needs no carry: `sah serve` installs its
    // subscriber as the process-global default (`set_global_default`), which is
    // visible from every thread — including this `spawn_blocking` thread and the
    // nested current-thread runtime — with no dispatcher dance. (The earlier
    // `get_default`/`set_default` carry only mattered for a thread-local *scoped*
    // subscriber, which no production path uses; an integration test installs a
    // real global subscriber and asserts the engine lines surface.)
    // Serialize review pipelines process-wide: hold a permit for the whole run so
    // only one corpus + embedder + agent set is resident at a time (see
    // `REVIEW_PIPELINE_GATE`). Acquired here, *outside* the `spawn_blocking`, so a
    // second concurrent request waits before it builds any of those resources.
    let _permit = REVIEW_PIPELINE_GATE
        .acquire()
        .await
        .map_err(|e| format!("review pipeline gate closed: {e}"))?;

    let span = tracing::Span::current();
    // Only the synchronous `UnboundedSender` crosses into the blocking thread
    // and its nested current-thread runtime; the async drain task consuming
    // the mapped notifications was spawned by the caller on the OUTER runtime.
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("failed to build review runtime: {e}"))?;
        rt.block_on(run_review_request_inner(
            request,
            repo_path,
            embedder_factory,
            agent_factory,
            now,
            progress,
        ))
    })
    .await
    .map_err(|e| format!("review task join error: {e}"))?
}

/// The pipeline body, run inside the dedicated current-thread runtime.
async fn run_review_request_inner(
    request: ReviewRequest,
    repo_path: PathBuf,
    embedder_factory: EmbedderFactory,
    agent_factory: AgentFactory,
    now: String,
    progress: Option<ReviewProgressSender>,
) -> Result<ReviewReport, String> {
    let mut loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    // Honor the `validators` subset modifier: when the caller named a subset,
    // scope the fan-out to just those validators. Empty means "all matching".
    loader.retain_rulesets(&request.validators);
    let conn = open_index_connection(&repo_path)?;
    let embedder = embedder_factory().await?;

    let handle = agent_factory().await?;

    // Thread the `batch_size` modifier into the engine config; `None` keeps the
    // FleetConfig default (256 KiB).
    let fleet_config = FleetConfig {
        batch_size: request
            .batch_size
            .unwrap_or_else(|| FleetConfig::default().batch_size),
    };

    let report = run_review_over_agent(
        handle.agent,
        handle.notification_rx,
        request.scope,
        &repo_path,
        &loader,
        &conn,
        embedder.as_ref(),
        pool_config_for(request.backend.as_deref(), request.concurrency),
        fleet_config,
        progress,
        &now,
    )
    .await
    .map_err(|e| format!("review pipeline failed: {e}"))?;

    // The engine is a pure data barrier: it always returns a report, carrying the
    // fan-out task tally rather than erroring on it. Error policy lives here, at
    // the tool boundary — a run whose fan-out mostly failed is refused rather than
    // returned as an empty clean pass that a caller (or a `/finish` loop) would
    // mistake for "nothing wrong".
    check_review_completeness(&report)?;
    Ok(report)
}

/// Cumulative pair counters threaded across [`ReviewProgressEvent`]s so the
/// emitted wire `progress` value is monotonic.
///
/// `planned` sums every batch's `Planned { total_pairs }` (a multi-batch run
/// announces each batch as it plans it, so the wire `total` grows — the MCP
/// spec permits a growing total); `completed` counts `PairDone` events and
/// only ever increases, which is what keeps `progress` monotonic.
#[derive(Debug, Default)]
struct ReviewProgressState {
    /// Planned (validator, file) pairs summed across every batch.
    planned: u64,
    /// Completed pairs (`PairDone` count) — monotonically non-decreasing.
    completed: u64,
}

/// Map one engine [`ReviewProgressEvent`] to the wire
/// [`ProgressNotificationParam`], updating the cumulative counters.
///
/// The wire contract: `progress` = completed pairs, `total` = planned pairs
/// (floored at `progress` so `total >= progress` always holds), the request's
/// `token` echoed on every notification, and a human `message` naming the
/// validator and the FULL file path — never truncated.
fn review_progress_param(
    state: &mut ReviewProgressState,
    token: &ProgressToken,
    event: &ReviewProgressEvent,
) -> ProgressNotificationParam {
    let message = match event {
        ReviewProgressEvent::Planned { total_pairs } => {
            state.planned += *total_pairs as u64;
            format!("Planned {total_pairs} (validator, file) review pairs")
        }
        ReviewProgressEvent::PairStarted { validator, file } => {
            format!("Reviewing {file} against {validator}")
        }
        ReviewProgressEvent::PairDone { validator, file } => {
            state.completed += 1;
            format!("Reviewed {file} against {validator}")
        }
    };
    let progress = state.completed;
    let total = state.planned.max(progress);
    ProgressNotificationParam {
        progress_token: token.clone(),
        progress: progress as f64,
        total: Some(total as f64),
        message: Some(message),
    }
}

/// The two live halves of a review progress bridge: the engine-facing sender
/// and the drain task flushing mapped notifications to the client.
#[derive(Debug)]
pub struct ReviewProgressBridge {
    /// The sender to thread into [`run_review_request`]; dropping it (when the
    /// pipeline finishes) winds the whole bridge down.
    pub sender: ReviewProgressSender,
    /// The drain task forwarding mapped `notifications/progress` params to the
    /// MCP peer or in-process sink. Await it after the run so buffered
    /// notifications flush before the final result returns.
    pub drain: tokio::task::JoinHandle<()>,
}

/// Wire the review progress bridge for one tool call, when the caller opted in.
///
/// Precedence matches `code_context`'s `rebuild index`: `progress_token` plus
/// `progress_sink` (the explicit in-process opt-in) wins over `progress_token`
/// plus `peer` (the stdio/HTTP MCP client); no token — or a token with neither
/// transport (logged) — returns `None` and the review emits zero notifications.
///
/// Must be called on the OUTER runtime (it spawns the mapping and drain
/// tasks there) BEFORE [`run_review_request`] enters its `spawn_blocking`;
/// only the returned synchronous sender crosses into the nested runtime.
pub fn spawn_review_progress_bridge(context: &ToolContext) -> Option<ReviewProgressBridge> {
    let token = context.progress_token.clone()?;

    let (event_tx, mut event_rx) = tokio::sync::mpsc::unbounded_channel::<ReviewProgressEvent>();
    let (param_tx, param_rx) = tokio::sync::mpsc::unbounded_channel::<ProgressNotificationParam>();

    let drain = if let Some(sink) = context.progress_sink.clone() {
        tracing::debug!(?token, "review: wiring progress bridge to in-process sink");
        spawn_in_process_drain_task(sink, param_rx)
    } else if let Some(peer) = context.peer.clone() {
        tracing::debug!(?token, "review: wiring progress bridge to MCP peer");
        spawn_drain_task(peer, param_rx)
    } else {
        tracing::warn!(
            "review: progressToken present but no MCP peer or progress sink — emitting no progress"
        );
        return None;
    };

    // The mapping task owns the cumulative counters. It ends when the engine
    // drops its sender; dropping `param_tx` then closes the drain's source so
    // the drain flushes and exits.
    tokio::spawn(async move {
        let mut state = ReviewProgressState::default();
        while let Some(event) = event_rx.recv().await {
            let _ = param_tx.send(review_progress_param(&mut state, &token, &event));
        }
    });

    Some(ReviewProgressBridge {
        sender: event_tx,
        drain,
    })
}

/// The fraction of attempted fan-out tasks that must fail before a review is
/// refused as incomplete rather than returned.
///
/// Set above one-half so a review is failed loudly only when a *majority* of its
/// fan-out tasks did not run — the calcutron symptom was 60/60 (100%) failing,
/// returned as a clean empty pass. A minority of failures still produces a
/// trustworthy report (the rendered markdown flags the gap and the counts carry
/// the tally), so it is returned, not errored; the threshold draws the line at
/// the point where the empty findings set no longer means "nothing wrong" but
/// "the review did not actually run".
const INCOMPLETE_REVIEW_FAILURE_RATE: f64 = 0.5;

/// Decide whether a finished review is trustworthy enough to return.
///
/// Returns `Err` with an incomplete-review message naming the failed/attempted
/// task counts when more than [`INCOMPLETE_REVIEW_FAILURE_RATE`] of the attempted
/// fan-out tasks failed, so a wedged run surfaces as a tool error instead of an
/// empty clean report. A run that attempted no tasks (an empty diff) has no
/// failure rate and is always trustworthy.
fn check_review_completeness(report: &ReviewReport) -> Result<(), String> {
    let attempted = report.counts().tasks_attempted();
    let failed = report.counts().tasks_failed();
    if attempted == 0 {
        return Ok(());
    }
    if failed as f64 > attempted as f64 * INCOMPLETE_REVIEW_FAILURE_RATE {
        return Err(format!(
            "incomplete review: {failed}/{attempted} fan-out tasks failed \
             (over {:.0}%) — the review did not actually run, so the empty findings are not a \
             clean pass and were not returned. This is a genuine fan-out failure (the review \
             agent/backend erroring or timing out per task), NOT the diff being too large: a \
             large diff is reviewed in content-budgeted batches, and a single file over \
             `batch_size` fails fast with its own distinct error. Check the agent/backend health \
             (and `review.concurrency`), then retry.",
            INCOMPLETE_REVIEW_FAILURE_RATE * 100.0
        ));
    }
    Ok(())
}

/// Open an owned read-only connection to the workspace's code_context index.
///
/// The engine's probe runner takes a `&Connection` it holds across `await`s, so
/// the tool owns a dedicated connection for the run rather than borrowing the
/// workspace's shared (std-`Mutex`-guarded) write handle.
///
/// # Errors
///
/// Returns a message when the index database is absent (the workspace was never
/// indexed) or cannot be opened read-only.
fn open_index_connection(repo_path: &Path) -> Result<Connection, String> {
    let db_path: PathBuf = repo_path.join(CONTEXT_DIR).join(DB_NAME);
    if !db_path.exists() {
        return Err(format!(
            "no code_context index at {} — run `code_context rebuild index` first",
            db_path.display()
        ));
    }
    // Mirror the workspace follower: a read-only connection (WAL lets it read
    // while the leader writes), then the shared connection configuration.
    let flags =
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(&db_path, flags)
        .map_err(|e| format!("failed to open code_context index: {e}"))?;
    swissarmyhammer_code_context::db::configure_connection(&conn)
        .map_err(|e| format!("failed to configure code_context index connection: {e}"))?;
    Ok(conn)
}

/// The JSON shape returned for a `review file/working/sha` op: the rendered
/// markdown plus the per-verdict counts.
#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    pub markdown: String,
    /// The per-verdict tallies.
    pub counts: ReviewCountsView,
}

/// The serializable view of the engine's review counts.
///
/// Review is binary pass/fail — there is no graded severity — so the rendered
/// failures are a single `findings` count, not a per-tier breakdown.
#[derive(Debug, Serialize)]
pub struct ReviewCountsView {
    /// Confirmed findings rendered into the checklist (post-dedup).
    pub findings: usize,
    /// Findings the verifier confirmed.
    pub confirmed: usize,
    /// Findings the verifier refuted.
    pub refuted: usize,
    /// How many fan-out review tasks were attempted.
    pub attempted: usize,
    /// How many fan-out review tasks failed and degraded to zero findings. A
    /// non-zero value means the rendered findings are INCOMPLETE.
    pub failed: usize,
}

impl From<ReviewReport> for ReviewResponse {
    fn from(report: ReviewReport) -> Self {
        let counts = *report.counts();
        ReviewResponse {
            markdown: report.into_markdown(),
            counts: ReviewCountsView {
                findings: counts.findings(),
                confirmed: counts.confirmed(),
                refuted: counts.refuted(),
                attempted: counts.tasks_attempted(),
                failed: counts.tasks_failed(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use swissarmyhammer_validators::review::{synthesize, FleetTally, ReviewReport};

    /// Build a string-typed progress token for tests.
    fn token(s: &str) -> ProgressToken {
        ProgressToken(rmcp::model::NumberOrString::String(s.into()))
    }

    /// One event per state transition through a two-pair run: the wire params
    /// must echo the token, stay monotonic, name the validator and the FULL
    /// untruncated file path, and close with `progress == total`.
    #[test]
    fn review_progress_params_are_monotonic_and_close_at_the_planned_total() {
        let mut state = ReviewProgressState::default();
        let tok = token("review-tok");
        let long_path = "src/a/very/deeply/nested/module/path/payments_processing.rs";
        let events = [
            ReviewProgressEvent::Planned { total_pairs: 2 },
            ReviewProgressEvent::PairStarted {
                validator: "duplication".to_string(),
                file: long_path.to_string(),
            },
            ReviewProgressEvent::PairStarted {
                validator: "reuse".to_string(),
                file: "src/util.rs".to_string(),
            },
            ReviewProgressEvent::PairDone {
                validator: "duplication".to_string(),
                file: long_path.to_string(),
            },
            ReviewProgressEvent::PairDone {
                validator: "reuse".to_string(),
                file: "src/util.rs".to_string(),
            },
        ];
        let params: Vec<_> = events
            .iter()
            .map(|event| review_progress_param(&mut state, &tok, event))
            .collect();

        // Every notification echoes the request's token.
        assert!(params.iter().all(|p| p.progress_token == tok));

        // `progress` is monotonically non-decreasing and never exceeds `total`.
        for w in params.windows(2) {
            assert!(
                w[1].progress >= w[0].progress,
                "progress regressed: {:?} -> {:?}",
                w[0],
                w[1]
            );
        }
        assert!(params.iter().all(|p| p.total.unwrap() >= p.progress));

        // The plan announces the pair total before any pair completes.
        assert_eq!(params[0].progress, 0.0);
        assert_eq!(params[0].total, Some(2.0));

        // Messages name the validator and the full untruncated file path.
        let started = params[1].message.as_deref().unwrap();
        assert!(
            started.contains("duplication") && started.contains(long_path),
            "message must name validator + full path: {started}"
        );
        let done = params[3].message.as_deref().unwrap();
        assert!(
            done.contains("duplication") && done.contains(long_path),
            "message must name validator + full path: {done}"
        );

        // The final PairDone closes the bar: progress == total == planned pairs.
        let last = params.last().unwrap();
        assert_eq!(Some(last.progress), last.total);
        assert_eq!(last.progress, 2.0);
    }

    /// A bare `ToolContext` (no token, no sink, no peer).
    fn bare_context() -> ToolContext {
        let git_ops = Arc::new(tokio::sync::Mutex::new(None));
        let tool_handlers = Arc::new(crate::mcp::tool_handlers::ToolHandlers::new());
        let agent_config = Arc::new(swissarmyhammer_config::ModelConfig::default());
        ToolContext::new(tool_handlers, git_ops, agent_config)
    }

    /// No `progressToken` (and no sink) → no bridge → the review threads a
    /// `None` sender and emits zero notifications — the pre-progress behavior.
    #[tokio::test]
    async fn no_progress_token_means_no_bridge() {
        assert!(spawn_review_progress_bridge(&bare_context()).is_none());
    }

    /// A token with neither transport (no peer, no sink) cannot ship
    /// notifications anywhere — the bridge is skipped, not half-built.
    #[tokio::test]
    async fn a_token_without_peer_or_sink_means_no_bridge() {
        let context = bare_context().with_progress_token(token("t"));
        assert!(spawn_review_progress_bridge(&context).is_none());
    }

    /// Token + in-process sink wires the bridge: engine events are mapped to
    /// wire params carrying the token, and dropping the engine sender flushes
    /// the drain to completion.
    #[tokio::test]
    async fn a_token_with_a_sink_bridges_engine_events_to_the_sink() {
        let (sink_tx, mut sink_rx) = tokio::sync::mpsc::unbounded_channel();
        let context = bare_context()
            .with_progress_token(token("bridge-tok"))
            .with_progress_sink(sink_tx);
        let bridge = spawn_review_progress_bridge(&context).expect("bridge wired");

        bridge
            .sender
            .send(ReviewProgressEvent::Planned { total_pairs: 1 })
            .unwrap();
        bridge
            .sender
            .send(ReviewProgressEvent::PairDone {
                validator: "v".to_string(),
                file: "src/a.rs".to_string(),
            })
            .unwrap();

        // Dropping the engine's sender winds the bridge down; awaiting the
        // drain proves every buffered notification flushed first.
        drop(bridge.sender);
        bridge.drain.await.expect("drain joins cleanly");

        let mut got = Vec::new();
        while let Ok(param) = sink_rx.try_recv() {
            got.push(param);
        }
        assert_eq!(got.len(), 2, "both events reach the sink: {got:#?}");
        assert!(got.iter().all(|p| p.progress_token == token("bridge-tok")));
        let last = got.last().unwrap();
        assert_eq!(
            Some(last.progress),
            last.total,
            "the single planned pair completed, closing the bar"
        );
    }

    /// A multi-batch run emits one `Planned` per batch; the wire `total` is the
    /// running sum so progress still closes at the whole run's pair count.
    #[test]
    fn review_progress_totals_accumulate_across_batches() {
        let mut state = ReviewProgressState::default();
        let tok = token("t");

        let first = review_progress_param(
            &mut state,
            &tok,
            &ReviewProgressEvent::Planned { total_pairs: 2 },
        );
        assert_eq!(first.total, Some(2.0));

        for file in ["src/a.rs", "src/b.rs"] {
            review_progress_param(
                &mut state,
                &tok,
                &ReviewProgressEvent::PairDone {
                    validator: "v".to_string(),
                    file: file.to_string(),
                },
            );
        }

        let second_plan = review_progress_param(
            &mut state,
            &tok,
            &ReviewProgressEvent::Planned { total_pairs: 3 },
        );
        assert_eq!(
            second_plan.total,
            Some(5.0),
            "totals accumulate across batches"
        );
        assert_eq!(second_plan.progress, 2.0, "completed pairs carry over");
    }

    /// A report carrying the given fan-out task tally and no findings, built
    /// through the engine's own `synthesize` (the one construction path a
    /// `ReviewReport` has now that its fields are encapsulated).
    fn report_with_tally(attempted: usize, failed: usize) -> ReviewReport {
        synthesize(vec![], &FleetTally::new(attempted, failed), "now")
    }

    /// Parity guard: the `backend` modifier influences ONLY the pool's worker
    /// count, never which agent/model runs.
    ///
    /// The review pipeline drives a single agent built by `agent_factory()` from
    /// the resolved review `ModelConfig` (default `claude-code-haiku`), shared
    /// across every pool worker. `backend` reaches only `pool_config_for`, so a
    /// `local` and a `session` run over the same config resolve the SAME model —
    /// the two backends differ exclusively in worker count and AIMD, never in the
    /// agent. This asserts that contract so a future change cannot quietly route
    /// `local` to a different agent and drift the model.
    #[test]
    fn backend_only_governs_pool_policy_not_the_agent_model() {
        let local = pool_config_for(Some("local"), None);
        let session = pool_config_for(Some("session"), None);

        // The local backend serializes to one in-process model/GPU worker; the
        // session backend runs the remote default fan-out. This is the ONLY
        // axis `backend` controls.
        assert_eq!(local.workers, 1, "local backend is single-worker");
        assert_eq!(
            session.workers, DEFAULT_REMOTE_WORKERS,
            "session backend runs the remote default fan-out"
        );

        // A pinned `review.concurrency` overrides the worker count for BOTH
        // backends identically, confirming the only difference is the policy —
        // not the agent the worker drives.
        let local_pinned = pool_config_for(Some("local"), Some(3));
        let session_pinned = pool_config_for(Some("session"), Some(3));
        assert_eq!(local_pinned.workers, session_pinned.workers);
        assert_eq!(local_pinned.workers, 3);
    }

    #[test]
    fn a_majority_failed_review_is_an_incomplete_run_error_not_a_pass() {
        // The calcutron symptom: every fan-out task failed, yet the report is an
        // empty clean section. The tool boundary must refuse it as an error.
        let report = report_with_tally(60, 60);
        let err = check_review_completeness(&report)
            .expect_err("an all-failed review must surface as an error");
        assert!(err.contains("60"), "the error names the tally: {err}");
        assert!(
            err.to_lowercase().contains("incomplete"),
            "the error must flag the run incomplete: {err}"
        );
    }

    #[test]
    fn a_run_under_the_failure_threshold_is_not_an_error() {
        // A minority of tasks failed (1 of 10) — the report is still trustworthy,
        // so the tool returns it (the rendered markdown already flags the gap).
        let report = report_with_tally(10, 1);
        assert!(
            check_review_completeness(&report).is_ok(),
            "a minority of failures must not error the run"
        );
    }

    #[test]
    fn a_fully_successful_run_is_not_an_error() {
        let report = report_with_tally(8, 0);
        assert!(check_review_completeness(&report).is_ok());
    }

    #[test]
    fn a_run_that_attempted_no_tasks_is_not_an_error() {
        // An empty diff attempts no fan-out tasks; with zero attempted there is no
        // failure rate to exceed, so it is a clean pass, not an incomplete run.
        let report = report_with_tally(0, 0);
        assert!(
            check_review_completeness(&report).is_ok(),
            "a no-op review must not divide-by-zero into an error"
        );
    }

    fn mock() -> Arc<dyn model_embedding::TextEmbedder> {
        Arc::new(model_embedding::mock::MockEmbedder::new(4))
            as Arc<dyn model_embedding::TextEmbedder>
    }

    /// The shared-embedder cache runs `init` exactly once and hands every caller
    /// the same `Arc` — the load-once-share contract `default_embedder_factory`
    /// relies on so the model isn't reloaded per review run.
    #[tokio::test]
    async fn shared_embedder_initializes_once_and_shares_the_arc() {
        let cell = OnceCell::new();
        let calls = AtomicUsize::new(0);

        let first = shared_embedder(&cell, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(mock())
        })
        .await
        .expect("first init");

        let second = shared_embedder(&cell, || async {
            calls.fetch_add(1, Ordering::SeqCst);
            Ok(mock())
        })
        .await
        .expect("second call hits the cache");

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "init must run exactly once"
        );
        assert!(
            Arc::ptr_eq(&first, &second),
            "both calls must share the one cached Arc"
        );
    }

    /// A failed `init` is not cached: a later call retries rather than handing
    /// back a poisoned/failed cell forever.
    #[tokio::test]
    async fn shared_embedder_does_not_cache_a_failed_init() {
        let cell = OnceCell::new();

        let failed = shared_embedder(&cell, || async {
            Err::<Arc<dyn model_embedding::TextEmbedder>, String>("load failed".to_string())
        })
        .await;
        assert!(failed.is_err(), "the failed init surfaces as an error");

        let retried = shared_embedder(&cell, || async { Ok(mock()) }).await;
        assert!(
            retried.is_ok(),
            "a failed init must not poison the cache; a later init succeeds"
        );
    }
}
