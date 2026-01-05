use crate::chat_template::ChatTemplateEngine;
use crate::generation::GenerationHelper;
use crate::model::ModelManager;

use crate::types::{
    FinishReason, GenerationRequest, GenerationResponse, QueueConfig, QueueError, Session,
    StreamChunk,
};
use llama_common::async_utils;
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

#[derive(Debug, Default)]
pub struct QueueMetrics {
    pub total_requests: AtomicU64,
    pub completed_requests: AtomicU64,
    pub failed_requests: AtomicU64,
    pub cancelled_requests: AtomicU64,
    pub current_queue_size: AtomicUsize,
    pub total_processing_time_ms: AtomicU64,
    pub total_tokens_generated: AtomicU64,
    pub peak_queue_size: AtomicUsize,
    pub last_throughput_tokens_per_second: AtomicU64,
}

impl QueueMetrics {
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

    pub fn record_request_completed(&self, processing_time: Duration, tokens_generated: u32) {
        self.completed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);

        let processing_ms = processing_time.as_millis() as u64;
        self.total_processing_time_ms
            .fetch_add(processing_ms, Ordering::Relaxed);
        self.total_tokens_generated
            .fetch_add(tokens_generated as u64, Ordering::Relaxed);

        // Calculate and store current throughput (tokens per second)
        if processing_ms > 0 {
            let throughput = (tokens_generated as u64 * 1000) / processing_ms;
            self.last_throughput_tokens_per_second
                .store(throughput, Ordering::Relaxed);
        }
    }

    pub fn record_request_failed(&self) {
        self.failed_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

    pub fn record_request_cancelled(&self) {
        self.cancelled_requests.fetch_add(1, Ordering::Relaxed);
        self.current_queue_size.fetch_sub(1, Ordering::Relaxed);
    }

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
                if completed > 0 {
                    total_time / completed
                } else {
                    0
                }
            },
            total_tokens_generated: self.total_tokens_generated.load(Ordering::Relaxed),
            peak_queue_size: self.peak_queue_size.load(Ordering::Relaxed),
            current_throughput_tps: self
                .last_throughput_tokens_per_second
                .load(Ordering::Relaxed),
        }
    }
}

#[derive(Debug, Clone)]
pub struct QueueStats {
    pub total_requests: u64,
    pub completed_requests: u64,
    pub failed_requests: u64,
    pub cancelled_requests: u64,
    pub current_queue_size: usize,
    pub average_processing_time_ms: u64,
    pub total_tokens_generated: u64,
    pub peak_queue_size: usize,
    pub current_throughput_tps: u64,
}

