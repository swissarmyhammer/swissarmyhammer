use crate::chat_template::ChatTemplateEngine;
use crate::generation::GenerationHelper;
use crate::model::ModelManager;

use crate::types::{
    FinishReason, GenerationRequest, GenerationResponse, QueueConfig, QueueError, Session,
    StreamChunk,
};
use async_trait::async_trait;
use llama_common::async_utils;
use llama_cpp_2::context::LlamaContext;
use llama_cpp_2::model::LlamaModel;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use swissarmyhammer_common::Pretty;
use tokio::sync::{mpsc, oneshot, Mutex as TokioMutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::{debug, error, info, trace, warn};
use ulid::Ulid;

/// In-memory cache of session states for efficient multi-turn conversations
/// Maps session_id -> (state_bytes, message_count)
///
/// The state_bytes contain the complete llama.cpp context state including KV cache,
/// allowing us to restore a session without disk I/O.
type SessionStateCache = Arc<Mutex<HashMap<String, Vec<u8>>>>;

/// Check whether we have a cached KV-state for this session, and log the
/// resume/fresh-start decision. Returns true if the cache is usable.
fn check_and_log_session_cache(
    worker_id: usize,
    session: &Session,
    session_state_cache: &SessionStateCache,
) -> bool {
    let has_cached_state = {
        let cache = session_state_cache.lock().unwrap();
        cache.contains_key(&session.id.to_string())
    };
    let can_use_cache = has_cached_state && session.cached_message_count > 0;

    if can_use_cache {
        info!(
            "Worker {} continuing session {} from memory: {} cached messages, {} new messages to process",
            worker_id,
            session.id,
            session.cached_message_count,
            session.messages.len() - session.cached_message_count
        );
    } else {
        info!(
            "Worker {} starting new session {}: processing all {} messages",
            worker_id,
            session.id,
            session.messages.len()
        );
    }
    can_use_cache
}

/// Always render the FULL conversation — the restored KV cache will already
/// have the already-processed tokens, so llama.cpp only processes new ones.
fn render_session_prompt(
    worker_id: usize,
    chat_template: &ChatTemplateEngine,
    session: &Session,
    model: &LlamaModel,
    model_manager: &ModelManager,
) -> Result<String, QueueError> {
    info!(
        "Worker {} rendering full conversation: {} messages",
        worker_id,
        session.messages.len()
    );
    let prompt = chat_template
        .render_session_with_config(session, model, Some(model_manager.get_config()))
        .map_err(|e| {
            error!("Failed to render session prompt: {}", e);
            QueueError::WorkerError(format!("Template rendering failed: {}", e))
        })?;
    debug!(
        "Worker {} rendered prompt length: {} bytes",
        worker_id,
        prompt.len()
    );
    Ok(prompt)
}

/// Create the llama.cpp context for this inference request, with error wrapping.
fn create_session_context<'m>(
    model_manager: &ModelManager,
    model: &'m LlamaModel,
    session: &Session,
) -> Result<LlamaContext<'m>, QueueError> {
    model_manager
        .create_session_context(model, &session.id)
        .map_err(|e| {
            error!("Failed to create session context: {}", e);
            QueueError::WorkerError(format!("Session context creation failed: {}", e))
        })
}

/// Translate the KV-cache position into the `template_token_count` that
/// GenerationHelper expects (i.e. the *next* token position).
fn compute_template_token_count(worker_id: usize, kv_cache_position: i32) -> Option<usize> {
    if kv_cache_position < 0 {
        return None;
    }
    let next_position = (kv_cache_position + 1) as usize;
    info!(
        "Worker {} using token offset: {} tokens already in KV cache (position 0 to {})",
        worker_id, next_position, kv_cache_position
    );
    Some(next_position)
}

/// Build the `(prompt, llama_ctx)` pair needed for a streaming generation. On
/// either failure the error is pushed onto the stream and we return `None` so
/// the caller can exit early.
fn prepare_streaming_inference<'m>(
    chat_template: &ChatTemplateEngine,
    session: &Session,
    model: &'m LlamaModel,
    model_manager: &ModelManager,
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
) -> Option<(String, LlamaContext<'m>)> {
    let prompt = match render_streaming_prompt(chat_template, session, model, model_manager) {
        Ok(p) => p,
        Err(e) => {
            report_stream_error(stream_sender, "Template rendering failed", &e);
            return None;
        }
    };
    let ctx = match create_streaming_context(model_manager, model, session) {
        Ok(c) => c,
        Err(e) => {
            report_stream_error(stream_sender, "Session context creation failed", &e);
            return None;
        }
    };
    Some((prompt, ctx))
}

/// Render the session prompt for a streaming request. Matches the non-streaming
/// path's behaviour; errors are wrapped in the caller's preferred channel.
fn render_streaming_prompt(
    chat_template: &ChatTemplateEngine,
    session: &Session,
    model: &LlamaModel,
    model_manager: &ModelManager,
) -> Result<String, crate::types::TemplateError> {
    chat_template.render_session_with_config(session, model, Some(model_manager.get_config()))
}

/// Create the per-request llama.cpp context for streaming. Wraps the model
/// manager call so the caller can keep its error handling linear.
fn create_streaming_context<'m>(
    model_manager: &ModelManager,
    model: &'m LlamaModel,
    session: &Session,
) -> Result<LlamaContext<'m>, crate::types::ModelError> {
    model_manager.create_session_context(model, &session.id)
}

/// Push a worker-side error onto the streaming channel without ever blocking.
fn report_stream_error<E: std::fmt::Display>(
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
    context: &str,
    error: &E,
) {
    error!("Streaming error: {}: {}", context, error);
    let _ = stream_sender.try_send(Err(QueueError::WorkerError(format!(
        "{}: {}",
        context, error
    ))));
}

/// Block on the shared receiver until a request arrives or the channel closes.
/// Logs the close event and returns None so the caller can break its loop.
async fn recv_next_request(
    receiver: &Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
    worker_id: usize,
) -> Option<QueuedRequest> {
    let mut receiver = receiver.lock().await;
    match receiver.recv().await {
        Some(request) => Some(request),
        None => {
            info!("Worker {} shutting down - channel closed", worker_id);
            None
        }
    }
}

/// Handle a request whose cancellation token was fired before we got around to
/// processing it: reply with a cancellation error and bump the metric.
fn reject_cancelled_request(
    worker_id: usize,
    queued_request: QueuedRequest,
    queue_time: Duration,
    metrics: &QueueMetrics,
) {
    warn!(
        "Worker {} dropping cancelled request {} (queued for {:?})",
        worker_id, queued_request.id, queue_time
    );
    let _ = queued_request
        .response_sender
        .send(Err(QueueError::WorkerError(
            "Request cancelled".to_string(),
        )));
    metrics.record_request_cancelled();
}

/// Drop the oldest entries from the in-memory session state cache until it is
/// under the per-process limit (cpu_cores / 2, minimum 1). Callers must hold
/// the cache lock.
fn evict_oldest_session_states(worker_id: usize, cache: &mut HashMap<String, Vec<u8>>) {
    let cache_limit = std::thread::available_parallelism()
        .map(|n| (n.get() / 2).max(1))
        .unwrap_or(4);

    if cache.len() <= cache_limit {
        return;
    }

    // Simple approach: remove entries until we're at limit.
    // In production, would track access time for proper LRU.
    let to_remove: Vec<String> = cache
        .keys()
        .take(cache.len() - cache_limit)
        .cloned()
        .collect();
    for key in to_remove {
        cache.remove(&key);
    }
    info!(
        "Worker {} evicted old session states (limit: {}), now have {} cached",
        worker_id,
        cache_limit,
        cache.len()
    );
}

/// Lock-free counters describing the live state of a [`RequestQueue`].
///
/// Every counter is an atomic so workers and submitters can update them without
/// contending on a mutex. Call [`QueueMetrics::get_stats`] to take a
/// point-in-time snapshot.
#[derive(Debug, Default)]
pub struct QueueMetrics {
    /// Total number of requests ever submitted to the queue (including failures and cancels).
    pub total_requests: AtomicU64,
    /// Requests that completed successfully.
    pub completed_requests: AtomicU64,
    /// Requests that ended in a worker error.
    pub failed_requests: AtomicU64,
    /// Requests that were cancelled before or during processing.
    pub cancelled_requests: AtomicU64,
    /// Number of requests currently queued or in flight.
    pub current_queue_size: AtomicUsize,
    /// Sum of processing wall-time (milliseconds) across all completed requests.
    pub total_processing_time_ms: AtomicU64,
    /// Total tokens generated across all completed requests.
    pub total_tokens_generated: AtomicU64,
    /// Largest `current_queue_size` value observed since startup.
    pub peak_queue_size: AtomicUsize,
    /// Throughput (tokens/second) measured on the most recent completed request.
    pub last_throughput_tokens_per_second: AtomicU64,
}

impl QueueMetrics {
    /// Construct a fresh metrics block with every counter at zero.
    pub fn new() -> Self {
        Self {
            total_requests: AtomicU64::new(0),
            completed_requests: AtomicU64::new(0),
            failed_requests: AtomicU64::new(0),
            cancelled_requests: AtomicU64::new(0),
            current_queue_size: AtomicUsize::new(0),
            total_processing_time_ms: AtomicU64::new(0),
            total_tokens_generated: AtomicU64::new(0),
            peak_queue_size: AtomicUsize::new(0),
            last_throughput_tokens_per_second: AtomicU64::new(0),
        }
    }

    /// Increment submission counters and update `peak_queue_size` if we just
    /// surpassed the previous high-water mark.
    pub fn record_request_submitted(&self) {
        self.total_requests.fetch_add(1, Ordering::Relaxed);
        let current_size = self.current_queue_size.fetch_add(1, Ordering::Relaxed) + 1;

        // Update peak queue size if necessary
        let mut peak = self.peak_queue_size.load(Ordering::Relaxed);
        while current_size > peak {
            match self.peak_queue_size.compare_exchange_weak(
                peak,
                current_size,
                Ordering::Relaxed,
                Ordering::Relaxed,
            ) {
                Ok(_) => break,
                Err(actual) => peak = actual,
            }
        }
    }

    /// Record a successful completion: updates totals, processing time, and
    /// the rolling throughput measurement.
    pub fn record_request_completed(&self, processing_time: Duration, tokens_generated: u32) {
        self.completed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);

        let processing_ms = processing_time.as_millis() as u64;
        self.total_processing_time_ms
            .fetch_add(processing_ms, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(tokens_generated as u64, Ordering::Relaxed);

        // Calculate and store current throughput (tokens per second)
        if let Some(throughput) = (tokens_generated as u64 * 1000).checked_div(processing_ms) {
            self.last_throughput_tokens_per_second
                .store(throughput, Ordering::Relaxed);
        }
    }

