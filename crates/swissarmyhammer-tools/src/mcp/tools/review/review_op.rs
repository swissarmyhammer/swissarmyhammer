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
use rmcp::model::{
    LoggingLevel, LoggingMessageNotification, LoggingMessageNotificationParam,
    ProgressNotificationParam, ProgressToken,
};
use rmcp::{Peer, RoleServer};
use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::{broadcast, OnceCell, Semaphore};

use swissarmyhammer_embedding::{DownloadEvent, DownloadObserver};
use swissarmyhammer_validators::review::{
    run_review_over_agent, FleetConfig, ReviewProgressEvent, ReviewProgressSender, ReviewReport,
    Scope,
};
use swissarmyhammer_validators::{load_rules, AvpError, PoolConfig};

use crate::mcp::progress::{spawn_drain_task, spawn_in_process_drain_task};
use crate::mcp::tool_registry::ToolContext;

/// The two halves of a ready-to-drive ACP agent handle: its
/// [`DynConnectTo<Client>`] component and the broadcast receiver of its streamed
/// `session/update` notifications. This is exactly the shape of
/// `swissarmyhammer_agent::AcpAgentHandle`, supplied to the tool so this crate
/// (which `swissarmyhammer-agent` depends on) never constructs an agent itself.
pub struct AgentHandle {
    /// The agent component the driver runs as the ACP server side. Consumed by
    /// value through [`into_parts`](Self::into_parts); private so the handle's
    /// layout is not a field-level API commitment.
    agent: DynConnectTo<Client>,
    /// The receiver of the agent's streamed notifications. Consumed through
    /// [`into_parts`](Self::into_parts); private for the same reason as
    /// [`agent`](Self::into_parts).
    notification_rx: broadcast::Receiver<SessionNotification>,
}

impl AgentHandle {
    /// Assemble a handle from its two halves (the shape a factory mints).
    pub fn new(
        agent: DynConnectTo<Client>,
        notification_rx: broadcast::Receiver<SessionNotification>,
    ) -> Self {
        Self {
            agent,
            notification_rx,
        }
    }

    /// Consume the handle into its two halves.
    ///
    /// The engine driver ([`run_review_over_agent`]) takes both by value — the
    /// agent component to run as the ACP server side and the notification
    /// receiver to collect from — so the honest accessor is a by-value split,
    /// not borrowing getters.
    pub fn into_parts(
        self,
    ) -> (
        DynConnectTo<Client>,
        broadcast::Receiver<SessionNotification>,
    ) {
        (self.agent, self.notification_rx)
    }
}

impl std::fmt::Debug for AgentHandle {
    /// Manual impl: the agent component is a type-erased connector with no
    /// `Debug` of its own, so it renders by type name instead.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentHandle")
            .field("agent", &"DynConnectTo<Client>")
            .field("notification_rx", &self.notification_rx)
            .finish()
    }
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
///
/// The factory takes an optional [`DownloadObserver`]: the caller wires one when
/// the run has a `progressToken` so a FIRST-run review's model download streams
/// [`ReviewProgressEvent::DownloadingModel`] progress instead of minutes of
/// silence. The default factory attaches it to the load that populates the
/// process-global embedder cache; the mock factories ignore it (they download
/// nothing).
pub type EmbedderFactory = Arc<
    dyn Fn(
            Option<DownloadObserver>,
        ) -> Pin<
            Box<
                dyn Future<Output = Result<Arc<dyn model_embedding::TextEmbedder>, EmbedderError>>
                    + Send,
            >,
        > + Send
        + Sync,
>;

/// Errors from resolving the review embedder through an [`EmbedderFactory`].
///
/// The factory is a type-erased seam implemented by heterogeneous backends (the
/// platform embedder in production, mocks in tests), so each variant carries the
/// backend's rendered message rather than a concrete source type.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum EmbedderError {
    /// The configured embedder model could not be resolved.
    #[error("failed to resolve embedder: {0}")]
    Resolve(String),
    /// The resolved embedder failed to load its weights.
    #[error("failed to load embedder: {0}")]
    Load(String),
}

