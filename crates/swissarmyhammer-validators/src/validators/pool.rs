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
    CancelNotification, ContentBlock, NewSessionRequest, PromptRequest, SessionId,
    SessionNotification, TextContent,
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

/// Idle-progress window for a single prompt turn (`new_session` → `prompt`).
///
/// A turn is abandoned only when NO streaming progress — no `session/update`
/// notification for the turn's session — has arrived for this long. Total wall
/// clock is deliberately NOT capped at this value: a legitimate turn on a local
/// 35B model (big review prompt, agentic loop, one shared GPU serializing
/// decodes across fleet tasks) routinely needs more than 300s, but while it is
/// decoding it streams chunks continuously, so it keeps resetting this window.
/// Only a wedged turn (e.g. a nested agent request the client failed to answer)
/// goes silent for the whole window, and that degrades to a single-task error —
/// the fleet reports zero findings for it and the review COMPLETES — instead of
/// hanging the whole review forever.
pub const PROMPT_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Defensive absolute ceiling on a single prompt turn.
///
/// Catches the one pathology the idle window cannot: a turn that keeps emitting
/// notifications forever without ever completing (e.g. a runaway agentic loop).
/// It must sit far above local-model reality — a single legitimate local-35B
/// turn can take well over five minutes of wall clock when one shared GPU
/// serializes decodes across fleet tasks — so it is set to 45 minutes: long
/// enough to never false-fire on a slow-but-live turn, short enough that a
/// runaway turn cannot pin a worker forever.
pub const PROMPT_TURN_CEILING: std::time::Duration = std::time::Duration::from_secs(45 * 60);

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
    /// Abandon a turn after this long with no streaming progress
    /// (see [`PROMPT_IDLE_TIMEOUT`]).
    pub idle_timeout: std::time::Duration,
    /// Defensive absolute cap on a turn's total wall clock
    /// (see [`PROMPT_TURN_CEILING`]).
    pub turn_ceiling: std::time::Duration,
}

impl PoolConfig {
    /// Policy for a local in-process model/GPU backend: exactly one worker.
    pub fn local() -> Self {
        Self {
            workers: 1,
            aimd: false,
            max_tokens: DEFAULT_MAX_TOKENS,
            idle_timeout: PROMPT_IDLE_TIMEOUT,
            turn_ceiling: PROMPT_TURN_CEILING,
        }
    }

