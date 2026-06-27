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
    CancelNotification, ContentBlock, ExtRequest, NewSessionRequest, PromptRequest, SessionId,
    SessionNotification, TextContent,
};
use agent_client_protocol::{Agent, ClientRequest, ConnectionTo};
use agent_client_protocol_extras::{
    SessionForkRequest, SessionForkResponse, SessionPinRequest, SessionPinResponse,
    SessionStateStatusRequest, SessionStateStatusResponse, MAX_TOKENS_META_KEY,
    PIN_ON_SAVE_META_KEY, SESSION_FORK_METHOD, SESSION_PIN_METHOD, SESSION_STATE_STATUS_METHOD,
};
use tokio::sync::{broadcast, mpsc, oneshot, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

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
/// notification for the turn's session — has arrived for this long AFTER the
/// turn's first progress. The window is not armed until that first progress:
/// before it, the turn may be waiting in the GPU queue (behind earlier decodes
/// on the one shared GPU) and is NOT idle, so only the absolute ceiling
/// ([`PROMPT_TURN_CEILING`]) bounds it. Anchoring the window at submission
/// instead counted innocent queue-wait as a stall and abandoned forks queued
/// behind a deep prime+fork batch before they ever decoded.
///
/// Total wall clock is deliberately NOT capped at this value: a legitimate turn
/// on a local 35B model (big review prompt, agentic loop, one shared GPU
/// serializing decodes across fleet tasks) routinely needs more than 300s, but
/// while it is decoding it streams chunks continuously, so it keeps resetting
/// this window. Only a turn that started then went silent (e.g. a nested agent
/// request the client failed to answer) trips it, and that degrades to a
/// single-task error — the fleet reports zero findings for it and the review
/// COMPLETES — instead of hanging the whole review forever.
pub const PROMPT_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(300);

/// Defensive absolute ceiling on a single prompt turn.
///
/// Catches the two pathologies the idle window cannot:
/// - a turn that keeps emitting notifications forever without ever completing
///   (a runaway agentic loop);
/// - a turn that never streams any progress (wedged before the first token) —
///   the idle window stays disarmed until first progress, so the ceiling is its
///   only bound.
///
/// It is the bound on a turn's TOTAL wall clock, which now explicitly includes
/// time spent queued for the GPU before the first decode: with the prime+fork
/// path a fork can wait behind a deep batch of primes serialized on the one
/// shared GPU before it streams anything. It must therefore sit far above
/// local-model reality — a single legitimate local-35B turn can take well over
/// five minutes of decode, and a queued fork adds the wait ahead of it — so it
/// is set to 45 minutes: long enough to never false-fire on a slow turn that
/// waited a while in the queue then decoded, short enough that a runaway or
/// wedged turn cannot pin a worker forever.
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
    /// The caller fired the pool's external cancel handle (e.g. the `expect`
    /// spec wall-clock timeout elapsed), so the in-flight turn was abandoned and
    /// its session actively cancelled (ACP `session/cancel`) rather than being
    /// orphaned by a dropped future. Typed — like [`PoolError::TurnIdle`] /
    /// [`PoolError::TurnCeiling`] — so a caller-cancelled turn is distinguishable
    /// from a liveness abandonment or a genuine agent failure.
    #[error("prompt turn was cancelled by the caller and its session was cancelled")]
    TurnCancelled,
    /// The `session/fork` extension call failed, so the turn's prompt never
    /// ran. The submitter still holds the payload and can fall back to a
    /// fresh-session monolithic prompt — a fork failure must never lose a task.
    #[error("session fork from parent {parent_session_id} failed: {message}")]
    ForkFailed {
        /// The parent session the fork was requested from.
        parent_session_id: String,
        /// The failure the agent reported (or the transport surfaced).
        message: String,
    },
    /// A quick session-extension request (`session/state_status` /
    /// `session/pin`) failed. Typed — like [`PoolError::ForkFailed`] — so
    /// callers can match on "the backend lacks this extension / the call
    /// failed" without parsing message text.
    #[error("{method} for session {session_id} failed: {message}")]
    Extension {
        /// The extension method that failed (a
        /// [`agent_client_protocol_extras`] method-name constant).
        method: &'static str,
        /// The session the request targeted.
        session_id: String,
        /// The failure the agent reported (or the transport surfaced).
        message: String,
    },
    /// The agent connection itself failed the turn.
    #[error(transparent)]
    Agent(#[from] claude_agent::AgentError),
}

/// Result of a single submitted prompt.
pub type PromptResult = Result<claude_agent::CollectedResponse, PoolError>;

/// A completed turn delivered with the session it ran on and, when the session
/// was forked from a primed parent, what the fork attached.
#[derive(Debug, Clone)]
pub struct SessionTurn {
    /// The id of the session the turn ran on (fresh or forked). A typed
    /// [`SessionId`] so it cannot be silently swapped with prompt text at the
    /// fork/pin call sites.
    pub session_id: SessionId,
    /// The collected streamed response text.
    pub content: String,
    /// The agent's reported stop reason.
    pub stop_reason: agent_client_protocol::schema::StopReason,
    /// `Some` when the turn ran on a forked session — what the fork attached.
    pub fork: Option<ForkAttachment>,
    /// Per-turn Anthropic prompt-cache usage, parsed from the prompt response's
    /// `_meta` (`cache_usage` key) when the backend reported it. `None` for
    /// backends that report no cache metrics (e.g. the native KV/llama path,
    /// which signals reuse via [`ForkAttachment::prefix_tokens`] instead). On
    /// the claude backend this is the only signal of warm (cache read) vs cold
    /// (cache write) prefix reuse, since the fork attaches no token counts.
    pub cache_usage: Option<claude_agent::protocol_translator::CacheUsage>,
}

/// What a `session/fork` actually attached, per the fork response — lets a
/// caller tell a warm fork (parent state reused) from a degraded one (history
/// cloned, cold prefill).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ForkAttachment {
    /// Whether the parent's saved generation state was attached to the fork.
    pub state_attached: bool,
    /// Number of prompt tokens the attached state covers (`None` when the
    /// backend does not track token counts).
    pub prefix_tokens: Option<u64>,
}

/// Result of a primed or forked submission: the full [`SessionTurn`].
pub type SessionTurnResult = Result<SessionTurn, PoolError>;