    /// Record a failed request and decrement the live queue-size counter.
    pub fn record_request_failed(&self) {
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

    /// Record a cancelled request and decrement the live queue-size counter.
    pub fn record_request_cancelled(&self) {
        self.cancelled_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

    /// Take a consistent snapshot of all counters as a plain `QueueStats`.
    pub fn get_stats(&self) -> QueueStats {
        QueueStats {
            total_requests: self.total_requests.load(Ordering::Relaxed),
            completed_requests: self.completed_requests.load(Ordering::Relaxed),
            failed_requests: self.failed_requests.load(Ordering::Relaxed),
            cancelled_requests: self.cancelled_requests.load(Ordering::Relaxed),
            current_queue_size: self.current_queue_size.load(Ordering::Relaxed),
            average_processing_time_ms: {
                let total_time = self.total_processing_time_ms.load(Ordering::Relaxed);
                let completed = self.completed_requests.load(Ordering::Relaxed);
                total_time.checked_div(completed).unwrap_or(0)
            },
            total_tokens_generated: self.total_tokens_generated.load(Ordering::Relaxed),
            peak_queue_size: self.peak_queue_size.load(Ordering::Relaxed),
            current_throughput_tps: self
                .last_throughput_tokens_per_second
                .load(Ordering::Relaxed),
        }
    }
}

/// Point-in-time snapshot of the counters in a [`QueueMetrics`].
#[derive(Debug, Clone)]
pub struct QueueStats {
    /// See [`QueueMetrics::total_requests`].
    pub total_requests: u64,
    /// See [`QueueMetrics::completed_requests`].
    pub completed_requests: u64,
    /// See [`QueueMetrics::failed_requests`].
    pub failed_requests: u64,
    /// See [`QueueMetrics::cancelled_requests`].
    pub cancelled_requests: u64,
    /// See [`QueueMetrics::current_queue_size`].
    pub current_queue_size: usize,
    /// Mean per-request processing time in milliseconds (0 if no completions yet).
    pub average_processing_time_ms: u64,
    /// See [`QueueMetrics::total_tokens_generated`].
    pub total_tokens_generated: u64,
    /// See [`QueueMetrics::peak_queue_size`].
    pub peak_queue_size: usize,
    /// Most recently observed throughput in tokens/second.
    pub current_throughput_tps: u64,
}

/// The inference half of the queue, abstracted behind a trait so the worker
/// loop's lifecycle (dequeue → run a turn → release the worker → record metrics)
/// can be exercised deterministically without a live llama.cpp model.
///
/// The single production implementation is [`ModelManagerExecutor`], which runs
/// the real `with_model(...)` + `GenerationHelper` inference path byte-for-byte.
/// Tests substitute a scripted executor so they can drive every turn outcome
/// (normal / EOS / max-tokens / context-full / error / cancel) and assert that
/// the worker is always released afterwards — the regression guard for the
/// "Queue is full on retry" bug.
///
/// Each method performs only the inference itself: it returns the outcome (or,
/// for streaming, pushes chunks onto the supplied sender) and leaves
/// metric-recording and response relay to the worker, exactly as the original
/// inline dispatch did.
#[async_trait]
pub(crate) trait QueueExecutor: Send + Sync {
    /// Run a batch (non-streaming) turn and return the full response, or a
    /// queue-level error if inference failed.
    async fn execute_batch(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
    ) -> Result<GenerationResponse, QueueError>;

    /// Run a streaming turn, pushing `StreamChunk`s onto `stream_sender` as they
    /// are produced. Returns `Ok(())` when the turn finished (the worker is then
    /// released regardless of outcome) or an error to relay onto the stream.
    async fn execute_streaming(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
        stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
    ) -> Result<(), QueueError>;
}

/// Production [`QueueExecutor`]: drives the real llama.cpp model through the
/// `ModelManager::with_model` borrow and `GenerationHelper`. This carries the
/// exact inference logic that previously lived inline in
/// `RequestQueue::dispatch_{batch,streaming}_request`.
pub(crate) struct ModelManagerExecutor {
    model_manager: Arc<ModelManager>,
    chat_template: Arc<ChatTemplateEngine>,
    session_config: crate::types::SessionConfig,
    session_state_cache: SessionStateCache,
}

impl ModelManagerExecutor {
    /// Build the production executor from the shared model and queue state.
    fn new(
        model_manager: Arc<ModelManager>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
    ) -> Self {
        Self {
            model_manager,
            chat_template,
            session_config,
            session_state_cache,
        }
    }
}

#[async_trait]
impl QueueExecutor for ModelManagerExecutor {
    async fn execute_batch(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
    ) -> Result<GenerationResponse, QueueError> {
        if !self.model_manager.is_loaded().await {
            return Err(QueueError::WorkerError("Model not loaded".to_string()));
        }
        let request_id = queued_request.id.clone();
        let start_time = Instant::now();
        let result = self
            .model_manager
            .with_model(|model| {
                RequestQueue::process_batch_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    &self.model_manager,
                    &queued_request.cancellation_token,
                    &self.chat_template,
                    &self.session_config,
                    &self.session_state_cache,
                )
            })
            .await;
        let _ = start_time;
        match result {
            Ok(inner) => inner,
            Err(model_error) => Err(QueueError::WorkerError(format!(
                "Model error: {}",
                model_error
            ))),
        }
    }

    async fn execute_streaming(
        &self,
        worker_id: usize,
        queued_request: &QueuedRequest,
        stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
    ) -> Result<(), QueueError> {
        if !self.model_manager.is_loaded().await {
            return Err(QueueError::WorkerError("Model not loaded".to_string()));
        }
        let request_id = queued_request.id.clone();
        let result = self
            .model_manager
            .with_model(|model| {
                RequestQueue::process_streaming_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    &self.model_manager,
                    stream_sender.clone(),
                    &queued_request.cancellation_token,
                    &self.chat_template,
                )
            })
            .await;
        match result {
            Ok(inner) => inner,
            Err(model_error) => Err(QueueError::WorkerError(format!(
                "Model error: {}",
                model_error
            ))),
        }
    }
}

/// Envelope carrying a single request from `submit_request` to a worker task.
#[derive(Debug)]
pub struct QueuedRequest {
    /// Per-request ULID used for logging/tracing.
    pub id: String,
    /// The user-visible generation request.
    pub request: GenerationRequest,
    /// Session that owns this request (messages, KV cache identity, etc.).
    pub session: Session,
    /// oneshot channel for the batch response.
    pub response_sender: oneshot::Sender<Result<GenerationResponse, QueueError>>,
    /// Optional streaming channel. When set, the request is dispatched via the
    /// streaming code path instead of the batch path.
    pub stream_sender: Option<mpsc::Sender<Result<StreamChunk, QueueError>>>,
    /// When the request was enqueued (used for queue-time metrics).
    pub submitted_at: Instant,
    /// Token used by callers to cancel this specific request.
    pub cancellation_token: CancellationToken,
}

/// Bounded, multi-worker queue that routes `QueuedRequest`s through the
/// llama.cpp model and streams responses back to the caller.
pub struct RequestQueue {
    sender: Option<mpsc::Sender<QueuedRequest>>,
    worker_handles: Vec<JoinHandle<()>>,
    metrics: Arc<QueueMetrics>,
    _chat_template: Arc<ChatTemplateEngine>,
    _session_config: crate::types::SessionConfig,
    /// Track active requests by session ID for cancellation support
    active_requests: Arc<TokioMutex<HashMap<crate::types::SessionId, CancellationToken>>>,
    /// Kept alive for duration of queue - workers hold references to this cache
    #[allow(dead_code)]
    session_state_cache: SessionStateCache,
}

impl RequestQueue {
    /// Build a new `RequestQueue`, spawning `config.worker_threads` worker
    /// tasks that each share the provided model manager. Workers stay alive
    /// until the queue is dropped or `shutdown` is called.
    pub fn new(
        model_manager: Arc<ModelManager>,
        config: QueueConfig,
        session_config: crate::types::SessionConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let metrics = Arc::new(QueueMetrics::new());
        // The chat template engine needs the right strategy so it renders
        // tools and parses tool calls in the format the loaded model was
        // trained on. We derive the identifier from the model config in the
        // same way `AgentServer::initialize` does — see
        // `crate::agent::model_identifier_for_strategy`. Without this the
        // queue's engine stays strategy-less and silently falls back to the
        // legacy HashMap parsers.
        let model_identifier =
            crate::agent::model_identifier_for_strategy(model_manager.get_config());
        let chat_template = Arc::new(ChatTemplateEngine::with_model_strategy(&model_identifier));
        let session_state_cache: SessionStateCache = Arc::new(Mutex::new(HashMap::new()));

        let executor: Arc<dyn QueueExecutor> = Arc::new(ModelManagerExecutor::new(
            model_manager.clone(),
            chat_template.clone(),
            session_config.clone(),
            session_state_cache.clone(),
        ));

        Self::assemble(
            sender,
            receiver,
            config,
            metrics,
            chat_template,
            session_config,
            session_state_cache,
            executor,
        )
    }

    /// Shared constructor body: spawn the workers against `executor` and build
    /// the `RequestQueue`. Both the production [`RequestQueue::new`] and the
    /// test-only `with_executor` constructor funnel through here so worker setup
    /// stays in one place.
    #[allow(clippy::too_many_arguments)]
    fn assemble(
        sender: mpsc::Sender<QueuedRequest>,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        config: QueueConfig,
        metrics: Arc<QueueMetrics>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
        executor: Arc<dyn QueueExecutor>,
    ) -> Self {
        let worker_handles = Self::spawn_workers(&config, &receiver, &metrics, &executor);

        info!(
            "RequestQueue initialized with {} workers, max queue size: {}",
            config.worker_threads, config.max_queue_size
        );

        Self {
            sender: Some(sender),
            worker_handles,
            metrics,
            _chat_template: chat_template,
            _session_config: session_config,
            active_requests: Arc::new(TokioMutex::new(HashMap::new())),
            session_state_cache,
        }
    }

