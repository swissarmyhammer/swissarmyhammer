//! Shared bounded agent pool.
//!
//! [`AgentPool`] is the single place parallelism is controlled for the whole
//! review pipeline. Callers submit prompts at any time via [`AgentPool::submit`];
//! a fixed set of worker tasks drains one shared internal queue and runs each
//! prompt against an ACP agent connection. Fan-out and verify both submit to the
//! same pool, so verify tasks pipeline alongside still-running fan-out tasks.
//!
//! ## Worker count is the only concurrency control
//!
//! Worker count is legitimate and physical, not arbitrary:
//! - **local Llama backend → 1 worker** (one in-process model/GPU).
//! - **remote/Claude-API backend → N workers**, default from config. The
//!   [`PoolConfig::aimd`] flag is reserved for adapting that count to discover
//!   the API ceiling, but the adaptive logic is not yet wired up (see the field
//!   note); the count is fixed for the lifetime of the pool today.
//! - a `review.concurrency` config value pins N when set (see
//!   [`PoolConfig::with_concurrency`]).
//!
//! Submission is unbounded and non-blocking (the queue absorbs it); only the
//! worker count's worth of prompts run at once. The per-call token cap
//! ([`PoolConfig::max_tokens`]) is retained and attached to every prompt.

use std::sync::Arc;

use agent_client_protocol::schema::{
    ContentBlock, NewSessionRequest, PromptRequest, SessionNotification, TextContent,
};
use agent_client_protocol::{Agent, ConnectionTo};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;

/// Minimum worker count. A pool always has at least one worker so it can make
/// forward progress.
const MIN_WORKERS: usize = 1;

/// Maximum worker count for the remote backend default.
const MAX_REMOTE_WORKERS: usize = 8;

/// Default per-call cap on generation tokens for a single `submit` prompt.
pub const DEFAULT_MAX_TOKENS: u64 = 16 * 1024;

/// Defensive ceiling on a single prompt turn (`new_session` → `prompt`).
///
/// A correctly wired client answers every nested agent→client request, so a turn
/// completes in seconds-to-minutes. This generous backstop exists only so a
/// future wedge (e.g. an unanswered agent request) degrades that one task to an
/// error — the fleet reports zero findings for it and the review COMPLETES —
/// instead of hanging the whole review forever. It is tuned far above any
/// legitimately slow turn so it never false-fires in normal operation.
const PROMPT_TURN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Backend-aware policy describing how many workers the pool runs and whether it
/// adapts that count under load.
#[derive(Debug, Clone, Copy)]
pub struct PoolConfig {
    /// Number of worker tasks that drain the shared queue.
    pub workers: usize,
    /// Whether the pool adapts its active worker count under pressure (AIMD).
    // reserved for AIMD; not yet consumed — set by the constructors and asserted
    // in tests, but `AgentPool::new`/`worker_loop` do not yet read it. The
    // adaptive logic lands with the follow-up task that wires the pool into a
    // live backend.
    pub aimd: bool,
    /// Per-call cap on generation tokens attached to every submitted prompt.
    pub max_tokens: u64,
}