/// How a queued turn obtains its session.
enum SessionSource {
    /// Mint a fresh session via `session/new` (the default path).
    New,
    /// Fork from a primed parent via the `session/fork` extension.
    Fork {
        /// The session whose conversation and saved state seed the fork.
        parent_session_id: SessionId,
    },
}

/// The channel a job's result is delivered back on, shaped per submission API.
enum Respond {
    /// [`AgentPool::submit`]: deliver the collected response only.
    Collected(oneshot::Sender<PromptResult>),
    /// [`AgentPool::submit_primed`] / [`AgentPool::submit_forked`]: deliver the
    /// full turn including its session id and fork attachment.
    Turn(oneshot::Sender<SessionTurnResult>),
}

impl Respond {
    /// Deliver one resolved turn on whichever channel shape the submitter
    /// asked for. The submitter may have dropped its receiver; that is fine.
    fn deliver(self, result: SessionTurnResult) {
        match self {
            Respond::Collected(tx) => {
                let _ = tx.send(result.map(|turn| claude_agent::CollectedResponse {
                    content: turn.content,
                    stop_reason: turn.stop_reason,
                    cache_usage: turn.cache_usage,
                }));
            }
            Respond::Turn(tx) => {
                let _ = tx.send(result);
            }
        }
    }
}

/// A unit of work on the shared queue: a prompt, the session source it runs
/// on, and the channel to deliver its result back to the submitter.
struct Job {
    prompt: String,
    session: SessionSource,
    respond_to: Respond,
    /// Whether this turn should ask the agent to save its session state born
    /// pinned (carried over ACP in `_meta` under [`PIN_ON_SAVE_META_KEY`]).
    /// Set only by [`AgentPool::submit_primed`]: a prime turn's prefix must be
    /// pinned atomically at save time so a concurrent save cannot evict it
    /// before the fan-out forks from it. An agent without a KV cache ignores
    /// the intent (pin = no-op), consistent with the fork/pin contract.
    pin_on_save: bool,
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
    /// The shared agent connection, kept for the quick session-extension
    /// requests ([`AgentPool::session_state_status`], [`AgentPool::pin_session`])
    /// that are not generation turns and so bypass the worker queue.
    agent: ConnectionTo<Agent>,
}

impl AgentPool {
    /// Create a pool of `config.workers` workers draining a shared queue, each
    /// issuing prompts over a clone of `agent` and subscribing to `notifier`
    /// for streaming response content.
    ///
    /// The pool has no external cancel handle: turns are bounded only by the
    /// liveness supervisor's idle/ceiling deadlines. Use [`AgentPool::new_cancellable`]
    /// when a caller needs to actively abandon in-flight turns out of band.
    pub fn new(
        agent: ConnectionTo<Agent>,
        notifier: Arc<claude_agent::NotificationSender>,
        config: PoolConfig,
    ) -> Self {
        // A token nobody holds the cancel end of never fires, so the pool's turns
        // are bounded purely by their idle/ceiling deadlines — identical to the
        // pre-cancel-handle behavior.
        Self::new_cancellable(agent, notifier, config, CancellationToken::new())
    }