    /// Enqueue a batch request without awaiting its response, returning only the
    /// enqueue outcome. Used by capacity tests to fill the bounded channel and
    /// observe `QueueError::Full` at — and only at — capacity, exercising
    /// [`RequestQueue::enqueue_request`] directly.
    #[cfg(test)]
    fn try_enqueue_for_test(&self, session: &Session) -> Result<(), QueueError> {
        let (response_sender, _response_receiver) = oneshot::channel();
        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request: GenerationRequest {
                session_id: session.id,
                max_tokens: Some(8),
                temperature: Some(0.0),
                top_p: None,
                stop_tokens: Vec::new(),
                stopping_config: None,
            },
            session: session.clone(),
            response_sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token: CancellationToken::new(),
        };
        self.metrics.record_request_submitted();
        self.enqueue_request(queued_request)
    }

    /// Build a `RequestQueue` whose workers run turns through a caller-supplied
    /// [`QueueExecutor`] instead of the production model-backed executor.
    ///
    /// This is the seam the queue-lifecycle tests use to drive deterministic
    /// turn outcomes (normal / EOS / max-tokens / context-full / error / cancel)
    /// without a live llama.cpp model, exercising the real worker loop, release
    /// invariants, FIFO ordering, backpressure, and queue-full handling.
    #[cfg(test)]
    fn with_executor(config: QueueConfig, executor: Arc<dyn QueueExecutor>) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let metrics = Arc::new(QueueMetrics::new());
        let chat_template = Arc::new(ChatTemplateEngine::new());
        let session_state_cache: SessionStateCache = Arc::new(Mutex::new(HashMap::new()));
        let session_config = crate::types::SessionConfig::default();

        Self::assemble(
            sender,
            receiver,
            config,
            metrics,
            chat_template,
            session_config,
            session_state_cache,
            executor,
        )
    }

    /// Spawn the configured number of worker tasks, cloning the shared receiver,
    /// metrics, and executor each iteration. Kept out of `new` so the
    /// constructor stays concise.
    fn spawn_workers(
        config: &QueueConfig,
        receiver: &Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        metrics: &Arc<QueueMetrics>,
        executor: &Arc<dyn QueueExecutor>,
    ) -> Vec<JoinHandle<()>> {
        (0..config.worker_threads)
            .map(|worker_id| {
                let receiver = receiver.clone();
                let metrics = metrics.clone();
                let executor = executor.clone();
                tokio::spawn(async move {
                    Self::worker_loop(worker_id, receiver, metrics, executor).await;
                })
            })
            .collect()
    }

    /// Submit a batch (non-streaming) generation request and await the full
    /// `GenerationResponse`. Returns [`QueueError::Full`] if the queue is at
    /// capacity and [`QueueError::WorkerError`] if the worker fails.
    pub async fn submit_request(
        &self,
        request: GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, QueueError> {
        let (response_sender, response_receiver) = oneshot::channel();
        let cancellation_token = CancellationToken::new();
        let session_id = request.session_id;

        self.track_cancellation_token(session_id, cancellation_token.clone())
            .await;

        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request,
            session: session.clone(),
            response_sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token,
        };
        debug!("Submitting request to queue: {}", queued_request.id);
        self.metrics.record_request_submitted();

        self.enqueue_request(queued_request)?;

        let result = response_receiver
            .await
            .map_err(|_| QueueError::WorkerError("Response channel closed".to_string()))?;
        self.active_requests.lock().await.remove(&session_id);
        result
    }

    /// Register a cancellation token for this session so concurrent cancels can
    /// find it.
    async fn track_cancellation_token(
        &self,
        session_id: crate::types::SessionId,
        token: CancellationToken,
    ) {
        let mut active = self.active_requests.lock().await;
        active.insert(session_id, token);
    }

    /// Push the fully-built request onto the worker queue, translating channel
    /// errors into queue-level errors and updating metrics on failure.
    fn enqueue_request(&self, queued_request: QueuedRequest) -> Result<(), QueueError> {
        let sender = self.sender.as_ref().ok_or_else(|| {
            warn!("Queue is shutting down, rejecting request");
            self.metrics.record_request_failed();
            QueueError::WorkerError("Queue is shutting down".to_string())
        })?;
        if sender.try_send(queued_request).is_err() {
            warn!("Queue is full, rejecting request");
            self.metrics.record_request_failed();
            return Err(QueueError::Full);
        }
        Ok(())
    }

    /// Submit a streaming generation request and receive an `mpsc::Receiver`
    /// that will emit `StreamChunk`s (or errors) as the model produces them.
    /// The receiver closes once generation completes.
    pub async fn submit_streaming_request(
        &self,
        request: GenerationRequest,
        session: &Session,
    ) -> Result<mpsc::Receiver<Result<StreamChunk, QueueError>>, QueueError> {
        let (response_sender, _) = oneshot::channel();
        let (stream_sender, stream_receiver) = mpsc::channel(100);

        let cancellation_token = CancellationToken::new();

        // Track the cancellation token for this session
        {
            let mut active = self.active_requests.lock().await;
            active.insert(request.session_id, cancellation_token.clone());
        }

        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request,
            session: session.clone(),
            response_sender,
            stream_sender: Some(stream_sender),
            submitted_at: Instant::now(),
            cancellation_token: cancellation_token.clone(),
        };

        debug!(
            "Submitting streaming request to queue: {}",
            queued_request.id
        );

        // Record request submission
        self.metrics.record_request_submitted();

        // Try to send to queue
        let sender = self.sender.as_ref().ok_or_else(|| {
            warn!("Queue is shutting down, rejecting streaming request");
            self.metrics.record_request_failed();
            QueueError::WorkerError("Queue is shutting down".to_string())
        })?;

        if sender.try_send(queued_request).is_err() {
            warn!("Queue is full, rejecting streaming request");
            self.metrics.record_request_failed(); // Adjust queue size back down
            return Err(QueueError::Full);
        }

        Ok(stream_receiver)
    }

    /// Return the number of requests currently queued or in flight, read from
    /// the live metrics counter.
    pub fn get_queue_size(&self) -> usize {
        // Use metrics for more accurate queue size
        self.metrics.current_queue_size.load(Ordering::Relaxed)
    }

    /// Cancel an active request for a session
    ///
    /// This triggers the cancellation token for any active request associated with
    /// the given session ID. If the session has an active request, the generation
    /// will be cancelled gracefully.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The session ID whose request should be cancelled
    ///
    /// # Returns
    ///
    /// * `true` if an active request was found and cancelled
    /// * `false` if no active request was found for this session
    pub async fn cancel_session(&self, session_id: &crate::types::SessionId) -> bool {
        let mut active = self.active_requests.lock().await;
        if let Some(token) = active.remove(session_id) {
            debug!("Cancelling request for session: {}", session_id);
            token.cancel();
            true
        } else {
            debug!("No active request found for session: {}", session_id);
            false
        }
    }

    /// Convenience shortcut for `self.metrics.get_stats()` — returns a
    /// consistent snapshot of the queue's counters.
    pub fn get_stats(&self) -> QueueStats {
        self.metrics.get_stats()
    }

    async fn worker_loop(
        worker_id: usize,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        metrics: Arc<QueueMetrics>,
        executor: Arc<dyn QueueExecutor>,
    ) {
        info!("Worker {} started", worker_id);
        while let Some(queued_request) = recv_next_request(&receiver, worker_id).await {
            let queue_time = queued_request.submitted_at.elapsed();
            debug!(
                "Worker {} processing request {} (queue time: {:?})",
                worker_id, queued_request.id, queue_time
            );
            if queued_request.cancellation_token.is_cancelled() {
                reject_cancelled_request(worker_id, queued_request, queue_time, &metrics);
                continue;
            }
            Self::process_request(worker_id, queued_request, &metrics, executor.as_ref()).await;
        }
    }

    /// Run a single dequeued request through the executor, then release the
    /// worker by recording the outcome and relaying the response. This is the
    /// heart of the worker-release invariant: every path through here ends with
    /// a metric update and a response send, so the live queue size always
    /// returns to its pre-request value once the turn finishes — regardless of
    /// whether the turn completed, hit EOS, ran out of budget, filled the
    /// context, errored, or was cancelled.
    async fn process_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
    ) {
        let start_time = Instant::now();
        let request_id = queued_request.id.clone();

        if queued_request.stream_sender.is_some() {
            Self::dispatch_streaming_request(
                worker_id,
                queued_request,
                metrics,
                executor,
                start_time,
            )
            .await;
        } else {
            Self::dispatch_batch_request(worker_id, queued_request, metrics, executor, start_time)
                .await;
        }

        let processing_time = start_time.elapsed();
        debug!(
            "Worker {} completed request {} in {:?}",
            worker_id, request_id, processing_time
        );
    }

    /// Drive a streaming request through the executor and relay completion/error
    /// back onto the stream sender and metrics.
    async fn dispatch_streaming_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
        start_time: Instant,
    ) {
        let stream_sender = queued_request
            .stream_sender
            .as_ref()
            .expect("streaming dispatch requires stream_sender")
            .clone();
        let result = executor
            .execute_streaming(worker_id, &queued_request, stream_sender.clone())
            .await;
        match result {
            Ok(_) => {
                // Tokens are tracked inside the executor's streaming path.
                metrics.record_request_completed(start_time.elapsed(), 0);
            }
            Err(queue_error) => {
                let _ = stream_sender.send(Err(queue_error)).await;
                metrics.record_request_failed();
            }
        }
    }

    /// Drive a batch request through the executor and send the
    /// GenerationResponse back on the request's oneshot response channel.
    async fn dispatch_batch_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        metrics: &QueueMetrics,
        executor: &dyn QueueExecutor,
        start_time: Instant,
    ) {
        // Run the turn while only borrowing the request, then move the response
        // sender out afterwards to deliver the result on its oneshot channel.
        let final_result = executor.execute_batch(worker_id, &queued_request).await;
        match &final_result {
            Ok(response) => {
                metrics.record_request_completed(start_time.elapsed(), response.tokens_generated)
            }
            Err(_) => metrics.record_request_failed(),
        }
        let _ = queued_request.response_sender.send(final_result);
    }

    #[allow(clippy::too_many_arguments)]
    fn process_batch_request_sync(
        worker_id: usize,
        request_id: String,
        request: &GenerationRequest,
        session: &Session,
        model: &LlamaModel,
        model_manager: &ModelManager,
        cancellation_token: &CancellationToken,
        chat_template: &ChatTemplateEngine,
        _session_config: &crate::types::SessionConfig,
        session_state_cache: &SessionStateCache,
    ) -> Result<GenerationResponse, QueueError> {
        let start_time = Instant::now();
        debug!(
            "Worker {} starting batch inference for request {}",
            worker_id, request_id
        );
        let can_use_cache = check_and_log_session_cache(worker_id, session, session_state_cache);
        let prompt =
            render_session_prompt(worker_id, chat_template, session, model, model_manager)?;
        let mut ctx = create_session_context(model_manager, model, session)?;
        let kv_cache_position = if can_use_cache {
            Self::restore_session_kv_cache(worker_id, session, &mut ctx, session_state_cache)?
        } else {
            -1
        };
        let template_token_count = compute_template_token_count(worker_id, kv_cache_position);
        let generation_result = Self::run_generation(
            worker_id,
            &request_id,
            model,
            &mut ctx,
            &prompt,
            request,
            cancellation_token,
            model_manager.get_batch_size(),
            template_token_count,
        )?;
        Ok(Self::finalize_batch_response(
            worker_id,
            request_id,
            session,
            &mut ctx,
            chat_template,
            session_state_cache,
            generation_result,
            start_time,
        ))
    }

    /// After generation completes, promote the finish reason for detected tool
    /// calls, persist the session state, and build the final response.
    #[allow(clippy::too_many_arguments)]
    fn finalize_batch_response(
        worker_id: usize,
        request_id: String,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        chat_template: &ChatTemplateEngine,
        session_state_cache: &SessionStateCache,
        generation_result: GenerationResponse,
        start_time: Instant,
    ) -> GenerationResponse {
        let final_finish_reason = Self::refine_finish_reason_for_tool_calls(
            worker_id,
            &request_id,
            chat_template,
            &generation_result.generated_text,
            generation_result.finish_reason.clone(),
        );
        let generation_time = start_time.elapsed();
        debug!(
            "Worker {} completed batch inference for request {} in {:?} ({} tokens, finish_reason: {:?})",
            worker_id,
            request_id,
            generation_time,
            generation_result.tokens_generated,
            final_finish_reason
        );
        Self::save_session_state(worker_id, &request_id, session, ctx, session_state_cache);
        GenerationResponse {
            generated_text: generation_result.generated_text,
            tokens_generated: generation_result.tokens_generated,
            generation_time,
            finish_reason: final_finish_reason,
            complete_token_sequence: generation_result.complete_token_sequence,
        }
    }

    /// Run a single generation pass against `ctx` with appropriate logging and
    /// error wrapping. Extracted from `process_batch_request_sync` so the main
    /// function stays at a manageable length.
    #[allow(clippy::too_many_arguments)]
    fn run_generation(
        worker_id: usize,
        request_id: &str,
        model: &LlamaModel,
        ctx: &mut LlamaContext<'_>,
        prompt: &str,
        request: &GenerationRequest,
        cancellation_token: &CancellationToken,
        batch_size: usize,
        template_token_count: Option<usize>,
    ) -> Result<GenerationResponse, QueueError> {
        debug!(
            "Queue worker {} calling GenerationHelper for request {}",
            worker_id, request_id
        );
        match GenerationHelper::generate_text_with_borrowed_model_and_template_offset(
            model,
            ctx,
            prompt,
            request,
            cancellation_token,
            batch_size,
            template_token_count,
        ) {
            Ok(result) => {
                debug!(
                    "Queue worker {} GenerationHelper returned success for request {}",
                    worker_id, request_id
                );
                Ok(result)
            }
            Err(e) => {
                error!(
                    "GenerationHelper failed for worker {} request {}: {}",
                    worker_id, request_id, e
                );
                debug!(
                    "Queue worker {} GenerationHelper error details: {:?}",
                    worker_id, e
                );
                Err(QueueError::WorkerError(format!("Generation failed: {}", e)))
            }
        }
    }

    /// Restore a session's llama.cpp context state from the in-memory cache and
    /// return the KV cache position after restoration. Errors if we expected a
    /// cache entry but the cache has been evicted.
    fn restore_session_kv_cache(
        worker_id: usize,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        session_state_cache: &SessionStateCache,
    ) -> Result<i32, QueueError> {
        info!(
            "Worker {} restoring session state from memory for session {}",
            worker_id, session.id
        );

        let state_bytes = {
            let cache = session_state_cache.lock().unwrap();
            cache.get(&session.id.to_string()).cloned()
        };

        let Some(bytes) = state_bytes else {
            warn!(
                "Worker {} expected cached state but not found in memory - will process all messages",
                worker_id
            );
            return Err(QueueError::WorkerError(
                "Expected state cache missing from memory".to_string(),
            ));
        };

        let bytes_len = bytes.len();
        let bytes_read = unsafe { ctx.set_state_data(&bytes) };
        let kv_cache_position = ctx.kv_cache_seq_pos_max(0);

        info!(
            "Worker {} restored state: {} bytes available, {} bytes read, {} cached messages, KV cache position: {}",
            worker_id, bytes_len, bytes_read, session.cached_message_count, kv_cache_position
        );

        Ok(kv_cache_position)
    }

    /// Inspect the generated text for tool calls when the model stopped for a
    /// "natural" reason, and upgrade the finish reason accordingly.
    fn refine_finish_reason_for_tool_calls(
        worker_id: usize,
        request_id: &str,
        chat_template: &ChatTemplateEngine,
        generated_text: &str,
        finish_reason: FinishReason,
    ) -> FinishReason {
        let should_check = matches!(
            &finish_reason,
            FinishReason::Stopped(reason)
                if reason == "End of sequence token detected"
                    || reason == "Stop token detected"
                    || reason == "Maximum tokens reached"
        );
        if !should_check {
            return finish_reason;
        }

        match chat_template.extract_tool_calls(generated_text) {
            Ok(tool_calls) if !tool_calls.is_empty() => {
                debug!(
                    "Worker {} detected {} tool calls in generated text for request {}",
                    worker_id,
                    tool_calls.len(),
                    request_id
                );
                FinishReason::Stopped("Tool call detected".to_string())
            }
            Ok(_) => {
                debug!(
                    "Worker {} no tool calls detected in generated text for request {}",
                    worker_id, request_id
                );
                finish_reason
            }
            Err(e) => {
                warn!(
                    "Worker {} failed to extract tool calls for request {}: {}",
                    worker_id, request_id, e
                );
                finish_reason
            }
        }
    }

    /// Copy the llama.cpp context state into the session cache so the next
    /// turn can resume without reprocessing prior messages. Applies a simple
    /// size-based eviction.
    fn save_session_state(
        worker_id: usize,
        request_id: &str,
        session: &Session,
        ctx: &mut LlamaContext<'_>,
        session_state_cache: &SessionStateCache,
    ) {
        let state_size = ctx.get_state_size();
        info!(
            "Worker {} saving session state to memory: {} bytes for {} messages",
            worker_id,
            state_size,
            session.messages.len()
        );

        let mut state_bytes = vec![0u8; state_size];
        let bytes_written = unsafe { ctx.copy_state_data(state_bytes.as_mut_ptr()) };

        if bytes_written == 0 {
            warn!(
                "Worker {} failed to copy state data (wrote 0 bytes) for request {}",
                worker_id, request_id
            );
            return;
        }

        state_bytes.truncate(bytes_written);

        let mut cache = session_state_cache.lock().unwrap();
        cache.insert(session.id.to_string(), state_bytes);
        info!(
            "Worker {} cached {} bytes of state for session {} ({} messages)",
            worker_id,
            bytes_written,
            session.id,
            session.messages.len()
        );

        evict_oldest_session_states(worker_id, &mut cache);
    }

    #[allow(clippy::too_many_arguments)]
    fn process_streaming_request_sync(
        worker_id: usize,
        request_id: String,
        request: &GenerationRequest,
        session: &Session,
        model: &LlamaModel,
        model_manager: &ModelManager,
        stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
        cancellation_token: &CancellationToken,
        chat_template: &ChatTemplateEngine,
    ) -> Result<(), QueueError> {
        debug!(
            "Worker {} starting streaming inference for request {}",
            worker_id, request_id
        );
        let Some((prompt, mut ctx)) = prepare_streaming_inference(
            chat_template,
            session,
            model,
            model_manager,
            &stream_sender,
        ) else {
            return Ok(());
        };
        trace!("Formatted prompt for streaming: {}", prompt);

        let result = GenerationHelper::generate_stream_with_borrowed_model_and_template_offset(
            model,
            &mut ctx,
            &prompt,
            request,
            &stream_sender,
            cancellation_token,
            model_manager.get_batch_size(),
            None, // No template offset - session state caching handles this
        );
        log_streaming_result(worker_id, &request_id, &stream_sender, result);
        Ok(())
    }
}