impl PoolConfig {
    /// Policy for a local in-process model/GPU backend: exactly one worker.
    pub fn local() -> Self {
        Self {
            workers: 1,
            aimd: false,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }

    /// Policy for a remote/Claude-API backend.
    pub fn remote(default_workers: usize) -> Self {
        Self {
            workers: default_workers.clamp(MIN_WORKERS, MAX_REMOTE_WORKERS),
            aimd: true,
            max_tokens: DEFAULT_MAX_TOKENS,
        }
    }

    /// Pin the worker count to an explicit value (the `review.concurrency`
    /// override). A pinned count disables AIMD.
    pub fn with_concurrency(mut self, workers: usize) -> Self {
        self.workers = workers.max(MIN_WORKERS);
        self.aimd = false;
        self
    }

    /// Override the per-call generation token cap.
    pub fn with_max_tokens(mut self, max_tokens: u64) -> Self {
        self.max_tokens = max_tokens;
        self
    }
}

/// Result of a single submitted prompt.
pub type PromptResult = Result<claude_agent::CollectedResponse, claude_agent::AgentError>;

/// A unit of work on the shared queue: a prompt plus the channel to deliver its
/// result back to the submitter.
struct Job {
    prompt: String,
    respond_to: oneshot::Sender<PromptResult>,
}

/// Shared bounded pool of agent workers draining a single submission queue.
///
/// Construct with [`AgentPool::new`], submit prompts with [`AgentPool::submit`].
/// Dropping the pool closes the queue; in-flight prompts finish and idle workers
/// wind down.
pub struct AgentPool {
    /// Sender half of the shared unbounded queue. Submission never blocks.
    tx: mpsc::UnboundedSender<Job>,
    /// Worker task handles, aborted on drop.
    workers: Vec<JoinHandle<()>>,
    /// Number of workers (the in-flight cap).
    worker_count: usize,
    /// Per-call generation token cap attached to every prompt.
    max_tokens: u64,
}

impl AgentPool {
    /// Create a pool of `config.workers` workers draining a shared queue, each
    /// issuing prompts over a clone of `agent` and subscribing to `notifier`
    /// for streaming response content.
    pub fn new(
        agent: ConnectionTo<Agent>,
        notifier: Arc<claude_agent::NotificationSender>,
        config: PoolConfig,
    ) -> Self {
        let worker_count = config.workers.max(MIN_WORKERS);
        let (tx, rx) = mpsc::unbounded_channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let rx = Arc::clone(&rx);
            let agent = agent.clone();
            let notifier = Arc::clone(&notifier);
            let max_tokens = config.max_tokens;
            workers.push(tokio::spawn(async move {
                worker_loop(rx, agent, notifier, max_tokens).await;
            }));
        }

        Self {
            tx,
            workers,
            worker_count,
            max_tokens: config.max_tokens,
        }
    }

    /// Submit a prompt to the shared queue and return a future that resolves to
    /// its result once a worker has run it.
    ///
    /// Submission is non-blocking: the prompt is enqueued immediately even if
    /// all workers are busy. Only [`AgentPool::worker_count`] prompts run
    /// concurrently. Tasks submitted while the pool is draining are picked up by
    /// the next free worker (pipelining).
    pub fn submit(&self, prompt: impl Into<String>) -> oneshot::Receiver<PromptResult> {
        let (respond_to, rx) = oneshot::channel();
        let job = Job {
            prompt: prompt.into(),
            respond_to,
        };
        if let Err(returned) = self.tx.send(job) {
            // The pool's workers are all gone; deliver an error rather than
            // hanging the submitter's future forever.
            let _ = returned
                .0
                .respond_to
                .send(Err(claude_agent::AgentError::Internal(
                    "agent pool is shut down".to_string(),
                )));
        }
        rx
    }

    /// Number of worker tasks draining the queue — the in-flight cap.
    pub fn worker_count(&self) -> usize {
        self.worker_count
    }

    /// Per-call generation token cap attached to every submitted prompt.
    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }
}

impl Drop for AgentPool {
    fn drop(&mut self) {
        for worker in &self.workers {
            worker.abort();
        }
    }
}

/// Drain the shared queue until it closes, running each job's prompt against the
/// agent connection and delivering the result back to the submitter.
///
/// Workers contend on the shared receiver: whichever worker is free pulls the
/// next job. The receiver lock is held only across `recv`, not across prompt
/// execution, so the other workers pull the next job concurrently. A slow or
/// erroring prompt only ties up the one worker running it.
async fn worker_loop(
    rx: Arc<Mutex<mpsc::UnboundedReceiver<Job>>>,
    agent: ConnectionTo<Agent>,
    notifier: Arc<claude_agent::NotificationSender>,
    max_tokens: u64,
) {
    loop {
        let job = {
            let mut guard = rx.lock().await;
            guard.recv().await
        };
        let Some(job) = job else {
            // Queue closed and drained: the pool was dropped.
            break;
        };

        let notifications = notifier.sender().subscribe();
        // Backstop the turn with a generous timeout so a wedged prompt (e.g. a
        // nested agent request the client failed to answer) degrades this one
        // task to an error rather than hanging the whole review forever.
        let result = match tokio::time::timeout(
            PROMPT_TURN_TIMEOUT,
            run_prompt(&agent, notifications, job.prompt, max_tokens),
        )
        .await
        {
            Ok(result) => result,
            Err(_elapsed) => Err(claude_agent::AgentError::Internal(format!(
                "prompt turn exceeded {}s and was abandoned",
                PROMPT_TURN_TIMEOUT.as_secs()
            ))),
        };
        // The submitter may have dropped its receiver; that is fine.
        let _ = job.respond_to.send(result);
    }
}