    /// Create a pool whose in-flight turns can be abandoned out of band by
    /// cancelling `cancel`.
    ///
    /// Identical to [`AgentPool::new`], except every worker's turn supervisor
    /// also races `cancel`: when it fires, the in-flight session is actively
    /// cancelled (ACP `session/cancel`, the same teardown the idle/ceiling
    /// deadlines use) and the turn resolves to [`PoolError::TurnCancelled`]. This
    /// is the seam the `expect` spec-timeout teardown drives so a wall-clock
    /// timeout stops the agent rather than orphaning it behind a dropped future.
    pub fn new_cancellable(
        agent: ConnectionTo<Agent>,
        notifier: Arc<claude_agent::NotificationSender>,
        config: PoolConfig,
        cancel: CancellationToken,
    ) -> Self {
        let worker_count = config.workers.max(MIN_WORKERS);
        let (tx, rx) = mpsc::unbounded_channel::<Job>();
        let rx = Arc::new(Mutex::new(rx));

        let mut workers = Vec::with_capacity(worker_count);
        for _ in 0..worker_count {
            let rx = Arc::clone(&rx);
            let agent = agent.clone();
            let notifier = Arc::clone(&notifier);
            let cancel = cancel.clone();
            workers.push(tokio::spawn(async move {
                worker_loop(rx, agent, notifier, config, cancel).await;
            }));
        }

        Self {
            tx,
            workers,
            worker_count,
            max_tokens: config.max_tokens,
            agent,
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
        self.enqueue(Job {
            prompt: prompt.into(),
            session: SessionSource::New,
            respond_to: Respond::Collected(respond_to),
            pin_on_save: false,
        });
        rx
    }

    /// Submit a prefix-priming prompt: a normal fresh-session turn whose
    /// delivered [`SessionTurn`] reports the session id, so the caller can
    /// confirm the saved state, pin it, and fork batch turns from it.
    ///
    /// Queueing, liveness supervision, and the token cap are identical to
    /// [`AgentPool::submit`].
    pub fn submit_primed(&self, prompt: impl Into<String>) -> oneshot::Receiver<SessionTurnResult> {
        let (respond_to, rx) = oneshot::channel();
        self.enqueue(Job {
            prompt: prompt.into(),
            session: SessionSource::New,
            respond_to: Respond::Turn(respond_to),
            // A prime turn's saved prefix must be born pinned so a concurrent
            // save cannot evict it before the fan-out forks from it — the
            // structural close of the prime→pin eviction race.
            pin_on_save: true,
        });
        rx
    }

    /// Submit a prompt that runs on a session forked from `parent_session_id`
    /// (the `session/fork` extension), inheriting the parent's conversation
    /// and — when the backend supports it — its saved generation state, so the
    /// prompt decodes strictly forward from the parent's primed prefix.
    ///
    /// A failed fork resolves to the typed [`PoolError::ForkFailed`] without
    /// running any prompt: the submitter still holds the payload and falls
    /// back to a fresh-session monolithic prompt. A fork that succeeds but
    /// attaches no parent state is reported via [`SessionTurn::fork`] (the
    /// turn still ran, just cold).
    ///
    /// The parent is a typed [`SessionId`] — distinct from the stringly
    /// `prompt` — so swapping the two arguments is a compile error rather
    /// than a runtime `ForkFailed`.
    pub fn submit_forked(
        &self,
        parent_session_id: &SessionId,
        prompt: impl Into<String>,
    ) -> oneshot::Receiver<SessionTurnResult> {
        let (respond_to, rx) = oneshot::channel();
        self.enqueue(Job {
            prompt: prompt.into(),
            session: SessionSource::Fork {
                parent_session_id: parent_session_id.clone(),
            },
            respond_to: Respond::Turn(respond_to),
            // A forked batch turn saves its own (cold) state unpinned; only the
            // primed parent prefix is pinned for the fan-out.
            pin_on_save: false,
        });
        rx
    }

    /// Enqueue one job, delivering a shutdown error instead of hanging the
    /// submitter's future when the pool's workers are all gone.
    fn enqueue(&self, job: Job) {
        if let Err(returned) = self.tx.send(job) {
            returned
                .0
                .respond_to
                .deliver(Err(claude_agent::AgentError::Internal(
                    "agent pool is shut down".to_string(),
                )
                .into()));
        }
    }

    /// Query a session's saved-state status (`session/state_status`) over the
    /// shared connection — a quick request, not a generation turn, so it
    /// bypasses the worker queue. Failures are the typed
    /// [`PoolError::Extension`].
    pub async fn session_state_status(
        &self,
        session_id: &SessionId,
    ) -> Result<SessionStateStatusResponse, PoolError> {
        send_extension_request(
            &self.agent,
            SESSION_STATE_STATUS_METHOD,
            &SessionStateStatusRequest {
                session_id: session_id.to_string(),
            },
        )
        .await
        .map_err(|message| PoolError::Extension {
            method: SESSION_STATE_STATUS_METHOD,
            session_id: session_id.to_string(),
            message,
        })
    }

    /// Pin or unpin a session's saved state (`session/pin`) over the shared
    /// connection — a quick request, not a generation turn, so it bypasses the
    /// worker queue. The response reports the *effective* pin state (a backend
    /// without pinning reports `false`). Failures are the typed
    /// [`PoolError::Extension`].
    pub async fn pin_session(
        &self,
        session_id: &SessionId,
        pinned: bool,
    ) -> Result<SessionPinResponse, PoolError> {
        send_pin(&self.agent, session_id, pinned).await
    }

    /// Pin `session_id`'s saved state for a scope: on success the returned
    /// [`SessionPinGuard`] guarantees the eventual unpin — explicitly via
    /// [`SessionPinGuard::release`], or from `Drop` when the owning future is
    /// cancelled mid-flight — so a pinned prefix session can never outlive
    /// the fan-out that pinned it.
    pub async fn pin_session_scoped(
        &self,
        session_id: &SessionId,
    ) -> Result<(SessionPinResponse, SessionPinGuard), PoolError> {
        let response = send_pin(&self.agent, session_id, true).await?;
        Ok((
            response,
            SessionPinGuard {
                agent: Some(self.agent.clone()),
                session_id: session_id.clone(),
            },
        ))
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

/// RAII guard for a pinned prefix session: it guarantees the pin is released,
/// even when the owning future is dropped mid fan-out (review cancelled,
/// caller timeout), so a pinned entry — exempt from cache eviction — can never
/// outlive its scope. Created by [`AgentPool::pin_session_scoped`].
///
/// The normal path calls [`SessionPinGuard::release`] to unpin inline and
/// observe the result; `Drop` is the cancellation backstop. `Drop` is
/// synchronous and the unpin is an async request, so — mirroring llama-agent's
/// `ActiveRequestGuard` — the drop path spawns the unpin onto the runtime.
pub struct SessionPinGuard {
    /// `Some` until the pin is released (explicitly or by `Drop`).
    agent: Option<ConnectionTo<Agent>>,
    session_id: SessionId,
}

impl SessionPinGuard {
    /// The pinned session's id (the fork parent).
    pub fn session_id(&self) -> &SessionId {
        &self.session_id
    }

    /// Release the pin now and report the result, disarming the `Drop` unpin.
    pub async fn release(mut self) -> Result<SessionPinResponse, PoolError> {
        let agent = self
            .agent
            .take()
            .expect("pin guard releases exactly once by construction");
        send_pin(&agent, &self.session_id, false).await
    }
}

impl Drop for SessionPinGuard {
    fn drop(&mut self) {
        let Some(agent) = self.agent.take() else {
            // Already released explicitly.
            return;
        };
        let session_id = self.session_id.clone();
        // The guard lives inside pool-driven async code, so a runtime handle
        // is normally available; without one (a purely synchronous teardown)
        // the leak is at least logged rather than silent.
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => {
                handle.spawn(async move {
                    match send_pin(&agent, &session_id, false).await {
                        Ok(_) => tracing::debug!(
                            session = %session_id,
                            "released prefix session pin on guard drop"
                        ),
                        Err(err) => tracing::warn!(
                            session = %session_id,
                            error = %err,
                            "failed to release prefix session pin on guard drop"
                        ),
                    }
                });
            }
            Err(_) => tracing::warn!(
                session = %session_id,
                "prefix pin guard dropped outside a runtime; pin not released"
            ),
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
    cancel: CancellationToken,
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

        let result = run_turn_with_liveness(
            &agent,
            &notifier,
            job.session,
            job.prompt,
            job.pin_on_save,
            config,
            &cancel,
        )
        .await;
        // The submitter may have dropped its receiver; that is fine.
        job.respond_to.deliver(result);
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
///   abandoned regardless of total duration. The idle window is **not armed
///   until the turn's FIRST progress** — a turn waiting in the GPU queue
///   (behind earlier decodes on the one shared GPU) streams nothing yet is not
///   idle, so before first progress only the ceiling bounds it. Anchoring the
///   idle clock at submission instead wrongly abandoned forks queued behind a
///   deep prime+fork batch on the single GPU.
/// - **ceiling**: `config.turn_ceiling` of total wall clock, the defensive cap
///   on a turn that streams forever without completing — and the only bound on
///   a turn still waiting in the queue for the GPU.
///
/// On abandonment the in-flight session is actively cancelled (ACP
/// `session/cancel`) so the agent stops decoding, rather than being detached to
/// keep generating into a dropped receiver. The session id is learned from
/// `run_prompt` through a shared slot once `new_session` completes; if the turn
/// wedged before that there is no session to cancel.
///
/// A third abandonment trigger races alongside idle and ceiling: the external
/// `cancel` handle. When the caller fires it (the `expect` spec wall-clock
/// timeout), the in-flight session is cancelled the same way and the turn
/// resolves to [`PoolError::TurnCancelled`].
async fn run_turn_with_liveness(
    agent: &ConnectionTo<Agent>,
    notifier: &claude_agent::NotificationSender,
    session: SessionSource,
    prompt: String,
    pin_on_save: bool,
    config: PoolConfig,
    cancel: &CancellationToken,
) -> SessionTurnResult {
    let session_slot: Arc<std::sync::Mutex<Option<SessionId>>> = Arc::default();
    // Two independent subscriptions: one consumed by `run_prompt`'s content
    // collector, one watched here for liveness.
    let mut liveness = notifier.sender().subscribe();
    let notifications = notifier.sender().subscribe();

    let turn = run_prompt(
        agent,
        notifications,
        session,
        prompt,
        pin_on_save,
        config.max_tokens,
        Arc::clone(&session_slot),
    );
    tokio::pin!(turn);

    let started = tokio::time::Instant::now();
    let ceiling_deadline = started + config.turn_ceiling;
    // `None` until the turn's first progress arms the idle window. A turn that
    // has streamed nothing yet is queued for the GPU, not idle — only the
    // ceiling bounds it.
    let mut last_progress: Option<tokio::time::Instant> = None;
    let mut liveness_open = true;

    loop {
        let abandon_at = match last_progress {
            Some(progress) => (progress + config.idle_timeout).min(ceiling_deadline),
            None => ceiling_deadline,
        };
        tokio::select! {
            result = &mut turn => return result,
            received = liveness.recv(), if liveness_open => {
                note_progress(received, &session_slot, &mut last_progress, &mut liveness_open);
            }
            _ = cancel.cancelled() => {
                cancel_in_flight_session(agent, &session_slot);
                return Err(PoolError::TurnCancelled);
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
/// - a notification for **our** session is progress; it arms (on first
///   progress) and resets `last_progress`. Other sessions' traffic is not.
/// - a `Lagged` receiver counts as progress: the dropped messages may have
///   included ours, and abandonment is a backstop for a *wedged* turn — a
///   wedged turn produces no traffic to lag behind.
/// - a `Closed` channel means no further progress can ever be observed; close
///   the liveness arm (`liveness_open = false`) and let the turn race the
///   remaining deadlines.
///
/// `last_progress` is `None` until the first progress event, which is what
/// keeps the idle window disarmed while a turn is still queued for the GPU.
fn note_progress(
    received: Result<SessionNotification, broadcast::error::RecvError>,
    session_slot: &std::sync::Mutex<Option<SessionId>>,
    last_progress: &mut Option<tokio::time::Instant>,
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
                *last_progress = Some(tokio::time::Instant::now());
            }
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            *last_progress = Some(tokio::time::Instant::now());
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
    cancel_in_flight_session(agent, session_slot);
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

/// Actively cancel the in-flight turn's session (ACP `session/cancel`) so the
/// agent stops decoding instead of being detached to generate into a dropped
/// receiver.
///
/// The single place every abandonment trigger — idle, ceiling, and the external
/// [`CancellationToken`] — sends the cancel, so they share one definition of
/// "stop the agent." The session id is learned through `session_slot` once
/// `new_session` completes; if the turn wedged before that there is no session
/// to cancel, so this is a no-op.
fn cancel_in_flight_session(
    agent: &ConnectionTo<Agent>,
    session_slot: &std::sync::Mutex<Option<SessionId>>,
) {
    let session = session_slot
        .lock()
        .expect("session slot lock poisoned")
        .clone();
    if let Some(session_id) = session {
        if let Err(e) = agent.send_notification(CancelNotification::new(session_id)) {
            tracing::warn!("failed to cancel abandoned session: {}", e);
        }
    }
}

/// Drive a single prompt turn against an ACP 0.12 agent connection.
///
/// Routes the per-prompt ACP requests (session establishment → `prompt`)
/// through the typed [`ConnectionTo<Agent>`] handle, spawning a per-session
/// notification collector so streaming `session/update` content is captured
/// concurrently with the prompt. The session is established per the job's
/// [`SessionSource`]: a fresh `session/new`, or a `session/fork` from a primed
/// parent (see [`establish_session`]). The per-call token cap is attached to
/// the prompt request's `meta` map under [`MAX_TOKENS_META_KEY`].
///
/// `initialize` is deliberately NOT here: it is a once-per-connection handshake
/// the driver performs before any worker runs (see
/// `review::drive::run_pipeline_in_connection`). Issuing it per prompt raced N
/// concurrent handshakes over the single shared connection and wedged the real
/// agent.
///
/// `session_slot` is filled with the session's id as soon as it is established,
/// giving the liveness supervisor a handle for progress attribution and
/// cancel-on-abandonment.
async fn run_prompt(
    agent: &ConnectionTo<Agent>,
    notifications: broadcast::Receiver<SessionNotification>,
    session: SessionSource,
    prompt: String,
    pin_on_save: bool,
    max_tokens: u64,
    session_slot: Arc<std::sync::Mutex<Option<SessionId>>>,
) -> SessionTurnResult {
    let (session_id, fork) = establish_session(agent, session).await?;
    // Publish the session id to the liveness supervisor
    // (`run_turn_with_liveness`) so it can attribute streaming progress to this
    // turn and cancel the session if the turn is abandoned.
    *session_slot.lock().expect("session slot lock poisoned") = Some(session_id.clone());
    // Captured for the per-reply audit log and the delivered turn below:
    // `session_id` is moved into the `PromptRequest` before the response is
    // collected.
    let session_label = session_id.clone();

    // 3. spawn notification collector before prompt() so streaming content is
    //    captured as it arrives.
    let (collector, collected_text, notification_count, _matched_count) =
        claude_agent::spawn_notification_collector(notifications, session_id.clone());

    // 4. prompt, with the per-call token cap attached via `meta`. A prime turn
    //    also carries the born-pinned save intent so the prefix it leaves
    //    cached is pinned atomically at save time — the structural close of the
    //    prime→pin eviction race. An ordinary turn omits the key (no pin).
    let mut meta = serde_json::Map::new();
    meta.insert(
        MAX_TOKENS_META_KEY.to_string(),
        serde_json::json!(max_tokens),
    );
    if pin_on_save {
        meta.insert(PIN_ON_SAVE_META_KEY.to_string(), serde_json::json!(true));
    }
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

    // Parse per-turn prompt-cache usage from the response `_meta` (the
    // `cache_usage` key the claude agent attaches via
    // `agent_prompt_handling::build_streaming_response`), mirroring
    // `claude_agent::execute_prompt_with_agent`. `None` for backends that report
    // no usage. This is the only warm/cold reuse signal on the claude backend,
    // whose fork attaches no native token counts.
    let cache_usage = prompt_response
        .meta
        .as_ref()
        .and_then(|meta| meta.get("cache_usage"))
        .and_then(claude_agent::protocol_translator::CacheUsage::from_meta_json);

    Ok(SessionTurn {
        session_id: session_label,
        content,
        stop_reason: prompt_response.stop_reason,
        fork,
        cache_usage,
    })
}

/// Establish the session a turn runs on, per its [`SessionSource`].
///
/// - [`SessionSource::New`] issues `session/new` (today's path).
/// - [`SessionSource::Fork`] issues the `session/fork` extension against the
///   parent; the returned [`ForkAttachment`] reports whether the parent's
///   saved state was attached (and how many prefix tokens it covers) so the
///   caller can log warm vs degraded forks. Any fork failure maps onto the
///   typed [`PoolError::ForkFailed`] so the submitter can fall back to a
///   fresh-session monolithic prompt rather than lose the task.
async fn establish_session(
    agent: &ConnectionTo<Agent>,
    session: SessionSource,
) -> Result<(SessionId, Option<ForkAttachment>), PoolError> {
    match session {
        SessionSource::New => {
            let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
            let session_response = agent
                .send_request(NewSessionRequest::new(cwd))
                .block_task()
                .await
                .map_err(|e| {
                    claude_agent::AgentError::Internal(format!("failed to create session: {}", e))
                })?;
            Ok((session_response.session_id, None))
        }
        SessionSource::Fork { parent_session_id } => {
            let response: SessionForkResponse = send_extension_request(
                agent,
                SESSION_FORK_METHOD,
                &SessionForkRequest {
                    parent_session_id: parent_session_id.to_string(),
                },
            )
            .await
            .map_err(|message| PoolError::ForkFailed {
                parent_session_id: parent_session_id.to_string(),
                message,
            })?;
            let attachment = ForkAttachment {
                state_attached: response.state_attached,
                prefix_tokens: response.prefix_tokens,
            };
            Ok((SessionId::new(response.session_id), Some(attachment)))
        }
    }
}

/// Pin or unpin `session_id`'s saved state over `agent`, mapping failures onto
/// the typed [`PoolError::Extension`]. Shared by [`AgentPool::pin_session`]
/// and the pin guard's release paths.
async fn send_pin(
    agent: &ConnectionTo<Agent>,
    session_id: &SessionId,
    pinned: bool,
) -> Result<SessionPinResponse, PoolError> {
    send_extension_request(
        agent,
        SESSION_PIN_METHOD,
        &SessionPinRequest {
            session_id: session_id.to_string(),
            pinned,
        },
    )
    .await
    .map_err(|message| PoolError::Extension {
        method: SESSION_PIN_METHOD,
        session_id: session_id.to_string(),
        message,
    })
}

/// Send one session-extension request (`session/fork` / `session/state_status`
/// / `session/pin`) over the connection and parse its typed response.
///
/// The wire method is the canonical bare name prefixed with `_`, which is how
/// the ACP SDK routes extension methods (`ClientRequest::parse_message` only
/// dispatches `_`-prefixed methods to `ExtMethodRequest`; the receiver strips
/// the prefix back off). Failures are returned as the human-readable message
/// so each caller can wrap them in its own typed error.
async fn send_extension_request<Request, Response>(
    agent: &ConnectionTo<Agent>,
    method: &str,
    request: &Request,
) -> Result<Response, String>
where
    Request: serde::Serialize,
    Response: serde::de::DeserializeOwned,
{
    let params = serde_json::value::to_raw_value(request)
        .map_err(|e| format!("failed to serialize {method} params: {e}"))?;
    let value: serde_json::Value = agent
        .send_request(ClientRequest::ExtMethodRequest(ExtRequest::new(
            format!("_{method}"),
            Arc::from(params),
        )))
        .block_task()
        .await
        .map_err(|e| e.to_string())?;
    serde_json::from_value(value).map_err(|e| format!("malformed {method} response: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::atomic::{AtomicUsize, Ordering};

    use crate::review::test_support::{
        new_notifier, with_pool, ForkMode, ScriptedAgent, ScriptedAgentConfig, MOCK_PREFIX_TOKENS,
    };

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

    /// Returns a passing response whose `_meta` carries a fixed prompt-cache
    /// `cache_usage` object — the wire shape a real claude agent attaches — so a
    /// test can prove the pool threads it onto [`SessionTurn::cache_usage`].
    struct CacheUsageAgent {
        next_session: AtomicUsize,
        usage: claude_agent::protocol_translator::CacheUsage,
    }

    impl CacheUsageAgent {
        fn new(usage: claude_agent::protocol_translator::CacheUsage) -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                usage,
            }
        }
    }

    impl MockAgent for CacheUsageAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            numbered_session_response(&self.next_session, "cache-sess")
        }

        fn prompt<'a>(
            &'a self,
            _request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let usage = self.usage;
            Box::pin(async move {
                let mut meta = serde_json::Map::new();
                meta.insert("cache_usage".to_string(), usage.to_meta_json());
                Ok(
                    PromptResponse::new(agent_client_protocol::schema::StopReason::EndTurn)
                        .meta(meta),
                )
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

    /// Records the `max_tokens` value and the pin-on-save intent from each
    /// prompt's `meta` map.
    struct MetaRecordingAgent {
        next_session: AtomicUsize,
        recorded_max_tokens: std::sync::Mutex<Vec<Option<u64>>>,
        recorded_pin_on_save: std::sync::Mutex<Vec<bool>>,
    }

    impl MetaRecordingAgent {
        fn new() -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                recorded_max_tokens: std::sync::Mutex::new(Vec::new()),
                recorded_pin_on_save: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn recorded(&self) -> Vec<Option<u64>> {
            self.recorded_max_tokens.lock().unwrap().clone()
        }

        /// The pin-on-save intent (`_meta` boolean, defaulting to `false`) seen
        /// on each prompt, in order.
        fn recorded_pin_on_save(&self) -> Vec<bool> {
            self.recorded_pin_on_save.lock().unwrap().clone()
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
                .and_then(|m| m.get(MAX_TOKENS_META_KEY))
                .and_then(|v| v.as_u64());
            self.recorded_max_tokens.lock().unwrap().push(max_tokens);
            let pin_on_save = request
                .meta
                .as_ref()
                .and_then(|m| m.get(PIN_ON_SAVE_META_KEY))
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            self.recorded_pin_on_save.lock().unwrap().push(pin_on_save);
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

    /// Holds every `prompt` behind a shared gate (a [`tokio::sync::Semaphore`])
    /// so the test controls exactly how many prompts may decode at once — the
    /// shape of the single-GPU llama backend, where a `prompt` request is sent
    /// to the agent but blocks behind the GpuLock until earlier decodes finish.
    /// A gated prompt streams nothing while it waits, then emits one
    /// `agent_message_chunk` and completes once it acquires a permit.
    ///
    /// `new_session` always answers immediately (it is not a generation turn),
    /// so a gated turn has its session id published to the liveness supervisor
    /// and is genuinely "queued for the GPU, not idle".
    struct GatedAgent {
        next_session: AtomicUsize,
        gate: Arc<tokio::sync::Semaphore>,
        notifier: Arc<claude_agent::NotificationSender>,
        decode_ms: u64,
        cancelled_sessions: std::sync::Mutex<Vec<String>>,
    }

    impl GatedAgent {
        /// `permits` prompts may decode concurrently; the rest queue behind the
        /// gate. Once a prompt acquires a permit it "decodes" for `decode_ms`
        /// (holding the permit, so queued prompts keep waiting) before emitting
        /// its first and only `agent_message_chunk` and completing — modelling a
        /// non-trivial decode that keeps the GPU busy. `notifier` is the same
        /// channel the pool's liveness supervisor watches.
        fn new(
            permits: usize,
            decode_ms: u64,
            notifier: Arc<claude_agent::NotificationSender>,
        ) -> Self {
            Self {
                next_session: AtomicUsize::new(0),
                gate: Arc::new(tokio::sync::Semaphore::new(permits)),
                notifier,
                decode_ms,
                cancelled_sessions: std::sync::Mutex::new(Vec::new()),
            }
        }

        fn cancelled(&self) -> Vec<String> {
            self.cancelled_sessions.lock().unwrap().clone()
        }
    }

    impl MockAgent for GatedAgent {
        fn new_session<'a>(
            &'a self,
            _request: NewSessionRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<NewSessionResponse>> {
            numbered_session_response(&self.next_session, "gated-sess")
        }

        fn prompt<'a>(
            &'a self,
            request: PromptRequest,
        ) -> BoxFuture<'a, agent_client_protocol::Result<PromptResponse>> {
            let gate = Arc::clone(&self.gate);
            let notifier = Arc::clone(&self.notifier);
            let session_id = request.session_id.clone();
            let decode_ms = self.decode_ms;
            Box::pin(async move {
                // Wait for the GPU (a permit). While waiting, this turn streams
                // nothing — exactly the queue-wait the idle clock must not count.
                let _permit = gate
                    .acquire()
                    .await
                    .map_err(|_| agent_client_protocol::Error::internal_error())?;
                // Hold the GPU for the decode duration (queued prompts keep
                // waiting), streaming nothing until the end.
                tokio::time::sleep(std::time::Duration::from_millis(decode_ms)).await;
                // Now decoding: emit one streaming chunk so the turn shows
                // progress, then complete.
                let _ = notifier
                    .send_update(SessionNotification::new(
                        session_id,
                        SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                            TextContent::new("decoded"),
                        ))),
                    ))
                    .await;
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

    /// A turn that STARTS streaming (first progress arms the idle window) then
    /// goes silent for the idle window is abandoned as idle, and degrades to a
    /// single-task error; the worker survives to run later jobs (the fleet
    /// continues). A feeder emits a few chunks then stops, modelling a turn that
    /// decoded a little then wedged (e.g. an unanswered nested agent request).
    #[tokio::test]
    async fn test_pool_started_then_stalled_turn_abandons_as_idle() {
        let agent = Arc::new(StallingAgent::new(1));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);
        let notifier_feeder = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            // Idle window short relative to the feeder's burst, but the burst
            // ends well before the ceiling so the post-progress silence trips
            // idle, not the ceiling.
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(600))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            // Emit a short burst of progress for the turn's session, then stop —
            // the turn "started" but then stalls silent for the idle window.
            let feeder = spawn_progress_feeder(notifier_feeder, "stall-sess-0", 100);
            let submit = pool.submit("started then stalled");
            tokio::time::sleep(std::time::Duration::from_millis(350)).await;
            feeder.abort();

            let err = submit
                .await
                .expect("worker must deliver a result, not hang")
                .expect_err("a turn that stalls after starting must be abandoned");
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

    /// A turn that NEVER starts streaming (zero progress, ever) is not treated
    /// as idle — the idle window is never armed — and is bounded only by the
    /// absolute ceiling, which abandons it. This is the wedged-before-first-token
    /// case; a queued-for-the-GPU turn shares the same disarmed-idle behaviour
    /// but completes when its turn comes (see the gated-queue tests).
    #[tokio::test]
    async fn test_pool_never_started_turn_abandons_at_ceiling() {
        let agent = Arc::new(StallingAgent::new(1));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(200))
                .with_turn_ceiling(std::time::Duration::from_millis(700));
            let pool = AgentPool::new(conn, notifier_body, config);

            let err = pool
                .submit("never starts")
                .await
                .expect("worker must deliver a result, not hang")
                .expect_err("a turn that never streams progress is bounded by the ceiling");
            assert!(
                matches!(err, PoolError::TurnCeiling { .. }),
                "a never-started turn must be abandoned by the ceiling, not idle, got: {err:?}",
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
            // The StallingAgent never streams progress, so the idle window is
            // never armed; a short ceiling abandons it quickly. Cancel-on-abandon
            // fires on either abandonment path, which is what this test asserts.
            let config = PoolConfig::local()
                .with_idle_timeout(std::time::Duration::from_millis(200))
                .with_turn_ceiling(std::time::Duration::from_millis(700));
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

    /// A turn that waits in the GPU queue longer than the idle window before it
    /// emits its FIRST streaming progress, then decodes normally, must NOT be
    /// abandoned: a queued turn is not idle. The idle clock only starts at first
    /// progress. This is the core fix for the qwen prime+fork run, where a fork
    /// queued behind ~15 primes on the single GPU waited well past 300s before
    /// decoding and was wrongly abandoned.
    #[tokio::test]
    async fn test_pool_queued_turn_not_abandoned_before_first_progress() {
        // A single GPU permit, held for ~1.5s of decode by the first prompt; the
        // second prompt queues behind it and so streams nothing for longer than
        // the idle window before it ever decodes.
        let notifier = new_notifier();
        let agent = Arc::new(GatedAgent::new(1, 1500, Arc::clone(&notifier)));
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            // Two workers so both turns are simultaneously inside
            // `run_turn_with_liveness` (post-new_session, awaiting the gated
            // prompt) — the second turn is queued for the GPU, not queued in
            // the pool's mpsc channel. The idle window sits above claude_agent's
            // fixed 500ms trailing notification drain, below the 1.5s queue wait.
            let config = PoolConfig::remote(2)
                .with_idle_timeout(std::time::Duration::from_millis(700))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            let first = pool.submit("decodes first");
            // Let the first turn acquire the single permit before the second is
            // submitted, so the second deterministically queues behind it.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let second = pool.submit("queues for the gpu");

            // The first decodes, releases the permit; the second — which waited
            // > the 300ms idle window with zero progress — then decodes.
            let first = first.await.expect("result");
            let second = second.await.expect("result");
            assert!(
                first.is_ok(),
                "the first (decoding) turn completes: {:?}",
                first.err()
            );
            assert!(
                second.is_ok(),
                "a turn that waited in the GPU queue past the idle window before \
                 its first progress must NOT be abandoned, it completes once it \
                 decodes: {:?}",
                second.err()
            );
        })
        .await;
    }

    /// A queue of N turns where only ONE may decode at a time (a single GPU
    /// permit): none of the turns waiting their turn-to-run is abandoned while
    /// pending under the ceiling. The whole batch completes. This is the
    /// prime+fork serialization shape — many turns deep on one GPU.
    #[tokio::test]
    async fn test_pool_gated_queue_completes_without_abandonment() {
        let notifier = new_notifier();
        let agent = Arc::new(GatedAgent::new(1, 300, Arc::clone(&notifier)));
        let agent_probe = Arc::clone(&agent);
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            // Enough workers that every turn sits in `run_turn_with_liveness`
            // at once, all serialized behind the single GPU permit (300ms decode
            // each). The later turns wait > 1s in the queue — well past the idle
            // window (set above the 500ms trailing drain) yet under the ceiling.
            let config = PoolConfig::remote(8)
                .with_idle_timeout(std::time::Duration::from_millis(700))
                .with_turn_ceiling(std::time::Duration::from_secs(30));
            let pool = AgentPool::new(conn, notifier_body, config);

            let n = 6;
            let receivers: Vec<_> = (0..n).map(|i| pool.submit(format!("turn {i}"))).collect();

            let mut completed = 0;
            for rx in receivers {
                let result = rx.await.expect("worker must deliver a result");
                assert!(
                    result.is_ok(),
                    "no gated-queue turn may be abandoned while waiting its \
                     turn-to-run under the ceiling: {:?}",
                    result.err()
                );
                completed += 1;
            }
            assert_eq!(completed, n, "every gated turn must complete");
            assert!(
                agent_probe.cancelled().is_empty(),
                "no queued turn may be cancelled: {:?}",
                agent_probe.cancelled()
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

    /// A `submit_primed` turn carries the born-pinned save intent
    /// (`pin_on_save: true`) in its prompt `_meta`, while an ordinary `submit`
    /// turn does not. This is the producer half of the prime→pin race fix: the
    /// prime turn tells the agent to save its prefix born pinned, so a
    /// concurrent save can never evict it before a separate pin would land.
    #[tokio::test]
    async fn test_pool_submit_primed_carries_pin_on_save_intent() {
        let agent = Arc::new(MetaRecordingAgent::new());
        let agent_probe = Arc::clone(&agent);
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::remote(1));

            // Ordinary submit: no pin-on-save.
            pool.submit("ordinary").await.expect("result").expect("ok");
            // Prime turn: born-pinned save requested.
            pool.submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("ok");

            assert_eq!(
                agent_probe.recorded_pin_on_save(),
                vec![false, true],
                "only the primed turn carries the pin-on-save intent in _meta",
            );
        })
        .await;
    }

    /// A fork-capable shared scripted agent (`crate::review::test_support`) —
    /// the same mock the fleet tests run, so the pool and the fleet exercise
    /// one implementation of the session-fork wire contract.
    fn fork_capable_agent() -> Arc<ScriptedAgent> {
        ScriptedAgent::with_config(
            vec![],
            ScriptedAgentConfig {
                fork_mode: ForkMode::Supported,
                ..ScriptedAgentConfig::default()
            },
        )
    }

    /// `submit_primed` runs a normal fresh-session turn but reports the session
    /// id back, so the caller can confirm/pin and fork from it.
    #[tokio::test]
    async fn test_pool_submit_primed_reports_session_id() {
        let agent = Arc::new(PassingAgent::new());
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            let turn = pool
                .submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("prime turn should succeed");
            assert_eq!(turn.session_id, SessionId::new("pass-sess-0"));
            assert!(turn.fork.is_none(), "a primed turn is not a fork");
            assert!(
                turn.cache_usage.is_none(),
                "a turn whose response carried no usage reports no cache_usage"
            );
        })
        .await;
    }

    /// A turn whose `PromptResponse._meta` carries a `cache_usage` object —
    /// exactly what a real claude agent attaches — propagates it onto
    /// [`SessionTurn::cache_usage`], so the fleet can log warm vs cold reuse on
    /// the claude backend.
    #[tokio::test]
    async fn test_pool_turn_propagates_cache_usage_from_response() {
        let usage = claude_agent::protocol_translator::CacheUsage {
            cache_read_input_tokens: Some(1500),
            cache_creation_input_tokens: Some(60),
            input_tokens: Some(1560),
            output_tokens: Some(30),
        };
        let agent = Arc::new(CacheUsageAgent::new(usage));
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            let turn = pool
                .submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("prime turn should succeed");
            assert_eq!(
                turn.cache_usage,
                Some(usage),
                "the turn must carry the response's cache usage"
            );
        })
        .await;
    }

    /// `submit_forked` forks the parent via the `session/fork` extension, runs
    /// the prompt on the child session, and reports what the fork attached.
    #[tokio::test]
    async fn test_pool_submit_forked_prompts_the_forked_session() {
        let agent = fork_capable_agent();
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::local(), move |pool| async move {
            // Prime a parent session whose completed turn the fork attaches.
            let parent = pool
                .submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("prime turn should succeed");
            let turn = pool
                .submit_forked(&parent.session_id, "payload only")
                .await
                .expect("result")
                .expect("forked turn should succeed");
            assert_eq!(
                turn.session_id,
                SessionId::new("sess-1"),
                "the fork minted a child session"
            );
            assert_eq!(
                turn.fork,
                Some(ForkAttachment {
                    state_attached: true,
                    prefix_tokens: Some(MOCK_PREFIX_TOKENS),
                }),
                "a forked turn reports its full attachment"
            );
            assert_eq!(
                agent_probe.prompted_sessions(),
                vec!["sess-0".to_string(), "sess-1".to_string()],
                "the payload prompt must run on the forked session, not a fresh one"
            );
        })
        .await;
    }

    /// A backend without the fork extension fails a forked submission with the
    /// typed `ForkFailed` error, so the caller can fall back to a monolithic
    /// fresh-session prompt instead of losing the task.
    #[tokio::test]
    async fn test_pool_submit_forked_without_ext_support_is_fork_failed() {
        // PassingAgent inherits MockAgent's default ext_method: method_not_found.
        let agent = Arc::new(PassingAgent::new());
        let notifier = new_notifier();
        let notifier_body = Arc::clone(&notifier);

        run_with_mock_agent(agent, notifier, move |conn| async move {
            let pool = AgentPool::new(conn, notifier_body, PoolConfig::local());
            let err = pool
                .submit_forked(&SessionId::new("parent-1"), "payload")
                .await
                .expect("result")
                .expect_err("a fork against a fork-less backend must fail");
            assert!(
                matches!(err, PoolError::ForkFailed { .. }),
                "the failure must be the typed fork variant, got: {err:?}"
            );
        })
        .await;
    }

    /// The pool's direct extension helpers round-trip `session/state_status`
    /// and `session/pin` over the shared connection (no worker involved).
    #[tokio::test]
    async fn test_pool_state_status_and_pin_helpers_round_trip() {
        let agent = fork_capable_agent();
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::local(), move |pool| async move {
            // A completed turn gives the session saved, pinnable state.
            let turn = pool
                .submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("prime turn should succeed");

            let status = pool
                .session_state_status(&turn.session_id)
                .await
                .expect("state status should round-trip");
            assert!(status.saved);
            assert_eq!(status.prompt_tokens, Some(MOCK_PREFIX_TOKENS));

            let pin = pool
                .pin_session(&turn.session_id, true)
                .await
                .expect("pin should round-trip");
            assert!(pin.pinned);
            assert_eq!(
                agent_probe.pin_calls(),
                vec![(turn.session_id.to_string(), true)]
            );
        })
        .await;
    }

    /// The quick extension helpers fail with the typed [`PoolError::Extension`]
    /// variant — like the fork path's `ForkFailed` — so callers can tell "the
    /// backend lacks this extension" apart without parsing message text.
    #[tokio::test]
    async fn test_pool_extension_helper_failure_is_typed() {
        // The default shared agent implements NO extension methods.
        let agent = ScriptedAgent::new(vec![]);

        with_pool(agent, PoolConfig::local(), move |pool| async move {
            let session = SessionId::new("sess-0");
            let err = pool
                .session_state_status(&session)
                .await
                .expect_err("a backend without the extension must fail the status call");
            assert!(
                matches!(
                    err,
                    PoolError::Extension {
                        method: SESSION_STATE_STATUS_METHOD,
                        ..
                    }
                ),
                "the failure must be the typed extension variant, got: {err:?}"
            );

            let err = pool
                .pin_session(&session, true)
                .await
                .expect_err("a backend without the extension must fail the pin call");
            assert!(
                matches!(
                    err,
                    PoolError::Extension {
                        method: SESSION_PIN_METHOD,
                        ..
                    }
                ),
                "the failure must be the typed extension variant, got: {err:?}"
            );
        })
        .await;
    }

    /// `pin_session_scoped` returns a guard whose explicit `release()` unpins
    /// inline AND disarms the `Drop` backstop: exactly one unpin is issued.
    #[tokio::test]
    async fn test_pool_pin_guard_release_unpins_exactly_once() {
        let agent = fork_capable_agent();
        let agent_probe = Arc::clone(&agent);

        with_pool(agent, PoolConfig::local(), move |pool| async move {
            let turn = pool
                .submit_primed("the shared prefix")
                .await
                .expect("result")
                .expect("prime turn should succeed");
            let (pin, guard) = pool
                .pin_session_scoped(&turn.session_id)
                .await
                .expect("scoped pin should round-trip");
            assert!(pin.pinned);

            guard.release().await.expect("release should round-trip");

            // Give a (would-be) spawned drop-unpin time to land, then prove it
            // never fired: exactly one pin and one unpin.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            assert_eq!(
                agent_probe.pin_calls(),
                vec![
                    (turn.session_id.to_string(), true),
                    (turn.session_id.to_string(), false),
                ],
                "release() must unpin exactly once — the Drop backstop is disarmed"
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