/// Log the outcome of a streaming generation and, on error, relay the failure
/// onto the client's stream channel.
fn log_streaming_result(
    worker_id: usize,
    request_id: &str,
    stream_sender: &mpsc::Sender<Result<StreamChunk, QueueError>>,
    result: Result<(), impl std::fmt::Display + std::fmt::Debug>,
) {
    match result {
        Ok(()) => debug!(
            "Worker {} completed streaming inference for request {} using GenerationHelper",
            worker_id, request_id
        ),
        Err(e) => {
            error!(
                "GenerationHelper streaming failed for worker {} request {}: {}",
                worker_id, request_id, e
            );
            report_stream_error(stream_sender, "Generation failed", &e);
        }
    }
}

impl RequestQueue {
    /// Gracefully shutdown the queue, waiting for all workers to complete
    ///
    /// This method implements cooperative shutdown where workers complete their current
    /// requests naturally without artificial time limits. Workers detect shutdown when
    /// the channel closes and stop accepting new requests, but continue processing any
    /// request they have already started. This ensures request integrity and proper
    /// resource cleanup without forced termination.
    pub async fn shutdown(mut self) {
        info!("RequestQueue shutting down gracefully");
        let shutdown_start = Instant::now();
        let stats = self.get_stats();

        info!(
            "Shutdown initiated with {} requests in queue, {} total processed",
            stats.current_queue_size, stats.total_requests
        );

        // Close the sender to signal workers to shut down. This MUST happen
        // before we await the worker handles: a worker only exits its loop once
        // `recv()` returns `None`, which only happens after every sender is
        // dropped. Dropping the sender here (rather than letting `self` drop at
        // the end of the method) is what lets the awaits below complete instead
        // of deadlocking.
        self.sender = None;

        // Wait for all worker handles to complete gracefully
        let mut successful_shutdowns = 0;
        let total_workers = self.worker_handles.len();

        info!(
            "Waiting for {} workers to complete current requests...",
            total_workers
        );

        for (i, handle) in self.worker_handles.drain(..).enumerate() {
            info!("Waiting for worker {} to complete current request...", i);

            match handle.await {
                Ok(()) => {
                    info!("Worker {} shutdown successfully", i);
                    successful_shutdowns += 1;
                }
                Err(join_error) => {
                    warn!("Worker {} panicked during shutdown: {}", i, join_error);
                }
            }
        }

        let shutdown_duration = shutdown_start.elapsed();

        info!(
            "RequestQueue shutdown complete in {:?}: {}/{} workers successful",
            shutdown_duration, successful_shutdowns, total_workers
        );
    }

    /// Shutdown with timeout and return statistics
    pub async fn shutdown_with_timeout(self, timeout: Duration) -> QueueStats {
        let stats_before = self.get_stats();
        info!(
            "Starting RequestQueue shutdown with {} timeout",
            Pretty(&timeout)
        );

        let shutdown_future = async {
            self.shutdown().await;
            Ok::<_, ()>(())
        };

        let _result = async_utils::with_timeout_action(
            shutdown_future,
            timeout,
            async_utils::TimeoutAction::LogWarning,
            &format!(
                "RequestQueue shutdown (had {} requests in queue)",
                stats_before.current_queue_size
            ),
        )
        .await;

        if _result.is_ok() && _result.as_ref().unwrap().is_some() {
            info!("RequestQueue shutdown completed within timeout");
        }

        stats_before
    }
}

