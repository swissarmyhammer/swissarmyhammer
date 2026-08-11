//! The three pipeline ops of the `review` tool: `review file/working/sha`.
//!
//! These ops are a thin dispatch shim. Each maps its op + args onto a
//! [`Scope`](swissarmyhammer_validators::review::Scope), resolves the engine's
//! inputs from the MCP session/work-dir — the repo root (CWD), the full validator
//! loader, the code_context index connection, the embedder, and a live ACP agent
//! — and calls the engine's
//! [`run_review_over_agent`](swissarmyhammer_validators::review::run_review_over_agent)
//! driver, returning the [`ReviewReport`]. No pipeline logic lives here.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use rusqlite::Connection;
use tokio::sync::Semaphore;

use swissarmyhammer_validators::review::{
    run_review_over_agent, FleetConfig, ReviewProgressSender, ReviewReport, Scope,
};
use swissarmyhammer_validators::{load_rules, AvpError};

mod backend;
mod progress;
mod response;

use backend::*;
pub use backend::{
    default_embedder_factory, AgentFactory, AgentHandle, EmbedderError, EmbedderFactory,
};
#[cfg_attr(
    not(test),
    allow(
        unused_imports,
        reason = "the progress internals are re-imported for the unit tests"
    )
)]
use progress::*;
pub use progress::{spawn_review_progress_bridge, ReviewProgressBridge};
pub use response::{ReviewCountsView, ReviewResponse};

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
#[derive(Debug)]
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
    /// The rendered-prompt batch budget in BYTES, from the `batch_size`
    /// modifier. `None` defers to [`FleetConfig`]'s default, which is the
    /// agent's prompt cap; any value above that cap is clamped down to it.
    /// Applies to every scope.
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
    pub fn with_backend<S: Into<String>>(mut self, backend: Option<S>) -> Self {
        self.backend = backend.map(Into::into);
        self
    }

    /// Scope the fan-out to just these validators; empty means every matching
    /// validator.
    pub fn with_validators(
        mut self,
        validators: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.validators = validators.into_iter().map(Into::into).collect();
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
    // The project validator layer belongs to the repository under review, which
    // `repo_path` names — never the process current directory.
    let mut loader = load_rules(Some(&repo_path)).map_err(ReviewError::ValidatorLoad)?;
    // Honor the `validators` subset modifier: when the caller named a subset,
    // scope the fan-out to just those validators. Empty means "all matching".
    let validator_subset: Vec<&str> = request.validators.iter().map(String::as_str).collect();
    loader.retain_rulesets(&validator_subset);
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
    // FleetConfig default (the agent's prompt cap). `FleetConfig::new` clamps a
    // caller-supplied value to that cap, so no modifier can ask for a prompt
    // the agent would reject.
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

/// Open an owned read-only connection to the workspace's code_context index.
///
/// The engine's probe runner takes a `&Connection` it holds across `await`s, so
/// the tool owns a dedicated connection for the run rather than borrowing the
/// workspace's shared (std-`Mutex`-guarded) write handle.
///
/// `repo_path` is trusted input here, not request data: it is never a `review`
/// tool argument (the op only pulls `backend`/`validators`/`batch_size`/scope
/// out of the call's JSON args — see [`ReviewTool::execute_review`]'s
/// `resolve_repo_path`), so there is no `..`/absolute-path string arriving
/// from a caller for this function to validate. It is the server's own
/// `ToolContext::working_dir` (set once, at MCP-server/session start from the
/// launch `working_dir` or ACP session cwd — never per-call), walked up to its
/// git root by `find_git_repository_root_from`. The only join performed below
/// appends the two hardcoded literal components `CONTEXT_DIR`/`DB_NAME`, so
/// there is no dynamic path segment for a traversal to smuggle in even if
/// `repo_path` were hostile.
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

#[cfg(test)]
mod tests;