/// Drive a single prompt turn against an ACP 0.12 agent connection.
///
/// Routes the two per-prompt ACP requests (`new_session` → `prompt`) through the
/// typed [`ConnectionTo<Agent>`] handle, spawning a per-session notification
/// collector so streaming `session/update` content is captured concurrently with
/// the prompt. The per-call token cap is attached to the prompt request's `meta`
/// map under the key `"max_tokens"`.
///
/// `initialize` is deliberately NOT here: it is a once-per-connection handshake
/// the driver performs before any worker runs (see
/// `review::drive::run_pipeline_in_connection`). Issuing it per prompt raced N
/// concurrent handshakes over the single shared connection and wedged the real
/// agent.
async fn run_prompt(
    agent: &ConnectionTo<Agent>,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: String,
    max_tokens: u64,
) -> PromptResult {
    // new_session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_response = agent
        .send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("Failed to create session: {}", e))
        })?;
    let session_id = session_response.session_id;

    // 3. spawn notification collector before prompt() so streaming content is
    //    captured as it arrives.
    let (collector, collected_text, notification_count, _matched_count) =
        claude_agent::spawn_notification_collector(notifications, session_id.clone());

    // 4. prompt, with the per-call token cap attached via `meta`.
    let mut meta = serde_json::Map::new();
    meta.insert("max_tokens".to_string(), serde_json::json!(max_tokens));
    let prompt_request = PromptRequest::new(
        session_id,
        vec![ContentBlock::Text(TextContent::new(prompt))],
    )
    .meta(meta);
    let prompt_response = agent
        .send_request(prompt_request)
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("Failed to execute prompt: {}", e))
        })?;

    // 5. drain trailing notifications and assemble the collected response.
    let content = claude_agent::collect_response_content(
        collector,
        collected_text,
        notification_count,
        &prompt_response,
    )
    .await;

    Ok(claude_agent::CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use agent_client_protocol::schema::{
        AuthenticateRequest, AuthenticateResponse, CancelNotification, ExtNotification, ExtRequest,
        ExtResponse, InitializeRequest, InitializeResponse, LoadSessionRequest,
        LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest, PromptResponse,
        SessionNotification, SetSessionModeRequest, SetSessionModeResponse,
    };
    use agent_client_protocol::{Channel, Client, ConnectTo};
    use agent_client_protocol_extras::PlaybackAgent;
    use futures::future::BoxFuture;
    use tempfile::TempDir;

    // ------------------------------------------------------------------
    // Mock-agent harness (same shape as the retired runner's harness:
    // MockAgent trait + adapter + in-process client wiring).
    // ------------------------------------------------------------------

    /// Trait the test mock agents implement so a single [`MockAgentAdapter`] can
    /// route incoming `ClientRequest` variants onto the right handler.
    trait MockAgent: Send + Sync {
        fn initialize<'a>(
            &'a self,
            _request: InitializeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<InitializeResponse>> {
            Box::pin(async move { Ok(InitializeResponse::new(1.into())) })
        }

        fn authenticate<'a>(
            &'a self,
            _request: AuthenticateRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<AuthenticateResponse>> {
            Box::pin(async move { Ok(AuthenticateResponse::new()) })
        }

        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn load_session<'a>(
            &'a self,
            _request: LoadSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<LoadSessionResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn set_session_mode<'a>(
            &'a self,
            _request: SetSessionModeRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<SetSessionModeResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn cancel<'a>(
            &'a self,
            _notification: CancelNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move { Ok(()) })
        }

        fn ext_method<'a>(
            &'a self,
            _request: ExtRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<ExtResponse>> {
            Box::pin(async move { Err(agent_client_protocol::Error::method_not_found()) })
        }

        fn ext_notification<'a>(
            &'a self,
            _notification: ExtNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            Box::pin(async move { Ok(()) })
        }
    }

    /// `ConnectTo<Client>` adapter that drives a [`MockAgent`] as an ACP server.
    struct MockAgentAdapter<M: MockAgent + 'static>(Arc<M>);

    impl<M: MockAgent + 'static> ConnectTo<Client> for MockAgentAdapter<M> {
        async fn connect_to(
            self,
            client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
        ) -> agent_client_protocol::Result<()> {
            let mock = Arc::clone(&self.0);
            let mock_for_notifications = Arc::clone(&self.0);

            agent_client_protocol::Agent
                .builder()
                .name("mock-agent")
                .on_receive_request(
                    {
                        let mock = Arc::clone(&mock);
                        async move |req: agent_client_protocol::ClientRequest, responder, cx| {
                            dispatch_mock_request(&mock, req, responder, &cx)
                        }
                    },
                    agent_client_protocol::on_receive_request!(),
                )
                .on_receive_notification(
                    async move |notif: agent_client_protocol::ClientNotification, _cx| {
                        dispatch_mock_notification(&mock_for_notifications, notif).await;
                        Ok(())
                    },
                    agent_client_protocol::on_receive_notification!(),
                )
                .connect_to(client)
                .await
        }
    }

    /// Demultiplex an incoming `ClientRequest` onto the mock's per-method
    /// handlers. Each dispatch is offloaded to `cx.spawn` so the SDK event loop
    /// keeps dispatching new requests while a slow handler is awaiting — without
    /// the spawn, two concurrent prompts on one connection would serialise.
    fn dispatch_mock_request<M: MockAgent + 'static>(
        mock: &Arc<M>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
        cx: &ConnectionTo<Client>,
    ) -> agent_client_protocol::Result<()> {
        use agent_client_protocol::ClientRequest as Req;

        let mock = Arc::clone(mock);
        cx.spawn(async move {
            match request {
                Req::InitializeRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.initialize(req).await),
                Req::AuthenticateRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.authenticate(req).await),
                Req::NewSessionRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.new_session(req).await),
                Req::LoadSessionRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.load_session(req).await),
                Req::SetSessionModeRequest(req) => responder
                    .cast()
                    .respond_with_result(mock.set_session_mode(req).await),
                Req::PromptRequest(req) => {
                    responder.cast().respond_with_result(mock.prompt(req).await)
                }
                Req::ExtMethodRequest(req) => {
                    let result = mock.ext_method(req).await.and_then(|ext_response| {
                        serde_json::from_str::<serde_json::Value>(ext_response.0.get())
                            .map_err(|_| agent_client_protocol::Error::internal_error())
                    });
                    responder.respond_with_result(result)
                }
                _ => responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found()),
            }
        })
    }

    /// Demultiplex an incoming `ClientNotification` onto the mock.
    async fn dispatch_mock_notification<M: MockAgent + ?Sized>(
        mock: &Arc<M>,
        notification: agent_client_protocol::ClientNotification,
    ) {
        use agent_client_protocol::ClientNotification as Notif;

        match notification {
            Notif::CancelNotification(n) => {
                let _ = mock.cancel(n).await;
            }
            Notif::ExtNotification(n) => {
                let _ = mock.ext_notification(n).await;
            }
            _ => {}
        }
    }

    /// Wire a [`MockAgent`] up to a fresh `Client` and run `body` against the
    /// resulting `ConnectionTo<Agent>` handle.
    async fn run_with_mock_agent<M, F, Fut, R>(
        mock: Arc<M>,
        notifier: Arc<claude_agent::NotificationSender>,
        body: F,
    ) -> R
    where
        M: MockAgent + 'static,
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (channel_a, channel_b) = Channel::duplex();

        let agent_task = tokio::spawn(async move {
            let _ = MockAgentAdapter(mock).connect_to(channel_a).await;
        });

        let result = run_client_against(channel_b, notifier, "mock-test-client", body).await;

        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// Wire a [`PlaybackAgent`] the same way as a [`MockAgent`].
    async fn run_with_playback_agent<F, Fut, R>(
        agent: PlaybackAgent,
        notifier: Arc<claude_agent::NotificationSender>,
        body: F,
    ) -> R
    where
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let (channel_a, channel_b) = Channel::duplex();

        let agent_task = tokio::spawn(async move {
            let _ = agent.connect_to(channel_a).await;
        });

        let result = run_client_against(channel_b, notifier, "playback-test-client", body).await;

        agent_task.abort();
        let _ = agent_task.await;
        result
    }

    /// Shared client-side wiring: stand up `Client.builder().connect_with(...)`,
    /// forward incoming notifications to `notifier`, and run `body`.
    async fn run_client_against<F, Fut, R>(
        transport: Channel,
        notifier: Arc<claude_agent::NotificationSender>,
        name: &'static str,
        body: F,
    ) -> R
    where
        F: FnOnce(ConnectionTo<Agent>) -> Fut + Send + 'static,
        Fut: std::future::Future<Output = R> + Send + 'static,
        R: Send + 'static,
    {
        let notifier_for_handler = Arc::clone(&notifier);
        Client
            .builder()
            .name(name)
            .on_receive_notification(
                async move |notif: SessionNotification, _cx| {
                    let _ = notifier_for_handler.send_update(notif).await;
                    Ok(())
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, async move |conn: ConnectionTo<Agent>| {
                Ok(body(conn).await)
            })
            .await
            .expect("client connect_with failed")
    }

    fn new_notifier() -> Arc<claude_agent::NotificationSender> {
        let (notifier, _) = claude_agent::NotificationSender::new(64);
        Arc::new(notifier)
    }

    // ------------------------------------------------------------------
    // Mock agents
    // ------------------------------------------------------------------

    /// Mints a fresh session per `new_session` and returns a passing response.
    struct PassingAgent {
        next_session: AtomicUsize,
    }

    impl PassingAgent {
        fn new() -> Self {
            Self {
                next_session: AtomicUsize::new(0),
            }
        }
    }

    impl MockAgent for PassingAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            let n = self.next_session.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(NewSessionResponse::new(
                    agent_client_protocol::schema::SessionId::new(format!("pass-sess-{}", n)),
                ))
            })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            Box::pin(async move {
                Ok(PromptResponse::new(
                    agent_client_protocol::schema::StopReason::EndTurn,
                ))
            })
        }
    }

    /// Sleeps `sleep_ms` inside `prompt`, recording the peak number of prompts
    /// concurrently in flight. The peak is a time-free proof of how many prompts
    /// overlapped, used to assert the pool never exceeds its worker count.
    struct PeakProbeAgent {
        next_session: AtomicUsize,
        sleep_ms: u64,
        current: AtomicUsize,
        peak: AtomicUsize,
    }

    impl PeakProbeAgent {
        fn new(sleep_ms: u64) -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                sleep_ms,
                current: AtomicUsize::new(0),
                peak: AtomicUsize::new(0),
            }
        }

        fn peak_in_flight(&self) -> usize {
            self.peak.load(Ordering::SeqCst)
        }
    }

    impl MockAgent for PeakProbeAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            let n = self.next_session.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(NewSessionResponse::new(
                    agent_client_protocol::schema::SessionId::new(format!("peak-sess-{}", n)),
                ))
            })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let now = self.current.fetch_add(1, Ordering::SeqCst) + 1;
            self.peak.fetch_max(now, Ordering::SeqCst);
            let sleep_ms = self.sleep_ms;
            Box::pin(async move {
                tokio::time::sleep(std::time::Duration::from_millis(sleep_ms)).await;
                self.current.fetch_sub(1, Ordering::SeqCst);
                Ok(PromptResponse::new(
                    agent_client_protocol::schema::StopReason::EndTurn,
                ))
            })
        }
    }

    /// Errors on the first `error_count` prompts, then passes — used to prove a
    /// failing task does not deadlock the pool.
    struct ErroringAgent {
        next_session: AtomicUsize,
        remaining_errors: AtomicUsize,
    }

    impl ErroringAgent {
        fn new(error_count: usize) -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                remaining_errors: AtomicUsize::new(error_count),
            }
        }
    }

    impl MockAgent for ErroringAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            let n = self.next_session.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(NewSessionResponse::new(
                    agent_client_protocol::schema::SessionId::new(format!("err-sess-{}", n)),
                ))
            })
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let should_error = self
                .remaining_errors
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| {
                    if n > 0 {
                        Some(n - 1)
                    } else {
                        None
                    }
                })
                .is_ok();
            Box::pin(async move {
                if should_error {
                    Err(agent_client_protocol::Error::internal_error())
                } else {
                    Ok(PromptResponse::new(
                        agent_client_protocol::schema::StopReason::EndTurn,
                    ))
                }
            })
        }
    }

    /// Records the `max_tokens` value from each prompt's `meta` map.
    struct MetaRecordingAgent {
        next_session: AtomicUsize,
        recorded_max_tokens: std::sync::Mutex<Vec<Option<u64>>>,
    }

    impl MetaRecordingAgent {
        fn new() -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                recorded_max_tokens: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn recorded(&self) -> Vec<Option<u64>> {
            self.recorded_max_tokens.lock().unwrap().clone()
        }
    }

    impl MockAgent for MetaRecordingAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            let n = self.next_session.fetch_add(1, Ordering::SeqCst);
            Box::pin(async move {
                Ok(NewSessionResponse::new(
                    agent_client_protocol::schema::SessionId::new(format!("meta-sess-{}", n)),
                ))
            })
        }

        fn prompt<'a>(
            &'a self,
            request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let max_tokens = request
                .meta
                .as_ref()
                .and_then(|m| m.get("max_tokens"))
                .and_then(|v| v.as_u64());
            self.recorded_max_tokens.lock().unwrap().push(max_tokens);
            Box::pin(async move {
                Ok(PromptResponse::new(
                    agent_client_protocol::schema::StopReason::EndTurn,
                ))
            })
        }
    }

    fn create_playback_fixture(response_json: &str) -> (TempDir, std::path::PathBuf) {
        let temp = TempDir::new().unwrap();
        let fixture_path = temp.path().join("playback.json");
        std::fs::write(&fixture_path, response_json).unwrap();
        (temp, fixture_path)
    }

    /// A recorded session that round-trips new_session → prompt, with the
    /// prompt's assistant content delivered via a streamed `agent_message_chunk`
    /// notification (the same shape the production agents emit). The content marks
    /// the validation as passed.
    ///
    /// No `initialize` call is recorded: the pool's `run_prompt` no longer issues
    /// `initialize` per prompt — that is a once-per-connection handshake the
    /// driver performs (`review::drive::run_pipeline_in_connection`), so a worker
    /// only round-trips `new_session` → `prompt`.
    const PLAYBACK_PASS: &str = r#"{
  "calls": [
    {
      "method": "new_session",
      "request": { "cwd": "/tmp", "mcpServers": [] },
      "response": { "sessionId": "test-session" },
      "notifications": []
    },
    {
      "method": "prompt",
      "request": { "prompt": [{"type": "text", "text": "validate this"}], "sessionId": "test-session" },
      "response": { "stopReason": "end_turn" },
      "notifications": [
        {
          "sessionId": "test-session",
          "update": { "sessionUpdate": "agent_message_chunk", "content": {"type":"text","text":"{\"status\": \"passed\", \"message\": \"All checks passed\"}"} }
        }
      ]
    }
  ]
}"#;

    // ------------------------------------------------------------------
    // PoolConfig tests
    // ------------------------------------------------------------------

    #[test]
    fn test_pool_config_local_is_single_worker() {
        let config = PoolConfig::local();
        assert_eq!(config.workers, 1, "local backend must run exactly 1 worker");
        assert!(!config.aimd, "local backend must not adapt worker count");
    }

    #[test]
    fn test_pool_config_remote_clamps_workers() {
        assert_eq!(PoolConfig::remote(4).workers, 4);
        assert_eq!(
            PoolConfig::remote(100).workers,
            8,
            "remote default must clamp to the friendly ceiling"
        );
        assert_eq!(
            PoolConfig::remote(0).workers,
            1,
            "remote default must keep at least one worker"
        );
        assert!(PoolConfig::remote(4).aimd, "remote backend enables AIMD");
    }

    #[test]
    fn test_pool_config_with_concurrency_pins_and_disables_aimd() {
        let config = PoolConfig::remote(8).with_concurrency(3);
        assert_eq!(config.workers, 3, "review.concurrency override pins N");
        assert!(!config.aimd, "a pinned worker count disables AIMD");
    }

    // ------------------------------------------------------------------
    // AgentPool behavioural tests
    // ------------------------------------------------------------------

    /// Submit M tasks to a pool of N workers: all M results return.
    #[tokio::test]
    async fn test_pool_returns_all_results() {
        let agent = Arc::new(PassingAgent::new());
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(3));

            let m = 7;
            let receivers: Vec<_> = (0..m)
                .map(|i| pool.submit(format!("prompt {}", i)))
                .collect();

            let mut completed = 0;
            for rx in receivers {
                let result = rx.await.expect("worker should deliver a result");
                assert!(result.is_ok(), "each prompt should succeed: {:?}", result);
                completed += 1;
            }
            assert_eq!(completed, m, "all M submitted prompts must return a result");
        })
        .await;
    }

    /// Never more than N agents in flight at once.
    #[tokio::test]
    async fn test_pool_never_exceeds_worker_count() {
        let workers = 2;
        let agent = Arc::new(PeakProbeAgent::new(100));
        let agent_probe = Arc::clone(&agent);
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(workers));

            // Submit far more tasks than workers.
            let receivers: Vec<_> = (0..8).map(|i| pool.submit(format!("p{}", i))).collect();
            for rx in receivers {
                rx.await.expect("result").expect("ok");
            }

            assert!(
                agent_probe.peak_in_flight() <= workers,
                "pool must never run more than {} prompts at once, peak was {}",
                workers,
                agent_probe.peak_in_flight(),
            );
            assert!(
                agent_probe.peak_in_flight() >= 2,
                "with multiple workers and 8 tasks the pool should overlap at least 2",
            );
        })
        .await;
    }

    /// Tasks submitted mid-drain (pipelining) are picked up.
    #[tokio::test]
    async fn test_pool_pipelines_late_submissions() {
        let agent = Arc::new(PassingAgent::new());
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(2));

            // First batch.
            let first = pool.submit("first");
            first.await.expect("result").expect("ok");

            // Submit a second batch *after* the pool already drained the first —
            // the same long-lived workers must pick these up.
            let second: Vec<_> = (0..4).map(|i| pool.submit(format!("late{}", i))).collect();
            for rx in second {
                rx.await
                    .expect("late submission must be picked up by a worker")
                    .expect("ok");
            }
        })
        .await;
    }

    /// A local-backend policy runs strictly one prompt at a time.
    #[tokio::test]
    async fn test_pool_local_runs_one_at_a_time() {
        let agent = Arc::new(PeakProbeAgent::new(50));
        let agent_probe = Arc::clone(&agent);
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            assert_eq!(pool.worker_count(), 1, "local pool must have one worker");

            let receivers: Vec<_> = (0..5).map(|i| pool.submit(format!("p{}", i))).collect();
            for rx in receivers {
                rx.await.expect("result").expect("ok");
            }

            assert_eq!(
                agent_probe.peak_in_flight(),
                1,
                "local backend must never run two prompts concurrently",
            );
        })
        .await;
    }

    /// One slow/erroring task doesn't deadlock the pool: other tasks still
    /// complete and the erroring task returns an Err rather than hanging.
    #[tokio::test]
    async fn test_pool_erroring_task_does_not_deadlock() {
        // The agent errors on its first prompt then passes.
        let agent = Arc::new(ErroringAgent::new(1));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(2));

            let receivers: Vec<_> = (0..4).map(|i| pool.submit(format!("p{}", i))).collect();

            let mut oks = 0;
            let mut errs = 0;
            for rx in receivers {
                match rx.await.expect("worker must deliver a result, not hang") {
                    Ok(_) => oks += 1,
                    Err(_) => errs += 1,
                }
            }
            assert_eq!(errs, 1, "exactly one prompt should have errored");
            assert_eq!(oks, 3, "the remaining prompts must still complete");
        })
        .await;
    }

    /// The per-call token cap is attached to every submitted prompt's `meta`.
    #[tokio::test]
    async fn test_pool_attaches_max_tokens_cap() {
        let agent = Arc::new(MetaRecordingAgent::new());
        let agent_probe = Arc::clone(&agent);
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(1));
            assert_eq!(pool.max_tokens(), DEFAULT_MAX_TOKENS);

            pool.submit("p0").await.expect("result").expect("ok");

            let recorded = agent_probe.recorded();
            assert_eq!(recorded.len(), 1);
            assert_eq!(
                recorded[0],
                Some(DEFAULT_MAX_TOKENS),
                "every prompt must carry the per-call max_tokens cap in meta",
            );
        })
        .await;
    }

    /// A PlaybackAgent submitted through the pool returns its recorded content.
    #[tokio::test]
    async fn test_pool_with_playback_agent() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            let result = pool.submit("validate this").await.expect("result");
            let collected = result.expect("playback agent should respond");
            assert!(
                collected.content.contains("passed"),
                "playback content should round-trip through the pool, got: {}",
                collected.content,
            );
        })
        .await;
    }
}
