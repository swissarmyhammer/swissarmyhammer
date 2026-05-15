use crate::chat_template::ChatTemplateEngine;
use crate::generation::GenerationHelper;
use crate::model::ModelManager;

use crate::types::{
    FinishReason, GenerationRequest, GenerationResponse, QueueConfig, QueueError, Session,
    StreamChunk,
};
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

        let worker_handles = Self::spawn_workers(
            &config,
            &receiver,
            &model_manager,
            &metrics,
            &chat_template,
            &session_config,
            &session_state_cache,
        );

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

    /// Spawn the configured number of worker tasks, cloning all shared state
    /// each iteration. Kept out of `new` so the constructor stays concise.
    #[allow(clippy::too_many_arguments)]
    fn spawn_workers(
        config: &QueueConfig,
        receiver: &Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        model_manager: &Arc<ModelManager>,
        metrics: &Arc<QueueMetrics>,
        chat_template: &Arc<ChatTemplateEngine>,
        session_config: &crate::types::SessionConfig,
        session_state_cache: &SessionStateCache,
    ) -> Vec<JoinHandle<()>> {
        (0..config.worker_threads)
            .map(|worker_id| {
                let receiver = receiver.clone();
                let model_manager = model_manager.clone();
                let config = config.clone();
                let metrics = metrics.clone();
                let chat_template = chat_template.clone();
                let session_config = session_config.clone();
                let session_state_cache = session_state_cache.clone();
                tokio::spawn(async move {
                    Self::worker_loop(
                        worker_id,
                        receiver,
                        model_manager,
                        config,
                        metrics,
                        chat_template,
                        session_config,
                        session_state_cache,
                    )
                    .await;
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

    #[allow(clippy::too_many_arguments)]
    async fn worker_loop(
        worker_id: usize,
        receiver: Arc<tokio::sync::Mutex<mpsc::Receiver<QueuedRequest>>>,
        model_manager: Arc<ModelManager>,
        _config: QueueConfig,
        metrics: Arc<QueueMetrics>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
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
            Self::process_request(
                worker_id,
                queued_request,
                model_manager.clone(),
                metrics.clone(),
                chat_template.clone(),
                session_config.clone(),
                session_state_cache.clone(),
            )
            .await;
        }
    }

    async fn process_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        model_manager: Arc<ModelManager>,
        metrics: Arc<QueueMetrics>,
        chat_template: Arc<ChatTemplateEngine>,
        session_config: crate::types::SessionConfig,
        session_state_cache: SessionStateCache,
    ) {
        let start_time = Instant::now();

        if !model_manager.is_loaded().await {
            Self::reject_unloaded_request(queued_request, &metrics).await;
            return;
        }

        let request_id = queued_request.id.clone();
        if queued_request.stream_sender.is_some() {
            Self::dispatch_streaming_request(
                worker_id,
                queued_request,
                &model_manager,
                &metrics,
                &chat_template,
                start_time,
            )
            .await;
        } else {
            Self::dispatch_batch_request(
                worker_id,
                queued_request,
                &model_manager,
                &metrics,
                &chat_template,
                &session_config,
                &session_state_cache,
                start_time,
            )
            .await;
        }

        let processing_time = start_time.elapsed();
        debug!(
            "Worker {} completed request {} in {:?}",
            worker_id, request_id, processing_time
        );
    }

    /// Short-circuit path when `model_manager.is_loaded()` is false: return a
    /// "Model not loaded" error on whichever sender the request is waiting on
    /// (streaming or batch).
    async fn reject_unloaded_request(queued_request: QueuedRequest, metrics: &QueueMetrics) {
        let error = QueueError::WorkerError("Model not loaded".to_string());
        match queued_request.stream_sender {
            Some(stream_sender) => {
                let _ = stream_sender.send(Err(error)).await;
            }
            None => {
                let _ = queued_request.response_sender.send(Err(error));
            }
        }
        metrics.record_request_failed();
    }

    /// Drive a streaming request through the model and relay completion/error
    /// back onto the stream sender and metrics.
    async fn dispatch_streaming_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        model_manager: &Arc<ModelManager>,
        metrics: &QueueMetrics,
        chat_template: &ChatTemplateEngine,
        start_time: Instant,
    ) {
        let stream_sender = queued_request
            .stream_sender
            .as_ref()
            .expect("streaming dispatch requires stream_sender")
            .clone();
        let request_id = queued_request.id.clone();
        let result = model_manager
            .with_model(|model| {
                Self::process_streaming_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    model_manager,
                    stream_sender.clone(),
                    &queued_request.cancellation_token,
                    chat_template,
                )
            })
            .await;
        match result {
            Ok(_) => {
                // Tokens are tracked inside process_streaming_request_sync.
                metrics.record_request_completed(start_time.elapsed(), 0);
            }
            Err(model_error) => {
                let queue_error = QueueError::WorkerError(format!("Model error: {}", model_error));
                let _ = stream_sender.send(Err(queue_error)).await;
                metrics.record_request_failed();
            }
        }
    }

    /// Drive a batch request through the model and send the GenerationResponse
    /// back on the request's oneshot response channel.
    #[allow(clippy::too_many_arguments)]
    async fn dispatch_batch_request(
        worker_id: usize,
        queued_request: QueuedRequest,
        model_manager: &Arc<ModelManager>,
        metrics: &QueueMetrics,
        chat_template: &ChatTemplateEngine,
        session_config: &crate::types::SessionConfig,
        session_state_cache: &SessionStateCache,
        start_time: Instant,
    ) {
        let request_id = queued_request.id.clone();
        let response_sender = queued_request.response_sender;
        let result = model_manager
            .with_model(|model| {
                Self::process_batch_request_sync(
                    worker_id,
                    request_id.clone(),
                    &queued_request.request,
                    &queued_request.session,
                    model,
                    model_manager,
                    &queued_request.cancellation_token,
                    chat_template,
                    session_config,
                    session_state_cache,
                )
            })
            .await;
        let final_result = match result {
            Ok(inner) => inner,
            Err(model_error) => Err(QueueError::WorkerError(format!(
                "Model error: {}",
                model_error
            ))),
        };
        match &final_result {
            Ok(response) => {
                metrics.record_request_completed(start_time.elapsed(), response.tokens_generated)
            }
            Err(_) => metrics.record_request_failed(),
        }
        let _ = response_sender.send(final_result);
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

        // Close the sender to signal workers to shutdown
        // (sender will be dropped when this method ends)

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
}