    /// Policy for a remote/Claude-API backend.
    pub fn remote(default_workers: usize) -> Self {
        Self {
            workers: default_workers.clamp(MIN_WORKERS, MAX_REMOTE_WORKERS),
            aimd: true,
            max_tokens: DEFAULT_MAX_TOKENS,
            idle_timeout: PROMPT_IDLE_TIMEOUT,
            turn_ceiling: PROMPT_TURN_CEILING,
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

    /// Override the idle-progress window after which a silent turn is
    /// abandoned. Exists so tests can exercise abandonment with sub-second
    /// durations; production constructors use [`PROMPT_IDLE_TIMEOUT`].
    pub fn with_idle_timeout(mut self, idle_timeout: std::time::Duration) -> Self {
        self.idle_timeout = idle_timeout;
        self
    }

    /// Override the absolute wall-clock ceiling on a turn. Exists so tests can
    /// exercise ceiling abandonment with sub-second durations; production
    /// constructors use [`PROMPT_TURN_CEILING`].
    pub fn with_turn_ceiling(mut self, turn_ceiling: std::time::Duration) -> Self {
        self.turn_ceiling = turn_ceiling;
        self
    }
}

/// Failure of a single submitted prompt.
///
/// The two liveness-abandonment modes are typed variants so callers can tell
/// "the supervisor abandoned the turn (agent alive, single-task degradation)"
/// apart from a genuine agent failure without parsing message text.
#[derive(Debug, thiserror::Error)]
pub enum PoolError {
    /// The turn made no streaming progress for [`PoolConfig::idle_timeout`]
    /// and was abandoned (its session was cancelled).
    #[error("prompt turn made no streaming progress for {idle_timeout:?} and was abandoned")]
    TurnIdle {
        /// The idle-progress window that elapsed without a session update.
        idle_timeout: std::time::Duration,
    },
    /// The turn exceeded [`PoolConfig::turn_ceiling`] of total wall clock and
    /// was abandoned (its session was cancelled).
    #[error("prompt turn exceeded the absolute ceiling of {turn_ceiling:?} and was abandoned")]
    TurnCeiling {
        /// The absolute wall-clock cap the turn exceeded.
        turn_ceiling: std::time::Duration,
    },
    /// The agent connection itself failed the turn.
    #[error(transparent)]
    Agent(#[from] claude_agent::AgentError),
}

/// Result of a single submitted prompt.
pub type PromptResult = Result<claude_agent::CollectedResponse, PoolError>;

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
            workers.push(tokio::spawn(async move {
                worker_loop(rx, agent, notifier, config).await;
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
                )
                .into()));
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
    config: PoolConfig,
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

        let result = run_turn_with_liveness(&agent, &notifier, job.prompt, config).await;
        // The submitter may have dropped its receiver; that is fine.
        let _ = job.respond_to.send(result);
    }
}

/// Supervise one prompt turn with progress-aware liveness.
///
/// The turn (`run_prompt`) races against two abandonment conditions instead of
/// a single wall-clock cap:
///
/// - **idle**: no `session/update` notification for the turn's session within
///   `config.idle_timeout`. Every received update for the session resets the
///   window, so a slow-but-streaming turn (the local-35B case) is never
///   abandoned regardless of total duration.
/// - **ceiling**: `config.turn_ceiling` of total wall clock, the defensive cap
///   on a turn that streams forever without completing.
///
/// On abandonment the in-flight session is actively cancelled (ACP
/// `session/cancel`) so the agent stops decoding, rather than being detached to
/// keep generating into a dropped receiver. The session id is learned from
/// `run_prompt` through a shared slot once `new_session` completes; if the turn
/// wedged before that there is no session to cancel.
async fn run_turn_with_liveness(
    agent: &ConnectionTo<Agent>,
    notifier: &claude_agent::NotificationSender,
    prompt: String,
    config: PoolConfig,
) -> PromptResult {
    let session_slot: Arc<std::sync::Mutex<Option<SessionId>>> = Arc::default();
    // Two independent subscriptions: one consumed by `run_prompt`'s content
    // collector, one watched here for liveness.
    let mut liveness = notifier.sender().subscribe();
    let notifications = notifier.sender().subscribe();

    let turn = run_prompt(
        agent,
        notifications,
        prompt,
        config.max_tokens,
        Arc::clone(&session_slot),
    );
    tokio::pin!(turn);

    let started = tokio::time::Instant::now();
    let ceiling_deadline = started + config.turn_ceiling;
    let mut last_progress = started;
    let mut liveness_open = true;

    loop {
        let abandon_at = (last_progress + config.idle_timeout).min(ceiling_deadline);
        tokio::select! {
            result = &mut turn => return result,
            received = liveness.recv(), if liveness_open => {
                note_progress(received, &session_slot, &mut last_progress, &mut liveness_open);
            }
            _ = tokio::time::sleep_until(abandon_at) => {
                return Err(abandon_turn(agent, &session_slot, ceiling_deadline, config));
            }
        }
    }
}

/// Fold one liveness-subscription poll into the supervisor's progress state.
///
/// Encapsulates the progress policy:
/// - a notification for **our** session is progress (updates `last_progress`);
///   other sessions' traffic is not.
/// - a `Lagged` receiver counts as progress: the dropped messages may have
///   included ours, and abandonment is a backstop for a *wedged* turn — a
///   wedged turn produces no traffic to lag behind.
/// - a `Closed` channel means no further progress can ever be observed; close
///   the liveness arm (`liveness_open = false`) and let the turn race the
///   remaining deadlines.
fn note_progress(
    received: Result<SessionNotification, broadcast::error::RecvError>,
    session_slot: &std::sync::Mutex<Option<SessionId>>,
    last_progress: &mut tokio::time::Instant,
    liveness_open: &mut bool,
) {
    match received {
        Ok(notification) => {
            let is_ours = session_slot
                .lock()
                .expect("session slot lock poisoned")
                .as_ref()
                .is_some_and(|sid| notification.session_id == *sid);
            if is_ours {
                *last_progress = tokio::time::Instant::now();
            }
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            *last_progress = tokio::time::Instant::now();
        }
        Err(broadcast::error::RecvError::Closed) => {
            *liveness_open = false;
        }
    }
}

/// Abandon the in-flight turn: actively cancel its session (ACP
/// `session/cancel`) so the agent stops decoding, and pick the typed
/// abandonment reason — [`PoolError::TurnCeiling`] when the absolute ceiling
/// has passed, [`PoolError::TurnIdle`] otherwise. If the turn wedged before
/// `new_session` completed there is no session to cancel.
fn abandon_turn(
    agent: &ConnectionTo<Agent>,
    session_slot: &std::sync::Mutex<Option<SessionId>>,
    ceiling_deadline: tokio::time::Instant,
    config: PoolConfig,
) -> PoolError {
    let session = session_slot
        .lock()
        .expect("session slot lock poisoned")
        .clone();
    if let Some(session_id) = session {
        if let Err(e) = agent.send_notification(CancelNotification::new(session_id)) {
            tracing::warn!("failed to cancel abandoned session: {}", e);
        }
    }
    if tokio::time::Instant::now() >= ceiling_deadline {
        PoolError::TurnCeiling {
            turn_ceiling: config.turn_ceiling,
        }
    } else {
        PoolError::TurnIdle {
            idle_timeout: config.idle_timeout,
        }
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
///
/// `session_slot` is filled with the new session's id as soon as `new_session`
/// completes, giving the liveness supervisor a handle for progress attribution
/// and cancel-on-abandonment.
async fn run_prompt(
    agent: &ConnectionTo<Agent>,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: String,
    max_tokens: u64,
    session_slot: Arc<std::sync::Mutex<Option<SessionId>>>,
) -> PromptResult {
    // new_session
    let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let session_response = agent
        .send_request(NewSessionRequest::new(cwd))
        .block_task()
        .await
        .map_err(|e| {
            claude_agent::AgentError::Internal(format!("failed to create session: {}", e))
        })?;
    let session_id = session_response.session_id;
    // Publish the session id to the liveness supervisor
    // (`run_turn_with_liveness`) so it can attribute streaming progress to this
    // turn and cancel the session if the turn is abandoned.
    *session_slot.lock().expect("session slot lock poisoned") = Some(session_id.clone());
    // Captured for the per-reply audit log below: `session_id` is moved into the
    // `PromptRequest` before the response is collected.
    let session_label = session_id.to_string();

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
            claude_agent::AgentError::Internal(format!("failed to execute prompt: {}", e))
        })?;

    // 5. drain trailing notifications and assemble the collected response.
    let content = claude_agent::collect_response_content(
        collector,
        collected_text,
        notification_count,
        &prompt_response,
    )
    .await;

    // Audit log: record this reply's full text so a `review … backend=local` run
    // is auditable end-to-end. The review driver drains notifications through its
    // own collector (see `review::drive::build_pool_notifier`), bypassing
    // `TracingAgent`'s notification logger, so this is the only place the model's
    // reply is recorded. Both fan-out and verify prompts go through the pool, so
    // both are covered. The format mirrors `TracingAgent`'s `AgentMessage` line;
    // the text is logged in full and is never truncated.
    tracing::info!(
        "session={}, AgentMessage ({} chars): {}",
        session_label,
        content.len(),
        content
    );

    Ok(claude_agent::CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::review::test_support::new_notifier;

    use acp_conformance::test_utils::{numbered_session_response, MockAgent, MockAgentAdapter};
    use agent_client_protocol::schema::{
        ContentChunk, NewSessionResponse, PromptResponse, SessionUpdate,
    };
    use agent_client_protocol::{Channel, Client, ConnectTo};
    use agent_client_protocol_extras::PlaybackAgent;
    use futures::future::BoxFuture;
    use tempfile::TempDir;

    /// Wire a [`MockAgent`] up to a fresh `Client` and run `body` against the
    /// resulting `ConnectionTo<Agent>` handle.
    ///
    /// Pool-specific variant of `acp_conformance::test_utils::run_with_mock_agent`:
    /// the client side additionally forwards incoming `session/update`
    /// notifications into `notifier` (see [`run_client_against`]), which is the
    /// seam the pool's streaming collector and liveness supervisor subscribe to.
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

    /// Spawn a task that emits an `agent_message_chunk` notification for
    /// `session_id` every `every_ms` ms — the shape of a slow turn that is
    /// actively streaming tokens. Abort the returned handle to stop it.
    fn spawn_progress_feeder(
        notifier: Arc<claude_agent::NotificationSender>,
        session_id: &str,
        every_ms: u64,
    ) -> tokio::task::JoinHandle<()> {
        let session_id = session_id.to_string();
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(std::time::Duration::from_millis(every_ms)).await;
                let _ = notifier
                    .send_update(SessionNotification::new(
                        agent_client_protocol::schema::SessionId::new(session_id.clone()),
                        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                            TextContent::new("chunk"),
                        ))),
                    ))
                    .await;
            }
        })
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
            numbered_session_response(&self.next_session, "pass-sess")
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
            numbered_session_response(&self.next_session, "peak-sess")
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
            numbered_session_response(&self.next_session, "err-sess")
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
            numbered_session_response(&self.next_session, "meta-sess")
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

    /// Stalls (sleeps far longer than any test window) on the first
    /// `stall_count` prompts and passes afterwards. Records every
    /// `session/cancel` notification it receives, so tests can prove an
    /// abandoned turn was actively cancelled rather than detached.
    struct StallingAgent {
        next_session: AtomicUsize,
        remaining_stalls: AtomicUsize,
        cancelled_sessions: std::sync::Mutex<Vec<String>>,
    }

    impl StallingAgent {
        fn new(stall_count: usize) -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                remaining_stalls: AtomicUsize::new(stall_count),
                cancelled_sessions: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn cancelled(&self) -> Vec<String> {
            self.cancelled_sessions.lock().unwrap().clone()
        }
    }

    impl MockAgent for StallingAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            numbered_session_response(&self.next_session, "stall-sess")
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let should_stall = self
                .remaining_stalls
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
                .is_ok();
            Box::pin(async move {
                if should_stall {
                    // Far longer than any test's liveness windows; the client
                    // abandons the turn long before this resolves.
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                }
                Ok(PromptResponse::new(
                    agent_client_protocol::schema::StopReason::EndTurn,
                ))
            })
        }

        fn cancel<'a>(
            &'a self,
            notification: CancelNotification,
        ) -> BoxFuture<'a, agent_client_protocol::Result<()>> {
            self.cancelled_sessions
                .lock()
                .unwrap()
                .push(notification.session_id.to_string());
            Box::pin(async move { Ok(()) })
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

    /// A turn that streams a notification at least every idle-window interval
    /// is never abandoned, regardless of total duration — the local-35B case:
    /// slow, but demonstrably alive.
    #[tokio::test]
    async fn test_pool_streaming_turn_survives_beyond_idle_window() {
        // The prompt takes 4x the idle window of total wall clock.
        let agent = Arc::new(PeakProbeAgent::new(2000));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);
        let notifier_feeder = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(500))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            let feeder = spawn_progress_feeder(notifier_feeder, "peak-sess-0", 100);
            let result = pool.submit("slow but streaming").await.expect("result");
            feeder.abort();

            assert!(
                result.is_ok(),
                "a turn streaming progress every 100ms must never be abandoned even though \
                 its 2s total exceeds the 500ms idle window: {:?}",
                result.err(),
            );
        })
        .await;
    }

    /// A turn with zero streaming progress for the idle window is abandoned and
    /// degrades to a single-task error; the worker survives to run later jobs
    /// (the fleet continues).
    #[tokio::test]
    async fn test_pool_stalled_turn_abandons_after_idle_window() {
        let agent = Arc::new(StallingAgent::new(1));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            // The idle window must exceed claude_agent's fixed 500ms trailing
            // notification drain (`NOTIFICATION_COLLECTION_DELAY_MS`), or even
            // an instantly-completing turn would look stalled.
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(800))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            let err = pool
                .submit("stalled")
                .await
                .expect("worker must deliver a result, not hang")
                .expect_err("a turn with zero progress for the idle window must be abandoned");
            assert!(
                matches!(err, PoolError::TurnIdle { .. }),
                "abandonment must be the typed idle-window variant, got: {err:?}",
            );

            // The worker must survive the abandonment and run the next job.
            pool.submit("after abandonment")
                .await
                .expect("result")
                .expect("the fleet must continue after a single-task abandonment");
        })
        .await;
    }

    /// Abandoning a turn actively cancels the in-flight session (ACP
    /// `session/cancel`) so the agent stops decoding, rather than detaching it.
    #[tokio::test]
    async fn test_pool_abandoned_turn_cancels_session() {
        let agent = Arc::new(StallingAgent::new(1));
        let agent_probe = Arc::clone(&agent);
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(800))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            let result = pool.submit("stalled").await.expect("result");
            assert!(result.is_err(), "the stalled turn must be abandoned");

            // The cancel is a one-way notification; allow it a moment to land.
            let mut cancelled = agent_probe.cancelled();
            for _ in 0..200 {
                if !cancelled.is_empty() {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                cancelled = agent_probe.cancelled();
            }
            assert_eq!(
                cancelled,
                vec!["stall-sess-0".to_string()],
                "abandonment must send session/cancel for the in-flight session",
            );
        })
        .await;
    }

    /// The defensive absolute ceiling still fires on a turn that streams
    /// forever without completing (a runaway loop) — progress does not bypass
    /// the ceiling.
    #[tokio::test]
    async fn test_pool_turn_ceiling_abandons_streaming_runaway() {
        let agent = Arc::new(PeakProbeAgent::new(5000));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);
        let notifier_feeder = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_secs(10))
                .with_turn_ceiling(std::time::Duration::from_millis(400));
            let pool = AgentPool::new(conn, notifier_body, config);

            let feeder = spawn_progress_feeder(notifier_feeder, "peak-sess-0", 50);
            let result = pool.submit("runaway").await.expect("result");
            feeder.abort();

            let err =
                result.expect_err("the ceiling must abandon a never-finishing streaming turn");
            assert!(
                matches!(err, PoolError::TurnCeiling { .. }),
                "abandonment must be the typed ceiling variant, got: {err:?}",
            );
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

    /// Every prompt run through the pool logs the agent's full reply at the
    /// `run_prompt` seam, so a `review … backend=local` run is auditable
    /// end-to-end. The review path drains notifications through its own collector
    /// (bypassing `TracingAgent`), so this per-reply log is the only record of
    /// what the model said.
    #[tokio::test]
    #[tracing_test::traced_test]
    async fn test_pool_logs_agent_reply_at_run_prompt_seam() {
        let (_temp, fixture_path) = create_playback_fixture(PLAYBACK_PASS);
        let agent = PlaybackAgent::new(fixture_path, "test");
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_playback_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            pool.submit("validate this")
                .await
                .expect("result")
                .expect("playback agent should respond");
        })
        .await;

        // The reply is logged in the existing `AgentMessage (N chars): <text>`
        // format (the `(N chars)` shape is unique to the seam — it distinguishes
        // our deliberate per-reply log from the framework's raw transport traces),
        // and the FULL reply text appears inline (no truncation). The fixture
        // reply is 52 chars long.
        assert!(
            logs_contain(&format!(
                "AgentMessage ({} chars): {{\"status\": \"passed\", \"message\": \"All checks passed\"}}",
                r#"{"status": "passed", "message": "All checks passed"}"#.len()
            )),
            "run_prompt must log each reply in the `AgentMessage (N chars): <full text>` format with the untruncated reply text"
        );
    }
}