impl Drop for RequestQueue {
    fn drop(&mut self) {
        info!(
            "RequestQueue dropping - {} worker handles remaining",
            self.worker_handles.len()
        );

        // Drop sender to signal workers to shut down
        // This closes the channel, causing receiver.recv() to return None
        if self.sender.take().is_some() {
            info!("Closed sender channel to signal workers to shutdown");
        }

        // Clear session state cache to release any remaining references
        {
            let mut cache = self.session_state_cache.lock().unwrap();
            let cache_size = cache.len();
            if cache_size > 0 {
                info!(
                    "Clearing {} session state cache entries before cleanup",
                    cache_size
                );
                cache.clear();
            }
        }

        // Note: Worker handles will be aborted when dropped
        // For graceful shutdown with proper cleanup, use shutdown() or shutdown_with_timeout()
        // instead of relying on Drop, which must be non-blocking to avoid hanging tests
        info!("RequestQueue cleanup complete");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{
        Message, MessageRole, ModelConfig, ModelSource, QueueConfig, RetryConfig, Session,
        SessionId,
    };
    use std::path::PathBuf;
    use std::time::SystemTime;
    use tempfile::TempDir;

    fn create_test_model_config() -> ModelConfig {
        ModelConfig {
            source: ModelSource::Local {
                folder: PathBuf::from("/tmp"),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        }
    }

    fn create_test_queue_config() -> QueueConfig {
        QueueConfig {
            max_queue_size: 10,
            worker_threads: 2,
        }
    }

    fn create_test_session() -> Session {
        Session {
            cwd: std::path::PathBuf::from("/tmp"),
            id: SessionId::new(),
            messages: vec![Message {
                role: MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }],
            mcp_servers: Vec::new(),
            available_tools: Vec::new(),
            available_prompts: Vec::new(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            compaction_history: Vec::new(),
            transcript_path: None,
            context_state: None,

            available_commands: Vec::new(),
            current_mode: None,

            client_capabilities: None,
            cached_message_count: 0,
            cached_token_count: 0,
            title: None,
        }
    }

    async fn setup_loaded_model_manager() -> Arc<ModelManager> {
        let temp_dir = TempDir::new().unwrap();
        let model_file = temp_dir.path().join("test.gguf");

        // Create dummy model file
        tokio::fs::write(&model_file, b"dummy model").await.unwrap();

        let config = ModelConfig {
            source: ModelSource::Local {
                folder: temp_dir.path().to_path_buf(),
                filename: Some("test.gguf".to_string()),
            },
            batch_size: 512,
            n_seq_max: 1,
            n_threads: 1,
            n_threads_batch: 1,
            use_hf_params: false,
            retry_config: RetryConfig::default(),
            debug: false,
        };

        let manager = Arc::new(ModelManager::new(config).expect("Failed to create ModelManager"));

        // Note: We don't actually load the model since dummy GGUF files fail
        // The queue tests should focus on queue functionality, not model loading
        // In a real application, the model would be properly loaded

        // Note: temp_dir will be automatically cleaned up when it goes out of scope
        // For test purposes, this is fine as the model manager only needs the path
        // during initialization, not for the entire lifetime
        drop(temp_dir);

        manager
    }

    #[tokio::test]
    async fn test_request_queue_creation() {
        // llama-agent tests run serially via the llama-embedding-serial test
        // group (see .config/nextest.toml), so ModelManager::new is always the
        // first call in this process — any error here is a real failure.
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();

        let queue = RequestQueue::new(model_manager, config, session_config);
        assert_eq!(queue.get_queue_size(), 0);
    }

    #[tokio::test]
    async fn test_submit_request_model_not_loaded() {
        // Serialized via nextest test group (see test_request_queue_creation).
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        assert!(matches!(result, Err(QueueError::WorkerError(_))));
    }

    #[tokio::test]
    async fn test_submit_request_model_not_loaded_fails() {
        let model_manager = setup_loaded_model_manager().await;
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        // Should fail because model is not actually loaded in test setup
        assert!(result.is_err());
        match result.unwrap_err() {
            QueueError::WorkerError(msg) => {
                assert!(msg.contains("Model not loaded") || msg.contains("Model error"));
            }
            _ => panic!("Expected WorkerError for unloaded model"),
        }
    }

    #[tokio::test]
    async fn test_submit_streaming_request_with_unloaded_model() {
        let model_manager = setup_loaded_model_manager().await;
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let mut receiver = queue
            .submit_streaming_request(request, &session)
            .await
            .unwrap();

        // Should receive an error since the model is not loaded (dummy model fails to load)
        let chunk_result = receiver.recv().await;
        assert!(chunk_result.is_some());
        match chunk_result.unwrap() {
            Err(QueueError::WorkerError(msg)) => {
                assert!(msg.contains("Model not loaded"));
            }
            Ok(_) => panic!("Expected error for unloaded model"),
            Err(other) => panic!("Unexpected error type: {:?}", other),
        }
    }

    /// Queue-lifecycle regression for bug 01KSNJ7CBK9333J0T9G4TCA7DH.
    ///
    /// With a single worker (the production config), a streaming turn must
    /// release the worker and decrement the live queue size once it finishes —
    /// success, empty, or error — so that a subsequent prompt still enqueues
    /// instead of being rejected with "Queue is full". This test drives the real
    /// `RequestQueue` (no mocks) with `worker_threads: 1`, drains a streaming
    /// turn to completion, then asserts the queue drained and a second streaming
    /// request enqueues without `QueueError::Full`.
    #[tokio::test]
    async fn test_streaming_worker_released_after_turn() {
        let model_manager = setup_loaded_model_manager().await;
        let config = QueueConfig {
            max_queue_size: 10,
            worker_threads: 1,
        };
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let make_request = || GenerationRequest {
            session_id: session.id,
            max_tokens: Some(16),
            temperature: Some(0.0),
            top_p: None,
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        // First streaming turn: drain it fully so the worker finishes and the
        // single worker is released back to the pool.
        let mut receiver = queue
            .submit_streaming_request(make_request(), &session)
            .await
            .expect("first streaming request should enqueue");
        while receiver.recv().await.is_some() {
            // Drain every chunk (the unloaded dummy model yields a single error
            // chunk) until the stream closes.
        }

        // Give the worker a moment to record completion metrics after the stream
        // sender is dropped.
        for _ in 0..50 {
            if queue.get_queue_size() == 0 {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        assert_eq!(
            queue.get_queue_size(),
            0,
            "worker was not released after the first streaming turn — live queue \
             size should return to 0"
        );

        // Second streaming turn must NOT be rejected with Queue is full.
        let second = queue
            .submit_streaming_request(make_request(), &session)
            .await;
        assert!(
            !matches!(second, Err(QueueError::Full)),
            "second streaming request was rejected with Queue is full after the \
             first turn released: {:?}",
            second.err()
        );
        // Drain the second stream too so the test leaves no dangling work.
        if let Ok(mut receiver) = second {
            while receiver.recv().await.is_some() {}
        }
    }

    #[tokio::test]
    async fn test_streaming_request_functionality() {
        // Validates streaming queue submission and chunk handling when the
        // model is not loaded. Serialized via nextest test group (see
        // test_request_queue_creation).
        let model_manager = Arc::new(
            ModelManager::new(create_test_model_config())
                .expect("ModelManager::new should succeed in serial test process"),
        );
        let queue = RequestQueue::new(
            model_manager,
            create_test_queue_config(),
            crate::types::SessionConfig::default(),
        );

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(10),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let mut receiver = queue
            .submit_streaming_request(request, &session)
            .await
            .expect("Streaming request submission should succeed");

        assert_model_not_loaded_stream(&mut receiver).await;
    }

    /// Assert that the first chunk is a `Model not loaded` worker error and
    /// that no further chunks are produced.
    async fn assert_model_not_loaded_stream(
        receiver: &mut mpsc::Receiver<Result<StreamChunk, QueueError>>,
    ) {
        let chunk_result = receiver
            .recv()
            .await
            .expect("Should receive a chunk result");
        match chunk_result {
            Err(QueueError::WorkerError(msg)) => assert!(
                msg.contains("Model not loaded"),
                "Should receive 'Model not loaded' error, got: {}",
                msg
            ),
            Ok(chunk) => panic!(
                "Expected error for unloaded model, but got streaming chunk: {:?}",
                chunk
            ),
            Err(other) => panic!("Expected WorkerError for unloaded model, got: {:?}", other),
        }
        assert!(
            receiver.recv().await.is_none(),
            "Should not receive additional chunks after error"
        );
    }

    #[tokio::test]
    async fn test_queue_timeout() {
        // Create a loaded model manager but with very slow processing
        let model_manager = setup_loaded_model_manager().await;
        let config = QueueConfig {
            max_queue_size: 10,

            worker_threads: 1,
        };
        let queue = RequestQueue::new(
            model_manager,
            config,
            crate::types::SessionConfig::default(),
        );

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(100),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        let result = queue.submit_request(request, &session).await;
        // Should fail because model is not loaded
        assert!(result.is_err());
        // The error should be WorkerError about model not loaded
        match result.unwrap_err() {
            QueueError::WorkerError(msg) => {
                assert!(msg.contains("Model not loaded") || msg.contains("Model error"));
            }
            other => panic!("Unexpected error type: {:?}", other),
        }
    }

    #[test]
    fn test_queued_request_debug() {
        let (sender, _) = oneshot::channel();
        let session = create_test_session();
        let request = QueuedRequest {
            id: "test-123".to_string(),
            request: GenerationRequest {
                session_id: session.id,
                max_tokens: Some(100),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            },
            session,
            response_sender: sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token: CancellationToken::new(),
        };

        let debug_str = format!("{:?}", request);
        assert!(debug_str.contains("test-123"));
    }

    /// Test batch processing with various prompt sizes
    mod batch_processing_tests {
        use super::*;

        fn create_test_config_with_batch_size(batch_size: u32) -> ModelConfig {
            ModelConfig {
                source: ModelSource::Local {
                    folder: PathBuf::from("/tmp"),
                    filename: Some("test.gguf".to_string()),
                },
                batch_size,
                n_seq_max: 1,
                n_threads: 1,
                n_threads_batch: 1,
                use_hf_params: false,
                retry_config: RetryConfig::default(),
                debug: false,
            }
        }

        #[tokio::test]
        async fn test_small_prompt_within_batch_size() {
            // Test case: prompt smaller than batch size should work normally.
            // Serialized via nextest test group (see test_request_queue_creation).
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(512))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with a small prompt (well within batch size)
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "Small prompt".to_string(), // ~2-3 tokens
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // The test focuses on batch processing logic, not model loading
            // We expect this to fail due to model not being loaded, but not due to batch size
            if let Err(QueueError::WorkerError(msg)) = result {
                // Should not contain batch size error messages
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
            }
        }

        #[tokio::test]
        async fn test_prompt_exactly_at_batch_size() {
            // Test edge case: prompt exactly at batch size limit.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 8u32; // Small batch size for testing
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            assert_eq!(model_manager.get_batch_size(), batch_size as usize);

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with content that should tokenize to exactly batch_size tokens
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "word ".repeat(4), // Approximately 8 tokens including spaces
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // Should not fail due to batch size issues
            if let Err(QueueError::WorkerError(msg)) = result {
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
            }
        }

        #[tokio::test]
        async fn test_prompt_exceeding_batch_size() {
            // Test case: prompt larger than batch size should be processed in chunks.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 4u32; // Very small batch size for testing
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            assert_eq!(model_manager.get_batch_size(), batch_size as usize);

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            // Create a session with a large prompt (exceeding batch size)
            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "This is a longer prompt that should exceed the small batch size limit and require chunked processing to handle properly without errors".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let result = queue.submit_request(request, &session).await;
            // Most importantly: should NOT fail with batch size error
            if let Err(QueueError::WorkerError(msg)) = result {
                assert!(!msg.contains("exceeds batch size limit"));
                assert!(!msg.contains("Prompt too long"));
                // Other errors (like model not loaded) are acceptable for this test
            }
        }

        #[tokio::test]
        async fn test_streaming_with_large_prompt() {
            // Test streaming with prompt larger than batch size.
            // Serialized via nextest test group (see test_request_queue_creation).
            let batch_size = 4u32;
            let model_manager = Arc::new(
                ModelManager::new(create_test_config_with_batch_size(batch_size))
                    .expect("ModelManager::new should succeed in serial test process"),
            );

            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::new(
                model_manager,
                config,
                crate::types::SessionConfig::default(),
            );

            let mut session = create_test_session();
            session.messages = vec![Message {
                role: MessageRole::User,
                content: "This is another long prompt for streaming that should exceed the batch size and test chunked processing in streaming mode".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: SystemTime::now(),
            }];

            let request = GenerationRequest {
                session_id: session.id,
                max_tokens: Some(10),
                temperature: Some(0.7),
                top_p: Some(0.9),
                stop_tokens: Vec::new(),
                stopping_config: None,
            };

            let stream_result = queue.submit_streaming_request(request, &session).await;

            // Check that we don't get batch size errors
            match stream_result {
                Ok(mut stream) => {
                    if let Some(Err(QueueError::WorkerError(msg))) = stream.recv().await {
                        assert!(!msg.contains("exceeds batch size limit"));
                        assert!(!msg.contains("Prompt too long"));
                    }
                }
                Err(QueueError::WorkerError(msg)) => {
                    assert!(!msg.contains("exceeds batch size limit"));
                    assert!(!msg.contains("Prompt too long"));
                }
                Err(_) => {
                    // Other errors are acceptable for this test
                }
            }
        }

        #[tokio::test]
        async fn test_multiple_batch_sizes() {
            // Test with various batch sizes to ensure consistent behavior
            let batch_sizes = vec![1u32, 2, 4, 8, 16, 32];

            for batch_size in batch_sizes {
                let model_manager =
                    match ModelManager::new(create_test_config_with_batch_size(batch_size)) {
                        Ok(manager) => Arc::new(manager),
                        Err(_) => continue, // Skip if can't create manager
                    };

                assert_eq!(model_manager.get_batch_size(), batch_size as usize);

                let config = QueueConfig {
                    max_queue_size: 10,
                    worker_threads: 1,
                };
                let queue = RequestQueue::new(
                    model_manager,
                    config,
                    crate::types::SessionConfig::default(),
                );

                let mut session = create_test_session();
                session.messages = vec![Message {
                    role: MessageRole::User,
                    content:
                        "Test prompt with multiple words to ensure it exceeds smaller batch sizes"
                            .to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: SystemTime::now(),
                }];

                let request = GenerationRequest {
                    session_id: session.id,
                    max_tokens: Some(5),
                    temperature: Some(0.7),
                    top_p: Some(0.9),
                    stop_tokens: Vec::new(),
                    stopping_config: None,
                };

                let result = queue.submit_request(request, &session).await;

                // Key assertion: no batch size limit errors regardless of batch_size
                if let Err(QueueError::WorkerError(msg)) = result {
                    assert!(
                        !msg.contains("exceeds batch size limit"),
                        "Batch size {} failed with batch size error: {}",
                        batch_size,
                        msg
                    );
                    assert!(
                        !msg.contains("Prompt too long"),
                        "Batch size {} failed with prompt length error: {}",
                        batch_size,
                        msg
                    );
                }
            }
        }

        #[test]
        fn test_batch_size_configuration() {
            // Test that different batch sizes are correctly configured
            let test_sizes = vec![1u32, 64, 256, 512, 1024, 2048];

            for expected_size in test_sizes {
                let config = create_test_config_with_batch_size(expected_size);
                assert_eq!(config.batch_size, expected_size);

                if let Ok(model_manager) = ModelManager::new(config) {
                    assert_eq!(model_manager.get_batch_size(), expected_size as usize);
                }
            }
        }

        #[test]
        fn test_chunk_processing_logic() {
            // Test the chunking logic without actual model processing
            let batch_size = 4;
            let tokens: Vec<i32> = (0..10).collect(); // 10 tokens: [0,1,2,3,4,5,6,7,8,9]

            let chunks: Vec<_> = tokens.chunks(batch_size).collect();

            // Should create 3 chunks: [0,1,2,3], [4,5,6,7], [8,9]
            assert_eq!(chunks.len(), 3);
            assert_eq!(chunks[0], &[0, 1, 2, 3]);
            assert_eq!(chunks[1], &[4, 5, 6, 7]);
            assert_eq!(chunks[2], &[8, 9]);

            // Verify no tokens are lost
            let reconstructed: Vec<i32> = chunks.into_iter().flatten().copied().collect();
            assert_eq!(reconstructed, tokens);
        }
    }

    /// Worker-lifecycle / state-machine coverage driven by a deterministic,
    /// weight-free executor.
    ///
    /// These tests run the *real* `RequestQueue` worker loop — `worker_loop`,
    /// `process_request`, `dispatch_{batch,streaming}_request`, enqueue, FIFO,
    /// cancellation, and backpressure — but substitute a [`ScriptedExecutor`]
    /// for the model-backed `ModelManagerExecutor` so every turn outcome is
    /// reproducible without a GPU or weights. The central invariant under test
    /// is the one the "Queue is full on retry" bug violated: **after any turn
    /// outcome the single worker must be released and the live queue size must
    /// return to zero, so a subsequent enqueue succeeds** (never a spurious
    /// `QueueError::Full`).
    mod worker_lifecycle_tests {
        use super::*;
        use crate::generation::scripted::{ScriptToken, ScriptedModel};
        use crate::generation::TextGenerator;
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};

        /// What a scripted turn should do when the worker runs it. This lets a
        /// single executor cover the whole turn-outcome matrix the queue cares
        /// about: a turn that produces tokens and stops for some reason, a turn
        /// that produces nothing (the 0-token / immediate-EOS bug shape), and a
        /// turn that fails outright.
        #[derive(Clone)]
        enum TurnOutcome {
            /// Replay the scripted model to completion (reason determined by the
            /// script + the request's `max_tokens` / `stop_tokens` / context).
            Scripted(ScriptedModel),
            /// Fail the turn with a worker error, as a runaway/aborted turn
            /// would. The worker must still be released afterward.
            Error(String),
        }

        /// A [`QueueExecutor`] backed by [`TurnOutcome`] rather than a live
        /// model. Counts how many turns it has run so FIFO/serialization can be
        /// asserted.
        struct ScriptedExecutor {
            outcome: TurnOutcome,
            turns_run: Arc<AtomicUsize>,
        }

        impl ScriptedExecutor {
            fn new(outcome: TurnOutcome) -> Self {
                Self {
                    outcome,
                    turns_run: Arc::new(AtomicUsize::new(0)),
                }
            }

            /// Derive a deterministic prompt from the session's messages so the
            /// scripted model has something to record. Queue-lifecycle tests do
            /// not depend on a chat template, only on the worker mechanics.
            fn prompt_for(session: &Session) -> String {
                session
                    .messages
                    .iter()
                    .map(|m| m.content.as_str())
                    .collect::<Vec<_>>()
                    .join(" ")
            }
        }

        #[async_trait]
        impl QueueExecutor for ScriptedExecutor {
            async fn execute_batch(
                &self,
                _worker_id: usize,
                queued_request: &QueuedRequest,
            ) -> Result<GenerationResponse, QueueError> {
                self.turns_run.fetch_add(1, AtomicOrdering::SeqCst);
                match &self.outcome {
                    TurnOutcome::Error(msg) => Err(QueueError::WorkerError(msg.clone())),
                    TurnOutcome::Scripted(model) => {
                        let prompt = Self::prompt_for(&queued_request.session);
                        let mut model = model.clone();
                        model
                            .generate_text(
                                &prompt,
                                queued_request.request.clone(),
                                queued_request.cancellation_token.clone(),
                            )
                            .map_err(|e| {
                                QueueError::WorkerError(format!("Generation failed: {}", e))
                            })
                    }
                }
            }

            async fn execute_streaming(
                &self,
                _worker_id: usize,
                queued_request: &QueuedRequest,
                stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
            ) -> Result<(), QueueError> {
                self.turns_run.fetch_add(1, AtomicOrdering::SeqCst);
                if let TurnOutcome::Error(msg) = &self.outcome {
                    return Err(QueueError::WorkerError(msg.clone()));
                }
                let TurnOutcome::Scripted(model) = &self.outcome else {
                    unreachable!("Error handled above");
                };

                let prompt = Self::prompt_for(&queued_request.session);
                let mut model = model.clone();

                // The scripted model streams into an unbounded channel; bridge
                // those chunks onto the bounded client channel, mirroring the
                // production streaming relay.
                let (tx, mut rx) = mpsc::unbounded_channel();
                let gen_result = model.generate_stream(
                    &prompt,
                    queued_request.request.clone(),
                    tx,
                    queued_request.cancellation_token.clone(),
                );
                while let Ok(chunk) = rx.try_recv() {
                    if stream_sender.send(chunk).await.is_err() {
                        break;
                    }
                }
                gen_result.map_err(|e| QueueError::WorkerError(format!("Generation failed: {}", e)))
            }
        }

        /// A single-worker queue running every turn through `outcome`.
        fn scripted_queue(outcome: TurnOutcome) -> (RequestQueue, Arc<AtomicUsize>) {
            let executor = ScriptedExecutor::new(outcome);
            let turns_run = executor.turns_run.clone();
            let config = QueueConfig {
                max_queue_size: 10,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(config, Arc::new(executor));
            (queue, turns_run)
        }

        fn streaming_request(session: &Session, max_tokens: u32) -> GenerationRequest {
            GenerationRequest {
                session_id: session.id,
                max_tokens: Some(max_tokens),
                temperature: Some(0.0),
                top_p: None,
                stop_tokens: Vec::new(),
                stopping_config: None,
            }
        }

        /// Poll the live queue size until it drains to zero or the budget runs
        /// out, so completion metrics (recorded after the stream sender drops)
        /// have a chance to land.
        async fn await_queue_drained(queue: &RequestQueue) {
            for _ in 0..200 {
                if queue.get_queue_size() == 0 {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        }

        /// Drive one streaming turn to completion and return the chunks observed.
        async fn run_streaming_turn(
            queue: &RequestQueue,
            session: &Session,
            request: GenerationRequest,
        ) -> Vec<Result<StreamChunk, QueueError>> {
            let mut receiver = queue
                .submit_streaming_request(request, session)
                .await
                .expect("streaming request should enqueue");
            let mut chunks = Vec::new();
            while let Some(item) = receiver.recv().await {
                chunks.push(item);
            }
            chunks
        }

        /// The heart of the regression suite: run a streaming turn with the
        /// given outcome, assert the worker was released (queue drains to zero),
        /// and assert a second turn still enqueues without `Full`. Returns the
        /// first turn's chunks so callers can additionally assert the outcome
        /// shape.
        async fn assert_worker_released_after(
            outcome: TurnOutcome,
            max_tokens: u32,
        ) -> Vec<Result<StreamChunk, QueueError>> {
            let (queue, turns_run) = scripted_queue(outcome);
            let session = create_test_session();

            let chunks =
                run_streaming_turn(&queue, &session, streaming_request(&session, max_tokens)).await;

            await_queue_drained(&queue).await;
            assert_eq!(
                queue.get_queue_size(),
                0,
                "worker was not released after the turn — live queue size should return to 0"
            );

            // The single worker must accept a second turn (no spurious Full).
            let second = queue
                .submit_streaming_request(streaming_request(&session, max_tokens), &session)
                .await;
            assert!(
                !matches!(second, Err(QueueError::Full)),
                "second turn rejected with Queue is full after release: {:?}",
                second.err()
            );
            if let Ok(mut receiver) = second {
                while receiver.recv().await.is_some() {}
            }
            await_queue_drained(&queue).await;
            assert!(
                turns_run.load(AtomicOrdering::SeqCst) >= 2,
                "both turns should have reached the worker"
            );

            chunks
        }

        /// Extract the completion chunk's finish reason from a stream.
        fn completion_reason(chunks: &[Result<StreamChunk, QueueError>]) -> Option<FinishReason> {
            chunks.iter().rev().find_map(|c| match c {
                Ok(chunk) if chunk.is_complete => chunk.finish_reason.clone(),
                _ => None,
            })
        }

        // --- Worker-release-on-every-outcome matrix -------------------------

        #[tokio::test]
        async fn worker_released_after_normal_completion() {
            // A short script that ends on its own EndOfSequence — the ordinary
            // "model finished talking" turn.
            let model = ScriptedModel::from_texts(["Hello", " world"]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            let text: String = chunks
                .iter()
                .filter_map(|c| c.as_ref().ok())
                .filter(|c| !c.is_complete)
                .map(|c| c.text.clone())
                .collect();
            assert_eq!(text, "Hello world");
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("EndOfSequence".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_immediate_eos_zero_tokens() {
            // The 0-token bug shape: the model emits EOS before any token. The
            // worker must still be released and re-enqueue must succeed.
            let model = ScriptedModel::new([ScriptToken::EndOfSequence]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            let token_chunks = chunks
                .iter()
                .filter_map(|c| c.as_ref().ok())
                .filter(|c| !c.is_complete)
                .count();
            assert_eq!(token_chunks, 0, "immediate EOS yields zero token chunks");
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("EndOfSequence".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_max_tokens() {
            // A script longer than the budget stops at MaxTokens — the
            // runaway-but-bounded turn.
            let model = ScriptedModel::from_texts(["a", "b", "c", "d", "e", "f"]);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 3).await;
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("MaxTokens".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_context_full() {
            // A tiny context window trips the context-window guard mid-turn.
            // create_test_session()'s single message "Hello" is one word, so
            // simulated_prompt_tokens == 1; with context_size 3 the guard fires
            // when 1 + generated >= 2, i.e. after one generated token.
            let model = ScriptedModel::from_texts(["x", "y", "z", "w"]).with_context_size(3);
            let chunks = assert_worker_released_after(TurnOutcome::Scripted(model), 64).await;
            assert_eq!(
                completion_reason(&chunks),
                Some(FinishReason::Stopped("ContextWindowFull".to_string()))
            );
        }

        #[tokio::test]
        async fn worker_released_after_error() {
            // A turn that fails outright must still release the worker — this is
            // the literal second symptom of the shipped bug.
            let chunks =
                assert_worker_released_after(TurnOutcome::Error("runaway turn aborted".into()), 64)
                    .await;
            // The error is relayed onto the stream.
            let has_error = chunks.iter().any(|c| {
                matches!(c, Err(QueueError::WorkerError(msg)) if msg.contains("runaway turn aborted"))
            });
            assert!(
                has_error,
                "the worker error should be relayed onto the stream"
            );
        }

        #[tokio::test]
        async fn worker_released_after_cancelled_turn() {
            // A turn whose cancellation token is already fired releases the
            // worker without corrupting the queue, and a fresh turn still runs.
            let model = ScriptedModel::from_texts(["never", "emitted"]);
            let (queue, turns_run) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            // Submit, then immediately cancel this session's request. The worker
            // either rejects it pre-process (cancelled before dequeue) or the
            // scripted loop observes the cancel and stops cleanly — either way
            // the worker is released.
            let request = streaming_request(&session, 64);
            let mut receiver = queue
                .submit_streaming_request(request, &session)
                .await
                .expect("streaming request should enqueue");
            queue.cancel_session(&session.id).await;
            while receiver.recv().await.is_some() {}

            await_queue_drained(&queue).await;
            assert_eq!(
                queue.get_queue_size(),
                0,
                "cancelled turn must release the worker"
            );

            // A subsequent turn on a fresh session enqueues and runs.
            let session2 = create_test_session();
            let chunks =
                run_streaming_turn(&queue, &session2, streaming_request(&session2, 64)).await;
            assert!(
                !chunks.is_empty(),
                "a turn after cancellation should still produce a completion"
            );
            await_queue_drained(&queue).await;
            assert!(turns_run.load(AtomicOrdering::SeqCst) >= 1);
        }

        #[tokio::test]
        async fn worker_released_after_batch_completion() {
            // The batch (non-streaming) path must release the worker too, and
            // return the collected response.
            let model = ScriptedModel::from_texts(["one", "two", "three"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            let response = queue
                .submit_request(streaming_request(&session, 64), &session)
                .await
                .expect("batch turn should succeed");
            assert_eq!(response.generated_text, "onetwothree");
            assert_eq!(response.tokens_generated, 3);

            await_queue_drained(&queue).await;
            assert_eq!(queue.get_queue_size(), 0, "batch turn must release worker");

            // Re-enqueue succeeds.
            let second = queue
                .submit_request(streaming_request(&session, 64), &session)
                .await;
            assert!(second.is_ok(), "second batch turn should not be rejected");
        }

        // --- Queue-full only at capacity -----------------------------------

        /// An executor that parks every turn on a release gate until the test
        /// fires it, so the queue can be filled to capacity deterministically.
        struct GatedExecutor {
            gate: Arc<tokio::sync::Notify>,
            entered: Arc<AtomicUsize>,
        }
        #[async_trait]
        impl QueueExecutor for GatedExecutor {
            async fn execute_batch(
                &self,
                _worker_id: usize,
                _queued_request: &QueuedRequest,
            ) -> Result<GenerationResponse, QueueError> {
                self.entered.fetch_add(1, AtomicOrdering::SeqCst);
                self.gate.notified().await;
                Ok(GenerationResponse {
                    generated_text: String::new(),
                    tokens_generated: 0,
                    generation_time: Duration::from_millis(0),
                    finish_reason: FinishReason::Stopped("EndOfSequence".to_string()),
                    complete_token_sequence: None,
                })
            }
            async fn execute_streaming(
                &self,
                _worker_id: usize,
                _queued_request: &QueuedRequest,
                _stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
            ) -> Result<(), QueueError> {
                self.entered.fetch_add(1, AtomicOrdering::SeqCst);
                self.gate.notified().await;
                Ok(())
            }
        }

        #[tokio::test]
        async fn enqueue_returns_full_only_at_capacity() {
            // Park the single worker on a gated turn, then fill the bounded
            // channel to exactly capacity and prove the next enqueue — and only
            // it — returns Full, while every enqueue up to capacity succeeds.
            let gate = Arc::new(tokio::sync::Notify::new());
            let entered = Arc::new(AtomicUsize::new(0));
            let max_queue_size = 3;
            let config = QueueConfig {
                max_queue_size,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(
                config,
                Arc::new(GatedExecutor {
                    gate: gate.clone(),
                    entered: entered.clone(),
                }),
            );
            let session = create_test_session();

            // First request reaches the worker and parks on the gate, removing
            // itself from the channel buffer.
            let _busy = queue.submit_request(streaming_request(&session, 8), &session);
            tokio::pin!(_busy);
            // Poll it once to dispatch into the channel, then leave it pending.
            tokio::select! {
                _ = &mut _busy => panic!("gated turn returned early"),
                _ = tokio::time::sleep(Duration::from_millis(50)) => {}
            }
            assert_eq!(
                entered.load(AtomicOrdering::SeqCst),
                1,
                "the worker should be parked on the first turn"
            );

            // Now the worker is busy; the bounded channel holds `max_queue_size`
            // pending requests. Each enqueue up to capacity must succeed.
            let mut buffered = Vec::new();
            for i in 0..max_queue_size {
                let result = queue.try_enqueue_for_test(&session);
                assert!(
                    result.is_ok(),
                    "enqueue {} within capacity must succeed, got {:?}",
                    i,
                    result.err()
                );
                buffered.push(result);
            }

            // Capacity reached: the next enqueue must return Full.
            let overflow = queue.try_enqueue_for_test(&session);
            assert!(
                matches!(overflow, Err(QueueError::Full)),
                "enqueue past capacity must return QueueError::Full, got {:?}",
                overflow
            );

            // Release the worker so the test shuts down cleanly.
            gate.notify_waiters();
        }

        // --- FIFO ordering through the single worker ------------------------

        #[tokio::test]
        async fn batch_turns_processed_in_fifo_order() {
            // A single worker processes submitted requests in submission order.
            // We record the per-turn prompt the executor sees and assert it
            // matches submission order.
            let seen = Arc::new(Mutex::new(Vec::<String>::new()));

            struct RecordingExecutor {
                seen: Arc<Mutex<Vec<String>>>,
            }
            #[async_trait]
            impl QueueExecutor for RecordingExecutor {
                async fn execute_batch(
                    &self,
                    _worker_id: usize,
                    queued_request: &QueuedRequest,
                ) -> Result<GenerationResponse, QueueError> {
                    let content = queued_request.session.messages[0].content.clone();
                    self.seen.lock().unwrap().push(content.clone());
                    Ok(GenerationResponse {
                        generated_text: content,
                        tokens_generated: 1,
                        generation_time: Duration::from_millis(0),
                        finish_reason: FinishReason::Stopped("EndOfSequence".to_string()),
                        complete_token_sequence: None,
                    })
                }
                async fn execute_streaming(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                    _stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
                ) -> Result<(), QueueError> {
                    Ok(())
                }
            }

            let config = QueueConfig {
                max_queue_size: 16,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(
                config,
                Arc::new(RecordingExecutor { seen: seen.clone() }),
            );

            // Submit several batch requests in a fixed order, awaiting each so
            // the single worker handles them one at a time in submission order.
            let order = ["first", "second", "third", "fourth"];
            for label in order {
                let mut session = create_test_session();
                session.messages[0].content = label.to_string();
                let response = queue
                    .submit_request(streaming_request(&session, 8), &session)
                    .await
                    .expect("each batch turn should succeed");
                assert_eq!(response.generated_text, label);
            }

            let recorded = seen.lock().unwrap().clone();
            assert_eq!(
                recorded,
                order.iter().map(|s| s.to_string()).collect::<Vec<_>>(),
                "the single worker must process requests in FIFO order"
            );
        }

        // --- Backpressure: worker_threads = 1 serializes -------------------

        #[tokio::test]
        async fn single_worker_serializes_concurrent_turns() {
            // With worker_threads = 1, concurrently-submitted turns must not run
            // in parallel. The executor tracks concurrent entries and asserts
            // the peak is exactly 1.
            let in_flight = Arc::new(AtomicUsize::new(0));
            let peak = Arc::new(AtomicUsize::new(0));

            struct SerializingExecutor {
                in_flight: Arc<AtomicUsize>,
                peak: Arc<AtomicUsize>,
            }
            #[async_trait]
            impl QueueExecutor for SerializingExecutor {
                async fn execute_batch(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                ) -> Result<GenerationResponse, QueueError> {
                    let now = self.in_flight.fetch_add(1, AtomicOrdering::SeqCst) + 1;
                    self.peak.fetch_max(now, AtomicOrdering::SeqCst);
                    // Hold the worker briefly so any parallel entry would be
                    // observed as concurrency > 1.
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    self.in_flight.fetch_sub(1, AtomicOrdering::SeqCst);
                    Ok(GenerationResponse {
                        generated_text: String::new(),
                        tokens_generated: 0,
                        generation_time: Duration::from_millis(0),
                        finish_reason: FinishReason::Stopped("EndOfSequence".to_string()),
                        complete_token_sequence: None,
                    })
                }
                async fn execute_streaming(
                    &self,
                    _worker_id: usize,
                    _queued_request: &QueuedRequest,
                    _stream_sender: mpsc::Sender<Result<StreamChunk, QueueError>>,
                ) -> Result<(), QueueError> {
                    Ok(())
                }
            }

            let config = QueueConfig {
                max_queue_size: 16,
                worker_threads: 1,
            };
            let queue = Arc::new(RequestQueue::with_executor(
                config,
                Arc::new(SerializingExecutor {
                    in_flight: in_flight.clone(),
                    peak: peak.clone(),
                }),
            ));

            // Fire several requests concurrently; the single worker must
            // serialize them.
            let mut handles = Vec::new();
            for _ in 0..5 {
                let queue = queue.clone();
                let session = create_test_session();
                handles.push(tokio::spawn(async move {
                    let _ = queue
                        .submit_request(streaming_request(&session, 8), &session)
                        .await;
                }));
            }
            for h in handles {
                h.await.unwrap();
            }

            assert_eq!(
                peak.load(AtomicOrdering::SeqCst),
                1,
                "a single worker must never run two turns concurrently"
            );
        }

        // --- Stats / metrics snapshot --------------------------------------

        #[tokio::test]
        async fn stats_reflect_completed_turns() {
            // After a batch turn completes, the stats snapshot reports it as
            // completed with the generated token count, and the live size is 0.
            let model = ScriptedModel::from_texts(["a", "b"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();

            let _ = queue
                .submit_request(streaming_request(&session, 8), &session)
                .await
                .expect("batch turn should succeed");
            await_queue_drained(&queue).await;

            let stats = queue.get_stats();
            assert_eq!(stats.total_requests, 1);
            assert_eq!(stats.completed_requests, 1);
            assert_eq!(stats.failed_requests, 0);
            assert_eq!(stats.current_queue_size, 0);
            assert_eq!(stats.total_tokens_generated, 2);
            assert!(stats.peak_queue_size >= 1);
        }

        #[tokio::test]
        async fn stats_reflect_failed_turns() {
            // A failed turn is counted as failed (not completed) and still
            // releases the worker.
            let (queue, _turns) = scripted_queue(TurnOutcome::Error("boom".into()));
            let session = create_test_session();

            let result = queue
                .submit_request(streaming_request(&session, 8), &session)
                .await;
            assert!(matches!(result, Err(QueueError::WorkerError(_))));
            await_queue_drained(&queue).await;

            let stats = queue.get_stats();
            assert_eq!(stats.completed_requests, 0);
            assert_eq!(stats.failed_requests, 1);
            assert_eq!(stats.current_queue_size, 0);
        }

        // --- cancel_session bookkeeping ------------------------------------

        #[tokio::test]
        async fn cancel_session_returns_false_when_no_active_request() {
            // Cancelling a session with no in-flight request returns false and
            // does not disturb the queue.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let unknown = SessionId::new();
            assert!(
                !queue.cancel_session(&unknown).await,
                "cancelling an unknown session returns false"
            );
        }

        // --- Queue-full on the streaming submit path -----------------------

        #[tokio::test]
        async fn streaming_submit_returns_full_at_capacity() {
            // The streaming submit path has its own try_send + Full branch.
            // Park the worker and fill the bounded channel to prove it fires.
            let gate = Arc::new(tokio::sync::Notify::new());
            let entered = Arc::new(AtomicUsize::new(0));
            let max_queue_size = 2;
            let config = QueueConfig {
                max_queue_size,
                worker_threads: 1,
            };
            let queue = RequestQueue::with_executor(
                config,
                Arc::new(GatedExecutor {
                    gate: gate.clone(),
                    entered: entered.clone(),
                }),
            );
            let session = create_test_session();

            // Occupy the worker with one streaming turn parked on the gate.
            let _busy = queue
                .submit_streaming_request(streaming_request(&session, 8), &session)
                .await
                .expect("first streaming request occupies the worker");
            for _ in 0..40 {
                if entered.load(AtomicOrdering::SeqCst) == 1 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }

            // Fill the channel; eventually streaming submit must return Full.
            let mut held = Vec::new();
            let mut full_seen = false;
            for _ in 0..16 {
                match queue
                    .submit_streaming_request(streaming_request(&session, 8), &session)
                    .await
                {
                    Ok(rx) => held.push(rx),
                    Err(QueueError::Full) => {
                        full_seen = true;
                        break;
                    }
                    Err(other) => panic!("unexpected error: {:?}", other),
                }
            }
            assert!(full_seen, "streaming submit must return Full at capacity");
            gate.notify_waiters();
        }

        // --- Shutdown closes the sender, rejecting later enqueues ----------

        #[tokio::test]
        async fn graceful_shutdown_drains_workers() {
            // `shutdown()` closes the sender channel and joins every worker
            // handle, exercising the graceful shutdown loop.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();
            let _ = queue
                .submit_request(streaming_request(&session, 8), &session)
                .await;
            await_queue_drained(&queue).await;
            queue.shutdown().await;
        }

        #[tokio::test]
        async fn shutdown_with_timeout_returns_stats() {
            // shutdown_with_timeout drains workers within the budget and returns
            // a pre-shutdown stats snapshot.
            let model = ScriptedModel::from_texts(["x"]);
            let (queue, _turns) = scripted_queue(TurnOutcome::Scripted(model));
            let session = create_test_session();
            let _ = queue
                .submit_request(streaming_request(&session, 8), &session)
                .await;
            let stats = queue.shutdown_with_timeout(Duration::from_secs(5)).await;
            assert_eq!(stats.total_requests, 1);
        }
    }

    /// Unit tests for the module-private free functions that do not need a
    /// model, exercising branches the worker only reaches under specific
    /// conditions (e.g. cache growth past the per-process limit).
    mod free_fn_unit_tests {
        use super::*;

        /// The same limit the function computes internally, so the test can
        /// insert one more than the limit and guarantee eviction fires.
        fn cache_limit() -> usize {
            std::thread::available_parallelism()
                .map(|n| (n.get() / 2).max(1))
                .unwrap_or(4)
        }

        #[test]
        fn evict_session_states_is_a_noop_under_limit() {
            // At or below the limit nothing is evicted.
            let mut cache: HashMap<String, Vec<u8>> = HashMap::new();
            cache.insert("only".to_string(), vec![1, 2, 3]);
            evict_oldest_session_states(0, &mut cache);
            assert_eq!(cache.len(), 1, "a single entry is never evicted");
        }

        #[test]
        fn evict_session_states_drops_down_to_limit() {
            // Growing the cache past the limit evicts the overflow back down to
            // exactly the limit — the eviction branch the worker only hits after
            // many distinct sessions have been cached.
            let limit = cache_limit();
            let mut cache: HashMap<String, Vec<u8>> = HashMap::new();
            for i in 0..(limit + 5) {
                cache.insert(format!("session-{i}"), vec![i as u8]);
            }
            assert!(cache.len() > limit, "precondition: cache exceeds the limit");

            evict_oldest_session_states(0, &mut cache);

            assert_eq!(
                cache.len(),
                limit,
                "eviction must bring the cache back down to exactly the limit"
            );
        }

        #[test]
        fn template_token_count_maps_position_to_next() {
            // A non-negative KV-cache position maps to the next position; a
            // negative position (fresh context) maps to None.
            assert_eq!(compute_template_token_count(0, -1), None);
            assert_eq!(compute_template_token_count(0, 0), Some(1));
            assert_eq!(compute_template_token_count(0, 41), Some(42));
        }
    }
}
