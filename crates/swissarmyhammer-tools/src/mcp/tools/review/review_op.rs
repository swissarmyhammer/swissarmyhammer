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
use rusqlite::Connection;
use serde::Serialize;
use tokio::sync::{broadcast, OnceCell, Semaphore};

use swissarmyhammer_validators::review::{run_review_over_agent, FleetConfig, ReviewReport, Scope};
use swissarmyhammer_validators::{load_rules, PoolConfig};

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
) -> Result<ReviewReport, String> {
    let mut loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
    // Honor the `validators` subset modifier: when the caller named a subset,
    // scope the fan-out to just those validators. Empty means "all matching".
    loader.retain_rulesets(&request.validators);
    let conn = open_index_connection(&repo_path)?;
    let embedder = embedder_factory().await?;

    let handle = agent_factory().await?;

    run_review_over_agent(
        handle.agent,
        handle.notification_rx,
        request.scope,
        &repo_path,
        &loader,
        &conn,
        embedder.as_ref(),
        pool_config_for(request.backend.as_deref(), request.concurrency),
        FleetConfig::default(),
        &now,
    )
    .await
    .map_err(|e| format!("review pipeline failed: {e}"))
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
/// markdown plus the per-severity / per-verdict counts.
#[derive(Debug, Serialize)]
pub struct ReviewResponse {
    /// The dated GFM `## Review Findings (...)` section.
    pub markdown: String,
    /// The per-severity / per-verdict tallies.
    pub counts: ReviewCountsView,
}

/// The serializable view of the engine's review counts.
#[derive(Debug, Serialize)]
pub struct ReviewCountsView {
    /// Confirmed blocker findings.
    pub blockers: usize,
    /// Confirmed warning findings.
    pub warnings: usize,
    /// Confirmed nit findings.
    pub nits: usize,
    /// Findings the verifier confirmed.
    pub confirmed: usize,
    /// Findings the verifier refuted.
    pub refuted: usize,
}

impl From<ReviewReport> for ReviewResponse {
    fn from(report: ReviewReport) -> Self {
        ReviewResponse {
            markdown: report.markdown,
            counts: ReviewCountsView {
                blockers: report.counts.blockers,
                warnings: report.counts.warnings,
                nits: report.counts.nits,
                confirmed: report.counts.confirmed,
                refuted: report.counts.refuted,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

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
