//! Claude Agent Library
//!
//! A Rust library that implements an Agent Client Protocol (ACP) server,
//! wrapping Claude Code functionality to enable any ACP-compatible client
//! to interact with Claude Code.

/// Crate version, sourced from Cargo.toml at compile time.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

pub mod acp_error;
pub mod acp_error_conversion;
pub mod agent;
pub mod agent_cancellation;
pub mod agent_commands;
pub mod agent_elicitation;
pub mod agent_file_handlers;
pub mod agent_file_operations;
pub mod agent_notifications;
pub mod agent_permissions;
pub mod agent_prompt_handling;
pub mod agent_reasoning;
pub mod agent_terminal_handlers;
pub mod agent_trait_impl;
pub mod agent_validation;
pub mod base64_processor;
pub mod base64_validation;
pub mod capability_validation;
pub mod claude;
pub mod claude_backend;
pub mod claude_process;
pub mod config;
pub mod constants;
pub mod content_block_processor;
pub mod content_capability_validator;
pub mod content_security_validator;
pub mod conversation_manager;
pub mod editor_state;
pub mod elicitation_bridge;
pub mod json_rpc_codes;
pub mod mime_type_validator;

#[cfg(test)]
mod content_security_integration_tests;
pub mod error;
pub mod mcp;
pub mod mcp_error_handling;
pub mod path_validator;
pub mod permission_storage;
pub mod permissions;
pub mod plan;
pub mod protocol_translator;
#[cfg(test)]
// mod permission_interaction_tests; // Disabled: tests MockPromptHandler which was deleted
pub mod request_validation;
pub mod session;
pub mod session_errors;
pub mod session_fork;
pub mod session_resume;
pub mod session_validation;
pub mod size_validator;
pub mod terminal_manager;
#[cfg(test)]
pub(crate) mod test_support;
mod tool_call_lifecycle_tests;
pub mod tool_classification;
pub mod tool_types;
pub mod tools;
pub mod url_validation;

// Re-exports for convenient access to main types
pub use agent::ClaudeAgent;
pub use agent_client_protocol_extras::RawMessageManager;
/// The in-band end-of-turn marker, re-exported from
/// [`agent_client_protocol_extras`].
///
/// [`turn_complete_notification`] builds the notification an agent emits as the
/// last act of a turn; [`is_turn_complete`] recognizes it; and
/// [`TURN_COMPLETE_META_KEY`] is the `_meta` key that carries it on the wire.
/// Together they are the signal [`spawn_notification_collector`] stops on, so
/// they are re-exported here: a consumer of this crate's collector names the
/// same contract the agents emit, from one import.
pub use agent_client_protocol_extras::{
    is_turn_complete, turn_complete_notification, TURN_COMPLETE_META_KEY,
};
pub use agent_notifications::NotificationSender;
pub use claude_process::SpawnConfig;
pub use config::{AgentConfig, McpServerConfig};
pub use error::{AgentError, Result};
pub use plan::{
    todowrite_to_acp_plan, todowrite_to_agent_plan, AgentPlan, PlanEntry, PlanEntryStatus, Priority,
};
pub use tools::{ToolCallHandler, ToolCallResult, ToolPermissions};