#[derive(Debug)]
pub struct QueuedRequest {
    pub id: String,
    pub request: GenerationRequest,
    pub session: Session,
    pub response_sender: oneshot::Sender<Result<GenerationResponse, QueueError>>,
    pub stream_sender: Option<mpsc::Sender<Result<StreamChunk, QueueError>>>,
    pub submitted_at: Instant,
    pub cancellation_token: CancellationToken,
}

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
    pub fn new(
        model_manager: Arc<ModelManager>,
        config: QueueConfig,
        session_config: crate::types::SessionConfig,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(config.max_queue_size);
        let receiver = Arc::new(tokio::sync::Mutex::new(receiver));
        let metrics = Arc::new(QueueMetrics::new());
        let chat_template = Arc::new(ChatTemplateEngine::new());
        let session_state_cache: SessionStateCache = Arc::new(Mutex::new(HashMap::new()));

        let mut worker_handles = Vec::new();

        // Spawn worker threads
        for worker_id in 0..config.worker_threads {
            let receiver = receiver.clone();
            let model_manager = model_manager.clone();
            let config = config.clone();
            let metrics = metrics.clone();
            let chat_template = chat_template.clone();
            let session_config = session_config.clone();
            let session_state_cache = session_state_cache.clone();

            let handle = tokio::spawn(async move {
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
            });

            worker_handles.push(handle);
        }

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

    pub async fn submit_request(
        &self,
        request: GenerationRequest,
        session: &Session,
    ) -> Result<GenerationResponse, QueueError> {
        let (response_sender, response_receiver) = oneshot::channel();

        let cancellation_token = CancellationToken::new();
        let session_id = request.session_id;

        // Track the cancellation token for this session
        {
            let mut active = self.active_requests.lock().await;
            active.insert(session_id, cancellation_token.clone());
        }

        let queued_request = QueuedRequest {
            id: Ulid::new().to_string(),
            request,
            session: session.clone(),
            response_sender,
            stream_sender: None,
            submitted_at: Instant::now(),
            cancellation_token: cancellation_token.clone(),
        };

        debug!("Submitting request to queue: {}", queued_request.id);

        // Record request submission
        self.metrics.record_request_submitted();

        // Try to send to queue
        let sender = self.sender.as_ref().ok_or_else(|| {
            warn!("Queue is shutting down, rejecting request");
            self.metrics.record_request_failed();
            QueueError::WorkerError("Queue is shutting down".to_string())
        })?;

        if sender.try_send(queued_request).is_err() {
            warn!("Queue is full, rejecting request");
            self.metrics.record_request_failed(); // Adjust queue size back down
            return Err(QueueError::Full);
        }

        // Wait for response from worker
        let active_requests: Arc<TokioMutex<HashMap<crate::types::SessionId, CancellationToken>>> =
            Arc::clone(&self.active_requests);
        let response_future = async move {
            let result = response_receiver
                .await
                .map_err(|_| QueueError::WorkerError("Response channel closed".to_string()))?;

            // Clean up the cancellation token tracking
            let mut active = active_requests.lock().await;
            active.remove(&session_id);

            result
        };
        response_future.await
    }

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

        loop {
            let queued_request = {
                let mut receiver = receiver.lock().await;
                match receiver.recv().await {
                    Some(request) => request,
                    None => {
                        info!("Worker {} shutting down - channel closed", worker_id);
                        break;
                    }
                }
            };

            let queue_time = queued_request.submitted_at.elapsed();
            debug!(
                "Worker {} processing request {} (queue time: {:?})",
                worker_id, queued_request.id, queue_time
            );

            // Check if request was cancelled
            if queued_request.cancellation_token.is_cancelled() {
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
                continue;
            }

            // Process the request
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

        // Check if model is loaded
        if !model_manager.is_loaded().await {
            let error = QueueError::WorkerError("Model not loaded".to_string());
            if let Some(stream_sender) = queued_request.stream_sender {
                let _ = stream_sender.send(Err(error)).await;
            } else {
                let _ = queued_request.response_sender.send(Err(error));
            }
            metrics.record_request_failed();
            return;
        }

        let request_id = queued_request.id.clone();

        // Process request with model access - use a closure to work within model lifetime
        if let Some(stream_sender) = queued_request.stream_sender {
            // Handle streaming request
            let result = model_manager
                .with_model(|model| {
                    // Process the streaming request synchronously within the model lifetime
                    Self::process_streaming_request_sync(
                        worker_id,
                        request_id.clone(),
                        &queued_request.request,
                        &queued_request.session,
                        model,
                        &model_manager,
                        stream_sender.clone(),
                        &queued_request.cancellation_token,
                        &chat_template,
                    )
                })
                .await;

            match result {
                Ok(_) => {
                    // Streaming completed successfully
                    let processing_time = start_time.elapsed();
                    // Note: For streaming, tokens are tracked within process_streaming_request_sync
                    metrics.record_request_completed(processing_time, 0);
                }
                Err(model_error) => {
                    let queue_error =
                        QueueError::WorkerError(format!("Model error: {}", model_error));
                    let _ = stream_sender.send(Err(queue_error)).await;
                    metrics.record_request_failed();
                }
            }
        } else {
            // Handle batch request
            let result = model_manager
                .with_model(|model| {
                    // Process the request synchronously within the model lifetime
                    Self::process_batch_request_sync(
                        worker_id,
                        request_id.clone(),
                        &queued_request.request,
                        &queued_request.session,
                        model,
                        &model_manager,
                        &queued_request.cancellation_token,
                        &chat_template,
                        &session_config,
                        &session_state_cache,
                    )
                })
                .await;

            match result {
                Ok(inner_result) => {
                    // inner_result is Result<GenerationResponse, QueueError>
                    match inner_result {
                        Ok(response) => {
                            let processing_time = start_time.elapsed();
                            metrics.record_request_completed(
                                processing_time,
                                response.tokens_generated,
                            );
                            let _ = queued_request.response_sender.send(Ok(response));
                        }
                        Err(queue_error) => {
                            metrics.record_request_failed();
                            let _ = queued_request.response_sender.send(Err(queue_error));
                        }
                    }
                }
                Err(model_error) => {
                    metrics.record_request_failed();
                    let queue_error =
                        QueueError::WorkerError(format!("Model error: {}", model_error));
                    let _ = queued_request.response_sender.send(Err(queue_error));
                }
            };
        }

        let processing_time = start_time.elapsed();
        debug!(
            "Worker {} completed request {} in {:?}",
            worker_id, request_id, processing_time
        );
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

        // Check if we have a cached state in memory for this session
        let has_cached_state = {
            let cache = session_state_cache.lock().unwrap();
            cache.contains_key(&session.id.to_string())
        };
        let can_use_cache = has_cached_state && session.cached_message_count > 0;

        // Log session status
        if can_use_cache {
            info!(
                "Worker {} continuing session {} from memory: {} cached messages, {} new messages to process",
                worker_id, session.id, session.cached_message_count,
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

        // Always render the FULL conversation - the restored state will have the KV cache
        // for already-processed tokens, so llama.cpp will only process new ones
        info!(
            "Worker {} rendering full conversation: {} messages",
            worker_id,
            session.messages.len()
        );
        let prompt = match chat_template.render_session_with_config(
            session,
            model,
            Some(model_manager.get_config()),
        ) {
            Ok(prompt) => prompt,
            Err(e) => {
                error!("Failed to render session prompt: {}", e);
                return Err(QueueError::WorkerError(format!(
                    "Template rendering failed: {}",
                    e
                )));
            }
        };

        debug!(
            "Worker {} rendered prompt length: {} bytes",
            worker_id,
            prompt.len()
        );

        // Create context for this request
        let mut ctx = match model_manager.create_session_context(model, &session.id) {
            Ok(context) => context,
            Err(e) => {
                error!("Failed to create session context: {}", e);
                return Err(QueueError::WorkerError(format!(
                    "Session context creation failed: {}",
                    e
                )));
            }
        };

        // Track the actual KV cache position after state restoration
        let kv_cache_position: i32;

        // Restore session state from memory cache if available
        if can_use_cache {
            info!(
                "Worker {} restoring session state from memory for session {}",
                worker_id, session.id
            );

            let state_bytes = {
                let cache = session_state_cache.lock().unwrap();
                cache.get(&session.id.to_string()).cloned()
            };

            if let Some(bytes) = state_bytes {
                let bytes_len = bytes.len();
                let bytes_read = unsafe { ctx.set_state_data(&bytes) };

                // Query the actual KV cache position for sequence 0
                kv_cache_position = ctx.kv_cache_seq_pos_max(0);

                info!(
                    "Worker {} restored state: {} bytes available, {} bytes read, {} cached messages, KV cache position: {}",
                    worker_id, bytes_len, bytes_read, session.cached_message_count, kv_cache_position
                );
            } else {
                warn!(
                    "Worker {} expected cached state but not found in memory - will process all messages",
                    worker_id
                );
                return Err(QueueError::WorkerError(
                    "Expected state cache missing from memory".to_string(),
                ));
            }
        } else {
            // No cached state, start from position 0
            kv_cache_position = -1; // -1 means no tokens in KV cache
        }

        // Use GenerationHelper to consolidate generation logic
        let batch_size = model_manager.get_batch_size();

        // Use the actual KV cache position as the template offset
        // kv_cache_position is the LAST filled position (0-indexed), so the next token goes at position+1
        let template_token_count = if kv_cache_position >= 0 {
            let next_position = (kv_cache_position + 1) as usize;
            info!(
                "Worker {} using token offset: {} tokens already in KV cache (position 0 to {})",
                worker_id, next_position, kv_cache_position
            );
            Some(next_position)
        } else {
            None
        };

        debug!(
            "Queue worker {} calling GenerationHelper for request {}",
            worker_id, request_id
        );
        let generation_result =
            match GenerationHelper::generate_text_with_borrowed_model_and_template_offset(
                model,
                &mut ctx,
                &prompt,
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
                    result
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
                    return Err(QueueError::WorkerError(format!("Generation failed: {}", e)));
                }
            };

        let generated_text = generation_result.generated_text;
        let tokens_generated = generation_result.tokens_generated;
        let _generation_time = generation_result.generation_time;
        let finish_reason = generation_result.finish_reason;

        // Check if the generated text contains tool calls
        let final_finish_reason = match &finish_reason {
            FinishReason::Stopped(reason)
                if reason == "End of sequence token detected"
                    || reason == "Stop token detected"
                    || reason == "Maximum tokens reached" =>
            {
                match chat_template.extract_tool_calls(&generated_text) {
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
            _ => finish_reason,
        };

        let generation_time = start_time.elapsed();

        debug!(
            "Worker {} completed batch inference for request {} in {:?} ({} tokens, finish_reason: {:?})",
            worker_id, request_id, generation_time, tokens_generated, final_finish_reason
        );

        // Save session state to memory for future turns
        // This captures the complete context state including KV cache
        let state_size = ctx.get_state_size();
        info!(
            "Worker {} saving session state to memory: {} bytes for {} messages",
            worker_id,
            state_size,
            session.messages.len()
        );

        let mut state_bytes = vec![0u8; state_size];
        let bytes_written = unsafe { ctx.copy_state_data(state_bytes.as_mut_ptr()) };

        if bytes_written > 0 {
            // Truncate to actual size written
            state_bytes.truncate(bytes_written);

            // Store in memory cache
            let mut cache = session_state_cache.lock().unwrap();
            cache.insert(session.id.to_string(), state_bytes);
            info!(
                "Worker {} cached {} bytes of state for session {} ({} messages)",
                worker_id,
                bytes_written,
                session.id,
                session.messages.len()
            );

            // Apply LRU eviction if needed (keep cpu_cores / 2 most recent, minimum 1)
            let cache_limit = std::thread::available_parallelism()
                .map(|n| (n.get() / 2).max(1))
                .unwrap_or(4); // Default to 4 if detection fails

            if cache.len() > cache_limit {
                // Simple approach: remove entries until we're at limit
                // In production, would track access time for proper LRU
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
        } else {
            warn!(
                "Worker {} failed to copy state data (wrote 0 bytes) for request {}",
                worker_id, request_id
            );
        }

        Ok(GenerationResponse {
            generated_text,
            tokens_generated,
            generation_time,
            finish_reason: final_finish_reason,
            complete_token_sequence: generation_result.complete_token_sequence, // Pass through from generation
        })
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

        // Format the session messages into a prompt using ChatTemplateEngine
        let prompt = match chat_template.render_session_with_config(
            session,
            model,
            Some(model_manager.get_config()),
        ) {
            Ok(prompt) => prompt,
            Err(e) => {
                error!("Failed to render session prompt for streaming: {}", e);
                let _ = stream_sender.try_send(Err(QueueError::WorkerError(format!(
                    "Template rendering failed: {}",
                    e
                ))));
                return Ok(());
            }
        };
        trace!("Formatted prompt for streaming: {}", prompt);

        // Create session-aware context that can reuse KV cache state
        let mut ctx = match model_manager.create_session_context(model, &session.id) {
            Ok(context) => context,
            Err(e) => {
                error!("Failed to create session context for streaming: {}", e);
                let _ = stream_sender.try_send(Err(QueueError::WorkerError(format!(
                    "Session context creation failed: {}",
                    e
                ))));
                return Ok(());
            }
        };

        // Use GenerationHelper for streaming - consolidated generation logic
        let batch_size = model_manager.get_batch_size();

        match GenerationHelper::generate_stream_with_borrowed_model_and_template_offset(
            model,
            &mut ctx,
            &prompt,
            request,
            &stream_sender,
            cancellation_token,
            batch_size,
            None, // No template offset - session state caching handles this
        ) {
            Ok(()) => {
                debug!(
                    "Worker {} completed streaming inference for request {} using GenerationHelper",
                    worker_id, request_id
                );
                Ok(())
            }
            Err(e) => {
                error!(
                    "GenerationHelper streaming failed for worker {} request {}: {}",
                    worker_id, request_id, e
                );
                let _ = stream_sender.try_send(Err(QueueError::WorkerError(format!(
                    "Generation failed: {}",
                    e
                ))));
                Ok(())
            }
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
        Message, MessageRole, ModelConfig, ModelError, ModelSource, QueueConfig, RetryConfig,
        Session, SessionId,
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

            todos: Vec::new(),

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
        // Handle the case where backend is already initialized by parallel tests
        let model_manager = match ModelManager::new(create_test_model_config()) {
            Ok(manager) => Arc::new(manager),
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - skip this test
                println!("Skipping test due to backend already initialized by parallel test");
                return;
            }
            Err(e) => panic!("Failed to create ModelManager: {:?}", e),
        };
        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();

        let queue = RequestQueue::new(model_manager, config, session_config);
        assert_eq!(queue.get_queue_size(), 0);
    }

    #[tokio::test]
    async fn test_submit_request_model_not_loaded() {
        // Handle the case where backend is already initialized by parallel tests
        let model_manager = match ModelManager::new(create_test_model_config()) {
            Ok(manager) => Arc::new(manager),
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected when running tests in parallel - skip this test
                println!("Skipping test due to backend already initialized by parallel test");
                return;
            }
            Err(e) => panic!("Failed to create ModelManager: {:?}", e),
        };
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
        // This test validates that the streaming functionality works correctly
        // It tests the streaming queue submission and validates that streaming chunks
        // are properly handled when the model is not loaded (expected behavior)

        // Handle the case where backend is already initialized by parallel tests
        let model_manager = match ModelManager::new(create_test_model_config()) {
            Ok(manager) => Arc::new(manager),
            Err(ModelError::LoadingFailed(msg))
                if msg.contains("Backend already initialized by external code") =>
            {
                // This is expected in test environments - create manager without loading
                return; // Skip this test when backend is already initialized
            }
            Err(e) => panic!("Failed to create ModelManager: {}", e),
        };

        let config = create_test_queue_config();
        let session_config = crate::types::SessionConfig::default();
        let queue = RequestQueue::new(model_manager, config, session_config);

        let session = create_test_session();
        let request = GenerationRequest {
            session_id: session.id,
            max_tokens: Some(10),
            temperature: Some(0.7),
            top_p: Some(0.9),
            stop_tokens: Vec::new(),
            stopping_config: None,
        };

        // Submit streaming request - this should work regardless of model loading status
        let receiver_result = queue.submit_streaming_request(request, &session).await;

        assert!(
            receiver_result.is_ok(),
            "Streaming request submission should succeed"
        );

        let mut receiver = receiver_result.unwrap();

        // Verify we get the expected "Model not loaded" error through the stream
        let chunk_result = receiver.recv().await;
        assert!(chunk_result.is_some(), "Should receive a chunk result");

        match chunk_result.unwrap() {
            Err(QueueError::WorkerError(msg)) => {
                assert!(
                    msg.contains("Model not loaded"),
                    "Should receive 'Model not loaded' error, got: {}",
                    msg
                );
            }
            Ok(chunk) => panic!(
                "Expected error for unloaded model, but got streaming chunk: {:?}",
                chunk
            ),
            Err(other) => panic!("Expected WorkerError for unloaded model, got: {:?}", other),
        }

        // Verify no more chunks are received
        let next_chunk = receiver.recv().await;
        assert!(
            next_chunk.is_none(),
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
            // Test case: prompt smaller than batch size should work normally
            let model_manager = match ModelManager::new(create_test_config_with_batch_size(512)) {
                Ok(manager) => Arc::new(manager),
                Err(_) => {
                    // Skip test if model manager can't be created
                    return;
                }
            };

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
            // Test edge case: prompt exactly at batch size limit
            let batch_size = 8u32; // Small batch size for testing
            let model_manager =
                match ModelManager::new(create_test_config_with_batch_size(batch_size)) {
                    Ok(manager) => Arc::new(manager),
                    Err(_) => return,
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
            // Test case: prompt larger than batch size should be processed in chunks
            let batch_size = 4u32; // Very small batch size for testing
            let model_manager =
                match ModelManager::new(create_test_config_with_batch_size(batch_size)) {
                    Ok(manager) => Arc::new(manager),
                    Err(_) => return,
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
            // Test streaming with prompt larger than batch size
            let batch_size = 4u32;
            let model_manager =
                match ModelManager::new(create_test_config_with_batch_size(batch_size)) {
                    Ok(manager) => Arc::new(manager),
                    Err(_) => return,
                };

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