/// Errors from driving one resolved review request end to end.
///
/// Each variant names one failure point of [`run_review_request`]: resolving the
/// engine inputs (validators, index, embedder, agent), hosting the pipeline on
/// its dedicated runtime, or the pipeline itself. A run where fan-out tasks
/// failed is *not* an error case here — it returns a `ReviewReport` whose
/// markdown carries the INCOMPLETE banner and whose counts carry the failure
/// tally (see [`run_review_request`]'s `# Errors` section).
#[derive(Debug, thiserror::Error)]
pub enum ReviewError {
    /// The process-wide [`REVIEW_PIPELINE_GATE`] semaphore closed (process
    /// shutdown) while this request waited for its permit.
    #[error("review pipeline gate closed: {0}")]
    GateClosed(#[from] tokio::sync::AcquireError),
    /// The dedicated current-thread runtime hosting the pipeline failed to
    /// build.
    #[error("failed to build review runtime: {0}")]
    Runtime(#[source] std::io::Error),
    /// The blocking task hosting the pipeline panicked or was cancelled.
    #[error("review task join error: {0}")]
    Join(#[from] tokio::task::JoinError),
    /// The validator loader failed to load the RuleSet stack.
    #[error("failed to load validators: {0}")]
    ValidatorLoad(#[source] AvpError),
    /// No code_context index exists at the expected workspace path.
    #[error("no code_context index at {} — run `code_context rebuild index` first", .0.display())]
    IndexMissing(PathBuf),
    /// The index database exists but could not be opened read-only.
    #[error("failed to open code_context index: {0}")]
    IndexOpen(#[source] rusqlite::Error),
    /// The opened index connection could not be configured.
    #[error("failed to configure code_context index connection: {0}")]
    IndexConfigure(#[source] rusqlite::Error),
    /// The embedder factory failed to resolve or load the embedder.
    #[error(transparent)]
    Embedder(#[from] EmbedderError),
    /// The agent factory failed to build the review agent. Factory errors cross
    /// the type-erased [`AgentFactory`] seam as rendered strings, passed through
    /// verbatim.
    #[error("{0}")]
    Agent(String),
    /// The engine pipeline itself failed.
    #[error("review pipeline failed: {0}")]
    Pipeline(#[source] AvpError),
}

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
) -> Result<Arc<dyn model_embedding::TextEmbedder>, EmbedderError>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<Arc<dyn model_embedding::TextEmbedder>, EmbedderError>>,
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
    Arc::new(|observer: Option<DownloadObserver>| {
        Box::pin(shared_embedder(&DEFAULT_EMBEDDER, move || async move {
            use model_embedding::TextEmbedder as _;
            // Attach the download observer (when the run wired one) to the load
            // that actually populates the cache, so a cold first run streams
            // DownloadingModel progress. On a warm cache `shared_embedder` never
            // runs this init and the observer is simply dropped — events are
            // naturally first-run-only.
            let embedder = match observer {
                Some(observer) => {
                    swissarmyhammer_embedding::Embedder::with_download_observer(
                        swissarmyhammer_embedding::DEFAULT_MODEL_NAME,
                        observer,
                    )
                    .await
                }
                None => swissarmyhammer_embedding::Embedder::default().await,
            }
            .map_err(|e| EmbedderError::Resolve(e.to_string()))?;
            embedder
                .load()
                .await
                .map_err(|e| EmbedderError::Load(e.to_string()))?;
            Ok(Arc::new(embedder) as Arc<dyn model_embedding::TextEmbedder>)
        }))
    })
}

/// Build a [`DownloadObserver`] that forwards each model-download
/// [`DownloadEvent`] as a [`ReviewProgressEvent::DownloadingModel`] on the run's
/// progress channel, while the shared `armed` slot still holds the sender.
///
/// The slot is a disarmable indirection rather than a captured sender because the
/// llama embedder backend retains the observer inside the process-global
/// [`DEFAULT_EMBEDDER`] cache for its whole lifetime. A directly-captured
/// [`ReviewProgressSender`] would therefore outlive the run and hold the review
/// progress channel open forever, wedging the bridge drain. The caller clears the
/// slot (`None`) the moment the embedder load returns — after which this observer
/// holds no sender and the channel closes normally. A closed receiver on the send
/// is a no-op; progress is advisory.
fn review_download_observer(
    armed: Arc<std::sync::Mutex<Option<ReviewProgressSender>>>,
) -> DownloadObserver {
    Arc::new(move |event: DownloadEvent| {
        if let Some(tx) = armed.lock().unwrap_or_else(|p| p.into_inner()).as_ref() {
            let _ = tx.send(ReviewProgressEvent::DownloadingModel {
                file: event.file().to_string(),
                downloaded_bytes: event.downloaded_bytes(),
                total_bytes: event.total_bytes(),
            });
        }
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
///
/// Built with [`ReviewRequest::new`] plus the `with_*` modifiers (the same
/// builder shape as `ReviewTool`); read through the getters. All fields are
/// private so the request can evolve without a field-level API commitment.
pub struct ReviewRequest {
    /// The resolved scope (working / sha / file / glob).
    scope: Scope,
    /// The `backend` modifier (`session` | `local`), if supplied.
    backend: Option<String>,
    /// The optional validator-subset modifier. When non-empty, the fan-out is
    /// scoped to just these validators (via `retain_rulesets`); empty means
    /// every matching validator.
    validators: Vec<String>,
    /// The pinned pool worker count from `review.concurrency`, applied by the
    /// server at the wiring layer. `None` defers to the coarse `backend` policy.
    concurrency: Option<usize>,
    /// The content-budgeted batch size in BYTES, from the `batch_size` modifier.
    /// `None` defers to [`FleetConfig`]'s default (256 KiB). Applies to every scope.
    batch_size: Option<usize>,
}

impl ReviewRequest {
    /// A request over `scope` with every modifier at its default: no `backend`
    /// choice, all matching validators, no pinned concurrency, and the default
    /// batch size.
    pub fn new(scope: Scope) -> Self {
        Self {
            scope,
            backend: None,
            validators: Vec::new(),
            concurrency: None,
            batch_size: None,
        }
    }

    /// Set the `backend` modifier (`session` | `local`); `None` keeps the
    /// default policy.
    pub fn with_backend(mut self, backend: Option<String>) -> Self {
        self.backend = backend;
        self
    }

    /// Scope the fan-out to just these validators; empty means every matching
    /// validator.
    pub fn with_validators(mut self, validators: Vec<String>) -> Self {
        self.validators = validators;
        self
    }

    /// Pin the pool worker count (`review.concurrency`); `None` defers to the
    /// coarse `backend` policy.
    pub fn with_concurrency(mut self, concurrency: Option<usize>) -> Self {
        self.concurrency = concurrency;
        self
    }

    /// Set the content-budgeted batch size in BYTES; `None` keeps
    /// [`FleetConfig`]'s default.
    pub fn with_batch_size(mut self, batch_size: Option<usize>) -> Self {
        self.batch_size = batch_size;
        self
    }

    /// The resolved scope (working / sha / file / glob).
    pub fn scope(&self) -> &Scope {
        &self.scope
    }

    /// The `backend` modifier (`session` | `local`), if supplied.
    pub fn backend(&self) -> Option<&str> {
        self.backend.as_deref()
    }

    /// The validator-subset modifier; empty means every matching validator.
    pub fn validators(&self) -> &[String] {
        &self.validators
    }

    /// The pinned pool worker count, if any.
    pub fn concurrency(&self) -> Option<usize> {
        self.concurrency
    }

    /// The content-budgeted batch size in BYTES, if overridden.
    pub fn batch_size(&self) -> Option<usize> {
        self.batch_size
    }
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
/// Returns a [`ReviewError`] on loader failure, a missing/locked index, embedder
/// load failure, agent-construction failure, or a pipeline error. A run where
/// some (even all) fan-out tasks failed is *not* an error: it returns
/// `Ok(ReviewReport)` whose markdown carries the `results are INCOMPLETE`
/// banner and whose counts expose `tasks_failed`/`tasks_attempted` — there is
/// no completeness gate refusing the run.
pub async fn run_review_request(
    request: ReviewRequest,
    repo_path: &Path,
    embedder_factory: EmbedderFactory,
    agent_factory: AgentFactory,
    now: &str,
    progress: Option<ReviewProgressSender>,
) -> Result<ReviewReport, ReviewError> {
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
    let _permit = REVIEW_PIPELINE_GATE.acquire().await?;

    let span = tracing::Span::current();
    // The blocking closure needs owned copies of the borrowed inputs ('static).
    let repo_path = repo_path.to_path_buf();
    let now = now.to_string();
    // Only the synchronous `UnboundedSender` crosses into the blocking thread
    // and its nested current-thread runtime; the async drain task consuming
    // the mapped notifications was spawned by the caller on the OUTER runtime.
    tokio::task::spawn_blocking(move || {
        let _entered = span.enter();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(ReviewError::Runtime)?;
        rt.block_on(run_review_request_inner(
            request,
            repo_path,
            embedder_factory,
            agent_factory,
            now,
            progress,
        ))
    })
    .await?
}

/// The pipeline body, run inside the dedicated current-thread runtime.
async fn run_review_request_inner(
    request: ReviewRequest,
    repo_path: PathBuf,
    embedder_factory: EmbedderFactory,
    agent_factory: AgentFactory,
    now: String,
    progress: Option<ReviewProgressSender>,
) -> Result<ReviewReport, ReviewError> {
    let mut loader = load_rules().map_err(ReviewError::ValidatorLoad)?;
    // Honor the `validators` subset modifier: when the caller named a subset,
    // scope the fan-out to just those validators. Empty means "all matching".
    loader.retain_rulesets(&request.validators);
    let conn = open_index_connection(&repo_path)?;

    // Wire a download observer so a FIRST-run review's pre-scope model download
    // streams `DownloadingModel` progress instead of silence. The observer reads
    // a disarmable slot rather than capturing the sender directly: the llama
    // backend RETAINS the observer inside the process-global embedder cache for
    // its whole lifetime, so a captured `ReviewProgressSender` would outlive the
    // run and hold the progress channel open forever — the drain would never
    // finish. We disarm the slot the instant the load returns (all downloads
    // happen during that load), after which the retained observer holds no
    // sender and the channel closes normally. No `progressToken` → no observer →
    // unchanged behavior.
    let download_slot = progress
        .as_ref()
        .map(|tx| Arc::new(std::sync::Mutex::new(Some(tx.clone()))));
    let observer = download_slot
        .as_ref()
        .map(|slot| review_download_observer(Arc::clone(slot)));
    let embedder = embedder_factory(observer).await?;
    if let Some(slot) = &download_slot {
        *slot.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }

    let (agent, notification_rx) = agent_factory()
        .await
        .map_err(ReviewError::Agent)?
        .into_parts();

    // Thread the `batch_size` modifier into the engine config; `None` keeps the
    // FleetConfig default (256 KiB).
    let fleet_config = request.batch_size.map(FleetConfig::new).unwrap_or_default();

    let report = run_review_over_agent(
        agent,
        notification_rx,
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
    .map_err(ReviewError::Pipeline)?;

    // The engine is a pure data barrier: it always returns a report, carrying the
    // fan-out task tally rather than erroring on it. There is no retry at this
    // boundary either — a run whose fan-out mostly (or entirely) failed is
    // returned exactly as-is, never refused as a tool error. `synthesize` already
    // stamps the loud "results are INCOMPLETE" banner directly under the report
    // header whenever any task failed, and `ReviewCountsView` carries the
    // `tasks_failed`/`tasks_attempted` tally to callers — that is the whole
    // failure signal. Refusing here would only push a driving caller (e.g. a
    // `/finish` loop) to re-run the ENTIRE review, including the units that will
    // hit the same underlying failure (e.g. an agentic-loop iteration cap) again.
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
///
/// Returns `None` for the content-carrying variants
/// ([`Findings`](ReviewProgressEvent::Findings) /
/// [`Verdict`](ReviewProgressEvent::Verdict)): they carry content, not progress,
/// and route to `notifications/message` via [`review_content_log_param`] — they
/// must never move the wire counter.
fn review_progress_param(
    state: &mut ReviewProgressState,
    token: &ProgressToken,
    event: &ReviewProgressEvent,
) -> Option<ProgressNotificationParam> {
    let message = match event {
        ReviewProgressEvent::DownloadingModel {
            file,
            downloaded_bytes,
            total_bytes,
        } => {
            // Model download precedes planning: no pair counters exist yet, so
            // the wire values stay at their current (zero) state and cannot
            // regress when planning begins. The byte detail and the full,
            // untruncated filename ride in the message.
            format!("Downloading {file}: {downloaded_bytes}/{total_bytes} bytes")
        }
        ReviewProgressEvent::FileScoped { file } => {
            // Scope-phase announcement: no pair counters exist yet, so the
            // wire values stay at their current (zero) state — the event's
            // job is existence, not arithmetic.
            format!("Scoping {file}")
        }
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
        // Content-carrying variants have no progress param — they route to
        // notifications/message and must not touch the wire counters.
        ReviewProgressEvent::Findings { .. } | ReviewProgressEvent::Verdict { .. } => {
            return None;
        }
    };
    let progress = state.completed;
    let total = state.planned.max(progress);
    Some(ProgressNotificationParam {
        progress_token: token.clone(),
        progress: progress as f64,
        total: Some(total as f64),
        message: Some(message),
    })
}

/// The MCP logger name every review content notification carries.
const REVIEW_LOG_LOGGER: &str = "review";
/// The `kind` tag on a streamed findings payload.
const REVIEW_FINDINGS_KIND: &str = "review.findings";
/// The `kind` tag on a streamed verdict payload.
const REVIEW_VERDICT_KIND: &str = "review.verdict";

/// Map a content-carrying [`ReviewProgressEvent`] to its `notifications/message`
/// [`LoggingMessageNotificationParam`], or `None` for the progress-tick variants
/// (which route to `notifications/progress` via [`review_progress_param`]).
///
/// The two content shapes carry the FULL structured payload — never a summary,
/// never truncated (a finding is streamed as complete `Finding` JSON):
///
/// - [`Findings`](ReviewProgressEvent::Findings) →
///   `{"kind": "review.findings", "validator": …, "findings": [Finding…]}`
/// - [`Verdict`](ReviewProgressEvent::Verdict) →
///   `{"kind": "review.verdict", "finding": Finding, "confirmed": …, "reason": …}`
///
/// Logger `"review"`, level [`Info`](LoggingLevel::Info). Serialization of a
/// `Finding`/`Vec<Finding>` is infallible (plain data), so `serde_json::json!`
/// never panics here.
fn review_content_log_param(
    event: &ReviewProgressEvent,
) -> Option<LoggingMessageNotificationParam> {
    let data = match event {
        ReviewProgressEvent::Findings {
            validator,
            findings,
        } => serde_json::json!({
            "kind": REVIEW_FINDINGS_KIND,
            "validator": validator,
            "findings": findings,
        }),
        ReviewProgressEvent::Verdict {
            finding,
            confirmed,
            reason,
        } => serde_json::json!({
            "kind": REVIEW_VERDICT_KIND,
            "finding": finding,
            "confirmed": confirmed,
            "reason": reason,
        }),
        _ => return None,
    };
    Some(LoggingMessageNotificationParam {
        level: LoggingLevel::Info,
        logger: Some(REVIEW_LOG_LOGGER.to_string()),
        data,
    })
}

/// Send one review content notification to the MCP peer as a
/// `notifications/message`, logging the full payload first (never truncated) so
/// the send path is provable from the log. A failed send is logged at WARN and
/// swallowed — content streaming is advisory, never load-bearing.
async fn send_review_content_log(peer: &Peer<RoleServer>, param: LoggingMessageNotificationParam) {
    tracing::info!(
        logger = param.logger.as_deref().unwrap_or(""),
        data = %param.data,
        "sending review content notifications/message to MCP peer"
    );
    if let Err(err) = peer
        .send_notification(LoggingMessageNotification::new(param).into())
        .await
    {
        tracing::warn!(
            error = %err,
            "failed to send MCP review content notification — peer may have disconnected"
        );
    }
}

/// The two live halves of a review progress bridge: the engine-facing sender
/// and the drain task flushing mapped notifications to the client.
#[derive(Debug)]
pub struct ReviewProgressBridge {
    /// The sender to thread into [`run_review_request`]; dropping it (when the
    /// pipeline finishes) winds the whole bridge down. Consumed through
    /// [`into_parts`](Self::into_parts); private so the bridge's layout is not
    /// a field-level API commitment.
    sender: ReviewProgressSender,
    /// The drain task forwarding mapped `notifications/progress` params to the
    /// MCP peer or in-process sink. Await it after the run so buffered
    /// notifications flush before the final result returns. Consumed through
    /// [`into_parts`](Self::into_parts); private for the same reason as
    /// [`sender`](Self::into_parts).
    drain: tokio::task::JoinHandle<()>,
}

impl ReviewProgressBridge {
    /// Consume the bridge into its two halves: the engine-facing sender and the
    /// drain task.
    ///
    /// The caller hands the sender into [`run_review_request`] (which drops it
    /// when the pipeline finishes) and awaits the drain afterwards so buffered
    /// notifications flush before the final result returns — both halves are
    /// used by value, so the honest accessor is a by-value split.
    pub fn into_parts(self) -> (ReviewProgressSender, tokio::task::JoinHandle<()>) {
        (self.sender, self.drain)
    }
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

    let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel::<ReviewProgressEvent>();
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

    // The content channel (findings/verdicts → `notifications/message`) honors
    // the same sink-takes-priority rule as the progress drain above: when a
    // `progress_sink` is the chosen transport, the sink contract is progress
    // params ONLY, so no content is emitted — it is neither carried on the sink
    // (which is typed for progress params) nor leaked to the peer. Content
    // therefore reaches the peer only on the peer transport path (no sink), which
    // subsumes the no-peer-and-no-sink case (already returned above). Gate the
    // content peer on sink absence to make that contract explicit.
    let content_peer = if context.progress_sink.is_some() {
        None
    } else {
        context.peer.clone()
    };

    // The mapping task owns the cumulative counters and the content channel: it
    // maps progress-tick variants to `param_tx` (drained to sink or peer) and
    // sends content variants straight to `content_peer` as `notifications/message`.
    // It ends when the engine drops its sender; dropping `param_tx` then closes
    // the drain's source so the drain flushes and exits.
    tokio::spawn(run_review_progress_mapping(
        event_rx,
        param_tx,
        content_peer,
        token,
        REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL,
    ));

    Some(ReviewProgressBridge {
        sender: event_tx,
        drain,
    })
}

/// How long the bridge tolerates engine silence before re-sending the latest
/// wire param as a keep-alive.
///
/// Clients (Claude Code among them) reset their MCP tool timeout on every
/// `notifications/progress` they receive, so a long silent stretch — the scope
/// stage's whole-set diff + probe pass, one long agent turn, the verify stage —
/// must still produce periodic notifications or the call is aborted as dead.
/// Ten seconds is frequent enough to hold any realistic client timeout and far
/// too infrequent to matter as traffic.
const REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL: std::time::Duration = std::time::Duration::from_secs(10);

/// The bridge's mapping loop: map engine events to wire params, and re-send
/// the latest param as a keep-alive whenever the engine stays silent for
/// `keep_alive`.
///
/// The keep-alive re-send carries the exact latest param — identical
/// `progress`/`total`/`message` — so wire monotonicity is preserved by
/// construction. Before the first event there is nothing to re-send, so the
/// timer stays disarmed. The loop ends when the engine drops its sender (or
/// the drain side closes); dropping `param_tx` then closes the drain's source
/// so the drain flushes and exits.
///
/// Extracted from [`spawn_review_progress_bridge`] so the keep-alive schedule
/// is unit-testable under paused time.
async fn run_review_progress_mapping(
    mut event_rx: tokio::sync::mpsc::UnboundedReceiver<ReviewProgressEvent>,
    param_tx: tokio::sync::mpsc::UnboundedSender<ProgressNotificationParam>,
    peer: Option<Arc<Peer<RoleServer>>>,
    token: ProgressToken,
    keep_alive: std::time::Duration,
) {
    let mut state = ReviewProgressState::default();
    let mut latest: Option<ProgressNotificationParam> = None;
    loop {
        tokio::select! {
            event = event_rx.recv() => {
                let Some(event) = event else { break };
                // Content-carrying events (findings, verdicts) go to
                // notifications/message via the peer only — they carry no
                // progress, so they never move the wire counter nor arm the
                // keep-alive. With no peer (the in-process sink path), they are
                // skipped: the sink contract is progress params.
                if let Some(log_param) = review_content_log_param(&event) {
                    if let Some(peer) = &peer {
                        send_review_content_log(peer, log_param).await;
                    }
                    continue;
                }
                // Progress-tick variants map to notifications/progress as before.
                let Some(param) = review_progress_param(&mut state, &token, &event) else {
                    continue;
                };
                latest = Some(param.clone());
                if param_tx.send(param).is_err() {
                    break;
                }
            }
            // Re-armed on every loop iteration, so any event (or a prior
            // keep-alive tick) restarts the silence window. Disarmed until
            // the first event exists.
            _ = tokio::time::sleep(keep_alive), if latest.is_some() => {
                let param = latest.clone().expect("guarded by latest.is_some()");
                if param_tx.send(param).is_err() {
                    break;
                }
            }
        }
    }
}

/// Open an owned read-only connection to the workspace's code_context index.
///
/// The engine's probe runner takes a `&Connection` it holds across `await`s, so
/// the tool owns a dedicated connection for the run rather than borrowing the
/// workspace's shared (std-`Mutex`-guarded) write handle.
///
/// # Errors
///
/// Returns [`ReviewError::IndexMissing`] when the index database is absent (the
/// workspace was never indexed), or an open/configure variant when it cannot be
/// opened read-only.
fn open_index_connection(repo_path: &Path) -> Result<Connection, ReviewError> {
    let db_path: PathBuf = repo_path.join(CONTEXT_DIR).join(DB_NAME);
    if !db_path.exists() {
        return Err(ReviewError::IndexMissing(db_path));
    }
    // Mirror the workspace follower: a read-only connection (WAL lets it read
    // while the leader writes), then the shared connection configuration.
    let flags =
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY | rusqlite::OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(&db_path, flags).map_err(ReviewError::IndexOpen)?;
    swissarmyhammer_code_context::db::configure_connection(&conn)
        .map_err(ReviewError::IndexConfigure)?;
    Ok(conn)
}

/// The JSON shape returned for a `review file/working/sha` op: the rendered
/// markdown plus the per-verdict counts.
///
/// The fields are private (read through the getters); serde serializes them by
/// their field names, so the wire shape is unchanged by the encapsulation.
#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    markdown: String,
    /// The per-verdict tallies.
    counts: ReviewCountsView,
}

impl ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    pub fn markdown(&self) -> &str {
        &self.markdown
    }

    /// The per-verdict tallies.
    pub fn counts(&self) -> &ReviewCountsView {
        &self.counts
    }
}

/// The serializable view of the engine's review counts.
///
/// Review is binary pass/fail — there is no graded severity — so the rendered
/// failures are a single `findings` count, not a per-tier breakdown. The fields
/// are private (read through the getters); serde serializes them by their field
/// names, so the wire shape is unchanged by the encapsulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ReviewCountsView {
    /// Confirmed findings rendered into the checklist (post-dedup).
    findings: usize,
    /// Findings the verifier confirmed.
    confirmed: usize,
    /// Findings the verifier refuted.
    refuted: usize,
    /// How many fan-out review tasks were attempted.
    attempted: usize,
    /// How many fan-out review tasks failed and degraded to zero findings. A
    /// non-zero value means the rendered findings are INCOMPLETE.
    failed: usize,
}

impl ReviewCountsView {
    /// Confirmed findings rendered into the checklist (post-dedup).
    pub fn findings(&self) -> usize {
        self.findings
    }

    /// Findings the verifier confirmed.
    pub fn confirmed(&self) -> usize {
        self.confirmed
    }

    /// Findings the verifier refuted.
    pub fn refuted(&self) -> usize {
        self.refuted
    }

    /// How many fan-out review tasks were attempted.
    pub fn attempted(&self) -> usize {
        self.attempted
    }

    /// How many fan-out review tasks failed and degraded to zero findings. A
    /// non-zero value means the rendered findings are INCOMPLETE.
    pub fn failed(&self) -> usize {
        self.failed
    }
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
            .map(|event| review_progress_param(&mut state, &tok, event).expect("progress variant"))
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

    /// `DownloadingModel` events map to zero-progress params that name the full
    /// file and both byte counts, and never regress the wire progress when the
    /// plan/pair events that follow move the counters.
    #[test]
    fn downloading_model_events_map_to_zero_progress_params_naming_file_and_bytes() {
        let mut state = ReviewProgressState::default();
        let tok = token("dl-tok");
        let file = "models/qwen3-embedding/model-00001-of-00002.safetensors";
        let events = [
            ReviewProgressEvent::DownloadingModel {
                file: file.to_string(),
                downloaded_bytes: 0,
                total_bytes: 500,
            },
            ReviewProgressEvent::DownloadingModel {
                file: file.to_string(),
                downloaded_bytes: 500,
                total_bytes: 500,
            },
            ReviewProgressEvent::Planned { total_pairs: 1 },
            ReviewProgressEvent::PairDone {
                validator: "duplication".to_string(),
                file: "src/a.rs".to_string(),
            },
        ];
        let params: Vec<_> = events
            .iter()
            .map(|event| review_progress_param(&mut state, &tok, event).expect("progress variant"))
            .collect();

        // Every param echoes the request's token.
        assert!(params.iter().all(|p| p.progress_token == tok));

        // Downloads precede planning: their params sit at zero progress.
        assert_eq!(params[0].progress, 0.0);
        assert_eq!(params[1].progress, 0.0);

        // The message names the FULL untruncated path and both byte counts.
        let msg = params[1].message.as_deref().unwrap();
        assert!(msg.contains(file), "message must name the full file: {msg}");
        assert!(
            msg.contains("500"),
            "message must carry the byte counts: {msg}"
        );

        // Wire progress never regresses across download → plan → done.
        for w in params.windows(2) {
            assert!(
                w[1].progress >= w[0].progress,
                "progress regressed: {:?} -> {:?}",
                w[0],
                w[1]
            );
            assert!(w[1].total.unwrap() >= w[1].progress);
        }

        // The one planned pair completing closes the bar.
        let last = params.last().unwrap();
        assert_eq!(Some(last.progress), last.total);
        assert_eq!(last.progress, 1.0);
    }

    /// A single `DownloadingModel` event through the real bridge mapping must
    /// emit ONE token-echoing param (proving it is not a no-op) AND arm the
    /// keep-alive — the pre-scope model download is exactly what fills the
    /// otherwise-silent window before `scope_review` emits its first event, so
    /// its param must both reach the wire and re-send during continued silence.
    #[tokio::test(start_paused = true)]
    async fn a_downloading_model_event_emits_a_param_and_arms_the_keep_alive() {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(run_review_progress_mapping(
            event_rx,
            param_tx,
            None,
            token("dl"),
            TEST_KEEP_ALIVE,
        ));

        event_tx
            .send(ReviewProgressEvent::DownloadingModel {
                file: "models/qwen3-embedding/model.safetensors".to_string(),
                downloaded_bytes: 10,
                total_bytes: 100,
            })
            .unwrap();
        advance(std::time::Duration::ZERO).await;
        let mapped = take_buffered(&mut param_rx);
        assert_eq!(
            mapped.len(),
            1,
            "the download event maps to one real param, not a no-op"
        );
        assert_eq!(mapped[0].progress_token, token("dl"));
        let msg = mapped[0].message.as_deref().unwrap();
        assert!(
            msg.contains("models/qwen3-embedding/model.safetensors") && msg.contains("100"),
            "the param names the file and byte counts: {msg}"
        );

        // The emitted param armed the keep-alive: continued silence re-sends it
        // verbatim, holding the client's timeout through the download window.
        advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
        let tick = take_buffered(&mut param_rx);
        assert_eq!(tick.len(), 1, "the download param armed the keep-alive");
        assert_eq!(tick[0].message, mapped[0].message);
        assert_eq!(tick[0].progress, mapped[0].progress);
    }

    /// A scope-phase event names the file with no counter movement: progress
    /// and total stay at their current values (zero at the start of a run),
    /// so the run's first notifications are valid and monotonic.
    #[test]
    fn file_scoped_events_carry_a_scoping_message_without_moving_counters() {
        let mut state = ReviewProgressState::default();
        let tok = token("scope-tok");
        let param = review_progress_param(
            &mut state,
            &tok,
            &ReviewProgressEvent::FileScoped {
                file: "src/a/very/deep/path.rs".to_string(),
            },
        )
        .expect("a FileScoped event maps to a progress param");
        assert_eq!(param.progress, 0.0);
        assert_eq!(param.total, Some(0.0));
        assert_eq!(
            param.message.as_deref(),
            Some("Scoping src/a/very/deep/path.rs"),
            "the message names the full untruncated path"
        );
        assert_eq!(param.progress_token, tok);
    }

    /// A sample validator-tagged finding whose fields are all distinctive so a
    /// streamed payload can be asserted field-by-field.
    fn sample_finding() -> swissarmyhammer_validators::review::Finding {
        swissarmyhammer_validators::review::Finding {
            file: "src/payments.rs".to_string(),
            line: 8,
            validator: "duplication".to_string(),
            rule: Some("no-copy-paste".to_string()),
            claim: "copy-pasted block duplicates existing_total".to_string(),
            evidence: "`find_duplicates`: 0.94 match".to_string(),
            suggestion: Some("extract a shared helper".to_string()),
        }
    }

    /// A `Findings` event maps to a `notifications/message` log param carrying the
    /// FULL `Finding` JSON — never a progress param, and never truncated.
    #[test]
    fn findings_events_map_to_a_content_log_param_with_full_finding_json() {
        let event = ReviewProgressEvent::Findings {
            validator: "duplication".to_string(),
            findings: vec![sample_finding()],
        };

        // Content never produces a progress param — the wire counter must not move.
        let mut state = ReviewProgressState::default();
        assert!(
            review_progress_param(&mut state, &token("t"), &event).is_none(),
            "a Findings event must not map to a progress param"
        );

        let param = review_content_log_param(&event).expect("a Findings event maps to a log param");
        assert_eq!(param.logger.as_deref(), Some("review"));
        assert!(matches!(param.level, LoggingLevel::Info));
        assert_eq!(param.data["kind"], "review.findings");
        assert_eq!(param.data["validator"], "duplication");
        // The full Finding JSON is present — every load-bearing field, untruncated.
        let f = &param.data["findings"][0];
        assert_eq!(f["file"], "src/payments.rs");
        assert_eq!(f["line"], 8);
        assert_eq!(f["validator"], "duplication");
        assert_eq!(f["rule"], "no-copy-paste");
        assert_eq!(f["claim"], "copy-pasted block duplicates existing_total");
        assert_eq!(f["evidence"], "`find_duplicates`: 0.94 match");
    }

    /// A `Verdict` event maps to a `notifications/message` log param carrying the
    /// full finding, the confirmed flag, and the reason — never a progress param.
    #[test]
    fn verdict_events_map_to_a_content_log_param_with_full_finding_and_reason() {
        let event = ReviewProgressEvent::Verdict {
            finding: sample_finding(),
            confirmed: true,
            reason: "substantiated by the evidence".to_string(),
        };

        let mut state = ReviewProgressState::default();
        assert!(
            review_progress_param(&mut state, &token("t"), &event).is_none(),
            "a Verdict event must not map to a progress param"
        );

        let param = review_content_log_param(&event).expect("a Verdict event maps to a log param");
        assert_eq!(param.data["kind"], "review.verdict");
        assert_eq!(param.data["confirmed"], true);
        assert_eq!(param.data["reason"], "substantiated by the evidence");
        assert_eq!(
            param.data["finding"]["claim"],
            "copy-pasted block duplicates existing_total"
        );
        assert_eq!(param.data["finding"]["file"], "src/payments.rs");
    }

    /// The progress-tick variants carry no content — they route to
    /// notifications/progress, never notifications/message.
    #[test]
    fn progress_tick_events_have_no_content_log_param() {
        assert!(
            review_content_log_param(&ReviewProgressEvent::Planned { total_pairs: 1 }).is_none()
        );
        assert!(review_content_log_param(&ReviewProgressEvent::PairStarted {
            validator: "v".to_string(),
            file: "a.rs".to_string(),
        })
        .is_none());
        assert!(review_content_log_param(&ReviewProgressEvent::PairDone {
            validator: "v".to_string(),
            file: "a.rs".to_string(),
        })
        .is_none());
    }

    /// Drain everything currently buffered on `rx` without waiting.
    fn take_buffered(
        rx: &mut tokio::sync::mpsc::UnboundedReceiver<ProgressNotificationParam>,
    ) -> Vec<ProgressNotificationParam> {
        let mut out = Vec::new();
        while let Ok(param) = rx.try_recv() {
            out.push(param);
        }
        out
    }

    /// The keep-alive test's silence window. Chosen distinct from the
    /// production [`REVIEW_PROGRESS_KEEP_ALIVE_INTERVAL`] (10s) so the tests
    /// pin the schedule's *shape* (re-send after `keep_alive` of silence), not
    /// the production constant's value.
    const TEST_KEEP_ALIVE: std::time::Duration = std::time::Duration::from_secs(7);

    /// Let the paused-time runtime run every ready task, then advance the
    /// clock by `dur` and let timers fire.
    async fn advance(dur: std::time::Duration) {
        // Yield first so the mapping task processes any just-sent events
        // before the clock moves (paused time only advances when idle).
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(dur).await;
        for _ in 0..10 {
            tokio::task::yield_now().await;
        }
    }

    /// With no engine event for longer than the keep-alive interval, the
    /// mapping re-sends the latest param verbatim — and keeps re-sending it
    /// every interval while the silence lasts.
    #[tokio::test(start_paused = true)]
    async fn keep_alive_resends_the_latest_param_during_engine_silence() {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(run_review_progress_mapping(
            event_rx,
            param_tx,
            None,
            token("ka"),
            TEST_KEEP_ALIVE,
        ));

        event_tx
            .send(ReviewProgressEvent::Planned { total_pairs: 3 })
            .unwrap();
        advance(std::time::Duration::ZERO).await;
        let initial = take_buffered(&mut param_rx);
        assert_eq!(initial.len(), 1, "the event maps to one param");

        // One full silence window: the latest param is re-sent unchanged.
        advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
        let first_tick = take_buffered(&mut param_rx);
        assert_eq!(first_tick.len(), 1, "one keep-alive per silence window");
        assert_eq!(first_tick[0].progress, initial[0].progress);
        assert_eq!(first_tick[0].total, initial[0].total);
        assert_eq!(first_tick[0].message, initial[0].message);

        // The silence continues: another window, another identical re-send.
        advance(TEST_KEEP_ALIVE + std::time::Duration::from_millis(1)).await;
        let second_tick = take_buffered(&mut param_rx);
        assert_eq!(second_tick.len(), 1, "keep-alives repeat while silent");
        assert_eq!(second_tick[0].progress, initial[0].progress);
    }

    /// Before any engine event exists there is nothing to re-send: the timer
    /// stays disarmed no matter how long the run takes to produce its first
    /// event.
    #[tokio::test(start_paused = true)]
    async fn keep_alive_stays_disarmed_before_the_first_event() {
        let (_event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(run_review_progress_mapping(
            event_rx,
            param_tx,
            None,
            token("disarmed"),
            TEST_KEEP_ALIVE,
        ));

        advance(TEST_KEEP_ALIVE * 6).await;
        assert!(
            take_buffered(&mut param_rx).is_empty(),
            "no event yet means nothing to re-send"
        );
    }

    /// Every engine event restarts the silence window — a steadily streaming
    /// run never produces keep-alive duplicates.
    #[tokio::test(start_paused = true)]
    async fn keep_alive_window_resets_on_every_event() {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
        tokio::spawn(run_review_progress_mapping(
            event_rx,
            param_tx,
            None,
            token("reset"),
            TEST_KEEP_ALIVE,
        ));

        // Three events spaced just under the window: no tick ever fires.
        let just_under = TEST_KEEP_ALIVE - std::time::Duration::from_secs(1);
        for _ in 0..3 {
            event_tx
                .send(ReviewProgressEvent::FileScoped {
                    file: "src/streaming.rs".to_string(),
                })
                .unwrap();
            advance(just_under).await;
        }
        assert_eq!(
            take_buffered(&mut param_rx).len(),
            3,
            "steady streaming maps 1:1 with no keep-alive duplicates"
        );

        // Then real silence: the tick fires once the full window elapses.
        advance(TEST_KEEP_ALIVE).await;
        assert_eq!(
            take_buffered(&mut param_rx).len(),
            1,
            "the window re-arms from the last event"
        );
    }

    /// Dropping the engine sender ends the mapping (and with it the ticks):
    /// a finished run cannot keep emitting keep-alives forever.
    #[tokio::test(start_paused = true)]
    async fn keep_alive_stops_when_the_engine_sender_drops() {
        let (event_tx, event_rx) = tokio::sync::mpsc::unbounded_channel();
        let (param_tx, mut param_rx) = tokio::sync::mpsc::unbounded_channel();
        let mapping = tokio::spawn(run_review_progress_mapping(
            event_rx,
            param_tx,
            None,
            token("done"),
            TEST_KEEP_ALIVE,
        ));

        event_tx
            .send(ReviewProgressEvent::Planned { total_pairs: 1 })
            .unwrap();
        advance(std::time::Duration::ZERO).await;
        drop(event_tx);
        advance(TEST_KEEP_ALIVE * 3).await;

        assert!(mapping.is_finished(), "the mapping ends with the engine");
        // Only the one mapped event ever reached the wire side.
        assert_eq!(take_buffered(&mut param_rx).len(), 1);
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
        let (sender, drain) = bridge.into_parts();

        sender
            .send(ReviewProgressEvent::Planned { total_pairs: 1 })
            .unwrap();
        // A content event on the sink path must NOT reach the progress sink — the
        // sink contract is progress params only. It is dropped (no peer here), so
        // it neither becomes a sink param nor moves the wire counter.
        sender
            .send(ReviewProgressEvent::Findings {
                validator: "v".to_string(),
                findings: vec![],
            })
            .unwrap();
        sender
            .send(ReviewProgressEvent::PairDone {
                validator: "v".to_string(),
                file: "src/a.rs".to_string(),
            })
            .unwrap();

        // Dropping the engine's sender winds the bridge down; awaiting the
        // drain proves every buffered notification flushed first.
        drop(sender);
        drain.await.expect("drain joins cleanly");

        let mut got = Vec::new();
        while let Ok(param) = sink_rx.try_recv() {
            got.push(param);
        }
        assert_eq!(
            got.len(),
            2,
            "only the two progress events reach the sink; the content event does not: {got:#?}"
        );
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
        )
        .expect("a Planned event maps to a progress param");
        assert_eq!(first.total, Some(2.0));

        for file in ["src/a.rs", "src/b.rs"] {
            let _ = review_progress_param(
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
        )
        .expect("a Planned event maps to a progress param");
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
    fn a_majority_failed_review_is_never_refused_now_returned_with_the_incomplete_banner() {
        // The calcutron symptom: every fan-out task failed. There is no retry —
        // the run's report is returned as-is, never refused as a tool error;
        // `synthesize` already stamps the loud INCOMPLETE banner so an all-failed
        // run cannot be mistaken for a clean pass.
        let report = report_with_tally(60, 60);
        assert!(
            report.markdown().contains("results are INCOMPLETE"),
            "an all-failed report must render the INCOMPLETE banner: {}",
            report.markdown()
        );
        assert_eq!(report.counts().tasks_attempted(), 60);
        assert_eq!(report.counts().tasks_failed(), 60);
    }

    #[test]
    fn a_majority_failed_review_report_carries_the_failure_tally() {
        // A majority (not all) failing is the same "no retry, return flagged"
        // contract — the threshold that used to gate the refusal no longer
        // matters at all: every failure rate is returned.
        let report = report_with_tally(10, 7);
        assert!(
            report.markdown().contains("results are INCOMPLETE"),
            "a majority-failed report must render the INCOMPLETE banner: {}",
            report.markdown()
        );
        assert_eq!(report.counts().tasks_attempted(), 10);
        assert_eq!(report.counts().tasks_failed(), 7);
    }

    #[test]
    fn a_minority_failed_review_report_still_carries_the_incomplete_banner() {
        // A minority of tasks failed (1 of 10) — the report is returned with the
        // gap flagged, exactly as a majority/all-failed run is: there is no
        // separate threshold behavior left at this boundary.
        let report = report_with_tally(10, 1);
        assert!(
            report.markdown().contains("results are INCOMPLETE"),
            "any non-zero failure must render the INCOMPLETE banner: {}",
            report.markdown()
        );
        assert_eq!(report.counts().tasks_failed(), 1);
    }

    #[test]
    fn a_fully_successful_review_report_carries_no_incomplete_banner() {
        let report = report_with_tally(8, 0);
        assert!(
            !report.markdown().contains("INCOMPLETE"),
            "a clean run must not render the banner: {}",
            report.markdown()
        );
        assert_eq!(report.counts().tasks_failed(), 0);
    }

    #[test]
    fn a_run_that_attempted_no_tasks_carries_no_incomplete_banner() {
        // An empty diff attempts no fan-out tasks — there is no failure rate to
        // speak of, so no banner renders.
        let report = report_with_tally(0, 0);
        assert!(!report.markdown().contains("INCOMPLETE"));
        assert_eq!(report.counts().tasks_attempted(), 0);
        assert_eq!(report.counts().tasks_failed(), 0);
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
            Err::<Arc<dyn model_embedding::TextEmbedder>, EmbedderError>(EmbedderError::Load(
                "load failed".to_string(),
            ))
        })
        .await;
        assert!(failed.is_err(), "the failed init surfaces as an error");

        let retried = shared_embedder(&cell, || async { Ok(mock()) }).await;
        assert!(
            retried.is_ok(),
            "a failed init must not poison the cache; a later init succeeds"
        );
    }

    /// Encapsulating `ReviewResponse`/`ReviewCountsView` must not change the
    /// serialized wire shape: the same top-level keys and count keys as the
    /// public-field era, values readable back through the getters.
    #[test]
    fn review_response_wire_shape_and_getters_survive_encapsulation() {
        let response = ReviewResponse::from(report_with_tally(10, 1));

        let json = serde_json::to_value(&response).expect("serializes");
        assert!(json["markdown"].is_string(), "markdown key present: {json}");
        for key in ["findings", "confirmed", "refuted", "attempted", "failed"] {
            assert!(json["counts"][key].is_u64(), "counts.{key} present: {json}");
        }
        assert_eq!(json["counts"]["attempted"], serde_json::json!(10));
        assert_eq!(json["counts"]["failed"], serde_json::json!(1));

        // Getters read the same values the wire carries.
        assert!(response.markdown().starts_with("## Review Findings"));
        assert_eq!(response.counts().attempted(), 10);
        assert_eq!(response.counts().failed(), 1);
        assert_eq!(response.counts().findings(), 0);

        // The counts view is a value type: Copy + Eq hold.
        let copied = *response.counts();
        assert_eq!(copied, *response.counts());
    }
}
