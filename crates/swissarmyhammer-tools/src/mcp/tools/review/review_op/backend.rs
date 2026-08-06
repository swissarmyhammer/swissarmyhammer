//! The review tool's injected backends: the ACP agent seam, the embedder
//! seam with its process-global cache, and the pool concurrency policy.
//!
//! The tool resolves its agent and embedder through factories rather than
//! constructing them inline: the production server injects the configured
//! Claude backend and the loaded platform embedder, while tests inject a
//! scripted agent and a deterministic mock.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use agent_client_protocol::schema::SessionNotification;
use agent_client_protocol::{Client, DynConnectTo};
use tokio::sync::{broadcast, OnceCell};

use swissarmyhammer_embedding::{DownloadEvent, DownloadObserver};
use swissarmyhammer_validators::review::{ReviewProgressEvent, ReviewProgressSender};
use swissarmyhammer_validators::PoolConfig;

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
/// Claude backend from the session's `ChatModelConfig`, while tests inject a
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

/// Process-global cache of the loaded default embedder.
///
/// The default embedder is the platform `qwen-embedding` model — a
/// multi-hundred-MB-to-GB load. Building a fresh one per review run wastes that
/// load and, before the [`REVIEW_PIPELINE_GATE`] cap, multiplied the model's
/// resident footprint across concurrent runs. Caching it here loads it once and
/// shares one `Arc` across every default-factory run. Sharing is safe because
/// review pipelines are serialized by the gate and a run embeds sequentially, so
/// the shared model is never driven concurrently.
pub(super) static DEFAULT_EMBEDDER: OnceCell<Arc<dyn model_embedding::TextEmbedder>> =
    OnceCell::const_new();

/// Return the cached embedder from `cell`, initializing it once via `init`.
///
/// A thin wrapper over [`OnceCell::get_or_try_init`] that hands back an owned
/// `Arc` clone (the cache keeps its own). A failed `init` is *not* stored, so a
/// later call retries the load rather than caching the failure. Factored out so
/// the share-once contract is unit-testable without loading the real model.
pub(super) async fn shared_embedder<F, Fut>(
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
pub(super) fn review_download_observer(
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
pub(super) fn pool_config_for(backend: Option<&str>, concurrency: Option<usize>) -> PoolConfig {
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
pub(super) const DEFAULT_REMOTE_WORKERS: usize = 16;
