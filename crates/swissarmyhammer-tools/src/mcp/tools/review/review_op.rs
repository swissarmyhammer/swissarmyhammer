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
use tokio::sync::broadcast;

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

/// The default embedder factory: load the platform embedder.
///
/// `swissarmyhammer_embedding::Embedder::default()` resolves the default model;
/// the probe runner needs it *loaded*, so this awaits the load before handing it
/// back.
pub fn default_embedder_factory() -> EmbedderFactory {
    Arc::new(|| {
        Box::pin(async {
            use model_embedding::TextEmbedder as _;
            let embedder = swissarmyhammer_embedding::Embedder::default()
                .await
                .map_err(|e| format!("failed to resolve embedder: {e}"))?;
            embedder
                .load()
                .await
                .map_err(|e| format!("failed to load embedder: {e}"))?;
            Ok(Arc::new(embedder) as Arc<dyn model_embedding::TextEmbedder>)
        })
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
    /// The optional validator-subset modifier (currently advisory; the engine
    /// loads the full matching set — subsetting is a later refinement).
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
    tokio::task::spawn_blocking(move || {
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
    let _ = &request.validators; // honored by scope/match; subset is a later refinement.

    let loader = load_rules().map_err(|e| format!("failed to load validators: {e}"))?;
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