use agent_client_protocol::schema::{
    ContentBlock, InitializeRequest, NewSessionRequest, PromptRequest, SessionNotification,
    SessionUpdate, StopReason, TextContent,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::broadcast;
use typed_builder::TypedBuilder;

/// Hang guard for the post-turn notification drain, in milliseconds.
///
/// The drain ends when the agent's end-of-turn marker
/// ([`turn_complete_notification`]) reaches the collector, or when the
/// notification channel closes — both are real end-of-stream signals, and both
/// normally land within microseconds of the prompt response. This window is
/// the BACKSTOP for neither happening: an agent that never marks its turn
/// complete, or a collector starved past the marker. Hitting it is an error,
/// reported as one — never a silently truncated reply — so the window is sized
/// to be unreachable by ordinary scheduling latency rather than tuned against
/// it.
pub const NOTIFICATION_DRAIN_BACKSTOP_MS: u64 = 10_000;

/// Collected response from executing a prompt via streaming.
///
/// This collects the streamed content from SessionNotifications into a single response.
/// The stop_reason comes from the prompt response, while the actual content arrives via
/// streaming notifications.
#[derive(Debug, Clone)]
pub struct CollectedResponse {
    /// The collected text content from streaming notifications
    pub content: String,
    /// Why the agent stopped
    pub stop_reason: StopReason,
    /// Per-turn Anthropic prompt-cache usage, when the underlying `result`
    /// message reported a populated `usage` object. `None` when the turn
    /// reported no cache metrics (e.g. an empty or absent `usage`).
    pub cache_usage: Option<crate::protocol_translator::CacheUsage>,
}

/// Configuration for creating a ClaudeAgent.
///
/// Uses builder pattern to allow flexible configuration without
/// breaking changes when new options are added.
#[derive(Debug, Clone, TypedBuilder)]
pub struct CreateAgentConfig {
    /// Use ephemeral mode (haiku model + no session persistence)
    /// Ideal for validators and quick, stateless operations
    #[builder(default)]
    pub ephemeral: bool,
    /// MCP servers to configure for the agent
    #[builder(default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

/// Create a ClaudeAgent with the given configuration.
///
/// This is a convenience function that wraps ClaudeAgent::new() with
/// a simpler configuration interface.
///
/// # Example
///
/// ```ignore
/// use claude_agent::{CreateAgentConfig, create_agent};
///
/// // Create an ephemeral agent for quick operations
/// let config = CreateAgentConfig::builder()
///     .ephemeral(true)
///     .build();
/// let (agent, notifications) = create_agent(config).await?;
/// ```
pub async fn create_agent(
    config: CreateAgentConfig,
) -> Result<(
    ClaudeAgent,
    Arc<crate::agent_notifications::NotificationSender>,
)> {
    let mut agent_config = AgentConfig::default();
    agent_config.claude.ephemeral = config.ephemeral;
    agent_config.mcp_servers = config.mcp_servers;
    let (agent, _receiver) = ClaudeAgent::new(agent_config).await?;
    let notifier = Arc::clone(&agent.notification_sender);
    Ok((agent, notifier))
}

/// Execute a prompt and collect the response content.
///
/// This function handles the ACP streaming protocol:
/// 1. Creates a new session
/// 2. Subscribes to notifications
/// 3. Sends the prompt
/// 4. Collects text from SessionNotifications
/// 5. Returns the complete response
///
/// # Example
///
/// ```ignore
/// use claude_agent::{CreateAgentConfig, create_agent, execute_prompt};
///
/// let config = CreateAgentConfig::builder().ephemeral(true).build();
/// let (agent, notifications) = create_agent(config).await?;
/// let response = execute_prompt(&agent, notifications, "Hello!").await?;
/// println!("{}", response.content);
/// ```
pub async fn execute_prompt(
    agent: &ClaudeAgent,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<CollectedResponse> {
    execute_prompt_with_agent(agent, notifications, prompt).await
}

/// Execute a prompt against a [`ClaudeAgent`] and collect the streamed
/// response.
///
/// In ACP 0.10 this function was generic over an `Agent` trait so callers could
/// inject a `PlaybackAgent` for tests. ACP 0.11 removed the `Agent` trait —
/// `agent_client_protocol::Agent` is now a unit Role marker, and the inherent
/// methods on [`ClaudeAgent`] (`initialize`, `new_session`, `prompt`) are the
/// only thing this helper needs to drive a turn. Test-time injection of a
/// recorded session is now handled by wiring [`agent_client_protocol_extras::PlaybackAgent`]
/// to a real `ConnectionTo` peer rather than by parameterising this helper.
pub async fn execute_prompt_with_agent(
    agent: &ClaudeAgent,
    notifications: broadcast::Receiver<SessionNotification>,
    prompt: impl Into<String>,
) -> Result<CollectedResponse> {
    let prompt_text = prompt.into();

    initialize_agent(agent).await?;
    let session_id = create_session(agent).await?;
    let prompt_request = build_prompt_request(&session_id, prompt_text);

    let collector = spawn_notification_collector(notifications, session_id);

    let prompt_response = agent
        .prompt(prompt_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to execute prompt: {}", e)))?;

    let content = collect_response_content(collector, &prompt_response).await?;

    let cache_usage = prompt_response
        .meta
        .as_ref()
        .and_then(|meta| meta.get(crate::protocol_translator::CacheUsage::META_KEY))
        .and_then(crate::protocol_translator::CacheUsage::from_meta_json);

    Ok(CollectedResponse {
        content,
        stop_reason: prompt_response.stop_reason,
        cache_usage,
    })
}

/// Initialize the agent (required by ACP protocol).
async fn initialize_agent(agent: &ClaudeAgent) -> Result<()> {
    let init_request = InitializeRequest::new(1.into());
    agent
        .initialize(init_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to initialize agent: {}", e)))?;
    Ok(())
}

/// Create a new session with the agent.
async fn create_session(agent: &ClaudeAgent) -> Result<agent_client_protocol::schema::SessionId> {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("/tmp"));
    let session_request = NewSessionRequest::new(cwd);
    let session_response = agent
        .new_session(session_request)
        .await
        .map_err(|e| AgentError::Internal(format!("Failed to create session: {}", e)))?;
    Ok(session_response.session_id)
}

/// Build a prompt request for the given session and text.
fn build_prompt_request(
    session_id: &agent_client_protocol::schema::SessionId,
    prompt_text: String,
) -> PromptRequest {
    PromptRequest::new(
        session_id.clone(),
        vec![ContentBlock::Text(TextContent::new(prompt_text))],
    )
}

/// Extract text content from a notification if it matches our session.
async fn process_notification(
    notification: &SessionNotification,
    session_id: &agent_client_protocol::schema::SessionId,
    collected_text: &tokio::sync::Mutex<String>,
    matched_count: &std::sync::atomic::AtomicUsize,
) {
    if notification.session_id != *session_id {
        return;
    }

    matched_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    if let SessionUpdate::AgentMessageChunk(chunk) = &notification.update {
        if let ContentBlock::Text(text) = &chunk.content {
            let mut guard = collected_text.lock().await;
            guard.push_str(&text.text);
            tracing::trace!(
                session = %session_id,
                chunk_len = text.text.len(),
                total_len = guard.len(),
                "Collected text chunk"
            );
        }
    }
}

/// How a turn's notification stream ended.
///
/// Only [`TurnComplete`](Self::TurnComplete) proves the reply is whole. The
/// other variant is an end of stream, not an end of turn: nothing more can
/// arrive, but the agent never said it was finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CollectorEnd {
    /// The agent's end-of-turn marker arrived. Every hop between the agent and
    /// the collector is FIFO, so every chunk of the turn is already collected.
    TurnComplete,
    /// The notification channel closed before any end-of-turn marker. No
    /// further chunk can ever arrive, so the reply is whatever was collected —
    /// and it is incomplete unless the agent happened to finish first.
    StreamClosed,
}

/// A running collector for one turn's streamed reply.
///
/// [`spawn_notification_collector`] starts it; [`collect_response_content`]
/// consumes it once the prompt response has landed.
#[derive(Debug)]
pub struct NotificationCollector {
    /// The task draining the notification channel.
    task: tokio::task::JoinHandle<CollectorEnd>,
    /// The reply text reassembled so far.
    collected_text: Arc<tokio::sync::Mutex<String>>,
    /// Every notification the collector received, this session's or not.
    notification_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Notifications for this collector's own session.
    matched_count: Arc<std::sync::atomic::AtomicUsize>,
    /// Notifications the broadcast dropped because the collector fell behind.
    /// Any drop makes the reassembled reply unprovable: the lost messages may
    /// have carried this turn's chunks, or its end-of-turn marker.
    skipped: Arc<std::sync::atomic::AtomicU64>,
}

impl NotificationCollector {
    /// Every notification the collector has received so far, for any session.
    #[must_use]
    pub fn notification_count(&self) -> usize {
        self.notification_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Notifications received so far for this collector's own session.
    #[must_use]
    pub fn matched_count(&self) -> usize {
        self.matched_count
            .load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Notifications the broadcast dropped because the collector fell behind.
    #[must_use]
    pub fn skipped(&self) -> u64 {
        self.skipped.load(std::sync::atomic::Ordering::Relaxed)
    }
}

/// Spawn a task to collect text from session notifications.
///
/// The task runs until the turn's notification stream ends, which is one of
/// two real end-of-stream signals — never a timer:
///
/// - the agent's end-of-turn marker for `session_id`
///   ([`turn_complete_notification`]) arrives, giving
///   [`CollectorEnd::TurnComplete`]. Every hop between the agent and this
///   collector is FIFO, so the marker cannot overtake a chunk of the same
///   turn: seeing it means every chunk is already collected.
/// - the notification channel closes, giving [`CollectorEnd::StreamClosed`],
///   after which no chunk can ever arrive.
///
/// [`collect_response_content`] awaits the returned handle to observe which.
pub fn spawn_notification_collector(
    mut notifications: broadcast::Receiver<SessionNotification>,
    session_id: agent_client_protocol::schema::SessionId,
) -> NotificationCollector {
    let collected_text = Arc::new(tokio::sync::Mutex::new(String::new()));
    let collected_text_clone = Arc::clone(&collected_text);
    let notification_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let notification_count_clone = Arc::clone(&notification_count);
    let matched_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let matched_count_clone = Arc::clone(&matched_count);
    let skipped = Arc::new(std::sync::atomic::AtomicU64::new(0));
    let skipped_clone = Arc::clone(&skipped);

    tracing::debug!(session = %session_id, "Starting notification collector");

    let task = tokio::spawn(async move {
        loop {
            match notifications.recv().await {
                Ok(notification) => {
                    notification_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    if notification.session_id == session_id && is_turn_complete(&notification) {
                        // The marker is one of this session's notifications, so
                        // it counts. `matched_count` reports every notification
                        // received for this session, not only the ones that
                        // carried text.
                        matched_count_clone.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        tracing::debug!(
                            session = %session_id,
                            "Notification collector saw the end-of-turn marker"
                        );
                        break CollectorEnd::TurnComplete;
                    }
                    process_notification(
                        &notification,
                        &session_id,
                        &collected_text_clone,
                        &matched_count_clone,
                    )
                    .await;
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    // Dropped notifications may have included this turn's
                    // chunks, or its end-of-turn marker. Either way the reply
                    // can no longer be proven whole, so the drain reports it.
                    skipped_clone.fetch_add(n, std::sync::atomic::Ordering::Relaxed);
                    tracing::warn!(skipped = n, "Notification collector lagged");
                }
                Err(broadcast::error::RecvError::Closed) => {
                    tracing::debug!(
                        session = %session_id,
                        "Notification channel closed; no further chunks can arrive"
                    );
                    break CollectorEnd::StreamClosed;
                }
            }
        }
    });

    NotificationCollector {
        task,
        collected_text,
        notification_count,
        matched_count,
        skipped,
    }
}

/// Why a drain could not prove the reply whole.
///
/// One variant per failure path in [`collect_response_content`], each carrying
/// the detail that path adds to the report. [`DrainReport::incomplete`] turns a
/// variant into both the log line and the returned error, so the four paths
/// cannot describe their failures differently.
enum DrainFailure {
    /// The collector task ended before the turn's stream did, carrying the join
    /// error that says why.
    CollectorDied(tokio::task::JoinError),
    /// The drain hit its [`NOTIFICATION_DRAIN_BACKSTOP_MS`] hang guard without
    /// ever seeing an end-of-turn marker.
    Backstop,
    /// The broadcast dropped notifications because the collector fell behind.
    Lagged,
    /// The notification channel closed before the end-of-turn marker.
    StreamClosed,
}

/// Everything a failed drain reports, gathered once.
///
/// Every failure path in [`collect_response_content`] ends the same way: read
/// how much text was reassembled before things went wrong, log that at `error`
/// alongside the turn's counters, and hand the caller an
/// [`AgentError::Internal`] explaining it. Gathering the shared context once
/// and funnelling all four paths through [`incomplete`](Self::incomplete) is
/// what keeps the log line and the returned error from drifting apart — they
/// are the same string.
struct DrainReport<'a> {
    /// The reply text reassembled so far. Read at report time, because the
    /// collector may still have been appending to it.
    collected_text: &'a tokio::sync::Mutex<String>,
    /// The turn's prompt response, for its stop reason.
    prompt_response: &'a agent_client_protocol::schema::PromptResponse,
    /// Every notification the collector received, this session's or not.
    total_notifications: usize,
    /// Notifications received for this turn's own session.
    matched_notifications: usize,
    /// Notifications the broadcast dropped because the collector fell behind.
    skipped: u64,
}

impl DrainReport<'_> {
    /// Report a drain that could not prove the reply whole.
    ///
    /// The explanation is derived from `failure` once and used twice: as the
    /// `error`-level log message and as the text of the returned
    /// [`AgentError::Internal`], which therefore can never disagree. The
    /// per-failure details stay queryable as structured fields — `error` for a
    /// dead collector, `backstop_ms` for the hang guard — instead of living only
    /// as prose inside the message.
    async fn incomplete(&self, failure: DrainFailure) -> AgentError {
        let collected_so_far = self.collected_text.lock().await.len();
        let skipped = self.skipped;
        let (message, join_error, backstop_ms) = match failure {
            DrainFailure::CollectorDied(join_error) => (
                format!(
                    "the notification collector ended before the turn's stream did \
                     ({join_error}); the collected reply would be incomplete"
                ),
                Some(join_error.to_string()),
                None,
            ),
            DrainFailure::Backstop => (
                format!(
                    "the notification drain hit its {NOTIFICATION_DRAIN_BACKSTOP_MS}ms backstop \
                     without an end-of-turn marker ({skipped} notifications were dropped by \
                     lag); the collected reply would be incomplete"
                ),
                None,
                Some(NOTIFICATION_DRAIN_BACKSTOP_MS),
            ),
            DrainFailure::Lagged => (
                format!(
                    "the notification broadcast dropped {skipped} notifications while collecting \
                     this turn; the collected reply may be missing chunks"
                ),
                None,
                None,
            ),
            DrainFailure::StreamClosed => (
                "the notification channel closed before the turn's end-of-turn marker; the \
                 collected reply would be incomplete"
                    .to_string(),
                None,
                None,
            ),
        };
        tracing::error!(
            stop_reason = ?self.prompt_response.stop_reason,
            total_notifications = self.total_notifications,
            matched_notifications = self.matched_notifications,
            collected_so_far = collected_so_far,
            skipped = skipped,
            error = join_error.as_deref(),
            backstop_ms = backstop_ms,
            "{message}"
        );
        AgentError::Internal(message)
    }
}

/// Collect the response content after prompt execution.
///
/// Waits for the collector spawned by [`spawn_notification_collector`] to
/// reach the end of the turn's notification stream and then returns the
/// reassembled reply. The wait is on that signal, never on a wall-clock guess:
/// a chunk that is slow through the forwarding hops is still collected,
/// however loaded the machine is.
///
/// # Errors
///
/// Returns [`AgentError::Internal`] whenever the reply cannot be proven whole,
/// so an incomplete drain is never returned as a short string that reads like
/// a full reply:
///
/// - the collector lagged, so the broadcast dropped notifications that may
///   have carried this turn's chunks;
/// - the notification channel closed before the agent's end-of-turn marker;
/// - the drain hit the [`NOTIFICATION_DRAIN_BACKSTOP_MS`] hang guard (an agent
///   that never marks its turn complete, or a lag that ate the marker);
/// - the collector task died.
///
/// An empty reply is NOT one of those cases. A turn that marks itself complete
/// without streaming any text — cancelled before its first chunk, only tool
/// calls, or a request the agent rejected — drained completely, so it returns
/// `Ok("")`. Because every unprovable reply is an `Err`, a caller can read an
/// empty `Ok` as "the agent said nothing", never as "something was lost".
pub async fn collect_response_content(
    collector: NotificationCollector,
    prompt_response: &agent_client_protocol::schema::PromptResponse,
) -> Result<String> {
    let NotificationCollector {
        mut task,
        collected_text,
        notification_count,
        matched_count,
        skipped,
    } = collector;

    let backstop = std::time::Duration::from_millis(NOTIFICATION_DRAIN_BACKSTOP_MS);
    let drained = tokio::time::timeout(backstop, &mut task).await;
    let total_notifications = notification_count.load(std::sync::atomic::Ordering::Relaxed);
    let matched_notifications = matched_count.load(std::sync::atomic::Ordering::Relaxed);
    let skipped = skipped.load(std::sync::atomic::Ordering::Relaxed);

    let report = DrainReport {
        collected_text: &collected_text,
        prompt_response,
        total_notifications,
        matched_notifications,
        skipped,
    };

    let end = match drained {
        Ok(Ok(end)) => end,
        Ok(Err(join_error)) => {
            return Err(report
                .incomplete(DrainFailure::CollectorDied(join_error))
                .await);
        }
        Err(_elapsed) => {
            task.abort();
            return Err(report.incomplete(DrainFailure::Backstop).await);
        }
    };

    if skipped > 0 {
        return Err(report.incomplete(DrainFailure::Lagged).await);
    }

    if end == CollectorEnd::StreamClosed {
        return Err(report.incomplete(DrainFailure::StreamClosed).await);
    }

    let content = collected_text.lock().await.clone();

    // An empty reply is a fact about the turn, not a failure of the drain: the
    // marker proved the stream ended exactly where the agent said it did. A
    // turn cancelled before its first chunk, a turn that only made tool calls,
    // and a request rejected by validation all legitimately end this way. Every
    // case where the reply CANNOT be proven whole already returned `Err` above,
    // so this stays a success — reported at `warn` because an empty reply
    // usually disappoints the caller, not at `error`, which would claim a
    // failure the return value contradicts.
    if content.is_empty() {
        tracing::warn!(
            stop_reason = ?prompt_response.stop_reason,
            total_notifications = total_notifications,
            matched_notifications = matched_notifications,
            content_length = content.len(),
            "the turn ended with no streamed text content"
        );
    } else {
        tracing::debug!(
            stop_reason = ?prompt_response.stop_reason,
            total_notifications = total_notifications,
            matched_notifications = matched_notifications,
            content_length = content.len(),
            "the turn's streamed text content was collected"
        );
    }

    Ok(content)
}

#[cfg(test)]
mod collect_response_content_tests {
    use super::*;
    use agent_client_protocol::schema::{ContentChunk, PromptResponse, SessionId};
    use std::time::Duration;

    /// How long the feeder holds the tail chunk back before delivering it.
    /// Deliberately longer than the fixed 500 ms window the drain used to
    /// sleep, so a drain that ends on wall-clock time misses this chunk.
    const LATE_CHUNK_DELAY: Duration = Duration::from_millis(700);

    /// Broadcast ring capacity for a collector that must receive everything the
    /// test feeds it.
    ///
    /// A tokio broadcast channel drops the oldest notification once the ring is
    /// full, so the capacity has to exceed the number of notifications in
    /// flight before the collector task runs. No test here sends more than a
    /// handful, and 64 leaves that margin an order of magnitude over — a test
    /// that lags by accident would assert on a hole rather than on the behavior
    /// it names. [`LAGGING_NOTIFICATION_RING`] is the deliberate opposite.
    const TEST_NOTIFICATION_RING: usize = 64;

    /// Broadcast ring capacity small enough to FORCE a drop.
    ///
    /// Two slots, more than two notifications sent before the collector task
    /// runs: the broadcast must discard the oldest, which is exactly the lag
    /// `a_lagged_collector_is_an_error_not_a_reply_with_holes` asserts on.
    const LAGGING_NOTIFICATION_RING: usize = 2;

    /// How long a counter assertion waits for the collector task to catch up.
    ///
    /// The collector counts on its own task, so a test that reads a counter has
    /// to let that task run first. Generous enough to absorb scheduling latency
    /// on a loaded machine; a timeout means the count is genuinely wrong, not
    /// merely late.
    const COUNTER_SETTLE_TIMEOUT: Duration = Duration::from_secs(2);

    /// How often a counter assertion re-reads the collector's counters while
    /// waiting for [`COUNTER_SETTLE_TIMEOUT`].
    const COUNTER_POLL_INTERVAL: Duration = Duration::from_millis(5);

    /// Wait until `collector` has counted `expected` notifications for its own
    /// session, then return.
    ///
    /// The collector counts on its own task, so a test that asserts on a
    /// counter has to let that task run first. The collector ends its loop on
    /// the end-of-turn marker, so this settles as soon as the marker is
    /// processed. A timeout is a real failure: the count never reached
    /// `expected`.
    async fn await_matched_count(collector: &NotificationCollector, expected: usize) {
        let settle = async {
            while collector.matched_count() < expected {
                tokio::time::sleep(COUNTER_POLL_INTERVAL).await;
            }
        };
        tokio::time::timeout(COUNTER_SETTLE_TIMEOUT, settle)
            .await
            .unwrap_or_else(|_| {
                panic!(
                    "the collector must count {expected} notifications for its session; it counted {}",
                    collector.matched_count()
                )
            });
    }

    /// Broadcast one agent text chunk for `session`.
    async fn send_chunk(notifier: &NotificationSender, session: &SessionId, text: &str) {
        let update = SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
            TextContent::new(text.to_string()),
        )));
        notifier
            .send_update(SessionNotification::new(session.clone(), update))
            .await
            .expect("chunk broadcasts to the live collector");
    }

    /// The drain ends on the agent's end-of-turn marker, not on a timer: a
    /// chunk delivered LATER than the old fixed 500 ms window still lands in
    /// the collected reply.
    ///
    /// A slow hop is exactly what a loaded machine produces, and the fixed
    /// sleep dropped whatever had not arrived — returning a silently truncated
    /// reply.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_chunk_delivered_after_the_old_fixed_window_is_still_collected() {
        let session = SessionId::new("sess-late");
        let (notifier, _seed_rx) = NotificationSender::new(TEST_NOTIFICATION_RING);
        let notifier = Arc::new(notifier);

        let collector =
            spawn_notification_collector(notifier.sender().subscribe(), session.clone());

        let feeder = tokio::spawn({
            let notifier = Arc::clone(&notifier);
            let session = session.clone();
            async move {
                send_chunk(&notifier, &session, "early ").await;
                tokio::time::sleep(LATE_CHUNK_DELAY).await;
                send_chunk(&notifier, &session, "late").await;
                notifier
                    .send_update(turn_complete_notification(session))
                    .await
                    .expect("the end-of-turn marker broadcasts");
            }
        });

        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        let content = collect_response_content(collector, &prompt_response)
            .await
            .expect("the drain reaches the end of the turn's stream");

        feeder.await.expect("the feeder task completes");
        assert_eq!(
            content, "early late",
            "the drain must wait for the end-of-turn marker, not a fixed window"
        );
    }

    /// A channel that closes before the marker is an end of STREAM, not an end
    /// of turn: the agent never said the reply was finished, so whatever was
    /// collected is incomplete and must surface as an error rather than as a
    /// short string that reads like a full reply.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_channel_closed_before_the_marker_is_an_error() {
        let session = SessionId::new("sess-closed");
        let (notifier, _seed_rx) = NotificationSender::new(TEST_NOTIFICATION_RING);

        let collector =
            spawn_notification_collector(notifier.sender().subscribe(), session.clone());

        send_chunk(&notifier, &session, "partial").await;
        drop(notifier);

        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        let error = collect_response_content(collector, &prompt_response)
            .await
            .expect_err("a stream that closed before the marker is incomplete");
        assert!(
            error.to_string().contains("closed"),
            "the error must name the closed channel: {error}"
        );
    }

    /// A turn whose end-of-turn marker never arrives must NOT come back as a
    /// short string that reads like a complete reply. The backstop fires and
    /// the drain reports an error.
    ///
    /// Runs on a paused clock so the backstop elapses instantly instead of
    /// costing the suite ten real seconds.
    #[tokio::test(start_paused = true)]
    async fn a_missing_end_of_turn_marker_is_an_error_not_a_short_reply() {
        let session = SessionId::new("sess-unmarked");
        let (notifier, _seed_rx) = NotificationSender::new(TEST_NOTIFICATION_RING);

        let collector =
            spawn_notification_collector(notifier.sender().subscribe(), session.clone());

        send_chunk(&notifier, &session, "half a reply").await;

        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        let error = collect_response_content(collector, &prompt_response)
            .await
            .expect_err("an unfinished drain must be an error");
        assert!(
            error.to_string().contains("backstop"),
            "the error must name the backstop it hit: {error}"
        );
    }

    /// A collector that fell behind lost notifications the broadcast dropped —
    /// possibly this turn's chunks. The reply can no longer be proven whole, so
    /// the drain reports it instead of returning text that may be missing the
    /// middle.
    ///
    /// The lag is forced by the ORDER of the two steps, not by the scheduler.
    /// The receiver subscribes BEFORE the first send and the collector task
    /// starts only AFTER the last one, so the receiver is still holding its
    /// place at the first notification while the ring overwrites it. Its first
    /// `recv()` can answer nothing but `Lagged`.
    ///
    /// Starting the task first is what made this test race: a collector that
    /// the scheduler ran between two sends emptied the ring, nothing was ever
    /// dropped, and the drain rightly returned the whole reply.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn a_lagged_collector_is_an_error_not_a_reply_with_holes() {
        let session = SessionId::new("sess-lagged");
        let (notifier, _seed_rx) = NotificationSender::new(LAGGING_NOTIFICATION_RING);

        let notifications = notifier.sender().subscribe();

        for chunk in ["one ", "two ", "three ", "four "] {
            send_chunk(&notifier, &session, chunk).await;
        }
        notifier
            .send_update(turn_complete_notification(session.clone()))
            .await
            .expect("the end-of-turn marker broadcasts");

        let collector = spawn_notification_collector(notifications, session);

        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        let error = collect_response_content(collector, &prompt_response)
            .await
            .expect_err("a lagged collector cannot prove the reply is whole");
        // `DrainFailure::Backstop` also says "dropped", so the assertion names
        // the wording only `DrainFailure::Lagged` writes. A drain that hit the
        // hang guard instead would otherwise pass this test.
        assert!(
            error
                .to_string()
                .contains("the notification broadcast dropped"),
            "the error must name the notifications the broadcast dropped: {error}"
        );
    }

    /// The end-of-turn marker is a notification for the collector's own
    /// session, so it counts toward `matched_count`.
    ///
    /// `matched_count` reports "notifications received for this collector's own
    /// session". The marker is addressed to that session and the collector acts
    /// on it, so leaving it out would make the counter under-report the stream
    /// it is describing — and the drain reports that counter when it explains
    /// an incomplete reply.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_end_of_turn_marker_counts_as_a_notification_for_its_session() {
        let session = SessionId::new("sess-counted");
        let (notifier, _seed_rx) = NotificationSender::new(TEST_NOTIFICATION_RING);

        let collector =
            spawn_notification_collector(notifier.sender().subscribe(), session.clone());

        send_chunk(&notifier, &session, "one chunk").await;
        notifier
            .send_update(turn_complete_notification(session))
            .await
            .expect("the end-of-turn marker broadcasts");

        await_matched_count(&collector, 2).await;
        assert_eq!(
            collector.matched_count(),
            2,
            "the chunk and the end-of-turn marker are both this session's notifications"
        );
        assert_eq!(
            collector.notification_count(),
            2,
            "both notifications were received"
        );
    }

    /// A turn that marks itself complete without streaming any text is a
    /// SUCCESS with an empty reply, not a drain failure.
    ///
    /// The marker proves the stream ended where the agent said it did, so there
    /// is nothing incomplete to report: the agent simply produced no text. A
    /// turn cancelled before its first chunk, a turn that only made tool calls,
    /// and a prompt rejected by request validation all reach here. Every case
    /// where the reply CANNOT be proven whole returns `Err` instead, so `Ok("")`
    /// is unambiguous.
    ///
    /// The diagnostic has to agree with that: an empty reply is logged at
    /// `warn`, never at `error`, because an `error` line beside an `Ok` tells a
    /// reader the call failed when it did not. The assertions below pin the
    /// level as well as the return value.
    ///
    /// Captures through a scoped (thread-local) subscriber rather than
    /// `#[tracing_test::traced_test]`: the runtime here is single-threaded, so
    /// every event is emitted on the test thread and the capture is
    /// deterministic.
    #[tokio::test]
    async fn a_turn_that_streams_no_text_is_an_empty_reply_not_an_error() {
        let capture = swissarmyhammer_common::test_utils::CaptureWriter::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::WARN)
            .with_writer(capture.clone())
            .with_ansi(false)
            .finish();
        let _scope = tracing::subscriber::set_default(subscriber);

        let session = SessionId::new("sess-silent");
        let (notifier, _seed_rx) = NotificationSender::new(TEST_NOTIFICATION_RING);

        let collector =
            spawn_notification_collector(notifier.sender().subscribe(), session.clone());

        notifier
            .send_update(turn_complete_notification(session))
            .await
            .expect("the end-of-turn marker broadcasts");

        let prompt_response = PromptResponse::new(StopReason::EndTurn);
        let content = collect_response_content(collector, &prompt_response)
            .await
            .expect("a marked turn is a complete drain, however little it streamed");
        assert!(content.is_empty(), "the turn streamed no text: {content:?}");

        let logged = capture.contents();
        assert!(
            logged.contains("WARN") && logged.contains("the turn ended with no streamed text"),
            "an empty reply is reported at warn: {logged}"
        );
        assert!(
            !logged.contains("ERROR"),
            "an empty reply is a complete drain, so nothing may be logged as a failure: {logged}"
        );
    }
}
