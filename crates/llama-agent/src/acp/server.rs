use std::collections::HashMap;
use std::str::FromStr;
use std::sync::Arc;

use crate::agent::AgentServer;
use crate::types::ids::SessionId as LlamaSessionId;
use crate::types::AgentAPI;
use agent_client_protocol::schema::{ExtResponse, SessionId as AcpSessionId, SessionNotification};
use futures::StreamExt;
use swissarmyhammer_common::Pretty;
use tokio::sync::{broadcast, RwLock};

use agent_client_protocol_extras::{RawMessageManager, SessionStore};

use super::config::AcpConfig;
use super::filesystem::FilesystemOperations;
use super::permissions::PermissionPolicyEngine;
use super::session::AcpSessionState;
use super::session_record::session_record_from;
use super::terminal::TerminalManager;
use super::translation::ToJsonRpcError;

/// Default page size for the `session/list` handler.
///
/// [`ListSessionsRequest`](agent_client_protocol::schema::ListSessionsRequest)
/// carries no page-size field — paging is cursor-driven — so the agent picks
/// the size. A modest fixed value keeps each response bounded while remaining
/// useful for a session-picker UI.
const SESSION_LIST_PAGE_SIZE: usize = 50;

pub struct AcpServer {
    /// Underlying llama-agent server
    pub(crate) agent_server: Arc<AgentServer>,

    /// Active ACP sessions (ACP session ID → session state)
    sessions: Arc<RwLock<HashMap<AcpSessionId, AcpSessionState>>>,

    /// Reverse mapping from llama session ID to ACP session ID
    llama_to_acp: Arc<RwLock<HashMap<LlamaSessionId, AcpSessionId>>>,

    /// Broadcast channel for session notifications
    notification_tx: broadcast::Sender<SessionNotification>,

    /// Client capabilities from initialize request for capability gating
    client_capabilities: Arc<RwLock<Option<agent_client_protocol::schema::ClientCapabilities>>>,

    /// ACP server configuration
    config: AcpConfig,

    /// Permission policy engine for evaluating tool call permissions
    permission_engine: PermissionPolicyEngine,

    /// Filesystem operations handler
    filesystem_ops: Arc<FilesystemOperations>,

    /// Terminal manager for process handling
    terminal_manager: Arc<RwLock<TerminalManager>>,

    /// Shared endpoint that relays MCP elicitation requests to the connected ACP
    /// client. Populated once a client connects (see `start_with_streams`); read
    /// by each per-session MCP [`crate::mcp_client_handler::NotifyingClientHandler`].
    elicitation_endpoint: crate::mcp_client_handler::ElicitationEndpoint,

    /// How often the background task scans the session cache for eviction.
    ///
    /// Mirrors claude-agent's `SessionManager::cleanup_interval` so both agents
    /// sweep idle in-memory session state on the same cadence.
    cleanup_interval: std::time::Duration,

    /// Idle time after which a cached session is evicted from the in-memory
    /// maps.
    ///
    /// The in-memory `sessions` / `llama_to_acp` maps are a bounded cache over
    /// the durable [`SessionStore`]; an entry untouched for longer than this is
    /// dropped from the cache. Eviction is lossless — the session's
    /// [`SessionRecord`] persists on disk and [`get_session`](Self::get_session)
    /// transparently reloads it from the store on the next request. Mirrors
    /// claude-agent's `SessionManager::max_session_age`.
    max_session_age: std::time::Duration,

    /// Source of the agent's intrinsic Agent tools (files, web, skill, subagent,
    /// shell).
    ///
    /// Required, never optional: every session mounts a fresh connection from it
    /// during [`new_session`](Self::new_session), independent of the external
    /// MCP server list. This is what makes a llama-agent fully tooled even when
    /// the `session/new` request carries zero MCP servers.
    agent_tools_mount: Arc<dyn crate::mcp::AgentToolsMount>,
}

/// Default interval between session-cache eviction sweeps (5 minutes).
///
/// Matches claude-agent's `SessionManager` default so both agents sweep on the
/// same cadence.
const DEFAULT_CLEANUP_INTERVAL: std::time::Duration = std::time::Duration::from_secs(300);

/// Default idle time before a cached session is evicted (1 hour).
///
/// Matches claude-agent's `SessionManager` default so both agents expire
/// in-memory session state on the same idle-time policy.
const DEFAULT_MAX_SESSION_AGE: std::time::Duration = std::time::Duration::from_secs(3600);

/// Observable result of one agentic-loop generation step: how many tool calls
/// the model emitted and how many of them failed.
///
/// "Failed" means the tool produced an error result (a `ToolResult` with a
/// non-empty `error`, or a hard `handle_tool_call` error) — e.g. the MCP server
/// returned -32602 "tool not found". The runaway guard uses these two counts to
/// decide whether the loop is still making progress.
#[derive(Debug, Clone, Copy)]
struct AgenticStep {
    /// Number of tool calls the model emitted in this step.
    tool_calls: usize,
    /// How many of those tool calls failed.
    failed_tool_calls: usize,
}

/// What the agentic loop should do after a generation step.
#[derive(Debug)]
enum AgenticLoopAction {
    /// Keep looping: re-prompt the model with the tool results.
    Continue,
    /// Abort the turn with this human-readable reason. The loop has stopped
    /// making progress (every tool call failed), a single step emitted an
    /// absurd number of tool calls, or the per-turn iteration cap was hit —
    /// continuing would only hang.
    Abort(String),
}

/// Bounds that stop the agentic loop from running away.
///
/// The loop's normal exit is "the model emitted no tool calls" (it answered) or
/// a hard `StopReason` (MaxTokens/Cancelled/Refusal). These limits are the
/// backstop for the degenerate case the limits were added for: a model that
/// emits a flood of tool calls that all fail (e.g. mis-routed to a backend that
/// returns -32602 "tool not found"), where the loop would otherwise re-prompt
/// with ever-growing context until the caller's timeout fires.
struct AgenticLoopLimits {
    /// Maximum number of generation steps (re-prompts) in a single turn.
    max_iterations: usize,
    /// Maximum number of tool calls accepted from a single generation step.
    max_tool_calls_per_step: usize,
}

impl AgenticLoopLimits {
    /// Decide what the loop should do after a step, given the 1-based
    /// `iteration` that just ran and the step's observed tool-call stats.
    ///
    /// Abort when, in order: the iteration cap is exceeded; a single step
    /// emitted more than `max_tool_calls_per_step` tool calls; or the step
    /// emitted tool calls and *every* one failed (no forward progress).
    /// Otherwise continue.
    fn evaluate(&self, iteration: usize, step: &AgenticStep) -> AgenticLoopAction {
        if iteration > self.max_iterations {
            return AgenticLoopAction::Abort(format!(
                "agentic loop exceeded the per-turn iteration cap ({} iterations)",
                self.max_iterations
            ));
        }
        if step.tool_calls > self.max_tool_calls_per_step {
            return AgenticLoopAction::Abort(format!(
                "a single generation step emitted {} tool calls, exceeding the per-step cap of {}",
                step.tool_calls, self.max_tool_calls_per_step
            ));
        }
        if step.tool_calls > 0 && step.failed_tool_calls == step.tool_calls {
            return AgenticLoopAction::Abort(format!(
                "every one of the {} tool call(s) in this step failed; the loop is not making \
                 progress",
                step.tool_calls
            ));
        }
        AgenticLoopAction::Continue
    }
}

/// Default runaway-loop bounds for the agentic turn loop.
///
/// `max_iterations` is generous — a healthy multi-step tool turn rarely needs
/// more than a handful of re-prompts, but agentic plans can legitimately chain
/// several tools. `max_tool_calls_per_step` catches the pathological single
/// step (the production trace showed ~342 tool calls in one step).
const AGENTIC_LOOP_LIMITS: AgenticLoopLimits = AgenticLoopLimits {
    max_iterations: 32,
    max_tool_calls_per_step: 16,
};

impl AcpServer {
    /// Create an ACP server with the default session-cache eviction policy.
    ///
    /// The eviction policy (5-minute sweep interval, 1-hour idle TTL) matches
    /// claude-agent's `SessionManager` defaults. Use
    /// [`with_cleanup_settings`](Self::with_cleanup_settings) to override it.
    pub fn new(
        agent_server: Arc<AgentServer>,
        config: AcpConfig,
        agent_tools_mount: Arc<dyn crate::mcp::AgentToolsMount>,
    ) -> (
        Self,
        tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
    ) {
        Self::with_cleanup_settings(
            agent_server,
            config,
            agent_tools_mount,
            DEFAULT_CLEANUP_INTERVAL,
            DEFAULT_MAX_SESSION_AGE,
        )
    }

    /// Create an ACP server with an explicit session-cache eviction policy.
    ///
    /// The in-memory `sessions` / `llama_to_acp` maps are a bounded cache over
    /// the durable [`SessionStore`]; `cleanup_interval` controls how often the
    /// background task sweeps the cache and `max_session_age` is the idle time
    /// after which a cache entry is evicted. Mirrors claude-agent's
    /// `SessionManager::with_cleanup_settings`.
    ///
    /// # Parameters
    ///
    /// * `cleanup_interval` - How often to scan the cache for idle sessions.
    /// * `max_session_age` - Idle time after which a cached session is evicted.
    pub fn with_cleanup_settings(
        agent_server: Arc<AgentServer>,
        config: AcpConfig,
        agent_tools_mount: Arc<dyn crate::mcp::AgentToolsMount>,
        cleanup_interval: std::time::Duration,
        max_session_age: std::time::Duration,
    ) -> (
        Self,
        tokio::sync::broadcast::Receiver<agent_client_protocol::schema::SessionNotification>,
    ) {
        let (notification_tx, notification_rx) = broadcast::channel(1000);

        // Initialize permission policy engine from config
        let permission_engine = PermissionPolicyEngine::new(config.permission_policy.clone());

        // Initialize filesystem operations handler
        let filesystem_ops = Arc::new(FilesystemOperations::new(&config.filesystem));

        // Initialize terminal manager with configured buffer size and graceful shutdown timeout
        let terminal_manager = Arc::new(RwLock::new(TerminalManager::with_config(
            config.terminal.output_buffer_bytes,
            config.terminal.graceful_shutdown_timeout.as_duration(),
        )));

        // The raw JSON-RPC transcript recorder is per-session, not per-server:
        // it is created in `new_session` once the session ULID is known and
        // registered in the shared registry keyed by that ULID.
        let server = Self {
            agent_server,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            llama_to_acp: Arc::new(RwLock::new(HashMap::new())),
            notification_tx,
            client_capabilities: Arc::new(RwLock::new(None)),
            config,
            permission_engine,
            filesystem_ops,
            terminal_manager,
            elicitation_endpoint: Arc::new(RwLock::new(None)),
            cleanup_interval,
            max_session_age,
            agent_tools_mount,
        };

        (server, notification_rx)
    }

    /// Start the ACP server with stdio transport
    ///
    /// This is a convenience method that wraps `start_with_streams` using stdin/stdout.
    /// It's the typical way to run an ACP server for editor integration.
    ///
    /// Note: This method takes `self: Arc<Self>`, which means you need to wrap the server
    /// in an Arc before calling this method.
    ///
    /// # Example
    ///
    /// ```text
    /// use llama_agent::acp::{AcpServer, AcpConfig};
    /// use llama_agent::AgentServer;
    /// use std::sync::Arc;
    ///
    /// #[tokio::main]
    /// async fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let config = AcpConfig::default();
    ///     let agent_server = Arc::new(AgentServer::new(/* ... */).await?);
    ///     let (acp_server, _notification_rx) = AcpServer::new(agent_server, config);
    ///     let acp_server = Arc::new(acp_server);
    ///
    ///     // Start server with stdio transport
    ///     Arc::clone(&acp_server).start_stdio().await?;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub async fn start_stdio(self: Arc<Self>) -> Result<(), agent_client_protocol::Error> {
        let stdin = tokio::io::stdin();
        let stdout = tokio::io::stdout();
        self.start_with_streams(stdin, stdout).await
    }

    /// Validate MCP transport capabilities
    ///
    /// Per ACP spec: Agents must validate that MCP transport types match
    /// the capabilities advertised in the initialize response.
    fn validate_mcp_transports(
        &self,
        mcp_servers: &[agent_client_protocol::schema::McpServer],
    ) -> Result<(), agent_client_protocol::Error> {
        for server in mcp_servers {
            match server {
                agent_client_protocol::schema::McpServer::Stdio(_) => {
                    // stdio is always supported (baseline requirement)
                    continue;
                }
                agent_client_protocol::schema::McpServer::Http(_) => {
                    // For now, http is advertised as true in initialize, so allow
                    // TODO: Make this configurable when we add mcp_capabilities to AcpCapabilities
                    tracing::debug!("HTTP MCP server accepted");
                }
                agent_client_protocol::schema::McpServer::Sse(_) => {
                    // SSE is advertised as false, so reject
                    tracing::error!("SSE MCP server requested but sse capability not advertised");
                    return Err(agent_client_protocol::Error::invalid_params());
                }
                _ => {
                    // Unknown MCP server type (future-proofing for protocol extensions)
                    tracing::warn!("Unknown MCP server type, accepting");
                }
            }
        }
        Ok(())
    }

    /// Build the prompt capabilities llama-agent advertises in `initialize`.
    ///
    /// llama-agent only supports text content (see `translation.rs`), so image,
    /// audio, and embedded-context are all advertised as unsupported. This is
    /// the single source of truth: `initialize` advertises exactly these
    /// capabilities, and `prompt` enforces them via
    /// [`ContentCapabilityValidator`], so what is advertised always matches what
    /// is enforced.
    fn advertised_prompt_capabilities() -> agent_client_protocol::schema::PromptCapabilities {
        agent_client_protocol::schema::PromptCapabilities::new()
            .audio(false)
            .embedded_context(false)
            .image(false)
            .meta({
                let mut map = serde_json::Map::new();
                map.insert("streaming".to_string(), serde_json::Value::Bool(true));
                map
            })
    }

    /// Reject prompt content the agent advertised as unsupported.
    ///
    /// Validates every [`ContentBlock`] in the request against the advertised
    /// [`PromptCapabilities`]. This mirrors claude-agent's
    /// `ContentCapabilityValidator` step in its prompt path, so both agents
    /// reject exactly the content types they advertise as unsupported.
    fn validate_prompt_content(
        prompt: &[agent_client_protocol::schema::ContentBlock],
    ) -> Result<(), agent_client_protocol::Error> {
        let validator = super::content_validation::ContentCapabilityValidator::new(
            Self::advertised_prompt_capabilities(),
        );
        if let Err(capability_error) = validator.validate_content_blocks(prompt) {
            let acp_error = capability_error.to_acp_error();
            tracing::warn!(
                "Content capability validation failed: {}",
                acp_error.message
            );
            return Err(acp_error);
        }
        Ok(())
    }

    /// Convert a llama-agent error to ACP JSON-RPC error format
    ///
    /// This helper uses the ToJsonRpcError trait to convert llama-agent errors
    /// into properly formatted ACP errors with correct error codes and structured data.
    fn convert_error<E: ToJsonRpcError>(error: E) -> agent_client_protocol::Error {
        let json_rpc_error = error.to_json_rpc_error();
        let mut error =
            agent_client_protocol::Error::new(json_rpc_error.code, json_rpc_error.message);
        if let Some(data) = json_rpc_error.data {
            error = error.data(data);
        }
        error
    }

    /// Log an incoming ACP request at `debug` level.
    ///
    /// Every ACP method calls this on entry. It is the request half of the
    /// uniform request/response logging convention shared with claude-agent
    /// (`ClaudeAgent::log_request`), so a transcript shows the same
    /// `Handling <method> request: ...` line from either agent.
    fn log_request<T: std::fmt::Debug + serde::Serialize>(&self, method: &str, request: &T) {
        tracing::debug!("Handling {} request: {}", method, Pretty(request));
    }

    /// Log an outgoing ACP response at `debug` level.
    ///
    /// Every ACP method calls this before returning a successful response. It
    /// is the response half of the request/response logging convention shared
    /// with claude-agent (`ClaudeAgent::log_response`), so a transcript shows
    /// the same `Returning <method> response: ...` line from either agent.
    fn log_response<T: std::fmt::Debug + serde::Serialize>(&self, method: &str, response: &T) {
        tracing::debug!("Returning {} response: {}", method, Pretty(response));
    }

    /// Map llama-agent FinishReason to ACP StopReason
    ///
    /// This helper function translates the llama-agent's string-based finish reasons
    /// into the appropriate ACP protocol StopReason enum variants.
    ///
    /// # Mapping Strategy
    /// - "Maximum tokens reached" → MaxTokens
    /// - "Error: Request cancelled" → Cancelled
    /// - All other reasons (including "End of sequence token detected", "Stop token detected",
    ///   "Tool call detected", etc.) → EndTurn
    ///
    /// The ACP protocol uses EndTurn to indicate normal completion of a turn, which includes
    /// cases where the model naturally stops generating tokens (EOS, stop tokens) or when
    /// tool calls have been made and executed.
    fn map_finish_reason_to_stop_reason(
        finish_reason: &crate::types::FinishReason,
    ) -> agent_client_protocol::schema::StopReason {
        match finish_reason {
            crate::types::FinishReason::Stopped(reason) => match reason.as_str() {
                "Maximum tokens reached" => agent_client_protocol::schema::StopReason::MaxTokens,
                "Error: Request cancelled" => agent_client_protocol::schema::StopReason::Cancelled,
                _ => agent_client_protocol::schema::StopReason::EndTurn,
            },
        }
    }

    /// Start the ACP server with custom streams (stdio or other).
    ///
    /// Wires the supplied reader/writer to the ACP 0.11 builder/handler runtime.
    /// Each ACP method (`initialize`, `authenticate`, `session/new`, `session/load`,
    /// `session/set_mode`, `session/prompt`, `session/cancel`, plus the extension
    /// channel) is registered as a typed handler on `Agent.builder()` and
    /// delegates to the inherent method of the same name on `AcpServer`.
    ///
    /// # Concurrency model
    /// The SDK owns the dispatch loop. The closure passed to `connect_with`
    /// (the "bridge") forwards `SessionNotification`s from the internal
    /// broadcast channel to the connected client via `cx.send_notification`.
    ///
    /// Connection liveness is tracked through a [`tokio_util::sync::CancellationToken`]
    /// owned by this call. The reader stream wired into the SDK is wrapped so
    /// that when it returns EOF (clean client disconnect), the token is
    /// cancelled. The bridge races `rx.recv()` against `token.cancelled()` and
    /// returns `Ok(())` on cancel, which lets `connect_with` shut the
    /// connection down cleanly.
    ///
    /// Without this coordination the bridge would block forever on
    /// `rx.recv()` after a clean transport close, because the broadcast
    /// channel's senders are owned by `AcpServer` (which outlives the
    /// connection) and `cx.send_notification` only errors after the SDK has
    /// already torn down its outgoing actor.
    ///
    /// # Arguments
    /// * `reader` - Async reader for incoming JSON-RPC messages (typically stdin)
    /// * `writer` - Async writer for outgoing JSON-RPC messages (typically stdout)
    pub async fn start_with_streams<R, W>(
        self: Arc<Self>,
        reader: R,
        writer: W,
    ) -> Result<(), agent_client_protocol::Error>
    where
        R: tokio::io::AsyncRead + Unpin + Send + 'static,
        W: tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        tracing::info!("Starting ACP server with stdio streams (SDK 0.11 builder)");

        // Start the background task that bounds the in-memory session cache by
        // evicting idle sessions. Eviction is lossless — an evicted session's
        // durable `SessionRecord` is reloaded from the `SessionStore` on the
        // next request — so the cache stays bounded over a long-lived process.
        self.start_cleanup_task();

        // Cancellation token used to wake the notification bridge when the
        // transport's reader hits EOF. The transport's incoming stream is
        // wrapped so EOF triggers `connection_closed.cancel()`, and the bridge
        // exits its `rx.recv()` loop the moment the token fires.
        let connection_closed = tokio_util::sync::CancellationToken::new();

        let transport = build_lines_transport(reader, writer, connection_closed.clone());

        let server = Arc::clone(&self);

        agent_client_protocol::Agent
            .builder()
            .name("llama-agent")
            // A single handler keyed on `ClientRequest` covers every ACP request
            // (initialize, authenticate, session/*, plus extension methods). The
            // SDK demuxes by method name into the right enum variant, and we
            // delegate to the matching inherent method on `AcpServer`.
            .on_receive_request(
                {
                    let server = Arc::clone(&server);
                    async move |req: agent_client_protocol::ClientRequest, responder, _cx| {
                        AcpServer::dispatch_client_request(&server, req, responder).await
                    }
                },
                agent_client_protocol::on_receive_request!(),
            )
            // `ClientNotification` covers `session/cancel` and extension notifications.
            .on_receive_notification(
                {
                    let server = Arc::clone(&server);
                    async move |notif: agent_client_protocol::ClientNotification, _cx| {
                        AcpServer::dispatch_client_notification(&server, notif).await;
                        Ok(())
                    }
                },
                agent_client_protocol::on_receive_notification!(),
            )
            .connect_with(transport, async move |cx| {
                // Publish the live connection as the elicitation endpoint so MCP
                // client handlers can redirect inbound `elicitation/create`
                // requests to this connected ACP client. `cx` is the agent's
                // `ConnectionTo<Client>`; the endpoint is cleared when the bridge
                // exits below so a disconnected client falls back to declining.
                self.publish_client_connection(cx.clone()).await;

                // Bridge: forward broadcast `SessionNotification`s to the client.
                //
                // The bridge exits cleanly when any of the following happens:
                // - `connection_closed` is cancelled (reader EOF — see
                //   `build_lines_transport`).
                // - The broadcast channel reports `Closed` (all senders dropped).
                // - `cx.send_notification` errors (write side of the transport
                //   has shut down).
                //
                // Any of these returning `Ok(())` from the bridge causes
                // `run_until` inside `connect_with` to drop the background
                // dispatch loop and return — i.e. `start_with_streams`
                // completes.
                let mut rx = self.notification_tx.subscribe();
                let bridge_result = loop {
                    tokio::select! {
                        biased;
                        () = connection_closed.cancelled() => {
                            tracing::info!(
                                "Transport closed (reader EOF); shutting down notification bridge"
                            );
                            break Ok(());
                        }
                        recv_result = rx.recv() => {
                            match recv_result {
                                Ok(notification) => {
                                    if let Err(e) = cx.send_notification(notification) {
                                        tracing::error!(
                                            "Failed to forward session/update: {}",
                                            e
                                        );
                                        break Err(e);
                                    }
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                    tracing::info!(
                                        "Session notification channel closed; shutting down connection"
                                    );
                                    break Ok(());
                                }
                                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                                    tracing::warn!(
                                        "Notification bridge lagged; skipped {} updates",
                                        skipped
                                    );
                                }
                            }
                        }
                    }
                };

                // Tear down the elicitation endpoint: no client is connected once
                // the bridge returns, so further elicitations should decline.
                self.clear_client_connection().await;
                bridge_result
            })
            .await
    }

    /// Publish a live ACP client connection as the elicitation endpoint.
    ///
    /// Installs a [`crate::acp::elicitation::ConnectionElicitationSender`] wrapping
    /// `cx` (the agent's `ConnectionTo<Client>`) into the shared
    /// `elicitation_endpoint` cell so per-session MCP
    /// [`crate::mcp_client_handler::NotifyingClientHandler`]s can redirect
    /// inbound `elicitation/create` requests to this connected client. Without
    /// this the endpoint stays `None` and elicitations decline.
    ///
    /// This is the production wiring used both by [`Self::start_with_streams`]
    /// (stdio transport) and by callers that compose `Agent.builder()` /
    /// `connect_with` themselves (e.g. `swissarmyhammer-agent`'s
    /// `wrap_llama_into_handle`), which would otherwise bypass `connect_with`
    /// and never publish the connection. Pair with [`Self::clear_client_connection`]
    /// when the connection ends.
    pub async fn publish_client_connection(
        &self,
        cx: agent_client_protocol::ConnectionTo<agent_client_protocol::Client>,
    ) {
        let sender: Arc<dyn crate::acp::elicitation::ElicitationSender> = Arc::new(
            crate::acp::elicitation::ConnectionElicitationSender::new(cx),
        );
        *self.elicitation_endpoint.write().await = Some(sender);
    }

    /// Tear down the published elicitation endpoint.
    ///
    /// Sets the shared `elicitation_endpoint` cell back to `None` so that once
    /// the client connection ends, further elicitations decline rather than
    /// targeting a dead connection. Mirror of [`Self::publish_client_connection`].
    pub async fn clear_client_connection(&self) {
        *self.elicitation_endpoint.write().await = None;
    }

    /// Report whether an elicitation endpoint (client connection) is published.
    ///
    /// Reads the shared `elicitation_endpoint` cell populated by
    /// [`Self::publish_client_connection`]. Observability accessor for tests and
    /// diagnostics confirming the server can relay `elicitation/create` requests
    /// to a connected client before relying on it.
    pub async fn is_elicitation_endpoint_set(&self) -> bool {
        self.elicitation_endpoint.read().await.is_some()
    }

    /// Resolve a session by ACP session ID.
    ///
    /// The in-memory `sessions` map is a *bounded cache* over the durable
    /// [`SessionStore`], not the source of truth. Resolution is therefore
    /// two-tier:
    ///
    /// 1. **Cache hit** — return the cached [`AcpSessionState`], refreshing its
    ///    [`last_accessed`](AcpSessionState::last_accessed) timestamp so the
    ///    periodic cleanup task treats the session as recently used.
    /// 2. **Cache miss** — attempt to reload the session from the durable
    ///    [`SessionStore`] via [`reload_session_from_store`](Self::reload_session_from_store).
    ///    A miss happens for an evicted session or after a process restart.
    ///    Reload reconstructs the live llama session and the ACP session state
    ///    and re-populates the cache, so an evicted session stays fully
    ///    resolvable for the next request.
    ///
    /// Returns `None` only when the id has no live cache entry *and* no durable
    /// record — i.e. the session genuinely does not exist.
    ///
    /// This is what makes cache eviction lossless: dropping an entry never
    /// loses a session, because the next `get_session` reloads it from disk.
    async fn get_session(&self, session_id: &AcpSessionId) -> Option<AcpSessionState> {
        // Cache hit: refresh recency under the write lock so the cleanup task
        // sees the access, and return the refreshed state.
        {
            let mut sessions = self.sessions.write().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session.touch();
                return Some(session.clone());
            }
        }

        // Cache miss: the session may have been evicted or the process
        // restarted. Reload it from the durable store.
        self.reload_session_from_store(session_id).await
    }

    /// Reload an evicted (or post-restart) session from the durable
    /// [`SessionStore`] and re-populate the in-memory cache.
    ///
    /// This is the cache-miss branch of [`get_session`](Self::get_session). It
    /// reuses the same restore machinery as `session/resume`:
    ///
    /// 1. Load the durable [`SessionRecord`] for the id. A missing record means
    ///    the session never existed (or was never persisted) — returns `None`.
    /// 2. Restore the live llama session from the record via
    ///    [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore).
    /// 3. Reconstruct the [`AcpSessionState`] and insert it into the cache via
    ///    [`store_session`](Self::store_session).
    ///
    /// All failures are logged and surface as `None`: a caller that cannot
    /// resolve the session treats it as not found, exactly as before this
    /// cache existed. The session id stays opaque — a non-llama id simply
    /// fails to restore rather than being rejected on format.
    async fn reload_session_from_store(
        &self,
        session_id: &AcpSessionId,
    ) -> Option<AcpSessionState> {
        let record = match self.load_session_record(&session_id.0) {
            Ok(record) => record,
            Err(super::session_resume::SessionRestoreError::NotFound(_)) => return None,
            Err(e) => {
                tracing::warn!(
                    "Cannot reload evicted session {} from store: {}",
                    session_id.0,
                    e
                );
                return None;
            }
        };

        if let Err(e) = agent_client_protocol_extras::ResumeStrategy::restore(self, &record).await {
            tracing::warn!(
                "Failed to restore evicted session {} from store: {}",
                session_id.0,
                e
            );
            return None;
        }

        let llama_session_id = match crate::types::SessionId::from_str(&session_id.0) {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!(
                    "Reloaded session id {} is not a llama session id: {}",
                    session_id.0,
                    e
                );
                return None;
            }
        };

        let client_caps = self
            .client_capabilities
            .read()
            .await
            .clone()
            .unwrap_or_default();

        let reconstructed = AcpSessionState::with_capabilities(llama_session_id, client_caps);
        self.store_session(reconstructed.clone()).await;
        Self::wire_raw_message_manager(session_id);

        tracing::info!(
            "Reloaded evicted session {} from durable store",
            session_id.0
        );
        Some(reconstructed)
    }

    /// Evict idle sessions from the in-memory cache.
    ///
    /// Drops every cache entry whose [`last_accessed`](AcpSessionState::last_accessed)
    /// age exceeds [`max_session_age`](Self::max_session_age), and removes the
    /// matching reverse mapping from `llama_to_acp`. This bounds the cache for a
    /// long-lived process: without it the `sessions` / `llama_to_acp` maps would
    /// grow for the entire process lifetime.
    ///
    /// Eviction is **lossless** — the durable [`SessionRecord`] for an evicted
    /// session remains on disk, and [`get_session`](Self::get_session) reloads
    /// it transparently on the next request. This mirrors claude-agent's
    /// `SessionManager::cleanup_expired_sessions`.
    ///
    /// Returns the number of sessions evicted.
    async fn cleanup_expired_sessions(&self) -> usize {
        let now = std::time::SystemTime::now();

        // Identify expired sessions under the read lock.
        let expired: Vec<AcpSessionId> = {
            let sessions = self.sessions.read().await;
            sessions
                .iter()
                .filter(|(_, session)| {
                    now.duration_since(session.last_accessed)
                        .map(|age| age > self.max_session_age)
                        .unwrap_or(false)
                })
                .map(|(id, _)| id.clone())
                .collect()
        };

        if expired.is_empty() {
            return 0;
        }

        // Drop the expired entries and their reverse mappings.
        let mut sessions = self.sessions.write().await;
        let mut llama_to_acp = self.llama_to_acp.write().await;
        for acp_id in &expired {
            if let Some(state) = sessions.remove(acp_id) {
                // Only drop the reverse entry if it still points at this ACP
                // id — a concurrent reload may have re-registered the llama id.
                if llama_to_acp.get(&state.llama_session_id) == Some(acp_id) {
                    llama_to_acp.remove(&state.llama_session_id);
                }
                tracing::info!(
                    "Evicted idle session {} from in-memory cache (durable record retained)",
                    acp_id.0
                );
            }
        }

        expired.len()
    }

    /// Spawn the background task that periodically evicts idle sessions.
    ///
    /// Every [`cleanup_interval`](Self::cleanup_interval) the task runs
    /// [`cleanup_expired_sessions`](Self::cleanup_expired_sessions), bounding
    /// the in-memory session cache for a long-lived process. The task is
    /// detached and lives as long as the `Arc<AcpServer>` it holds — i.e. for
    /// the lifetime of the connection. Mirrors claude-agent's
    /// `SessionManager::start_cleanup_task`.
    fn start_cleanup_task(self: &Arc<Self>) {
        let server = Arc::clone(self);
        let interval = server.cleanup_interval;
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            tracing::info!(
                "ACP session-cache cleanup task started (interval: {:?}, max age: {:?})",
                interval,
                server.max_session_age
            );
            loop {
                ticker.tick().await;
                let evicted = server.cleanup_expired_sessions().await;
                if evicted > 0 {
                    tracing::debug!("Session-cache cleanup evicted {} idle sessions", evicted);
                }
            }
        });
    }

    /// Get a session by llama session ID
    /// Store a session and update bidirectional mapping
    async fn store_session(&self, session: AcpSessionState) {
        let acp_id = session.session_id.clone();
        let llama_id = session.llama_session_id;

        // Store the session
        self.sessions.write().await.insert(acp_id.clone(), session);

        // Update reverse mapping
        self.llama_to_acp.write().await.insert(llama_id, acp_id);
    }

    /// Broadcast a notification to all subscribers
    ///
    /// Sends a session update notification via the broadcast channel to all active subscribers.
    /// If there are no active subscribers, the send will fail but this is not considered an error
    /// since the notification handler may not be running yet or the channel may be empty.
    ///
    /// # Arguments
    ///
    /// * `notification` - The session notification to broadcast
    ///
    /// Clear session context on all MCP clients for a session
    async fn clear_mcp_session_context(&self, llama_session_id: &crate::types::SessionId) {
        let session_clients = self.agent_server.session_mcp_clients.read().await;
        if let Some(clients) = session_clients.get(llama_session_id) {
            for client in clients.clients() {
                client.clear_session().await;
            }
            tracing::debug!(
                "Cleared ACP session context on {} MCP clients",
                clients.clients().len()
            );
        }
    }

    /// Create and register the per-session raw JSON-RPC transcript recorder.
    ///
    /// The transcript path embeds the session ULID, so the manager can only be
    /// built once a session exists. The resolved manager writes
    /// `<acp-session-dir>/raw.jsonl` for the session and is registered in the
    /// shared registry keyed by the session ULID, so [`broadcast_notification`]
    /// (and any subagent) can [`RawMessageManager::lookup`] it.
    ///
    /// Failure to create the recorder is logged and otherwise ignored — raw
    /// transcript recording is a debugging aid, not a session requirement.
    ///
    /// # Parameters
    ///
    /// * `session_id` - The ACP session ID, whose string form is the session
    ///   ULID used both as the transcript directory name and the registry key.
    ///
    /// [`broadcast_notification`]: Self::broadcast_notification
    fn wire_raw_message_manager(session_id: &AcpSessionId) {
        let session_ulid = session_id.0.to_string();

        match RawMessageManager::new(&session_ulid) {
            Ok(manager) => {
                RawMessageManager::register(session_ulid.clone(), manager);
                tracing::info!(
                    "Raw ACP JSON-RPC messages recording to <acp-session-dir>/raw.jsonl for session {}",
                    session_ulid
                );
            }
            Err(e) => {
                tracing::warn!("Failed to create raw message recorder: {}", e);
            }
        }
    }

    /// Broadcast one [`super::visible_text::FilterSegment`] as its matching
    /// ACP notification kind. Centralised here so the prompt loop just
    /// iterates a segment list in source order without re-implementing the
    /// segment → notification mapping.
    fn broadcast_segment(
        &self,
        session_id: &agent_client_protocol::schema::SessionId,
        segment: super::visible_text::FilterSegment,
    ) {
        match segment {
            super::visible_text::FilterSegment::Visible(text) => {
                self.broadcast_notification(super::translation::agent_message_notification(
                    session_id.clone(),
                    text,
                ));
            }
            super::visible_text::FilterSegment::Thought(text) => {
                self.broadcast_notification(super::translation::agent_thought_notification(
                    session_id.clone(),
                    text,
                ));
            }
        }
    }

    fn broadcast_notification(&self, notification: SessionNotification) {
        tracing::trace!(
            "Broadcasting notification: {}",
            Pretty(&notification.update)
        );

        // Record the notification to the session's raw transcript for
        // debugging. The recorder is looked up from the shared registry by the
        // session ULID carried on the notification.
        if let Some(manager) = RawMessageManager::lookup(&notification.session_id.0) {
            if let Ok(json) = serde_json::to_string(&notification) {
                manager.record(json);
            }
        }

        match self.notification_tx.send(notification) {
            Ok(subscriber_count) => {
                tracing::trace!("Notification broadcast to {} subscribers", subscriber_count);
            }
            Err(e) => {
                tracing::warn!(
                    "Failed to broadcast notification, no active subscribers: {}",
                    e
                );
            }
        }
    }

    /// Send Plan notification from kanban tool result
    ///
    /// Extracts the `_plan` field from a kanban tool result and broadcasts it as an ACP Plan
    /// notification. The kanban tool includes plan data in its responses when tasks are modified.
    ///
    /// Per ACP spec: "Complete plan lists must be resent with each update; clients will replace
    /// prior plans entirely."
    ///
    /// # Arguments
    ///
    /// * `acp_session_id` - The ACP session ID to use in the notification
    /// * `tool_result` - The tool result containing the `_plan` field
    ///
    /// # Returns
    ///
    /// Returns Ok(()) if the plan was extracted and notification sent, or an error if:
    /// - The tool result doesn't contain a `_plan` field
    /// - The plan data is malformed
    fn send_plan_notification_from_result(
        &self,
        acp_session_id: &agent_client_protocol::schema::SessionId,
        tool_result: &crate::types::ToolResult,
    ) -> Result<(), agent_client_protocol::Error> {
        // Extract _plan from tool result
        // The result may be a string (JSON) or a Value object
        let plan_data = if let Some(plan) = tool_result.result.get("_plan") {
            plan.clone()
        } else if let Some(result_str) = tool_result.result.as_str() {
            // Try to parse as JSON string
            match serde_json::from_str::<serde_json::Value>(result_str) {
                Ok(parsed) => {
                    if let Some(plan) = parsed.get("_plan") {
                        plan.clone()
                    } else {
                        tracing::debug!("No _plan field in kanban tool result (string parsed)");
                        return Ok(());
                    }
                }
                Err(_) => {
                    tracing::debug!("Could not parse kanban tool result as JSON");
                    return Ok(());
                }
            }
        } else {
            tracing::debug!("No _plan field in kanban tool result");
            return Ok(());
        };

        // Convert plan data to ACP Plan format
        let plan = super::plan::plan_data_to_acp_plan(&plan_data);

        // Create and broadcast Plan notification
        let plan_notification = agent_client_protocol::schema::SessionNotification::new(
            acp_session_id.clone(),
            agent_client_protocol::schema::SessionUpdate::Plan(plan),
        );

        self.broadcast_notification(plan_notification);

        let entry_count = plan_data
            .get("entries")
            .and_then(|e| e.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        tracing::info!(
            "Sent Plan notification for session {} with {} entries",
            acp_session_id.0,
            entry_count
        );

        Ok(())
    }

    /// Send CurrentModeUpdate notification when session mode changes
    ///
    /// Broadcasts a CurrentModeUpdate notification to inform the client that the session's
    /// active mode has changed. This can be triggered either by client request (via set_session_mode)
    /// or by the agent autonomously switching modes.
    ///
    /// # Arguments
    ///
    /// * `session_id` - The ACP session ID
    /// * `mode_id` - The new mode ID that is now active
    async fn send_current_mode_update(
        &self,
        session_id: &agent_client_protocol::schema::SessionId,
        mode_id: agent_client_protocol::schema::SessionModeId,
    ) {
        let update = agent_client_protocol::schema::CurrentModeUpdate::new(mode_id.clone());
        let notification = agent_client_protocol::schema::SessionNotification::new(
            session_id.clone(),
            agent_client_protocol::schema::SessionUpdate::CurrentModeUpdate(update),
        );

        self.broadcast_notification(notification);

        tracing::debug!(
            "Sent CurrentModeUpdate notification for session {} to mode {}",
            session_id.0,
            mode_id.0
        );
    }

    /// Build one `session/update` notification for a replayed update, tagging
    /// it as a historical replay so clients can distinguish it from live
    /// output.
    ///
    /// The `_meta` shape is identical to claude-agent's
    /// `ClaudeAgent::build_replay_notification`
    /// (`message_type: "historical_replay"`, `message_index`,
    /// `total_messages`) so a client observes the same replayed-history stream
    /// from both agents.
    fn build_replay_notification(
        session_id: &AcpSessionId,
        update: &agent_client_protocol::schema::SessionUpdate,
        index: usize,
        total: usize,
    ) -> SessionNotification {
        let mut meta = serde_json::Map::new();
        meta.insert(
            "message_type".to_string(),
            serde_json::json!("historical_replay"),
        );
        meta.insert("message_index".to_string(), serde_json::json!(index));
        meta.insert("total_messages".to_string(), serde_json::json!(total));

        SessionNotification::new(session_id.clone(), update.clone()).meta(meta)
    }

    /// Handle the ACP `session/load` request: restore a session, then replay
    /// its recorded conversation to the client.
    ///
    /// `session/load` is `session/resume` plus replay. It:
    ///
    /// 1. Loads the durable
    ///    [`SessionRecord`](agent_client_protocol_extras::SessionRecord) from
    ///    the shared [`SessionStore`].
    /// 2. Restores agent state via
    ///    [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore),
    ///    which reconstructs the in-memory llama session from the record and
    ///    re-renders it through the chat template so the model is primed.
    /// 3. Reconstructs the ACP session state so the next `session/prompt`
    ///    resolves the session.
    /// 4. Replays [`SessionRecord::updates`](agent_client_protocol_extras::SessionRecord::updates)
    ///    as `session/update` notifications — the only step `session/load`
    ///    performs beyond `session/resume` — so the client can reconstruct the
    ///    full conversation history.
    ///
    /// The empty [`LoadSessionResponse`](agent_client_protocol::schema::LoadSessionResponse)
    /// is the correct response: the conversation is delivered through the
    /// replayed notifications, not the response body.
    ///
    /// A missing or unreadable record surfaces as a session-not-found error —
    /// the opaque session id is never rejected on format.
    pub async fn load_session(
        &self,
        req: agent_client_protocol::schema::LoadSessionRequest,
    ) -> Result<agent_client_protocol::schema::LoadSessionResponse, agent_client_protocol::Error>
    {
        self.log_request("load_session", &req);
        tracing::info!("Loading session {}", req.session_id.0);

        // Enforce the advertised `loadSession` capability before acting. The
        // `initialize` response advertises `load_session` from
        // `config.capabilities.supports_session_loading`; if it is disabled the
        // agent must reject `session/load` rather than silently honoring it.
        // This mirrors claude-agent's `check_load_session_requirements` gate so
        // both agents enforce exactly what they advertise. The error shape
        // (`method_not_found`, `requiredCapability: loadSession`) is identical
        // to claude-agent's `LoadSessionNotSupported` mapping.
        if !self.config.capabilities.supports_session_loading {
            tracing::error!(
                "Rejecting session/load: agent does not advertise loadSession capability"
            );
            return Err(super::acp_error::method_not_found(
                "Method not supported: agent does not support loadSession capability",
            )
            .data(serde_json::json!({
                "method": "session/load",
                "requiredCapability": "loadSession",
                "declared": false
            })));
        }

        // Resolve the durable record from the shared SessionStore. This is the
        // source of truth across process restarts — the in-memory llama
        // session may not exist yet.
        let record = self
            .load_session_record(&req.session_id.0)
            .map_err(|e| self.restore_error_to_acp(&req.session_id, e))?;

        // Restore state: reconstruct the in-memory llama session from the
        // record and prime the model via chat-template re-render.
        agent_client_protocol_extras::ResumeStrategy::restore(self, &record)
            .await
            .map_err(|e| self.session_restore_failed_error(&req.session_id, &e))?;

        // Ensure ACP session state exists so the next `session/prompt` resolves
        // the session. The ACP session id is the llama session id string.
        self.ensure_acp_session_state(&req.session_id).await?;

        // Replay the recorded conversation via session/update notifications.
        // This is the only thing `session/load` does beyond `session/resume`.
        //
        // Each replayed notification is tagged with `_meta` identical to
        // claude-agent's replay tagging (`message_type: "historical_replay"`,
        // `message_index`, `total_messages`) so a client can distinguish
        // replayed history from live updates, and observes the same shape from
        // both agents.
        let replayed = record.updates.len();
        for (index, update) in record.updates.iter().enumerate() {
            let notification =
                Self::build_replay_notification(&req.session_id, update, index, replayed);
            self.broadcast_notification(notification);
        }

        tracing::info!(
            "Loaded session {} ({} replayed updates)",
            req.session_id.0,
            replayed
        );

        let response = agent_client_protocol::schema::LoadSessionResponse::new();
        self.log_response("load_session", &response);
        Ok(response)
    }

    /// Handle the ACP `session/resume` request: restore a session and return.
    ///
    /// `session/resume` restores agent state and returns — it MUST NOT replay
    /// history. It loads the durable
    /// [`SessionRecord`](agent_client_protocol_extras::SessionRecord) from the
    /// shared [`SessionStore`] and restores state via
    /// [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore),
    /// which reconstructs the in-memory llama session and re-renders the
    /// conversation through the chat template so the next `session/prompt`
    /// continues it. The recorded conversation is *not* streamed back to the
    /// client; that is `session/load`.
    ///
    /// A missing or unreadable record surfaces as a session-not-found error —
    /// the opaque session id is never rejected on format.
    pub async fn resume_session(
        &self,
        req: agent_client_protocol::schema::ResumeSessionRequest,
    ) -> Result<agent_client_protocol::schema::ResumeSessionResponse, agent_client_protocol::Error>
    {
        self.log_request("resume_session", &req);
        tracing::info!("Resuming session {}", req.session_id.0);

        // Resolve the durable record from the shared SessionStore.
        let record = self
            .load_session_record(&req.session_id.0)
            .map_err(|e| self.restore_error_to_acp(&req.session_id, e))?;

        // Restore state only — no history replay, per the ACP resume contract.
        agent_client_protocol_extras::ResumeStrategy::restore(self, &record)
            .await
            .map_err(|e| self.session_restore_failed_error(&req.session_id, &e))?;

        // Ensure ACP session state exists so the next `session/prompt` resolves
        // the session.
        self.ensure_acp_session_state(&req.session_id).await?;

        tracing::info!("Resumed session {}", req.session_id.0);

        let response = agent_client_protocol::schema::ResumeSessionResponse::new();
        self.log_response("resume_session", &response);
        Ok(response)
    }

    /// Ensure ACP session state exists for a restored session.
    ///
    /// `session/resume` and `session/load` restore the *llama* session via
    /// [`ResumeStrategy::restore`](agent_client_protocol_extras::ResumeStrategy::restore),
    /// but the ACP layer also tracks per-session [`AcpSessionState`] (client
    /// capabilities, mode, permissions). After a process restart that state is
    /// gone, so it is reconstructed here from the current client capabilities.
    /// If the state already exists in memory it is left untouched.
    ///
    /// The ACP session id is the llama session id string, so a non-llama id
    /// surfaces as `invalid_params` — consistent with the restore error path.
    async fn ensure_acp_session_state(
        &self,
        session_id: &AcpSessionId,
    ) -> Result<(), agent_client_protocol::Error> {
        // Check the in-memory cache directly rather than via `get_session`:
        // the caller has just run `ResumeStrategy::restore`, so going through
        // `get_session`'s store-reload fallback would restore the session a
        // second time. A direct cache probe avoids that redundant work.
        if self.sessions.read().await.contains_key(session_id) {
            return Ok(());
        }

        let llama_session_id = crate::types::SessionId::from_str(&session_id.0).map_err(|e| {
            tracing::error!("Restored session id is not a llama session id: {}", e);
            agent_client_protocol::Error::invalid_params()
        })?;

        let client_caps = self
            .client_capabilities
            .read()
            .await
            .clone()
            .unwrap_or_default();

        let reconstructed = AcpSessionState::with_capabilities(llama_session_id, client_caps);
        self.store_session(reconstructed).await;

        // Wire up the per-session transcript recorder so any replayed history
        // and subsequent turns are recorded to the existing transcript.
        Self::wire_raw_message_manager(session_id);

        tracing::info!("Reconstructed ACP session state for {}", session_id.0);
        Ok(())
    }

    /// Handle the ACP `session/list` request.
    ///
    /// Returns persisted sessions from the shared [`SessionStore`], newest
    /// first, honoring the request's optional `cwd` filter and opaque
    /// pagination `cursor`. The returned page carries a `next_cursor` when more
    /// sessions remain.
    ///
    /// # Errors
    ///
    /// Returns [`agent_client_protocol::Error::internal_error`] if the session
    /// store cannot be scanned.
    pub async fn list_sessions(
        &self,
        request: agent_client_protocol::schema::ListSessionsRequest,
    ) -> Result<agent_client_protocol::schema::ListSessionsResponse, agent_client_protocol::Error>
    {
        self.log_request("list_sessions", &request);
        tracing::info!("Listing sessions (cursor: {:?})", request.cursor);

        let page = SessionStore::new()
            .list(
                request.cwd.as_deref(),
                request.cursor.as_deref(),
                SESSION_LIST_PAGE_SIZE,
            )
            .map_err(|e| {
                tracing::error!("Failed to list sessions: {}", e);
                super::acp_error::internal_error(format!("Failed to list sessions: {e}"))
            })?;

        tracing::info!("Listed {} sessions", page.sessions.len());
        let response = agent_client_protocol::schema::ListSessionsResponse::new(page.sessions)
            .next_cursor(page.next_cursor);
        self.log_response("list_sessions", &response);
        Ok(response)
    }

    /// Persist the current state of an ACP session as a durable [`SessionRecord`].
    ///
    /// Loads the live llama session for `acp_session`, projects its
    /// conversation onto an agent-neutral
    /// [`SessionRecord`](agent_client_protocol_extras::SessionRecord) via
    /// [`session_record_from`], and writes it to the shared [`SessionStore`].
    /// Called at the end of `session/new` and each `session/prompt` turn so the
    /// on-disk record tracks the conversation as it grows and `session/list`
    /// can enumerate it across process restarts.
    ///
    /// Persistence failures are logged and swallowed: a turn must not fail
    /// because the durable copy could not be written. The in-memory llama
    /// session remains the source of truth for the running process.
    ///
    /// # Parameters
    ///
    /// * `acp_session` - The ACP session whose live state should be persisted.
    async fn persist_session_record(&self, acp_session: &AcpSessionState) {
        let llama_session = match self
            .agent_server
            .session_manager()
            .get_session(&acp_session.llama_session_id)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => {
                tracing::warn!(
                    "Cannot persist session record: llama session {} not found",
                    acp_session.llama_session_id
                );
                return;
            }
            Err(e) => {
                tracing::warn!(
                    "Cannot persist session record for {}: {}",
                    acp_session.session_id.0,
                    e
                );
                return;
            }
        };

        let record = session_record_from(&acp_session.session_id.0, &llama_session);
        match SessionStore::new().persist(&record) {
            Ok(()) => tracing::debug!("Persisted session record for {}", acp_session.session_id.0),
            Err(e) => tracing::warn!(
                "Failed to persist session record for {}: {}",
                acp_session.session_id.0,
                e
            ),
        }
    }

    /// Generate the session title after the first meaningful exchange, if it is
    /// still missing.
    ///
    /// This implements the shared title contract documented in
    /// [`agent_client_protocol_extras::session_title`]: a title is generated
    /// exactly once, after the session has both a user message and an
    /// assistant response. llama-agent's generation source is a short model
    /// call ([`AgentServer::generate_session_title`]), which falls back to the
    /// first-user-message heuristic on model error.
    ///
    /// Generation is dispatched to a detached task so it never blocks the
    /// `session/prompt` response. The task generates the title, stores it on
    /// the live llama session, persists the updated [`SessionRecord`], and
    /// broadcasts a single
    /// [`SessionUpdate::SessionInfoUpdate`](agent_client_protocol::schema::SessionUpdate)
    /// notification carrying the new title and last-activity timestamp.
    ///
    /// Called at the end of every successful prompt turn. It is a cheap no-op
    /// once a title exists, so re-invoking it on later turns is harmless.
    ///
    /// # Parameters
    ///
    /// * `acp_session` - The ACP session whose title should be generated.
    fn maybe_generate_session_title(&self, acp_session: &AcpSessionState) {
        let agent_server = Arc::clone(&self.agent_server);
        let notification_tx = self.notification_tx.clone();
        let llama_session_id = acp_session.llama_session_id;
        let acp_session_id = acp_session.session_id.clone();

        // Detached task: title generation runs a model call and must never
        // block the prompt response.
        tokio::spawn(async move {
            Self::generate_and_emit_title(
                agent_server,
                notification_tx,
                llama_session_id,
                acp_session_id,
            )
            .await;
        });
    }

    /// Generate a session title and emit the built-in `SessionInfoUpdate`.
    ///
    /// This is the detached worker behind [`maybe_generate_session_title`]. It
    /// re-checks the trigger condition (so a racing turn cannot double-generate
    /// a title), runs the model call, stores the title on the live session,
    /// persists the record, and broadcasts exactly one `SessionInfoUpdate`.
    ///
    /// All failures are logged and swallowed — a missing title must never fail
    /// a turn that has already completed.
    ///
    /// [`maybe_generate_session_title`]: Self::maybe_generate_session_title
    async fn generate_and_emit_title(
        agent_server: Arc<AgentServer>,
        notification_tx: broadcast::Sender<SessionNotification>,
        llama_session_id: LlamaSessionId,
        acp_session_id: AcpSessionId,
    ) {
        let session = match agent_server
            .session_manager()
            .get_session(&llama_session_id)
            .await
        {
            Ok(Some(session)) => session,
            Ok(None) => return,
            Err(e) => {
                tracing::warn!(
                    "Cannot generate session title for {}: {}",
                    acp_session_id.0,
                    e
                );
                return;
            }
        };

        // Generate exactly once, and only after the first meaningful exchange.
        if session.title.is_some() || !super::session_record::has_first_exchange(&session.messages)
        {
            return;
        }

        let Some(first_user_text) =
            super::session_record::first_user_message_text(&session.messages)
        else {
            return;
        };

        let Some(title) = agent_server.generate_session_title(&first_user_text).await else {
            return;
        };

        // Store the title on the live session, guarding against a turn that
        // raced ahead and already set one. The mutation is applied in place
        // under the session write lock so a concurrent turn or MCP context
        // update on this same session is not clobbered by a read-modify-write.
        // `mutate_session` bumps `updated_at` itself, so the closure only
        // touches the title.
        let mut applied = false;
        match agent_server
            .session_manager()
            .mutate_session(&llama_session_id, |session| {
                if session.title.is_none() {
                    session.title = Some(title.clone());
                    applied = true;
                }
            })
            .await
        {
            Ok(()) => {}
            Err(e) => {
                tracing::warn!(
                    "Failed to store generated title for session {}: {}",
                    acp_session_id.0,
                    e
                );
                return;
            }
        }
        if !applied {
            return;
        }

        // Persist so `session/list` returns the title across restarts.
        if let Ok(Some(session)) = agent_server
            .session_manager()
            .get_session(&llama_session_id)
            .await
        {
            let record = session_record_from(&acp_session_id.0, &session);
            if let Err(e) = SessionStore::new().persist(&record) {
                tracing::warn!(
                    "Failed to persist session title for {}: {}",
                    acp_session_id.0,
                    e
                );
            }
        }

        // Emit the single built-in SessionInfoUpdate carrying the new title.
        let updated_at = chrono::Utc::now().to_rfc3339();
        let info_update = agent_client_protocol::schema::SessionInfoUpdate::new()
            .title(title.clone())
            .updated_at(updated_at);
        let notification = SessionNotification::new(
            acp_session_id.clone(),
            agent_client_protocol::schema::SessionUpdate::SessionInfoUpdate(info_update),
        );
        if let Some(manager) = RawMessageManager::lookup(&acp_session_id.0) {
            if let Ok(json) = serde_json::to_string(&notification) {
                manager.record(json);
            }
        }
        match notification_tx.send(notification) {
            Ok(_) => tracing::info!(
                "Generated session title for {}: {}",
                acp_session_id.0,
                title
            ),
            Err(e) => tracing::warn!(
                "Failed to broadcast SessionInfoUpdate for {}: {}",
                acp_session_id.0,
                e
            ),
        }
    }

    /// Get a session by ACP session ID
    ///
    /// This method retrieves an ACP session from the in-memory cache. If the session is not found
    /// in memory, it returns None. To load a session from persistent storage, use the load_session
    /// method instead.
    ///
    /// # Arguments
    /// * `session_id` - The ACP session ID to retrieve
    ///
    /// # Returns
    /// * `Some(AcpSessionState)` if the session exists in memory
    /// * `None` if the session is not found in the in-memory cache
    ///
    /// # Example
    /// ```text
    /// let session = server.get_session_by_id(&session_id).await;
    /// if let Some(session) = session {
    ///     println!("Found session: {}", session.session_id.0);
    /// }
    /// ```
    pub async fn get_session_by_id(&self, session_id: &AcpSessionId) -> Option<AcpSessionState> {
        self.get_session(session_id).await
    }

    /// Borrow the underlying llama [`AgentServer`].
    ///
    /// The `AgentServer` owns the session manager, model, and chat template.
    /// Exposing it lets the session-resume layer reconstruct sessions and
    /// re-render conversations, and lets integration tests inspect restored
    /// session state after `session/resume` and `session/load`.
    #[must_use]
    pub fn agent_server(&self) -> &Arc<AgentServer> {
        &self.agent_server
    }

    /// Supported ACP protocol versions (V0 and V1)
    const SUPPORTED_PROTOCOL_VERSIONS: &'static [agent_client_protocol::schema::ProtocolVersion] =
        &[
            agent_client_protocol::schema::ProtocolVersion::V0,
            agent_client_protocol::schema::ProtocolVersion::V1,
        ];

    /// Negotiate protocol version according to ACP specification
    ///
    /// Returns the client's requested version if supported, otherwise returns
    /// the agent's latest supported version (V1).
    ///
    /// Defined as a `pub(crate)` associated function (no `&self`): negotiation
    /// depends only on the client's requested version and the static
    /// [`Self::SUPPORTED_PROTOCOL_VERSIONS`] list, never on instance state.
    /// `claude-agent` carries the identical `pub(crate)` associated-function
    /// signature so the "one convention" claim holds at the signature level.
    ///
    /// # Arguments
    /// * `client_requested_version` - The protocol version requested by the client
    ///
    /// # Returns
    /// The negotiated protocol version to use for the session
    pub(crate) fn negotiate_protocol_version(
        client_requested_version: &agent_client_protocol::schema::ProtocolVersion,
    ) -> agent_client_protocol::schema::ProtocolVersion {
        // If client's requested version is supported, use it
        if Self::SUPPORTED_PROTOCOL_VERSIONS.contains(client_requested_version) {
            *client_requested_version
        } else {
            // Otherwise, return agent's latest supported version
            *Self::SUPPORTED_PROTOCOL_VERSIONS
                .iter()
                .max()
                .unwrap_or(&agent_client_protocol::schema::ProtocolVersion::V1)
        }
    }

    /// Build the session mode state from configured modes
    ///
    /// Creates a SessionModeState using the modes provided in the config.
    /// If no modes are configured, returns None.
    ///
    /// # Arguments
    /// * `current_mode` - The current mode ID to set
    ///
    /// # Returns
    /// SessionModeState with configured modes, or None if no modes available
    fn build_session_mode_state_with_current(
        &self,
        current_mode: &str,
    ) -> Option<agent_client_protocol::schema::SessionModeState> {
        use agent_client_protocol::schema::{SessionModeId, SessionModeState};

        if self.config.available_modes.is_empty() {
            return None;
        }

        Some(SessionModeState::new(
            SessionModeId::new(current_mode),
            self.config.available_modes.clone(),
        ))
    }
}

// ACP protocol entry-points used by the SDK 0.11 builder/handler layer.
// Each method matches a JSON-RPC request handler registered on
// `Agent.builder().on_receive_request(...)` in `start_with_streams`.
impl AcpServer {
    pub async fn initialize(
        &self,
        request: agent_client_protocol::schema::InitializeRequest,
    ) -> Result<agent_client_protocol::schema::InitializeResponse, agent_client_protocol::Error>
    {
        self.log_request("initialize", &request);

        // `initialize` is a light, non-fatal handshake: negotiate the protocol
        // version and never hard-fail on a mismatch. There is no request-body
        // validation beyond negotiation — claude-agent follows the identical
        // convention.
        let negotiated_version = Self::negotiate_protocol_version(&request.protocol_version);

        tracing::trace!(
            "Negotiated protocol version: {}",
            Pretty(&negotiated_version)
        );

        // Store client capabilities for capability gating
        {
            let mut client_caps = self.client_capabilities.write().await;
            *client_caps = Some(request.client_capabilities.clone());
        }

        // Update terminal manager with client capabilities
        {
            let mut terminal_mgr = self.terminal_manager.write().await;
            terminal_mgr.set_client_capabilities(request.client_capabilities.clone());
        }

        tracing::trace!(
            "Stored client capabilities for capability enforcement: {}",
            Pretty(&request.client_capabilities)
        );

        // Build agent capabilities from config. Only advertise capabilities we
        // actually support. The advertised prompt capabilities come from
        // `advertised_prompt_capabilities()` — the same source of truth that
        // `prompt` enforces against, so advertise and enforce never drift.
        let prompt_caps = Self::advertised_prompt_capabilities();

        let mcp_caps = agent_client_protocol::schema::McpCapabilities::new()
            .http(true)
            .sse(false);

        // Advertise `session/list` and `session/resume`. Sessions are
        // persisted to the shared SessionStore in `session/new` and at the end
        // of each prompt turn, so the agent can enumerate them and resume them
        // across process restarts. `session/resume` restores the conversation
        // by re-rendering it through the model's chat template; the matching
        // `session/load` (resume plus history replay) is gated separately by
        // the `load_session` capability flag below.
        let session_caps = agent_client_protocol::schema::SessionCapabilities::new()
            .list(agent_client_protocol::schema::SessionListCapabilities::new())
            .resume(agent_client_protocol::schema::SessionResumeCapabilities::new());

        // Build the agent capability `_meta` map. ACP's `AgentCapabilities`
        // has no first-class field for these flags, so they live in `_meta` as
        // genuine agent-specific extras (per the ACP extensibility contract).
        //
        // `supports_slash_commands` is deliberately NOT advertised: the
        // `CommandRegistry` exists but is not wired into the session lifecycle
        // and no `AvailableCommandsUpdate` is ever emitted, so the agent does
        // not actually deliver slash commands. Advertising a capability that is
        // not delivered is the bug this card fixes — honest behavior is to omit
        // it until the registry is wired (tracked by the notification-parity
        // card). `supports_modes`/`supports_plans` ARE genuinely delivered
        // (modes via `session/new` + `session/set_mode`, plans via plan
        // notifications), so they remain advertised.
        let agent_capabilities = agent_client_protocol::schema::AgentCapabilities::new()
            .load_session(self.config.capabilities.supports_session_loading)
            .prompt_capabilities(prompt_caps)
            .mcp_capabilities(mcp_caps)
            .session_capabilities(session_caps)
            .meta({
                let mut map = serde_json::Map::new();
                map.insert("streaming".to_string(), serde_json::Value::Bool(true));
                map.insert(
                    "supports_modes".to_string(),
                    serde_json::Value::Bool(self.config.capabilities.supports_modes),
                );
                map.insert(
                    "supports_plans".to_string(),
                    serde_json::Value::Bool(self.config.capabilities.supports_plans),
                );
                map
            });

        // Build Implementation using builder pattern
        let agent_info = agent_client_protocol::schema::Implementation::new(
            "llama-agent",
            env!("CARGO_PKG_VERSION"),
        )
        .title(format!("LLaMA Agent v{}", env!("CARGO_PKG_VERSION")));

        // Return InitializeResponse with agent capabilities using builder pattern
        let response = agent_client_protocol::schema::InitializeResponse::new(negotiated_version)
            .agent_capabilities(agent_capabilities)
            .auth_methods(vec![])
            .agent_info(agent_info);

        self.log_response("initialize", &response);
        Ok(response)
    }

    pub async fn authenticate(
        &self,
        request: agent_client_protocol::schema::AuthenticateRequest,
    ) -> Result<agent_client_protocol::schema::AuthenticateResponse, agent_client_protocol::Error>
    {
        self.log_request("authenticate", &request);

        // AUTHENTICATION ARCHITECTURE DECISION:
        // llama-agent declares NO authentication methods in initialize().
        // According to ACP spec, clients should not call authenticate when no methods are declared.
        // If they do call authenticate anyway, we reject it with a clear error.
        tracing::warn!(
            "Authentication attempt rejected - no auth methods declared: {:?}",
            request.method_id
        );

        Err(super::acp_error::method_not_found(
            "Authentication is not supported: llama-agent declares no auth methods in initialize.",
        ))
    }

    pub async fn new_session(
        &self,
        request: agent_client_protocol::schema::NewSessionRequest,
    ) -> Result<agent_client_protocol::schema::NewSessionResponse, agent_client_protocol::Error>
    {
        self.log_request("new_session", &request);
        tracing::info!(
            "Creating new ACP session with cwd: {:?}, mcp_servers: {}",
            request.cwd,
            request.mcp_servers.len()
        );

        // Validate MCP transport capabilities before accepting servers
        self.validate_mcp_transports(&request.mcp_servers)?;

        // Create a new llama-agent session with the provided cwd
        let llama_session = self
            .agent_server
            .create_session_with_cwd(request.cwd)
            .await
            .map_err(|e| {
                tracing::error!("Failed to create llama session: {}", e);
                Self::convert_error(e)
            })?;

        // Inject the default mode's system prompt as the initial System message
        if let Some(system_prompt) = self
            .config
            .mode_system_prompts
            .get(&self.config.default_mode_id)
        {
            self.agent_server
                .set_session_system_prompt(&llama_session.id, system_prompt.clone())
                .await
                .map_err(|e| {
                    tracing::warn!("Failed to set default system prompt: {}", e);
                    agent_client_protocol::Error::internal_error()
                })?;
            tracing::info!(
                "Injected system prompt from mode '{}' ({} chars)",
                self.config.default_mode_id,
                system_prompt.len()
            );
        }

        // Merge default MCP servers from config with request MCP servers
        let mut all_mcp_servers = self.config.default_mcp_servers.clone();
        all_mcp_servers.extend(request.mcp_servers.clone());

        // Assemble the session's MCP clients. The Agent-tools mount comes first
        // and is ALWAYS present — the agent's intrinsic tools (files, web,
        // skill, subagent, shell) are mounted in-process regardless of how many
        // external MCP servers the request lists. An empty external server list
        // therefore still yields a fully-tooled agent.
        let mut clients: Vec<Arc<dyn crate::mcp::MCPClient>> = Vec::new();

        match self.agent_tools_mount.connect().await {
            Ok(mount_client) => {
                tracing::info!("Mounted in-process Agent-tools server for session");
                clients.push(mount_client);
            }
            Err(e) => {
                // The mount is intrinsic; failing to mount it leaves the agent
                // without its base tools. Unlike an external MCP server (optional,
                // log-and-continue below), the agent-tools mount is mandatory, so a
                // connect failure must FAIL session creation rather than silently
                // yield a tool-less agent. This enforces the load-bearing invariant
                // that a llama-agent session always has its intrinsic Agent tools.
                tracing::error!("Failed to mount in-process Agent-tools server: {}", e);
                return Err(Self::convert_error(e));
            }
        }

        if !all_mcp_servers.is_empty() {
            tracing::info!(
                "Creating {} external MCP clients for session ({} from config, {} from request)",
                all_mcp_servers.len(),
                self.config.default_mcp_servers.len(),
                request.mcp_servers.len()
            );

            // Create notifying handler that forwards MCP notifications as ACP
            // and relays MCP elicitation requests to the connected ACP client
            // through the shared, late-populated endpoint. The shared client
            // capabilities let the handler decline elicitations the client never
            // advertised support for, matching claude-agent's bridge.
            let handler = Arc::new(
                crate::mcp_client_handler::NotifyingClientHandler::with_elicitation_endpoint(
                    self.notification_tx.clone(),
                    self.elicitation_endpoint.clone(),
                    self.client_capabilities.clone(),
                ),
            );

            for server in &all_mcp_servers {
                match super::mcp_client_factory::create_mcp_client_from_acp(server, handler.clone())
                    .await
                {
                    Ok(client) => {
                        tracing::info!("Successfully created MCP client");
                        clients.push(client);
                    }
                    Err(e) => {
                        tracing::error!("Failed to create MCP client: {}", e);
                        // Don't fail the entire session creation if one MCP server fails
                        // Log and continue with other servers
                    }
                }
            }
        }

        if !clients.is_empty() {
            let client_count = clients.len();

            // Discover tools from all MCP clients, preserving each
            // tool's full JSON Schema (description + parameters) so
            // the chat-template renderer sees the real parameter
            // contract rather than a placeholder empty object.
            // No cross-client name-collision dedup: this assumes tool names are
            // unique across the mount and external servers. Holds today because
            // llama gets shell only from the intrinsic mount and external servers
            // serve `Shared`-only tools; a future external server exposing a
            // duplicate name would silently double-register and must dedup here.
            // Discover each client's tools exactly once. The discovery is used
            // for BOTH the model-visible `available_tools` set and the
            // dispatch-routing index, so a tool advertised to the model is
            // always routable to the client that advertised it. Discovering
            // once (rather than re-querying when building the routing index)
            // also closes the second discovery-race window where a transient
            // list-tools failure could drop a client's tools from the index and
            // mis-route its calls to the wrong backend (-32602 "tool not found"
            // — the runaway-loop trigger).
            let mut all_tools = Vec::new();
            let mut discovered: Vec<(Arc<dyn crate::mcp::MCPClient>, Vec<String>)> =
                Vec::with_capacity(clients.len());
            for client in clients {
                match client.list_tools_with_schemas().await {
                    Ok(tools) => {
                        tracing::info!("Discovered {} tools from MCP client", tools.len());
                        let names = tools.iter().map(|t| t.name.clone()).collect();
                        all_tools.extend(tools);
                        discovered.push((client, names));
                    }
                    Err(e) => {
                        tracing::warn!("Failed to list tools from MCP client: {}", e);
                        // Keep the client attached with no routed tools; a call
                        // to one of its tools falls back to first-client routing
                        // exactly as before, and the agentic-loop guard bounds
                        // any resulting failures.
                        discovered.push((client, Vec::new()));
                    }
                }
            }

            // Update session with discovered tools
            if !all_tools.is_empty() {
                if let Ok(Some(mut session)) = self
                    .agent_server
                    .session_manager()
                    .get_session(&llama_session.id)
                    .await
                {
                    tracing::info!(
                        "Adding {} MCP tools to session {}",
                        all_tools.len(),
                        llama_session.id
                    );
                    session.available_tools.extend(all_tools);
                    let _ = self
                        .agent_server
                        .session_manager()
                        .update_session(session)
                        .await;
                }
            }

            self.agent_server.session_mcp_clients.write().await.insert(
                llama_session.id,
                crate::agent::SessionMcpClients::from_discovered(discovered),
            );
            tracing::info!(
                "Stored {} MCP clients for session {}",
                client_count,
                llama_session.id
            );
        }

        // Get stored client capabilities
        let client_caps = self
            .client_capabilities
            .read()
            .await
            .clone()
            .unwrap_or_default();

        // Create ACP session state with client capabilities
        let acp_session = AcpSessionState::with_capabilities(llama_session.id, client_caps);
        let session_id = acp_session.session_id.clone();

        // Store the session
        self.store_session(acp_session.clone()).await;

        // Persist an initial SessionRecord so the session is enumerable via
        // `session/list` immediately, before the first prompt turn.
        self.persist_session_record(&acp_session).await;

        tracing::info!("Created new ACP session: {}", session_id.0);

        // Wire up the per-session raw JSON-RPC transcript recorder. The
        // transcript path embeds the session ULID, so the manager can only be
        // built once the session exists. It is registered in the shared
        // registry keyed by the session ULID; `broadcast_notification` looks
        // it up from there to record outgoing frames.
        Self::wire_raw_message_manager(&session_id);

        // Build session mode state if modes are supported
        let modes = if self.config.capabilities.supports_modes {
            self.build_session_mode_state_with_current(&self.config.default_mode_id)
        } else {
            None
        };

        let mut response = agent_client_protocol::schema::NewSessionResponse::new(session_id);
        if let Some(mode_state) = modes {
            response = response.modes(mode_state);
        }

        self.log_response("new_session", &response);
        Ok(response)
    }

    pub async fn set_session_mode(
        &self,
        request: agent_client_protocol::schema::SetSessionModeRequest,
    ) -> Result<agent_client_protocol::schema::SetSessionModeResponse, agent_client_protocol::Error>
    {
        self.log_request("set_session_mode", &request);

        // Parse mode ID from request
        let mode_id = &request.mode_id;
        let session_id = &request.session_id;

        tracing::info!(
            "set_session_mode called for session {} with mode_id: {}",
            session_id.0,
            mode_id.0
        );

        // Validate mode ID is in available modes list
        if !self.config.available_modes.is_empty() {
            let mode_exists = self
                .config
                .available_modes
                .iter()
                .any(|m| m.id.0.as_ref() == mode_id.0.as_ref());

            if !mode_exists {
                tracing::error!(
                    "Invalid mode '{}' requested. Available modes: {:?}",
                    mode_id.0,
                    self.config
                        .available_modes
                        .iter()
                        .map(|m| m.id.0.as_ref())
                        .collect::<Vec<_>>()
                );
                return Err(agent_client_protocol::Error::invalid_params());
            }
        }

        // Get ACP session to find llama session ID
        let acp_session = self.get_session(session_id).await.ok_or_else(|| {
            tracing::error!("Session not found: {}", session_id.0);
            agent_client_protocol::Error::invalid_params()
        })?;

        // Update the llama session's current_mode field
        let llama_session_id = acp_session.llama_session_id;
        self.agent_server
            .set_session_mode(&llama_session_id, mode_id.0.to_string())
            .await
            .map_err(|e| {
                tracing::error!("Failed to update session mode: {}", e);
                agent_client_protocol::Error::internal_error()
            })?;

        // Swap the system prompt to the new mode's agent instructions
        if let Some(system_prompt) = self.config.mode_system_prompts.get(mode_id.0.as_ref()) {
            self.agent_server
                .set_session_system_prompt(&llama_session_id, system_prompt.clone())
                .await
                .map_err(|e| {
                    tracing::warn!(
                        "Failed to update system prompt for mode '{}': {}",
                        mode_id.0,
                        e
                    );
                    agent_client_protocol::Error::internal_error()
                })?;
            tracing::info!(
                "Swapped system prompt for mode '{}' ({} chars)",
                mode_id.0,
                system_prompt.len()
            );
        }

        tracing::info!("Session mode set to: {}", mode_id.0);

        // Send CurrentModeUpdate notification to inform client of the mode change
        self.send_current_mode_update(session_id, mode_id.clone())
            .await;

        let mut response = agent_client_protocol::schema::SetSessionModeResponse::new();

        // Add metadata to indicate mode was successfully set
        let mut meta = serde_json::Map::new();
        meta.insert("mode_set".to_string(), serde_json::Value::Bool(true));
        meta.insert(
            "mode_id".to_string(),
            serde_json::Value::String(mode_id.0.to_string()),
        );
        response.meta = Some(meta);

        self.log_response("set_session_mode", &response);
        Ok(response)
    }

    pub async fn prompt(
        &self,
        request: agent_client_protocol::schema::PromptRequest,
    ) -> Result<agent_client_protocol::schema::PromptResponse, agent_client_protocol::Error> {
        self.log_request("prompt", &request);
        tracing::info!("Processing prompt for session {}", request.session_id.0);

        // Reject prompt content the agent advertised as unsupported (image,
        // audio, embedded resources). This enforces the `promptCapabilities`
        // advertised in `initialize` — mirroring claude-agent's
        // `ContentCapabilityValidator` step — so both agents reject exactly the
        // content types they declare unsupported, with the same ACP error
        // shape. Capability validation is a request-shape check independent of
        // session resolution, so it runs first.
        Self::validate_prompt_content(&request.prompt)?;

        // Get ACP session
        let acp_session = self.get_session(&request.session_id).await.ok_or_else(|| {
            tracing::error!("Session not found: {}", request.session_id.0);
            agent_client_protocol::Error::invalid_params()
        })?;

        // Optional per-request generation cap. The ACP `_meta` map is the
        // documented extensibility channel — callers (e.g. the validator
        // runner) attach a `"max_tokens"` key here to defend against runaway
        // generation. The ACP spec lets agents ignore unknown `_meta` keys, so
        // honoring it is a deliberate opt-in: this server clamps the per-turn
        // cap below to `min(MAX_GENERATION_TOKENS, available_tokens, requested)`
        // when present. Hitting the cap surfaces as `StopReason::MaxTokens` to
        // the caller via `map_finish_reason_to_stop_reason`.
        let requested_max_tokens = extract_request_max_tokens(request.meta.as_ref());

        // Translate ACP content to llama messages
        let messages = super::translation::acp_to_llama_messages(request.prompt).map_err(|e| {
            tracing::error!("Failed to translate ACP content to llama messages: {}", e);
            Self::convert_error(e)
        })?;

        if messages.is_empty() {
            tracing::error!("Empty prompt after translation");
            return Err(agent_client_protocol::Error::invalid_params());
        }

        // Add all translated messages to llama session
        for message in messages {
            self.agent_server
                .add_message(&acp_session.llama_session_id, message)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to add message to session: {}", e);
                    Self::convert_error(e)
                })?;
        }

        // Set session context on all MCP clients for this session
        // This ensures MCP notifications are tagged with the correct ACP session ID
        {
            let session_clients = self.agent_server.session_mcp_clients.read().await;
            if let Some(clients) = session_clients.get(&acp_session.llama_session_id) {
                for client in clients.clients() {
                    client.set_session(request.session_id.clone()).await;
                }
                tracing::debug!(
                    "Set ACP session context on {} MCP clients",
                    clients.clients().len()
                );
            }
        }

        // Agentic loop: Continue generating until no more tool calls are produced
        let mut total_tokens = 0usize;
        let mut total_tool_calls = 0usize;
        let mut final_stop_reason = agent_client_protocol::schema::StopReason::EndTurn;
        let mut all_generated_text = String::new();
        // 1-based count of generation steps in this turn, fed to the runaway
        // guard so a turn that keeps re-prompting cannot loop forever.
        let mut iteration = 0usize;

        loop {
            iteration += 1;
            // Calculate max_tokens based on available context space
            let model_context_size = self.agent_server.get_context_size().await.unwrap_or(4096); // Default fallback

            // Get current token usage from session
            let current_tokens = self
                .agent_server
                .get_session(&acp_session.llama_session_id)
                .await
                .ok()
                .flatten()
                .map(|session| session.token_usage().total)
                .unwrap_or(0);

            // Calculate available space in context window
            let available_tokens = model_context_size.saturating_sub(current_tokens);

            // Cap max_tokens to min(16k, available_space) to prevent hanging
            // and ensure reasonable generation limits.
            //
            // If the caller provided a stricter cap via `request.meta.max_tokens`,
            // honor it as an additional upper bound — this lets the validator
            // runner enforce a defense-in-depth limit per rule. We never raise
            // the cap above `MAX_GENERATION_TOKENS`; callers can only tighten,
            // not loosen, the server-side limit.
            const MAX_GENERATION_TOKENS: usize = 16384; // 16k tokens
            const MIN_GENERATION_TOKENS: usize = 512; // Minimum reasonable generation

            let server_cap = available_tokens.min(MAX_GENERATION_TOKENS);
            let max_tokens = if available_tokens < MIN_GENERATION_TOKENS {
                tracing::warn!(
                    "Very limited context space available: {} tokens (used: {}/{})",
                    available_tokens,
                    current_tokens,
                    model_context_size
                );
                MIN_GENERATION_TOKENS.min(available_tokens)
            } else {
                match requested_max_tokens {
                    Some(requested) => server_cap.min(requested),
                    None => server_cap,
                }
            };

            tracing::debug!(
                "Context usage: {}/{} tokens, max_tokens set to {}",
                current_tokens,
                model_context_size,
                max_tokens
            );

            // Use AgentServer's streaming generate method
            let generation_request = crate::types::GenerationRequest {
                session_id: acp_session.llama_session_id,
                max_tokens: Some(max_tokens as u32),
                temperature: None,
                top_p: None,
                stop_tokens: vec![],
                stopping_config: None,
            };

            let mut stream = self
                .agent_server
                .generate_stream(generation_request)
                .await
                .map_err(|e| {
                    tracing::error!("Agent streaming generation failed: {}", e);
                    Self::convert_error(e)
                })?;

            // Stream chunks and convert each to ACP notification.
            //
            // `generated_text` accumulates the FULL raw output (needed by
            // `extract_tool_calls` below and the response meta), but the text
            // streams to the client through `VisibleTextFilter`, which
            // partitions it into two ACP channels:
            //
            // - `visible` text → `AgentMessageChunk`s (assistant reply).
            // - `<think>` content → `AgentThoughtChunk`s (reasoning the UI
            //   renders distinctly; per card 01KSXAVM5Y2B0PMXQ4BR656NDR a
            //   truncated-mid-think turn now surfaces here instead of going
            //   silent).
            //
            // `<tool_call>` content is dropped from both — the structured
            // `ToolCall` notification is the only representation a client sees.
            let mut generated_text = String::new();
            let mut llama_finish_reason: Option<crate::types::FinishReason> = None;
            let mut turn_tokens = 0;
            let mut visible = super::visible_text::VisibleTextFilter::default();
            while let Some(chunk_result) = stream.next().await {
                match chunk_result {
                    Ok(chunk) => {
                        turn_tokens += chunk.token_count;
                        generated_text.push_str(&chunk.text);

                        // Capture finish reason from final chunk
                        if chunk.is_complete {
                            llama_finish_reason = chunk.finish_reason.clone();
                        }

                        // Segments come back IN SOURCE ORDER — visible runs
                        // and thought runs interleaved as the model wrote
                        // them. Broadcasting them in order is what makes the
                        // UI render `<think>` ahead of the visible text that
                        // followed it (and ahead of any tool call extracted
                        // from this turn). Aggregating into two flat fields
                        // per push lost that ordering and surfaced thinking
                        // after the surrounding text.
                        for segment in visible.push(&chunk.text) {
                            self.broadcast_segment(&request.session_id, segment);
                        }
                    }
                    Err(e) => {
                        tracing::error!("Stream chunk error: {}", e);
                        return Err(Self::convert_error(e));
                    }
                }
            }

            // Flush any text held back at the stream's end. Outside-a-span
            // tail goes to visible; an unterminated `<think>` tail goes to
            // thought (so a turn that ran out of budget mid-reasoning still
            // shows the user what the model was thinking about).
            for segment in visible.finish() {
                self.broadcast_segment(&request.session_id, segment);
            }

            total_tokens += turn_tokens;
            all_generated_text.push_str(&generated_text);

            tracing::info!(
                "Agent generation turn completed: {} tokens in this turn, {} total",
                turn_tokens,
                total_tokens
            );

            // Extract and execute tool calls from the generated text
            let tool_calls = self
                .agent_server
                .chat_template()
                .extract_tool_calls(&generated_text)
                .map_err(|e| {
                    tracing::error!("Failed to extract tool calls: {}", e);
                    Self::convert_error(e)
                })?;

            // Update stop reason from this turn's finish reason
            if let Some(ref reason) = llama_finish_reason {
                final_stop_reason = Self::map_finish_reason_to_stop_reason(reason);
            }

            if tool_calls.is_empty() {
                // No tool calls - agent is done. Persist this final assistant
                // turn so (1) the conversation history is complete — the model's
                // own last reply is available on the NEXT user prompt — and (2)
                // the cached KV (which ends with exactly these tokens) stays a
                // valid prefix of the next prompt, preserving cross-prompt cache
                // reuse instead of forcing a cold full reprocess. Mirrors the
                // per-turn assistant-message persistence below and the batch
                // loop in `AgentServer::generate`.
                tracing::info!("No tool calls detected, ending agentic loop");
                let final_assistant_message = crate::types::Message {
                    role: crate::types::MessageRole::Assistant,
                    content: generated_text.clone(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: std::time::SystemTime::now(),
                };
                self.agent_server
                    .add_message(&acp_session.llama_session_id, final_assistant_message)
                    .await
                    .map_err(|e| {
                        tracing::error!("Failed to add final assistant message to session: {}", e);
                        Self::convert_error(e)
                    })?;
                break;
            }

            let tool_calls_count = tool_calls.len();
            total_tool_calls += tool_calls_count;
            tracing::info!(
                "Detected {} tool calls in generated text, executing them",
                tool_calls_count
            );

            // Count how many of this step's tool calls failed so the runaway
            // guard below can tell forward progress from a loop spinning on
            // calls that never succeed.
            let mut failed_tool_calls = 0usize;

            // Persist the assistant's own turn (the RAW generated text, including
            // the `<tool_call>` markup) BEFORE its tool results. This mirrors the
            // batch agentic loop (`AgentServer::generate`) and matters for two
            // reasons:
            //   1. Correctness — the rendered conversation stays well-formed
            //      (user → assistant(tool_call) → tool(result)); without it the
            //      model never sees the call it is about to receive results for.
            //   2. KV-cache reuse — the next turn's prompt must EXTEND, not
            //      diverge from, the tokens already cached from this turn. The
            //      cached KV ends with these assistant tokens, so the re-rendered
            //      prompt must contain them at the same positions for the cached
            //      prefix to remain valid.
            let assistant_message = crate::types::Message {
                role: crate::types::MessageRole::Assistant,
                content: generated_text.clone(),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            };
            self.agent_server
                .add_message(&acp_session.llama_session_id, assistant_message)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to add assistant message to session: {}", e);
                    Self::convert_error(e)
                })?;

            // Execute each tool call
            for tool_call in tool_calls {
                let tool_name = tool_call.name.clone();
                let tool_call_id = tool_call.id;
                tracing::info!("Processing tool call: {} (id: {})", tool_name, tool_call_id);

                // Send initial ToolCall notification with pending status (per ACP spec)
                let initial_tool_call = agent_client_protocol::schema::ToolCall::new(
                    agent_client_protocol::schema::ToolCallId::new(tool_call_id.to_string()),
                    &tool_name,
                )
                .status(agent_client_protocol::schema::ToolCallStatus::Pending)
                .raw_input(tool_call.arguments.clone());

                let tool_call_notification =
                    agent_client_protocol::schema::SessionNotification::new(
                        request.session_id.clone(),
                        agent_client_protocol::schema::SessionUpdate::ToolCall(initial_tool_call),
                    );
                self.broadcast_notification(tool_call_notification);

                // Handle tool call with permission checking and execution
                let mut permission_storage = super::permissions::PermissionStorage::new();
                let tool_result = super::translation::handle_tool_call(
                    tool_call,
                    &acp_session,
                    Arc::clone(&self.agent_server),
                    &self.permission_engine,
                    &mut permission_storage,
                )
                .await;

                match tool_result {
                    Ok(result) => {
                        // A `ToolResult` carrying a non-empty `error` is a
                        // tool-level failure surfaced as a value (e.g. the MCP
                        // server returned -32602 "tool not found"), not a
                        // success. Count it so the runaway guard can tell a step
                        // that made no progress from one that did.
                        if result.error.is_some() {
                            failed_tool_calls += 1;
                            tracing::warn!(
                                "Tool call {} returned an error result: {}",
                                result.call_id,
                                result.error.as_deref().unwrap_or("")
                            );
                        } else {
                            tracing::info!("Tool call {} completed successfully", result.call_id);
                        }

                        // Convert tool result to ACP ToolCallUpdate and broadcast
                        let update = super::translation::tool_result_to_acp_update(result.clone());
                        let notification = agent_client_protocol::schema::SessionNotification::new(
                            request.session_id.clone(),
                            agent_client_protocol::schema::SessionUpdate::ToolCallUpdate(update),
                        );
                        self.broadcast_notification(notification);

                        // Add tool result to session
                        let tool_message = crate::types::Message {
                            role: crate::types::MessageRole::Tool,
                            content: result.result.to_string(),
                            tool_call_id: Some(result.call_id),
                            tool_name: None, // Tool name is not available in the ToolResult
                            timestamp: std::time::SystemTime::now(),
                        };
                        self.agent_server
                            .add_message(&acp_session.llama_session_id, tool_message)
                            .await
                            .map_err(|e| {
                                tracing::error!("Failed to add tool result to session: {}", e);
                                Self::convert_error(e)
                            })?;

                        // Send Plan notification if this was a kanban-related tool call
                        // The kanban tool includes _plan data in its response when tasks are modified
                        if tool_name == "mcp__sah__kanban" {
                            tracing::debug!(
                                "Kanban tool call '{}', checking for Plan data",
                                tool_name
                            );
                            if let Err(e) = self
                                .send_plan_notification_from_result(&request.session_id, &result)
                            {
                                tracing::warn!(
                                    "Failed to send Plan notification after '{}': {}",
                                    tool_name,
                                    e
                                );
                                // Don't fail the entire operation if Plan notification fails
                            }
                        }
                    }
                    Err(e) => {
                        failed_tool_calls += 1;
                        tracing::error!("Tool call execution failed: {}", e);
                        // Convert tool call error to ACP notification
                        let error_notification =
                            agent_client_protocol::schema::SessionNotification::new(
                                request.session_id.clone(),
                                agent_client_protocol::schema::SessionUpdate::AgentMessageChunk(
                                    agent_client_protocol::schema::ContentChunk::new(
                                        agent_client_protocol::schema::ContentBlock::from(format!(
                                            "Tool call failed: {}",
                                            e
                                        )),
                                    ),
                                ),
                            );
                        self.broadcast_notification(error_notification);
                        // Continue with other tool calls even if one fails
                    }
                }
            }

            // Runaway-loop guard: a turn that re-prompts forever, a single step
            // that emits an absurd number of tool calls, or a step where every
            // tool call failed (no forward progress — e.g. all dispatches return
            // -32602 "tool not found") must terminate with an error rather than
            // hang the caller until its timeout fires.
            let step = AgenticStep {
                tool_calls: tool_calls_count,
                failed_tool_calls,
            };
            if let AgenticLoopAction::Abort(reason) = AGENTIC_LOOP_LIMITS.evaluate(iteration, &step)
            {
                tracing::error!(
                    "Aborting agentic loop on iteration {}: {}",
                    iteration,
                    reason
                );
                return Err(agent_client_protocol::Error::internal_error()
                    .data(serde_json::json!({ "agentic_loop_aborted": reason })));
            }

            // Only stop on hard limits - otherwise continue to let model respond to tool results
            // Note: EOS/EndTurn with no tool calls is already handled above (tool_calls.is_empty() check)
            // If we reach here, tool calls existed, so we need to continue unless hitting a hard limit
            match final_stop_reason {
                agent_client_protocol::schema::StopReason::MaxTokens
                | agent_client_protocol::schema::StopReason::Cancelled
                | agent_client_protocol::schema::StopReason::Refusal => {
                    tracing::info!(
                        "Stopping agentic loop after executing {} tool calls (hard limit: {})",
                        tool_calls_count,
                        Pretty(&final_stop_reason)
                    );
                    break;
                }
                _ => {
                    // Continue loop to let model respond to tool results
                    tracing::info!(
                        "Continuing agentic loop after executing {} tool calls",
                        tool_calls_count
                    );
                }
            }
        }

        // Agentic loop completed
        tracing::info!(
            "Agentic loop completed: {} tokens generated, {} tool calls executed",
            total_tokens,
            total_tool_calls
        );

        let mut meta = serde_json::Map::new();
        meta.insert(
            "tokens_generated".to_string(),
            serde_json::json!(total_tokens),
        );
        if total_tool_calls > 0 {
            meta.insert(
                "tool_calls_executed".to_string(),
                serde_json::json!(total_tool_calls),
            );
        }
        // Include the response text in metadata for workflow JS expression evaluation
        // This mirrors claude-agent's behavior with "claude_response"
        meta.insert(
            "llama_response".to_string(),
            serde_json::json!(all_generated_text),
        );

        // Clear session context on MCP clients
        self.clear_mcp_session_context(&acp_session.llama_session_id)
            .await;

        // Persist the updated conversation as a durable SessionRecord so the
        // turn survives a process restart and stays answerable by
        // `session/list`. Failures are logged inside `persist_session_record`
        // and never fail the turn.
        self.persist_session_record(&acp_session).await;

        // After the first meaningful exchange, generate a human-readable
        // session title and emit the built-in `SessionInfoUpdate`. This runs
        // off the turn's critical path and is a no-op once a title exists.
        self.maybe_generate_session_title(&acp_session);

        let response =
            agent_client_protocol::schema::PromptResponse::new(final_stop_reason).meta(meta);
        self.log_response("prompt", &response);
        Ok(response)
    }

    /// Handle the ACP `session/cancel` notification.
    ///
    /// Cancels the active request for the session via the request queue and
    /// emits a final status update so the client observes the cancellation on
    /// the notification stream — not only via the in-flight `prompt` returning
    /// `StopReason::Cancelled`.
    ///
    /// The final update mirrors claude-agent's `send_final_cancellation_updates`
    /// exactly (an `AgentMessageChunk` carrying
    /// `[Session cancelled by client request]`, with `_meta` tagging the
    /// notification as a final cancellation update) so a client cancelling a
    /// turn observes the same notification stream from both agents. The
    /// internal cancellation mechanism — llama's request queue vs claude's
    /// subprocess/tool/permission fan-out — is an essential difference and is
    /// intentionally not unified.
    pub async fn cancel(
        &self,
        request: agent_client_protocol::schema::CancelNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("cancel", &request);

        let session_id = &request.session_id;
        tracing::info!("Processing cancellation for session: {}", session_id.0);

        // Get the ACP session to find the llama session ID
        let acp_session = self.get_session(session_id).await.ok_or_else(|| {
            tracing::error!("Session not found during cancellation: {}", session_id.0);
            agent_client_protocol::Error::invalid_params()
        })?;

        // Cancel the active request via the request queue
        let cancelled = self
            .agent_server
            .request_queue()
            .cancel_session(&acp_session.llama_session_id)
            .await;

        if cancelled {
            tracing::info!(
                "Successfully cancelled active request for session: {}",
                session_id.0
            );
        } else {
            tracing::info!(
                "No active request to cancel for session: {} (may have already completed)",
                session_id.0
            );
        }

        // Emit a final status update so a client cancelling a turn observes the
        // same notification stream from both agents.
        self.send_cancellation_update(session_id);

        Ok(())
    }

    /// Broadcast a final status update for a cancelled session.
    ///
    /// Mirrors claude-agent's `send_final_cancellation_updates`: an
    /// `AgentMessageChunk` carrying `[Session cancelled by client request]`,
    /// the text content tagged with `cancelled_at` / `reason` / `session_id`
    /// `_meta`, and the notification tagged with `final_update` / `cancellation`
    /// `_meta`. Keeping the shape identical means a client observes the same
    /// cancellation notification regardless of which agent it is talking to.
    fn send_cancellation_update(&self, session_id: &AcpSessionId) {
        let cancelled_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut text_meta = serde_json::Map::new();
        text_meta.insert("cancelled_at".to_string(), serde_json::json!(cancelled_at));
        text_meta.insert(
            "reason".to_string(),
            serde_json::json!("client_cancellation"),
        );
        text_meta.insert("session_id".to_string(), serde_json::json!(session_id.0));

        let text_content = agent_client_protocol::schema::TextContent::new(
            "[Session cancelled by client request]".to_string(),
        )
        .meta(text_meta);
        let content_chunk = agent_client_protocol::schema::ContentChunk::new(
            agent_client_protocol::schema::ContentBlock::Text(text_content),
        );

        let mut notif_meta = serde_json::Map::new();
        notif_meta.insert("final_update".to_string(), serde_json::json!(true));
        notif_meta.insert("cancellation".to_string(), serde_json::json!(true));

        let notification = SessionNotification::new(
            session_id.clone(),
            agent_client_protocol::schema::SessionUpdate::AgentMessageChunk(content_chunk),
        )
        .meta(notif_meta);

        self.broadcast_notification(notification);

        tracing::debug!(
            "Sent final cancellation update for session: {}",
            session_id.0
        );
    }

    pub async fn ext_method(
        &self,
        request: agent_client_protocol::schema::ExtRequest,
    ) -> Result<ExtResponse, agent_client_protocol::Error> {
        self.log_request("ext_method", &request);
        tracing::info!("Extension method called: {}", request.method);

        // Parse the request parameters from RawValue
        let params_value: serde_json::Value =
            serde_json::from_str(request.params.get()).map_err(|e| {
                tracing::error!("Failed to parse extension method parameters: {}", e);
                super::acp_error::invalid_params(format!(
                    "Extension method parameters are not valid JSON: {e}"
                ))
            })?;

        // Route extension methods to appropriate handlers
        let result: serde_json::Value = match request.method.as_ref() {
            // Filesystem operations
            "fs/read_text_file" => {
                // Validate client capabilities for filesystem read operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.fs.read_text_file => {
                            tracing::debug!("fs.read_text_file capability validated");
                        }
                        Some(_) => {
                            tracing::error!("fs/read_text_file capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "File system read capability not declared by client. Set client_capabilities.fs.read_text_file = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for fs/read_text_file validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized. Cannot perform file system operations without capability declaration.",
                            ));
                        }
                    }
                }

                // Parse request
                let fs_req: agent_client_protocol::schema::ReadTextFileRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse fs/read_text_file params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "fs/read_text_file parameters do not match the expected schema: {e}"
                        ))
                    })?;

                // Get session ID from the request
                let session_id = &fs_req.session_id;
                let session = self.get_session(session_id).await.ok_or_else(|| {
                    tracing::error!("Session not found for fs/read_text_file: {}", session_id.0);
                    super::acp_error::invalid_params(format!(
                        "Session not found for fs/read_text_file: {}",
                        session_id.0
                    ))
                })?;

                // Execute operation
                let response = self
                    .filesystem_ops
                    .read_text_file(&session, fs_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("fs/read_text_file failed: {}", e);
                        filesystem_error_to_protocol_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize fs/read_text_file response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize fs/read_text_file response: {e}"
                    ))
                })?
            }

            "fs/write_text_file" => {
                // Validate client capabilities for filesystem write operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.fs.write_text_file => {
                            tracing::debug!("fs.write_text_file capability validated");
                        }
                        Some(_) => {
                            tracing::error!("fs/write_text_file capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "File system write capability not declared by client. Set client_capabilities.fs.write_text_file = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for fs/write_text_file validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized. Cannot perform file system operations without capability declaration.",
                            ));
                        }
                    }
                }

                // Parse request
                let fs_req: agent_client_protocol::schema::WriteTextFileRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse fs/write_text_file params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "fs/write_text_file parameters do not match the expected schema: {e}"
                        ))
                    })?;

                // Get session ID from the request
                let session_id = &fs_req.session_id;
                let session = self.get_session(session_id).await.ok_or_else(|| {
                    tracing::error!("Session not found for fs/write_text_file: {}", session_id.0);
                    super::acp_error::invalid_params(format!(
                        "Session not found for fs/write_text_file: {}",
                        session_id.0
                    ))
                })?;

                // Execute operation
                let response = self
                    .filesystem_ops
                    .write_text_file(&session, fs_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("fs/write_text_file failed: {}", e);
                        filesystem_error_to_protocol_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize fs/write_text_file response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize fs/write_text_file response: {e}"
                    ))
                })?
            }

            // Terminal operations
            "terminal/create" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!("terminal/create capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/create requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/create validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/create without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::CreateTerminalRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/create params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "terminal/create parameters do not match the expected schema: {e}"
                        ))
                    })?;

                let response = self
                    .terminal_manager
                    .write()
                    .await
                    .create_terminal(term_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("terminal/create failed: {}", e);
                        Self::convert_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize terminal/create response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize terminal/create response: {e}"
                    ))
                })?
            }

            "terminal/output" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!("terminal/output capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/output requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/output validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/output without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::TerminalOutputRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/output params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "terminal/output parameters do not match the expected schema: {e}"
                        ))
                    })?;

                let response = self
                    .terminal_manager
                    .write()
                    .await
                    .get_output(term_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("terminal/output failed: {}", e);
                        Self::convert_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize terminal/output response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize terminal/output response: {e}"
                    ))
                })?
            }

            "terminal/wait_for_exit" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!(
                                "terminal/wait_for_exit capability not declared by client"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/wait_for_exit requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/wait_for_exit validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/wait_for_exit without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::WaitForExitRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/wait_for_exit params: {}", e);
                        super::acp_error::invalid_params(format!(
                        "terminal/wait_for_exit parameters do not match the expected schema: {e}"
                    ))
                    })?;

                let response = self
                    .terminal_manager
                    .write()
                    .await
                    .wait_for_exit(term_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("terminal/wait_for_exit failed: {}", e);
                        Self::convert_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize terminal/wait_for_exit response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize terminal/wait_for_exit response: {e}"
                    ))
                })?
            }

            "terminal/get" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!("terminal/get capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/get requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/get validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/get without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::GetTerminalRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/get params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "terminal/get parameters do not match the expected schema: {e}"
                        ))
                    })?;

                let response = self
                    .terminal_manager
                    .read()
                    .await
                    .get_terminal(term_req)
                    .map_err(|e| {
                        tracing::error!("terminal/get failed: {}", e);
                        Self::convert_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize terminal/get response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize terminal/get response: {e}"
                    ))
                })?
            }

            "terminal/kill" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!("terminal/kill capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/kill requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/kill validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/kill without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::KillTerminalRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/kill params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "terminal/kill parameters do not match the expected schema: {e}"
                        ))
                    })?;

                let response = self
                    .terminal_manager
                    .write()
                    .await
                    .kill_terminal(term_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("terminal/kill failed: {}", e);
                        Self::convert_error(e)
                    })?;

                serde_json::to_value(response).map_err(|e| {
                    tracing::error!("Failed to serialize terminal/kill response: {}", e);
                    super::acp_error::internal_error(format!(
                        "Failed to serialize terminal/kill response: {e}"
                    ))
                })?
            }

            "terminal/release" => {
                // Validate client capabilities for terminal operations
                {
                    let client_caps = self.client_capabilities.read().await;
                    match &*client_caps {
                        Some(caps) if caps.terminal => {
                            tracing::debug!("Terminal capability validated");
                        }
                        Some(_) => {
                            tracing::error!("terminal/release capability not declared by client");
                            return Err(super::acp_error::invalid_params(
                                "Terminal capability not declared by client; terminal/release requires client_capabilities.terminal = true during initialization.",
                            ));
                        }
                        None => {
                            tracing::error!(
                                "No client capabilities available for terminal/release validation"
                            );
                            return Err(super::acp_error::invalid_params(
                                "Client capabilities not initialized; cannot perform terminal/release without capability declaration.",
                            ));
                        }
                    }
                }

                let term_req: super::terminal::ReleaseTerminalRequest =
                    serde_json::from_value(params_value).map_err(|e| {
                        tracing::error!("Failed to parse terminal/release params: {}", e);
                        super::acp_error::invalid_params(format!(
                            "terminal/release parameters do not match the expected schema: {e}"
                        ))
                    })?;

                self.terminal_manager
                    .write()
                    .await
                    .release_terminal(term_req)
                    .await
                    .map_err(|e| {
                        tracing::error!("terminal/release failed: {}", e);
                        Self::convert_error(e)
                    })?;

                // Return null for successful release
                serde_json::Value::Null
            }

            // Unknown method. An extension method the agent does not
            // implement is genuinely "not found", so it is rejected with
            // `method_not_found` (`-32601`) — matching claude-agent, so a
            // client probing an unsupported extension observes the same
            // failure from either agent.
            _ => {
                tracing::warn!("Unknown extension method: {}", request.method);
                return Err(super::acp_error::method_not_found(format!(
                    "Extension method not found: {}",
                    request.method
                )));
            }
        };

        // Convert response to ExtResponse (RawValue)
        let response_json_str = serde_json::to_string(&result).map_err(|e| {
            tracing::error!("Failed to serialize extension method response: {}", e);
            super::acp_error::internal_error(format!(
                "Failed to serialize extension method response: {e}"
            ))
        })?;

        let raw_value = agent_client_protocol::schema::RawValue::from_string(response_json_str)
            .map_err(|e| {
                tracing::error!("Failed to create RawValue from response: {}", e);
                super::acp_error::internal_error(format!(
                    "Failed to build raw JSON for extension method response: {e}"
                ))
            })?;

        let response = ExtResponse::new(Arc::from(raw_value));
        self.log_response("ext_method", &response);
        Ok(response)
    }

    pub async fn ext_notification(
        &self,
        notification: agent_client_protocol::schema::ExtNotification,
    ) -> Result<(), agent_client_protocol::Error> {
        self.log_request("ext_notification", &notification);
        // Extension notifications are ignored for now
        Ok(())
    }
}

/// Convert FilesystemError to appropriate protocol error
///
/// Maps specific filesystem errors to meaningful JSON-RPC error codes:
/// - NotFound (file not found) -> invalid_params with details
/// - PermissionDenied -> invalid_params with details
/// - PathTraversal/security violations -> invalid_params with details
/// - Other IO errors -> internal_error with details
fn filesystem_error_to_protocol_error(
    error: super::filesystem::FilesystemError,
) -> agent_client_protocol::Error {
    use super::filesystem::FilesystemError;

    match error {
        // Security violations are invalid params
        FilesystemError::RelativePath(path) => agent_client_protocol::Error::invalid_params()
            .data(format!("Path must be absolute: {}", path)),
        FilesystemError::PathTraversal(path) => agent_client_protocol::Error::invalid_params()
            .data(format!("Path traversal detected: {}", path)),
        FilesystemError::NotAllowed(path) => agent_client_protocol::Error::invalid_params()
            .data(format!("Path not allowed: {}", path)),
        FilesystemError::Blocked(path) => agent_client_protocol::Error::invalid_params()
            .data(format!("Path is blocked: {}", path)),
        FilesystemError::FileTooLarge(size, max) => agent_client_protocol::Error::invalid_params()
            .data(format!(
                "File too large: {} bytes (max: {} bytes)",
                size, max
            )),

        // IO errors need more granular handling
        FilesystemError::Io(io_error) => match io_error.kind() {
            std::io::ErrorKind::NotFound => agent_client_protocol::Error::invalid_params()
                .data(format!("File not found: {}", io_error)),
            std::io::ErrorKind::PermissionDenied => agent_client_protocol::Error::invalid_params()
                .data(format!("Permission denied: {}", io_error)),
            std::io::ErrorKind::AlreadyExists => agent_client_protocol::Error::invalid_params()
                .data(format!("File already exists: {}", io_error)),
            std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
                agent_client_protocol::Error::invalid_params()
                    .data(format!("Invalid input: {}", io_error))
            }
            _ => agent_client_protocol::Error::internal_error()
                .data(format!("IO error: {}", io_error)),
        },
    }
}

impl AcpServer {
    /// Demultiplex an incoming `ClientRequest` enum variant onto the inherent
    /// method that handles it, then deliver the typed response back through
    /// the SDK-supplied `Responder`.
    ///
    /// The SDK gives us `Responder<serde_json::Value>` because `ClientRequest`
    /// is registered with `Response = serde_json::Value`. We cast the responder
    /// to the variant's typed response (`InitializeResponse`, `PromptResponse`,
    /// etc.) so each delegate just hands its `Result<T, Error>` to
    /// `respond_with_result` and the SDK handles serialization.
    async fn dispatch_client_request(
        server: &Arc<Self>,
        request: agent_client_protocol::ClientRequest,
        responder: agent_client_protocol::Responder<serde_json::Value>,
    ) -> Result<(), agent_client_protocol::Error> {
        use agent_client_protocol::ClientRequest as Req;

        match request {
            Req::InitializeRequest(req) => {
                let result = server.initialize(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::AuthenticateRequest(req) => {
                let result = server.authenticate(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::NewSessionRequest(req) => {
                let result = server.new_session(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::LoadSessionRequest(req) => {
                let result = server.load_session(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::ResumeSessionRequest(req) => {
                let result = server.resume_session(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::ListSessionsRequest(req) => {
                let result = server.list_sessions(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::SetSessionModeRequest(req) => {
                let result = server.set_session_mode(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::PromptRequest(req) => {
                let result = server.prompt(req).await;
                responder.cast().respond_with_result(result)
            }
            Req::ExtMethodRequest(req) => {
                // ExtResponse wraps an opaque `Arc<RawValue>`; the SDK expects
                // a `serde_json::Value` for `ClientRequest::Response`, so we
                // parse the raw JSON back and forward it. Parse failures are
                // surfaced as internal errors rather than being silently
                // dropped.
                let result = server.ext_method(req).await.and_then(|ext_response| {
                    serde_json::from_str::<serde_json::Value>(ext_response.0.get()).map_err(|e| {
                        tracing::error!("Failed to parse ExtResponse JSON: {}", e);
                        agent_client_protocol::Error::internal_error()
                    })
                });
                responder.respond_with_result(result)
            }
            // ClientRequest is `#[non_exhaustive]` and may grow new variants
            // (e.g. unstable list/fork/resume/close). Surface any we don't
            // model as method-not-found rather than silently ignoring them.
            other => {
                tracing::warn!(
                    "Received unsupported ClientRequest variant: {}",
                    other.method()
                );
                responder
                    .cast::<serde_json::Value>()
                    .respond_with_error(agent_client_protocol::Error::method_not_found())
            }
        }
    }

    /// Demultiplex an incoming `ClientNotification` enum variant onto the
    /// inherent notification handler. Notifications are fire-and-forget; any
    /// per-variant error is logged inside the delegate but never returned to
    /// the SDK (which would tear down the connection).
    async fn dispatch_client_notification(
        server: &Arc<Self>,
        notification: agent_client_protocol::ClientNotification,
    ) {
        use agent_client_protocol::ClientNotification as Notif;

        match notification {
            Notif::CancelNotification(n) => {
                if let Err(e) = server.cancel(n).await {
                    tracing::error!("cancel notification handler failed: {}", e);
                }
            }
            Notif::ExtNotification(n) => {
                if let Err(e) = server.ext_notification(n).await {
                    tracing::error!("ext notification handler failed: {}", e);
                }
            }
            other => {
                tracing::debug!(
                    "Ignoring unsupported ClientNotification variant: {}",
                    other.method()
                );
            }
        }
    }

    /// Static agent identifier used in fixtures and logs.
    ///
    /// In ACP 0.10 this implemented `agent_client_protocol_extras::AgentWithFixture`.
    /// That trait was removed when extras was reshaped for ACP 0.11; the inherent
    /// method preserves the value so existing fixture / playback paths can still
    /// query the agent type.
    pub fn agent_type(&self) -> &'static str {
        "llama"
    }
}

/// Wrap a tokio `AsyncRead`/`AsyncWrite` pair into the SDK's `Lines` transport.
///
/// The SDK 0.11 `Builder::connect_with` accepts any `ConnectTo<Counterpart>`. The
/// most convenient transport for newline-delimited JSON-RPC over byte streams is
/// `agent_client_protocol::Lines`, which takes a `futures::Stream<Item =
/// io::Result<String>>` for incoming lines and a `futures::Sink<String, Error =
/// io::Error>` for outgoing lines.
///
/// Both adapters are built with `futures::stream::unfold` / `futures::sink::unfold`
/// so we don't need the `tokio_util::compat` glue or extra crate features.
///
/// # Connection liveness
/// The incoming stream cancels `connection_closed` when the reader returns
/// EOF or an I/O error. The notification bridge in `start_with_streams`
/// races on this token so it can stop forwarding broadcasts as soon as the
/// transport is gone, instead of blocking forever on `broadcast::Receiver::recv`.
///
/// # Errors
/// The returned transport surfaces underlying I/O errors through the stream/sink
/// `io::Error` channel, which the SDK's dispatch loop maps onto the connection's
/// shutdown path.
fn build_lines_transport<R, W>(
    reader: R,
    writer: W,
    connection_closed: tokio_util::sync::CancellationToken,
) -> agent_client_protocol::Lines<
    impl futures::Sink<String, Error = std::io::Error> + Send + 'static,
    impl futures::Stream<Item = std::io::Result<String>> + Send + 'static,
>
where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    // Incoming: yield each line as `io::Result<String>`. Empty lines are passed
    // through; the SDK's parser ignores blank input.
    //
    // When the reader returns `Ok(None)` (EOF) or an `Err`, signal
    // `connection_closed` so the notification bridge in `start_with_streams`
    // wakes up and exits. The SDK's incoming protocol actor terminates cleanly
    // either way; the cancellation token is what releases the bridge from its
    // broadcast `recv()` await.
    let incoming = futures::stream::unfold(
        (BufReader::new(reader).lines(), connection_closed),
        |(mut lines, connection_closed)| async move {
            match lines.next_line().await {
                Ok(Some(line)) => Some((Ok(line), (lines, connection_closed))),
                Ok(None) => {
                    connection_closed.cancel();
                    None
                }
                Err(e) => {
                    connection_closed.cancel();
                    Some((Err(e), (lines, connection_closed)))
                }
            }
        },
    );

    // Outgoing: append `\n` to each line and write it to the underlying writer,
    // flushing after every message so clients see responses immediately.
    let outgoing = futures::sink::unfold(writer, |mut writer, line: String| async move {
        let mut bytes = line.into_bytes();
        bytes.push(b'\n');
        writer.write_all(&bytes).await?;
        writer.flush().await?;
        Ok::<_, std::io::Error>(writer)
    });

    agent_client_protocol::Lines::new(outgoing, incoming)
}

/// Extract the optional caller-supplied `max_tokens` cap from a
/// `PromptRequest`'s `_meta` map.
///
/// The ACP `_meta` field is the protocol's documented extensibility channel.
/// The validator runner (in `avp-common`) attaches a `"max_tokens"` key here
/// as a defense-in-depth cap against runaway generation. The ACP spec lets
/// agents ignore unknown `_meta` keys, but we choose to honor this one so a
/// misbehaving rule can't lock the entire hook.
///
/// Returns `Some(n)` when the meta map contains a positive integer under the
/// `"max_tokens"` key. Returns `None` for all other cases (key missing, value
/// not an integer, value zero, value larger than `usize::MAX`, or meta itself
/// is `None`). Callers treat `None` as "no caller-supplied cap" and fall back
/// to the server's own per-turn cap (`MAX_GENERATION_TOKENS`).
///
/// # Why a free function
///
/// Pulled out of the prompt loop so the parsing logic is unit-testable without
/// standing up a real `AcpServer` (which loads a model). The behavior is pure
/// JSON inspection — no I/O, no async — so a free function fits cleanly.
fn extract_request_max_tokens(
    meta: Option<&serde_json::Map<String, serde_json::Value>>,
) -> Option<usize> {
    let value = meta?.get("max_tokens")?;
    let raw = value.as_u64()?;
    if raw == 0 {
        return None;
    }
    usize::try_from(raw).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::time::Duration;

    /// A real in-process Agent-tools mount for tests, backed by the
    /// `EchoService` rmcp server. Satisfies the required `AcpServer::new` input
    /// without pulling in `swissarmyhammer-tools`.
    fn test_agent_tools_mount() -> Arc<dyn crate::mcp::AgentToolsMount> {
        Arc::new(crate::mcp::InProcessMount::new(
            crate::echo::EchoService::new(),
        ))
    }

    /// An [`AgentToolsMount`] whose `connect()` always fails, used to prove that
    /// a mount failure fails session creation rather than yielding a tool-less
    /// agent.
    struct FailingAgentToolsMount;

    #[async_trait::async_trait]
    impl crate::mcp::AgentToolsMount for FailingAgentToolsMount {
        async fn connect(
            &self,
        ) -> Result<Arc<dyn crate::mcp::MCPClient>, crate::types::errors::MCPError> {
            Err(crate::types::errors::MCPError::Connection(
                "simulated agent-tools mount failure".to_string(),
            ))
        }
    }

    async fn create_test_server() -> AcpServer {
        create_test_server_with_mount(test_agent_tools_mount()).await
    }

    async fn create_test_server_with_mount(
        agent_tools_mount: Arc<dyn crate::mcp::AgentToolsMount>,
    ) -> AcpServer {
        use crate::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_config = AgentConfig {
            model: ModelConfig {
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
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
            tool_execution_config: Default::default(),
        };

        // For testing, we'll create a minimal AgentServer without actually loading a model
        // This is acceptable for ACP protocol tests that don't need actual generation
        let model_manager =
            Arc::new(crate::model::ModelManager::new(test_config.model.clone()).unwrap());
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            test_config.queue_config.clone(),
            test_config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            test_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
            test_config.parallel_execution_config.clone(),
        ));

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            test_config,
        ));

        let config = AcpConfig::default();
        let (server, _notification_rx) = AcpServer::new(agent_server, config, agent_tools_mount);
        server
    }

    mod agentic_loop_guard {
        use super::super::{AgenticLoopAction, AgenticStep, AGENTIC_LOOP_LIMITS};

        #[test]
        fn a_step_whose_every_tool_call_failed_aborts_the_loop() {
            // The exact production pathology: a step emitted many tool calls and
            // every one failed (e.g. -32602 "tool not found"). The loop made no
            // progress, so re-prompting would only repeat the failure — abort.
            let step = AgenticStep {
                tool_calls: 342,
                failed_tool_calls: 342,
            };
            assert!(matches!(
                AGENTIC_LOOP_LIMITS.evaluate(1, &step),
                AgenticLoopAction::Abort(_)
            ));
        }

        #[test]
        fn a_step_with_some_successful_tool_calls_continues() {
            // Partial failure is not runaway: at least one tool call succeeded, so
            // the model has new information to act on. Continue the loop.
            let step = AgenticStep {
                tool_calls: 3,
                failed_tool_calls: 2,
            };
            assert!(matches!(
                AGENTIC_LOOP_LIMITS.evaluate(1, &step),
                AgenticLoopAction::Continue
            ));
        }

        #[test]
        fn a_single_step_exceeding_the_per_step_tool_cap_aborts() {
            // A single generation step that emits an absurd number of tool calls
            // is degenerate output even if some "succeed" — cap it per step so one
            // runaway step cannot blow the turn budget on its own.
            let over_cap = AGENTIC_LOOP_LIMITS.max_tool_calls_per_step + 1;
            let step = AgenticStep {
                tool_calls: over_cap,
                // Even with zero failures, the sheer count is the problem.
                failed_tool_calls: 0,
            };
            assert!(matches!(
                AGENTIC_LOOP_LIMITS.evaluate(1, &step),
                AgenticLoopAction::Abort(_)
            ));
        }

        #[test]
        fn exceeding_the_iteration_cap_aborts() {
            // Even with healthy per-step results, the loop must not iterate
            // forever; the iteration cap is the final backstop.
            let step = AgenticStep {
                tool_calls: 1,
                failed_tool_calls: 0,
            };
            let over_cap = AGENTIC_LOOP_LIMITS.max_iterations + 1;
            assert!(matches!(
                AGENTIC_LOOP_LIMITS.evaluate(over_cap, &step),
                AgenticLoopAction::Abort(_)
            ));
        }

        #[test]
        fn a_normal_step_within_all_limits_continues() {
            let step = AgenticStep {
                tool_calls: 1,
                failed_tool_calls: 0,
            };
            assert!(matches!(
                AGENTIC_LOOP_LIMITS.evaluate(1, &step),
                AgenticLoopAction::Continue
            ));
        }
    }

    /// Regression test for the llama client-wiring fix.
    ///
    /// In production llama-agent runs through
    /// `swissarmyhammer_agent::wrap_llama_into_handle`, whose `with_spawned`
    /// closure obtains the agent's `ConnectionTo<Client>` and calls
    /// [`AcpServer::publish_client_connection`] (and
    /// [`AcpServer::clear_client_connection`] when the connection ends). That
    /// wrapper bypasses [`AcpServer::start_with_streams`], so these methods are
    /// the only place the elicitation endpoint gets installed. Before the fix
    /// the wrapper never called them, leaving `elicitation_endpoint` `None` so
    /// MCP `elicitation/create` requests declined with "No client connection
    /// available".
    ///
    /// This test drives a real `ConnectionTo<Client>` over an in-process
    /// `Channel::duplex()` and asserts the publish/clear pair toggles the
    /// endpoint that the per-session MCP handler reads. A full end-to-end run
    /// through the wrapper is covered for the Claude path in
    /// `swissarmyhammer-agent`; the llama wrapper cannot be exercised there
    /// without depending on these internal `ModelManager`/`RequestQueue`
    /// constructors, so the wiring methods are verified here where they live.
    #[tokio::test]
    #[serial]
    async fn publish_client_connection_installs_and_clears_elicitation_endpoint() {
        use agent_client_protocol::{Agent, Channel, Client, ConnectionTo};

        let server = Arc::new(create_test_server().await);

        // Stand up a live agent→client connection: a no-op fake client on one
        // end, and on the other a `ConnectionTo<Client>` we hand to the server.
        let (client_side, agent_side) = Channel::duplex();

        let client_task = tokio::spawn(async move {
            let _ = Client
                .builder()
                .name("llama-wiring-fake-client")
                .connect_to(client_side)
                .await;
        });

        let server_for_conn = Arc::clone(&server);
        Agent
            .builder()
            .name("llama-wiring-test-agent")
            .connect_with(agent_side, async move |cx: ConnectionTo<Client>| {
                assert!(
                    !server_for_conn.is_elicitation_endpoint_set().await,
                    "elicitation endpoint must start unset"
                );

                server_for_conn.publish_client_connection(cx.clone()).await;
                assert!(
                    server_for_conn.is_elicitation_endpoint_set().await,
                    "publish_client_connection must install the elicitation endpoint"
                );

                server_for_conn.clear_client_connection().await;
                assert!(
                    !server_for_conn.is_elicitation_endpoint_set().await,
                    "clear_client_connection must tear the elicitation endpoint down"
                );

                Ok::<(), agent_client_protocol::Error>(())
            })
            .await
            .expect("agent connect_with should succeed");

        client_task.abort();
        let _ = client_task.await;
    }

    /// Build a test server backed by the given [`AcpConfig`].
    ///
    /// Mirrors [`create_test_server`] but lets a test override the advertised
    /// capabilities — used to verify that capability gating (e.g. `loadSession`)
    /// actually enforces what the config advertises.
    async fn create_test_server_with_config(config: AcpConfig) -> AcpServer {
        use crate::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_config = AgentConfig {
            model: ModelConfig {
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
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
            tool_execution_config: Default::default(),
        };

        let model_manager =
            Arc::new(crate::model::ModelManager::new(test_config.model.clone()).unwrap());
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            test_config.queue_config.clone(),
            test_config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            test_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
            test_config.parallel_execution_config.clone(),
        ));

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            test_config,
        ));

        let (server, _notification_rx) =
            AcpServer::new(agent_server, config, test_agent_tools_mount());
        server
    }

    /// Build a test server with an explicit session-cache eviction policy.
    ///
    /// Mirrors [`create_test_server`] but lets a test pin a short
    /// `max_session_age` so [`AcpServer::cleanup_expired_sessions`] evicts a
    /// just-created session without waiting out the production 1-hour TTL.
    async fn create_test_server_with_cleanup(
        cleanup_interval: std::time::Duration,
        max_session_age: std::time::Duration,
    ) -> AcpServer {
        use crate::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_config = AgentConfig {
            model: ModelConfig {
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
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
            tool_execution_config: Default::default(),
        };

        let model_manager =
            Arc::new(crate::model::ModelManager::new(test_config.model.clone()).unwrap());
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            test_config.queue_config.clone(),
            test_config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            test_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
            test_config.parallel_execution_config.clone(),
        ));

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            test_config,
        ));

        let (server, _notification_rx) = AcpServer::with_cleanup_settings(
            agent_server,
            AcpConfig::default(),
            test_agent_tools_mount(),
            cleanup_interval,
            max_session_age,
        );
        server
    }

    /// `session/load` must be rejected when the agent does not advertise the
    /// `loadSession` capability.
    ///
    /// This is the advertise-vs-enforce contract: the agent only advertises
    /// `loadSession` when `config.capabilities.supports_session_loading` is
    /// true, so when it is false the agent must refuse `session/load` rather
    /// than acting on it. The rejection mirrors claude-agent's
    /// `LoadSessionNotSupported` mapping (`-32601`,
    /// `requiredCapability: loadSession`).
    #[tokio::test]
    #[serial]
    async fn test_load_session_rejected_when_capability_disabled() {
        let _state = StateDirGuard::new();

        let config = AcpConfig {
            capabilities: crate::acp::config::AcpCapabilities {
                supports_session_loading: false,
                ..Default::default()
            },
            ..Default::default()
        };
        let server = Arc::new(create_test_server_with_config(config).await);

        let req = agent_client_protocol::schema::LoadSessionRequest::new(
            agent_client_protocol::schema::SessionId::new("any-session-id"),
            std::env::temp_dir(),
        );

        let result = server.load_session(req).await;
        let error = result.expect_err(
            "load_session must be rejected when loadSession capability is not advertised",
        );
        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::from(-32601),
            "loadSession rejection must use the method-not-found code"
        );
        let data = error.data.expect("rejection must carry structured data");
        assert_eq!(data["requiredCapability"], "loadSession");
        assert_eq!(data["method"], "session/load");
    }

    /// `session/load` is honored (reaches the record lookup) when the agent
    /// advertises the `loadSession` capability.
    ///
    /// With the capability advertised the gate is a no-op, so the request
    /// proceeds to the durable record lookup. No record exists for the bogus
    /// id, so the call still fails — but it must NOT fail with the
    /// capability-rejection error; it must reach the lookup and fail there.
    #[tokio::test]
    #[serial]
    async fn test_load_session_passes_capability_gate_when_advertised() {
        let _state = StateDirGuard::new();

        // Default config advertises loadSession.
        let server = Arc::new(create_test_server().await);

        let req = agent_client_protocol::schema::LoadSessionRequest::new(
            agent_client_protocol::schema::SessionId::new("missing-session-id"),
            std::env::temp_dir(),
        );

        let result = server.load_session(req).await;
        let error = result.expect_err("no record exists for a bogus session id");
        assert_ne!(
            error.code,
            agent_client_protocol::ErrorCode::from(-32601),
            "with loadSession advertised, the request must pass the capability gate"
        );
    }

    /// `session/prompt` must reject content types the agent advertises as
    /// unsupported.
    ///
    /// llama-agent advertises `image: false` / `audio: false` /
    /// `embeddedContext: false`, so an image content block in a prompt must be
    /// rejected with a structured `-32602` capability error — mirroring
    /// claude-agent's `ContentCapabilityValidator`.
    #[tokio::test]
    #[serial]
    async fn test_prompt_rejects_unsupported_content_type() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        // An image block — the agent does not advertise image support.
        const PNG: &str = "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mP8/5+hHgAHggJ/PchI7wAAAABJRU5ErkJggg==";
        let prompt = vec![agent_client_protocol::schema::ContentBlock::Image(
            agent_client_protocol::schema::ImageContent::new(PNG, "image/png"),
        )];
        let req = agent_client_protocol::schema::PromptRequest::new(
            agent_client_protocol::schema::SessionId::new("any-session-id"),
            prompt,
        );

        let result = server.prompt(req).await;
        let error =
            result.expect_err("prompt with image content must be rejected — image not advertised");
        assert_eq!(
            error.code,
            agent_client_protocol::ErrorCode::from(-32602),
            "unsupported content must be rejected with the invalid-params code"
        );
        let data = error.data.expect("rejection must carry structured data");
        assert_eq!(data["contentType"], "image");
        assert_eq!(data["required"], "promptCapabilities.image");
    }

    /// RAII guard that points `XDG_STATE_HOME` at a fresh temp directory for
    /// the lifetime of the guard, restoring the previous value on drop.
    ///
    /// `AcpServer::new_session` and `AcpServer::prompt` persist a
    /// `SessionRecord` to the shared `SessionStore`, which resolves its
    /// directory under `$XDG_STATE_HOME`. Tests that exercise those paths must
    /// isolate the state directory so they neither pollute the developer's
    /// real state tree nor observe records left by other tests. Hold the guard
    /// for the whole test body; it must be paired with `#[serial]` because the
    /// `XDG_STATE_HOME` env var is process-global.
    struct StateDirGuard {
        _temp: tempfile::TempDir,
        previous: Option<std::ffi::OsString>,
    }

    impl StateDirGuard {
        /// Create a fresh temp directory and point `XDG_STATE_HOME` at it.
        fn new() -> Self {
            let temp = tempfile::TempDir::new().unwrap();
            let previous = std::env::var_os("XDG_STATE_HOME");
            // SAFETY: callers are `#[serial]`, so no other thread reads or
            // writes the env var concurrently; the previous value is restored
            // in `Drop`.
            std::env::set_var("XDG_STATE_HOME", temp.path());
            Self {
                _temp: temp,
                previous,
            }
        }
    }

    impl Drop for StateDirGuard {
        fn drop(&mut self) {
            // SAFETY: see `StateDirGuard::new` — callers are `#[serial]`.
            match self.previous.take() {
                Some(value) => std::env::set_var("XDG_STATE_HOME", value),
                None => std::env::remove_var("XDG_STATE_HOME"),
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_initialize() {
        let server = Arc::new(create_test_server().await);

        let request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(
            agent_client_protocol::schema::ClientCapabilities::new()
                .fs(agent_client_protocol::schema::FileSystemCapabilities::new()
                    .read_text_file(true)
                    .write_text_file(true))
                .terminal(true),
        );

        let result = server.initialize(request).await;
        assert!(result.is_ok(), "Initialize should succeed");

        let response = result.unwrap();
        assert_eq!(
            response.protocol_version,
            agent_client_protocol::schema::ProtocolVersion::V1,
            "Agent should respond with V1 protocol version"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_new_session() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        // Create a new session request
        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());

        let result = server.new_session(new_session_request).await;
        assert!(result.is_ok(), "New session should succeed");

        let response = result.unwrap();

        // Verify session ID is returned and has correct format
        assert!(
            !response.session_id.0.is_empty(),
            "Session ID should not be empty"
        );

        // Verify the session was actually stored and can be retrieved
        let session = server.get_session_by_id(&response.session_id).await;
        assert!(
            session.is_some(),
            "Session should be stored and retrievable"
        );

        let session = session.unwrap();

        // Verify the llama session ID was created (by checking it's non-zero length string)
        assert!(
            !session.llama_session_id.to_string().is_empty(),
            "Llama session ID should exist"
        );

        // Verify session has default mode
        assert!(
            matches!(session.mode, crate::acp::session::SessionMode::Custom(ref s) if s == "general-purpose"),
            "Default session mode should be general-purpose"
        );

        // Verify session has client capabilities (even if default)
        // Just verify we can access the capabilities without panicking
        let _caps = &session.client_capabilities;
    }

    /// The intrinsic Agent-tools mount is mandatory: if its `connect()` fails,
    /// `new_session` MUST fail rather than silently create a tool-less session.
    /// This guards the load-bearing invariant that every llama-agent session has
    /// its intrinsic Agent tools.
    #[tokio::test]
    #[serial]
    async fn new_session_fails_when_agent_tools_mount_connect_fails() {
        let _state = StateDirGuard::new();
        let server =
            Arc::new(create_test_server_with_mount(Arc::new(FailingAgentToolsMount)).await);

        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());

        let result = server.new_session(new_session_request).await;
        assert!(
            result.is_err(),
            "new_session must fail when the mandatory agent-tools mount cannot connect, \
             not yield a tool-less session"
        );

        // No session should have been stored from the failed creation.
        let response = server
            .list_sessions(agent_client_protocol::schema::ListSessionsRequest::new())
            .await
            .expect("list_sessions should succeed");
        assert!(
            response.sessions.is_empty(),
            "a failed mount connect must not leave a session behind; got {:?}",
            response.sessions
        );
    }

    /// `session/list` returns nothing when no sessions have been created.
    #[tokio::test]
    #[serial]
    async fn test_list_sessions_empty() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        let response = server
            .list_sessions(agent_client_protocol::schema::ListSessionsRequest::new())
            .await
            .expect("list_sessions should succeed with no sessions");

        assert!(
            response.sessions.is_empty(),
            "no sessions should be listed before any are created"
        );
        assert!(
            response.next_cursor.is_none(),
            "an empty listing must not carry a cursor"
        );
    }

    /// Creating sessions via `session/new` persists a `SessionRecord` that
    /// `session/list` then enumerates — the durable, cross-restart path.
    #[tokio::test]
    #[serial]
    async fn test_list_sessions_after_new_session() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        let cwd = std::env::current_dir().unwrap();
        let first = server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                cwd.clone(),
            ))
            .await
            .expect("first new_session should succeed");
        let second = server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                cwd.clone(),
            ))
            .await
            .expect("second new_session should succeed");

        let response = server
            .list_sessions(agent_client_protocol::schema::ListSessionsRequest::new())
            .await
            .expect("list_sessions should succeed");

        let listed_ids: Vec<String> = response
            .sessions
            .iter()
            .map(|info| info.session_id.0.to_string())
            .collect();
        assert_eq!(
            listed_ids.len(),
            2,
            "both created sessions should be listed"
        );
        assert!(listed_ids.contains(&first.session_id.0.to_string()));
        assert!(listed_ids.contains(&second.session_id.0.to_string()));

        // `SessionStore` orders sessions by descending lexical ULID order.
        // Two sessions created within the same millisecond share the ULID
        // timestamp prefix and carry independent random suffixes, so creation
        // order is *not* a reliable predictor of list order. Assert against
        // the store's actual contract — descending lexical sort of the ids —
        // rather than assuming the second-created session sorts first.
        let first_id = first.session_id.0.to_string();
        let second_id = second.session_id.0.to_string();
        let mut expected_order = [first_id.clone(), second_id.clone()];
        expected_order.sort_unstable_by(|a, b| b.cmp(a));
        assert_eq!(
            listed_ids, expected_order,
            "sessions should be listed in descending lexical ULID order"
        );

        // The persisted record carries the session's working directory.
        for info in &response.sessions {
            assert_eq!(info.cwd, cwd, "listed session should carry its cwd");
        }
    }

    /// The `cwd` filter on `session/list` only returns sessions whose working
    /// directory matches exactly.
    #[tokio::test]
    #[serial]
    async fn test_list_sessions_cwd_filter() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        let cwd = std::env::current_dir().unwrap();
        server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                cwd.clone(),
            ))
            .await
            .expect("new_session should succeed");

        // A filter on the real cwd matches the created session.
        let matching = server
            .list_sessions(
                agent_client_protocol::schema::ListSessionsRequest::new().cwd(cwd.clone()),
            )
            .await
            .expect("filtered list_sessions should succeed");
        assert_eq!(
            matching.sessions.len(),
            1,
            "the session should match its own cwd filter"
        );

        // A filter on an unrelated cwd matches nothing.
        let non_matching = server
            .list_sessions(
                agent_client_protocol::schema::ListSessionsRequest::new()
                    .cwd(std::path::PathBuf::from("/nonexistent/path/for/test")),
            )
            .await
            .expect("filtered list_sessions should succeed");
        assert!(
            non_matching.sessions.is_empty(),
            "no session should match an unrelated cwd filter"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_capability_advertisement() {
        let server = Arc::new(create_test_server().await);

        let request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(agent_client_protocol::schema::ClientCapabilities::new());

        let result = server.initialize(request).await;
        assert!(result.is_ok(), "Initialize should succeed");

        let response = result.unwrap();

        // Serialize response to JSON to inspect the structure
        let response_json = serde_json::to_value(&response).expect("Should serialize response");

        // Verify agent capabilities are advertised
        let agent_caps = response_json
            .get("agentCapabilities")
            .expect("Agent capabilities should be advertised");

        // Verify session loading capability
        assert_eq!(
            agent_caps.get("loadSession"),
            Some(&serde_json::Value::Bool(true)),
            "Should advertise load_session capability from default config"
        );

        // Verify prompt capabilities
        let prompt_caps = agent_caps
            .get("promptCapabilities")
            .expect("Prompt capabilities should be advertised");
        // llama-agent currently only supports text content
        assert_eq!(
            prompt_caps.get("audio"),
            Some(&serde_json::Value::Bool(false)),
            "Should not advertise audio support (not yet implemented)"
        );
        assert_eq!(
            prompt_caps.get("embeddedContext"),
            Some(&serde_json::Value::Bool(false)),
            "Should not advertise embedded context support (not yet implemented)"
        );
        assert_eq!(
            prompt_caps.get("image"),
            Some(&serde_json::Value::Bool(false)),
            "Should not advertise image support (not yet implemented)"
        );
        // meta field is optional in prompt capabilities, only check if present
        if let Some(prompt_meta) = prompt_caps.get("meta") {
            assert_eq!(
                prompt_meta.get("streaming"),
                Some(&serde_json::Value::Bool(true)),
                "Should advertise streaming in prompt meta"
            );
        }

        // Verify MCP capabilities
        let mcp_caps = agent_caps
            .get("mcpCapabilities")
            .expect("MCP capabilities should be advertised");
        assert_eq!(
            mcp_caps.get("http"),
            Some(&serde_json::Value::Bool(true)),
            "Should advertise HTTP MCP support"
        );
        assert_eq!(
            mcp_caps.get("sse"),
            Some(&serde_json::Value::Bool(false)),
            "Should not advertise SSE MCP support"
        );

        // Verify session/list capability is advertised. The presence of the
        // `list` key (even as an empty object) signals `session/list` support.
        let session_caps = agent_caps
            .get("sessionCapabilities")
            .expect("Session capabilities should be advertised");
        assert!(
            session_caps.get("list").is_some(),
            "Should advertise session/list capability"
        );

        // Verify meta capabilities (modes, plans)
        // meta field is optional, only check if present
        if let Some(meta) = agent_caps.get("meta") {
            assert_eq!(
                meta.get("streaming"),
                Some(&serde_json::Value::Bool(true)),
                "Should advertise streaming support"
            );
            assert_eq!(
                meta.get("supports_modes"),
                Some(&serde_json::Value::Bool(true)),
                "Should advertise modes support from default config"
            );
            assert_eq!(
                meta.get("supports_plans"),
                Some(&serde_json::Value::Bool(true)),
                "Should advertise plans support from default config"
            );
            // Slash commands are intentionally NOT advertised — the agent does
            // not deliver them (the CommandRegistry is not wired into the
            // session lifecycle). Advertise-vs-deliver consistency requires the
            // key to be absent.
            assert!(
                meta.get("supports_slash_commands").is_none(),
                "Should NOT advertise slash commands — agent does not deliver them"
            );
        }

        // Verify authentication methods (should be empty)
        let auth_methods = response_json
            .get("authMethods")
            .expect("Auth methods field should exist");
        assert!(
            auth_methods.as_array().unwrap().is_empty(),
            "Should not advertise any authentication methods"
        );

        // Verify agent info
        let agent_info = response_json
            .get("agentInfo")
            .expect("Agent info should be present");
        assert_eq!(
            agent_info.get("name"),
            Some(&serde_json::Value::String("llama-agent".to_string())),
            "Agent name should be llama-agent"
        );
        assert_eq!(
            agent_info.get("version"),
            Some(&serde_json::Value::String(
                env!("CARGO_PKG_VERSION").to_string()
            )),
            "Agent version should match package version"
        );
        assert!(
            agent_info.get("title").is_some(),
            "Agent title should be present"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_capability_advertisement_with_custom_config() {
        use crate::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_config = AgentConfig {
            model: ModelConfig {
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
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
            tool_execution_config: Default::default(),
        };

        let model_manager =
            Arc::new(crate::model::ModelManager::new(test_config.model.clone()).unwrap());
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            test_config.queue_config.clone(),
            test_config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            test_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
            test_config.parallel_execution_config.clone(),
        ));

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            test_config,
        ));

        // Create custom ACP config with specific capabilities disabled
        let custom_acp_config = AcpConfig {
            protocol_version: "0.1.0".to_string(),
            capabilities: crate::acp::config::AcpCapabilities {
                supports_session_loading: false,
                supports_modes: false,
                supports_plans: false,
                filesystem: crate::acp::config::FilesystemCapabilities {
                    read_text_file: true,
                    write_text_file: false,
                },
                terminal: false,
            },
            permission_policy: crate::acp::permissions::PermissionPolicy::AlwaysAsk,
            ..Default::default()
        };

        let (acp_server, _notification_rx) =
            AcpServer::new(agent_server, custom_acp_config, test_agent_tools_mount());
        let server = Arc::new(acp_server);

        let request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(agent_client_protocol::schema::ClientCapabilities::new());

        let result = server.initialize(request).await;
        assert!(
            result.is_ok(),
            "Initialize should succeed with custom config"
        );

        let response = result.unwrap();

        // Serialize response to JSON to inspect the structure
        let response_json = serde_json::to_value(&response).expect("Should serialize response");
        let agent_caps = response_json
            .get("agentCapabilities")
            .expect("Agent capabilities should be present");

        // Verify custom capabilities are correctly advertised
        assert_eq!(
            agent_caps.get("loadSession"),
            Some(&serde_json::Value::Bool(false)),
            "Should advertise disabled load_session capability"
        );

        // meta field is optional, only check if present
        if let Some(meta) = agent_caps.get("meta") {
            assert_eq!(
                meta.get("supports_modes"),
                Some(&serde_json::Value::Bool(false)),
                "Should advertise disabled modes support"
            );
            assert_eq!(
                meta.get("supports_plans"),
                Some(&serde_json::Value::Bool(false)),
                "Should advertise disabled plans support"
            );
            // Slash commands are never advertised regardless of config —
            // the capability is not delivered.
            assert!(
                meta.get("supports_slash_commands").is_none(),
                "Should NOT advertise slash commands — agent does not deliver them"
            );
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_client_capabilities_stored_and_transferred_to_sessions() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        // Create initialize request with specific capabilities
        let fs_caps = agent_client_protocol::schema::FileSystemCapabilities::new()
            .read_text_file(true)
            .write_text_file(false);

        let client_caps = agent_client_protocol::schema::ClientCapabilities::new()
            .fs(fs_caps)
            .terminal(true);

        let init_request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(client_caps.clone());

        // Initialize server
        let init_result = server.initialize(init_request).await;
        assert!(init_result.is_ok(), "Initialize should succeed");

        // Verify capabilities are stored in server
        let stored_caps = server.client_capabilities.read().await;
        assert!(
            stored_caps.is_some(),
            "Client capabilities should be stored"
        );
        let stored_caps = stored_caps.clone().unwrap();
        assert!(stored_caps.fs.read_text_file);
        assert!(!stored_caps.fs.write_text_file);
        assert!(stored_caps.terminal);
        drop(stored_caps);

        // Create a new session
        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_result = server.new_session(new_session_request).await;
        assert!(session_result.is_ok(), "New session should succeed");
        let session_response = session_result.unwrap();

        // Verify the session has the client capabilities
        let session = server.get_session_by_id(&session_response.session_id).await;
        assert!(session.is_some(), "Session should exist");
        let session = session.unwrap();
        assert!(
            session.client_capabilities.fs.read_text_file,
            "Session should have client's fs.read_text_file capability"
        );
        assert!(
            !session.client_capabilities.fs.write_text_file,
            "Session should have client's fs.write_text_file capability"
        );
        assert!(
            session.client_capabilities.terminal,
            "Session should have client's terminal capability"
        );
    }

    #[test]
    fn test_filesystem_error_to_protocol_error() {
        use super::super::filesystem::FilesystemError;

        // Test security violations map to invalid_params
        let error = FilesystemError::RelativePath("relative/path".to_string());
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::PathTraversal("/etc/../../../etc/passwd".to_string());
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::NotAllowed("/blocked/path".to_string());
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::Blocked("/blocked/path".to_string());
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::FileTooLarge(1000000, 500000);
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        // Test IO errors map appropriately
        let error = FilesystemError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found",
        ));
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::Io(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied",
        ));
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        let error = FilesystemError::Io(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "already exists",
        ));
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
        assert!(proto_error.data.is_some());

        // Test generic IO error maps to internal_error
        let error = FilesystemError::Io(std::io::Error::other("other error"));
        let proto_error = filesystem_error_to_protocol_error(error);
        assert_eq!(
            proto_error.code,
            agent_client_protocol::ErrorCode::InternalError
        );
        assert!(proto_error.data.is_some());
    }

    #[tokio::test]
    #[serial]
    async fn test_session_id_bidirectional_mapping() {
        use crate::types::ids::SessionId as LlamaSessionId;

        let server = create_test_server().await;

        // Create a llama session ID
        let llama_session_id = LlamaSessionId::new();

        // Create an ACP session state
        let acp_session = AcpSessionState::new(llama_session_id);
        let acp_session_id = acp_session.session_id.clone();

        // Verify ACP session ID is derived from llama session ID
        assert_eq!(
            acp_session_id.0.as_ref(),
            llama_session_id.to_string(),
            "ACP session ID should be string representation of llama session ID"
        );

        // Store the session
        server.store_session(acp_session.clone()).await;

        // Verify we can retrieve by ACP session ID
        let retrieved = server
            .get_session(&acp_session_id)
            .await
            .expect("Session should exist");
        assert_eq!(
            retrieved.session_id, acp_session_id,
            "Retrieved session ID should match"
        );
        assert_eq!(
            retrieved.llama_session_id, llama_session_id,
            "Retrieved llama session ID should match"
        );

        // Verify reverse mapping exists (llama_to_acp)
        let reverse_mapping = server.llama_to_acp.read().await;
        let mapped_acp_id = reverse_mapping
            .get(&llama_session_id)
            .expect("Reverse mapping should exist");
        assert_eq!(
            *mapped_acp_id, acp_session_id,
            "Reverse mapping should point to correct ACP session ID"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_multiple_session_mappings() {
        use crate::types::ids::SessionId as LlamaSessionId;

        let server = create_test_server().await;

        // Create first session
        let llama_id1 = LlamaSessionId::new();
        let acp_session1 = AcpSessionState::new(llama_id1);
        let acp_id1 = acp_session1.session_id.clone();
        server.store_session(acp_session1).await;

        // Create second session
        let llama_id2 = LlamaSessionId::new();
        let acp_session2 = AcpSessionState::new(llama_id2);
        let acp_id2 = acp_session2.session_id.clone();
        server.store_session(acp_session2).await;

        // Verify sessions are different
        assert_ne!(llama_id1, llama_id2, "Llama session IDs should differ");
        assert_ne!(acp_id1, acp_id2, "ACP session IDs should differ");

        // Verify both sessions exist and have correct mappings
        let session1 = server
            .get_session(&acp_id1)
            .await
            .expect("First session should exist");
        assert_eq!(session1.llama_session_id, llama_id1);

        let session2 = server
            .get_session(&acp_id2)
            .await
            .expect("Second session should exist");
        assert_eq!(session2.llama_session_id, llama_id2);

        // Verify reverse mappings are correct
        let reverse_mapping = server.llama_to_acp.read().await;
        assert_eq!(
            reverse_mapping.get(&llama_id1),
            Some(&acp_id1),
            "Reverse mapping for session 1 should be correct"
        );
        assert_eq!(
            reverse_mapping.get(&llama_id2),
            Some(&acp_id2),
            "Reverse mapping for session 2 should be correct"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_get_nonexistent_session() {
        use agent_client_protocol::schema::SessionId;

        // `get_session` now falls back to the durable `SessionStore` on a cache
        // miss, so isolate the state directory for a clean lookup.
        let _state = StateDirGuard::new();
        let server = create_test_server().await;

        // Try to get a session that exists neither in the cache nor on disk.
        let fake_id = SessionId::new("nonexistent");
        let result = server.get_session(&fake_id).await;

        assert!(result.is_none(), "Nonexistent session should return None");
    }

    /// `cleanup_expired_sessions` evicts a session that has been idle longer
    /// than `max_session_age` from the in-memory cache — both the `sessions`
    /// map and the `llama_to_acp` reverse map — and the eviction is lossless:
    /// the durable `SessionRecord` is still readable from the `SessionStore`.
    ///
    /// This is the core unbounded-growth fix: without eviction the in-memory
    /// maps grow for the whole process lifetime.
    #[tokio::test]
    #[serial]
    async fn cleanup_expired_sessions_evicts_idle_session_and_retains_durable_record() {
        let _state = StateDirGuard::new();
        // Zero idle TTL: any session is immediately eligible for eviction.
        let server =
            create_test_server_with_cleanup(Duration::from_secs(300), Duration::from_secs(0)).await;

        // `new_session` populates the in-memory cache AND persists a durable
        // `SessionRecord` to the `SessionStore`.
        let created = server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .expect("new_session should succeed");
        let session_id = created.session_id.clone();
        let llama_id = crate::types::SessionId::from_str(&session_id.0).unwrap();

        // The session is in the cache and the reverse map.
        assert_eq!(
            server.sessions.read().await.len(),
            1,
            "session should be cached after creation"
        );
        assert!(
            server.llama_to_acp.read().await.contains_key(&llama_id),
            "reverse mapping should exist after creation"
        );

        // Sweep: with a zero TTL the session is idle and gets evicted.
        let evicted = server.cleanup_expired_sessions().await;
        assert_eq!(evicted, 1, "the idle session should be evicted");

        // The in-memory cache no longer holds the session — growth is bounded.
        assert!(
            server.sessions.read().await.is_empty(),
            "cache should be empty after eviction"
        );
        assert!(
            !server.llama_to_acp.read().await.contains_key(&llama_id),
            "reverse mapping should be removed on eviction"
        );

        // Eviction is lossless: the durable record survives in the store.
        let record = SessionStore::new()
            .load(&session_id.0)
            .expect("store load should succeed")
            .expect("durable record must survive cache eviction");
        assert_eq!(
            record.session_id.as_str(),
            &*session_id.0,
            "the persisted record is still the truth for this session"
        );
    }

    /// `cleanup_expired_sessions` keeps a session that was accessed within
    /// `max_session_age` — only genuinely idle entries are evicted.
    #[tokio::test]
    #[serial]
    async fn cleanup_expired_sessions_keeps_recently_used_session() {
        let _state = StateDirGuard::new();
        // A long idle TTL: a just-created session is well within it.
        let server =
            create_test_server_with_cleanup(Duration::from_secs(300), Duration::from_secs(3600))
                .await;

        let created = server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .expect("new_session should succeed");

        let evicted = server.cleanup_expired_sessions().await;
        assert_eq!(evicted, 0, "a recently-used session must not be evicted");
        assert!(
            server
                .sessions
                .read()
                .await
                .contains_key(&created.session_id),
            "the recently-used session should remain cached"
        );
    }

    /// After a session is evicted from the in-memory cache, `get_session`
    /// transparently reloads it from the durable `SessionStore` and
    /// re-populates the cache — proving eviction never loses a session.
    #[tokio::test]
    #[serial]
    async fn evicted_session_is_still_resolvable_via_get_session() {
        let _state = StateDirGuard::new();
        let server =
            create_test_server_with_cleanup(Duration::from_secs(300), Duration::from_secs(0)).await;

        let created = server
            .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                std::env::current_dir().unwrap(),
            ))
            .await
            .expect("new_session should succeed");
        let session_id = created.session_id.clone();
        let llama_id = crate::types::SessionId::from_str(&session_id.0).unwrap();

        // Evict the session from the in-memory cache.
        assert_eq!(server.cleanup_expired_sessions().await, 1);
        assert!(
            server.sessions.read().await.is_empty(),
            "session should be evicted from the cache"
        );

        // `get_session` for the evicted id reloads it from the durable store.
        let resolved = server
            .get_session(&session_id)
            .await
            .expect("an evicted session must still resolve via the SessionStore");
        assert_eq!(resolved.session_id, session_id);
        assert_eq!(resolved.llama_session_id, llama_id);

        // The reload re-populated the cache and the reverse mapping.
        assert!(
            server.sessions.read().await.contains_key(&session_id),
            "resolving an evicted session should re-populate the cache"
        );
        assert_eq!(
            server.llama_to_acp.read().await.get(&llama_id),
            Some(&session_id),
            "the reverse mapping should be restored on reload"
        );
    }

    /// Over a long-lived process the in-memory session maps must not grow
    /// without bound: once sessions go idle, a cleanup sweep drains them from
    /// the cache while every durable record is retained on disk.
    #[tokio::test]
    #[serial]
    async fn session_cache_does_not_grow_without_bound() {
        let _state = StateDirGuard::new();
        // Zero TTL so every created session is immediately idle-eligible.
        let server =
            create_test_server_with_cleanup(Duration::from_secs(300), Duration::from_secs(0)).await;

        // Simulate a long-lived process churning through many sessions.
        const SESSION_COUNT: usize = 25;
        let mut created_ids = Vec::with_capacity(SESSION_COUNT);
        for _ in 0..SESSION_COUNT {
            let created = server
                .new_session(agent_client_protocol::schema::NewSessionRequest::new(
                    std::env::current_dir().unwrap(),
                ))
                .await
                .expect("new_session should succeed");
            created_ids.push(created.session_id);
        }

        // Without eviction the cache would now hold every session forever.
        assert_eq!(
            server.sessions.read().await.len(),
            SESSION_COUNT,
            "all created sessions are cached before the sweep"
        );

        // A single cleanup sweep evicts every idle session.
        let evicted = server.cleanup_expired_sessions().await;
        assert_eq!(
            evicted, SESSION_COUNT,
            "every idle session should be evicted in one sweep"
        );
        assert!(
            server.sessions.read().await.is_empty(),
            "the in-memory cache must be bounded — empty after the sweep"
        );
        assert!(
            server.llama_to_acp.read().await.is_empty(),
            "the reverse mapping must be bounded — empty after the sweep"
        );

        // Every evicted session is still durably resolvable from the store.
        for id in &created_ids {
            assert!(
                SessionStore::new()
                    .load(&id.0)
                    .expect("store load should succeed")
                    .is_some(),
                "durable record for {} must survive eviction",
                id.0
            );
        }
    }

    // JSON-RPC parsing/dispatch tests removed when start_with_streams was
    // rewired onto Agent.builder()/connect_with: that wire-format behaviour
    // (parse errors, missing method, notification vs request, unknown method,
    // ext routing, error response shape) is now owned by the SDK runtime
    // itself and is not exercised through any AcpServer API. The
    // per-method semantics (initialize capability advertisement, session
    // mode handling, prompt behaviour, ext_method routing) are still covered
    // by the typed-method tests below that call the inherent methods
    // directly.

    async fn create_test_server_with_modes() -> AcpServer {
        use crate::types::{
            AgentConfig, ModelConfig, ModelSource, ParallelConfig, QueueConfig, RetryConfig,
            SessionConfig,
        };
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let test_config = AgentConfig {
            model: ModelConfig {
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
            },
            queue_config: QueueConfig::default(),
            mcp_servers: Vec::new(),
            session_config: SessionConfig::default(),
            parallel_execution_config: ParallelConfig::default(),
            tool_execution_config: Default::default(),
        };

        let model_manager =
            Arc::new(crate::model::ModelManager::new(test_config.model.clone()).unwrap());
        let request_queue = Arc::new(crate::queue::RequestQueue::new(
            model_manager.clone(),
            test_config.queue_config.clone(),
            test_config.session_config.clone(),
        ));
        let session_manager = Arc::new(crate::session::SessionManager::new(
            test_config.session_config.clone(),
        ));
        let mcp_client: Arc<dyn crate::mcp::MCPClient> = Arc::new(crate::mcp::NoOpMCPClient::new());
        let chat_template = Arc::new(crate::chat_template::ChatTemplateEngine::new());
        let dependency_analyzer = Arc::new(crate::dependency_analysis::DependencyAnalyzer::new(
            test_config.parallel_execution_config.clone(),
        ));

        let agent_server = Arc::new(AgentServer::new(
            model_manager,
            request_queue,
            session_manager,
            mcp_client,
            chat_template,
            dependency_analyzer,
            test_config,
        ));

        // Create config with modes
        let config = AcpConfig {
            available_modes: vec![
                agent_client_protocol::schema::SessionMode::new(
                    "general-purpose",
                    "General Purpose",
                )
                .description("General-purpose agent"),
                agent_client_protocol::schema::SessionMode::new(
                    "statusline-setup",
                    "Statusline Setup",
                )
                .description("Configure status line"),
                agent_client_protocol::schema::SessionMode::new("Explore", "Explore")
                    .description("Explore codebases"),
                agent_client_protocol::schema::SessionMode::new("Plan", "Plan")
                    .description("Plan implementations"),
            ],
            default_mode_id: "general-purpose".to_string(),
            ..Default::default()
        };

        let (server, _notification_rx) =
            AcpServer::new(agent_server, config, test_agent_tools_mount());
        server
    }

    #[tokio::test]
    #[serial]
    async fn test_session_modes_in_new_session_response() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server_with_modes().await);

        // Initialize with client capabilities
        let init_request = agent_client_protocol::schema::InitializeRequest::new(
            agent_client_protocol::schema::ProtocolVersion::V1,
        )
        .client_capabilities(agent_client_protocol::schema::ClientCapabilities::new());

        let _init_result = server.initialize(init_request).await;

        // Create a new session
        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_result = server.new_session(new_session_request).await;
        assert!(session_result.is_ok(), "New session should succeed");
        let session_response = session_result.unwrap();

        // Verify modes are included in the response
        assert!(
            session_response.modes.is_some(),
            "Session modes should be included in response"
        );

        let mode_state = session_response.modes.unwrap();

        // Verify current mode is "general-purpose"
        assert_eq!(
            mode_state.current_mode_id.0.as_ref(),
            "general-purpose",
            "Default mode should be 'general-purpose'"
        );

        // Verify available modes are present
        assert_eq!(
            mode_state.available_modes.len(),
            4,
            "Should have 4 available modes (agent types)"
        );

        // Check mode IDs match Claude Code agent types
        let mode_ids: Vec<&str> = mode_state
            .available_modes
            .iter()
            .map(|m| m.id.0.as_ref())
            .collect();
        assert!(
            mode_ids.contains(&"general-purpose"),
            "Should have 'general-purpose' mode"
        );
        assert!(
            mode_ids.contains(&"statusline-setup"),
            "Should have 'statusline-setup' mode"
        );
        assert!(mode_ids.contains(&"Explore"), "Should have 'Explore' mode");
        assert!(mode_ids.contains(&"Plan"), "Should have 'Plan' mode");

        // Verify mode names and descriptions
        for mode in &mode_state.available_modes {
            match mode.id.0.as_ref() {
                "general-purpose" => {
                    assert_eq!(mode.name, "General Purpose");
                    assert!(mode.description.is_some());
                }
                "statusline-setup" => {
                    assert_eq!(mode.name, "Statusline Setup");
                    assert!(mode.description.is_some());
                }
                "Explore" => {
                    assert_eq!(mode.name, "Explore");
                    assert!(mode.description.is_some());
                }
                "Plan" => {
                    assert_eq!(mode.name, "Plan");
                    assert!(mode.description.is_some());
                }
                _ => panic!("Unexpected mode: {}", mode.id.0),
            }
        }
    }

    #[tokio::test]
    #[serial]
    async fn test_set_session_mode_changes_mode() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        // Create a session first
        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_response = server.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Change mode to "Explore"
        let mode_id = agent_client_protocol::schema::SessionModeId::new("Explore");
        let set_mode_request =
            agent_client_protocol::schema::SetSessionModeRequest::new(session_id.clone(), mode_id);

        let result = server.set_session_mode(set_mode_request).await;
        assert!(result.is_ok(), "Setting session mode should succeed");

        // Verify the mode was changed in the llama session
        let acp_session = server.get_session_by_id(&session_id).await.unwrap();
        let llama_session = server
            .agent_server
            .session_manager()
            .get_session(&acp_session.llama_session_id)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            llama_session.current_mode,
            Some("Explore".to_string()),
            "Mode should be updated to 'Explore'"
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_set_session_mode_with_invalid_session() {
        let server = Arc::new(create_test_server().await);

        // Try to set mode on non-existent session
        let fake_session_id = agent_client_protocol::schema::SessionId::new("nonexistent");
        let mode_id = agent_client_protocol::schema::SessionModeId::new("Plan");
        let set_mode_request =
            agent_client_protocol::schema::SetSessionModeRequest::new(fake_session_id, mode_id);

        let result = server.set_session_mode(set_mode_request).await;
        assert!(result.is_err(), "Should fail with invalid session");
        assert_eq!(
            result.unwrap_err().code,
            agent_client_protocol::ErrorCode::InvalidParams
        );
    }

    #[tokio::test]
    #[serial]
    async fn test_build_session_mode_state() {
        use agent_client_protocol::schema::{SessionMode, SessionModeId, SessionModeState};

        // Build test mode state
        let available_modes = vec![
            SessionMode::new("general-purpose", "General Purpose")
                .description("General-purpose agent"),
            SessionMode::new("statusline-setup", "Statusline Setup")
                .description("Configure status line"),
            SessionMode::new("Explore", "Explore").description("Explore codebases"),
            SessionMode::new("Plan", "Plan").description("Plan implementations"),
        ];
        let mode_state =
            SessionModeState::new(SessionModeId::new("general-purpose"), available_modes);

        // Verify structure
        assert_eq!(mode_state.current_mode_id.0.as_ref(), "general-purpose");
        assert_eq!(mode_state.available_modes.len(), 4);

        // Verify each mode has required fields
        for mode in &mode_state.available_modes {
            assert!(!mode.id.0.is_empty(), "Mode ID should not be empty");
            assert!(!mode.name.is_empty(), "Mode name should not be empty");
            assert!(mode.description.is_some(), "Mode should have a description");
        }
    }

    // The legacy test_json_rpc_* dispatch tests (missing method, notification
    // vs request, null/missing/invalid params, unknown method, ext routing)
    // were deleted when the manual JSON-RPC dispatcher was replaced with
    // Agent.builder(). Wire-format behaviour for these cases is now owned by
    // the SDK runtime and is exercised by its own test suite. Per-method
    // semantics (initialize, set_session_mode, etc.) continue to be covered
    // by the typed-method tests below.

    #[tokio::test]
    #[serial]
    async fn test_set_session_mode() {
        let _state = StateDirGuard::new();
        let server = Arc::new(create_test_server().await);

        // Create a new session first
        let new_session_request =
            agent_client_protocol::schema::NewSessionRequest::new(std::env::current_dir().unwrap());
        let session_response = server.new_session(new_session_request).await.unwrap();
        let session_id = session_response.session_id;

        // Create a set_session_mode request with a test mode
        let mode_id_str = "test-mode";
        let mode_id = agent_client_protocol::schema::SessionModeId::new(mode_id_str);
        let set_mode_request =
            agent_client_protocol::schema::SetSessionModeRequest::new(session_id.clone(), mode_id);

        // Call set_session_mode
        let result = server.set_session_mode(set_mode_request).await;

        // Verify the request succeeds
        assert!(
            result.is_ok(),
            "set_session_mode should succeed: {:?}",
            result.err()
        );

        let response = result.unwrap();

        // Verify the response contains metadata
        assert!(response.meta.is_some(), "Response should contain metadata");

        let meta = response.meta.unwrap();

        // Verify mode was successfully set
        assert_eq!(
            meta.get("mode_set"),
            Some(&serde_json::Value::Bool(true)),
            "mode_set should be true since modes are now implemented"
        );

        // Verify mode_id is echoed back in metadata
        assert_eq!(
            meta.get("mode_id"),
            Some(&serde_json::Value::String(mode_id_str.to_string())),
            "Response should echo back the requested mode_id"
        );
    }

    // test_json_rpc_error_response_format_and_codes (parse error / invalid
    // request / method not found / invalid params / response-shape audit)
    // was deleted with the rest of the dispatch suite when handle_request
    // went away. The SDK 0.11 runtime owns wire-format responses; their
    // shape is verified in the SDK's own test suite.

    // --- set_session_system_prompt tests ---

    #[tokio::test]
    #[serial]
    async fn test_set_session_system_prompt_inserts_into_empty_session() {
        let server = create_test_server().await;
        let session = server.agent_server.create_session().await.unwrap();

        // Session starts with no messages
        let before = server
            .agent_server
            .session_manager()
            .get_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert!(before.messages.is_empty());

        // Set a system prompt
        server
            .agent_server
            .set_session_system_prompt(&session.id, "You are a planner.".to_string())
            .await
            .unwrap();

        let after = server
            .agent_server
            .session_manager()
            .get_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.messages.len(), 1);
        assert_eq!(after.messages[0].role, crate::types::MessageRole::System);
        assert_eq!(after.messages[0].content, "You are a planner.");
    }

    #[tokio::test]
    #[serial]
    async fn test_set_session_system_prompt_replaces_existing() {
        let server = create_test_server().await;
        let session = server.agent_server.create_session().await.unwrap();

        // Set initial system prompt
        server
            .agent_server
            .set_session_system_prompt(&session.id, "You are a planner.".to_string())
            .await
            .unwrap();

        // Swap to a different system prompt
        server
            .agent_server
            .set_session_system_prompt(&session.id, "You are an implementer.".to_string())
            .await
            .unwrap();

        let after = server
            .agent_server
            .session_manager()
            .get_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        // Should still be exactly 1 message (replaced, not appended)
        assert_eq!(after.messages.len(), 1);
        assert_eq!(after.messages[0].role, crate::types::MessageRole::System);
        assert_eq!(after.messages[0].content, "You are an implementer.");
    }

    #[tokio::test]
    #[serial]
    async fn test_set_session_system_prompt_inserts_before_user_message() {
        let server = create_test_server().await;
        let session = server.agent_server.create_session().await.unwrap();

        // Manually add a user message first (no system message present)
        {
            let mut s = server
                .agent_server
                .session_manager()
                .get_session(&session.id)
                .await
                .unwrap()
                .unwrap();
            s.messages.push(crate::types::Message {
                role: crate::types::MessageRole::User,
                content: "Hello".to_string(),
                tool_call_id: None,
                tool_name: None,
                timestamp: std::time::SystemTime::now(),
            });
            server
                .agent_server
                .session_manager()
                .update_session(s)
                .await
                .unwrap();
        }

        // Now set the system prompt — should insert at position 0
        server
            .agent_server
            .set_session_system_prompt(&session.id, "You are a reviewer.".to_string())
            .await
            .unwrap();

        let after = server
            .agent_server
            .session_manager()
            .get_session(&session.id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(after.messages.len(), 2);
        assert_eq!(after.messages[0].role, crate::types::MessageRole::System);
        assert_eq!(after.messages[0].content, "You are a reviewer.");
        assert_eq!(after.messages[1].role, crate::types::MessageRole::User);
        assert_eq!(after.messages[1].content, "Hello");
    }

    // =========================================================================
    // extract_request_max_tokens — caller-supplied generation cap from `_meta`
    // =========================================================================

    /// `extract_request_max_tokens` returns `None` when no meta map is provided.
    ///
    /// The validator runner only attaches `max_tokens` for rule executions;
    /// other callers leave `request.meta` as `None` and must keep the existing
    /// (uncapped) behavior.
    #[test]
    fn test_extract_request_max_tokens_none_when_meta_missing() {
        assert_eq!(extract_request_max_tokens(None), None);
    }

    /// Returns `None` when meta is present but doesn't contain `max_tokens`.
    ///
    /// This guards the "generic ACP client that uses `_meta` for something
    /// else" case — we must not interpret unrelated `_meta` keys.
    #[test]
    fn test_extract_request_max_tokens_none_when_key_missing() {
        let mut meta = serde_json::Map::new();
        meta.insert("other_key".to_string(), serde_json::json!(42));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// Returns `Some(n)` for the canonical case the validator runner produces:
    /// `max_tokens` set to a positive `u64`. This is the contract we share
    /// with `avp-common::validator::runner::build_rule_prompt_request`.
    #[test]
    fn test_extract_request_max_tokens_positive_integer() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(4096_u64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), Some(4096));
    }

    /// Returns `None` for `max_tokens: 0` — a zero cap would be useless and
    /// almost certainly indicates a bug at the caller. Treating it as "no cap"
    /// matches the runner's intent (defense-in-depth, not a hard requirement).
    #[test]
    fn test_extract_request_max_tokens_zero_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(0));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// Returns `None` for non-integer types. Strings, floats, booleans, and
    /// objects under the `max_tokens` key are all treated as "no cap" — we
    /// never coerce or guess.
    #[test]
    fn test_extract_request_max_tokens_non_integer_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!("4096"));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);

        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(4096.5));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);

        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(true));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    /// `i64`-formatted integers (negative or signed-positive) round-trip
    /// through `as_u64`: signed positives parse, negatives don't. We accept
    /// the positive case since `serde_json` may serialize positive ints as
    /// either `Number::U64` or `Number::I64` depending on source.
    #[test]
    fn test_extract_request_max_tokens_signed_positive_accepted() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(8192_i64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), Some(8192));
    }

    #[test]
    fn test_extract_request_max_tokens_negative_treated_as_unset() {
        let mut meta = serde_json::Map::new();
        meta.insert("max_tokens".to_string(), serde_json::json!(-1_i64));
        assert_eq!(extract_request_max_tokens(Some(&meta)), None);
    }

    // =========================================================================
    // generate_and_emit_title — session-title generation, trigger, and once-guard
    // =========================================================================

    /// Append a [`crate::types::Message`] to a llama session in place under the
    /// session lock, so a test can stage a conversation without racing the
    /// read-modify-write that `mutate_session` exists to avoid.
    async fn push_message(
        server: &AcpServer,
        session_id: &LlamaSessionId,
        role: crate::types::MessageRole,
        content: &str,
    ) {
        server
            .agent_server
            .session_manager()
            .mutate_session(session_id, |session| {
                session.messages.push(crate::types::Message {
                    role,
                    content: content.to_string(),
                    tool_call_id: None,
                    tool_name: None,
                    timestamp: std::time::SystemTime::now(),
                });
            })
            .await
            .unwrap();
    }

    /// With a first exchange present, `generate_and_emit_title` generates a
    /// title, stores it on the live session, persists it, and broadcasts one
    /// `SessionInfoUpdate`.
    ///
    /// The test server has no model loaded, so the model call fails and
    /// `generate_session_title` falls back to the first-user-message heuristic —
    /// this exercises the deterministic heuristic-fallback branch end to end.
    #[tokio::test]
    #[serial]
    async fn generate_and_emit_title_uses_heuristic_when_model_unavailable() {
        let _state = StateDirGuard::new();
        let server = create_test_server().await;
        let mut notifications = server.notification_tx.subscribe();

        let llama_session = server.agent_server.create_session().await.unwrap();
        let llama_session_id = llama_session.id;
        let acp_session_id = AcpSessionId::new("01TITLEHEURISTIC0000000000");

        // Stage a complete first exchange: user prompt + assistant reply.
        push_message(
            &server,
            &llama_session_id,
            crate::types::MessageRole::User,
            "Add dark mode to the settings page",
        )
        .await;
        push_message(
            &server,
            &llama_session_id,
            crate::types::MessageRole::Assistant,
            "Sure, here is how.",
        )
        .await;

        AcpServer::generate_and_emit_title(
            Arc::clone(&server.agent_server),
            server.notification_tx.clone(),
            llama_session_id,
            acp_session_id.clone(),
        )
        .await;

        // The heuristic title is the trimmed first user message.
        let session = server
            .agent_server
            .session_manager()
            .get_session(&llama_session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session.title.as_deref(),
            Some("Add dark mode to the settings page"),
            "title should be stored on the live session via the heuristic fallback"
        );

        // The record is persisted so `session/list` reflects the title.
        let loaded = SessionStore::new()
            .load(&acp_session_id.0)
            .unwrap()
            .expect("record should be persisted with the title");
        assert_eq!(
            loaded.title.as_deref(),
            Some("Add dark mode to the settings page")
        );

        // Exactly one SessionInfoUpdate carrying the title is broadcast.
        let notification = notifications
            .try_recv()
            .expect("a SessionInfoUpdate should have been broadcast");
        match notification.update {
            agent_client_protocol::schema::SessionUpdate::SessionInfoUpdate(info) => {
                assert_eq!(
                    info.title.as_opt_deref(),
                    Some(Some("Add dark mode to the settings page"))
                );
            }
            other => panic!("expected SessionInfoUpdate, got {other:?}"),
        }
        assert!(
            notifications.try_recv().is_err(),
            "exactly one notification should be emitted"
        );
    }

    /// `generate_and_emit_title` does not generate a title before the first
    /// agent response — a user message alone is not a full exchange.
    #[tokio::test]
    #[serial]
    async fn generate_and_emit_title_skips_without_first_exchange() {
        let _state = StateDirGuard::new();
        let server = create_test_server().await;
        let mut notifications = server.notification_tx.subscribe();

        let llama_session = server.agent_server.create_session().await.unwrap();
        let llama_session_id = llama_session.id;
        let acp_session_id = AcpSessionId::new("01TITLENOEXCHANGE000000000");

        // Only a user message — no assistant reply yet.
        push_message(
            &server,
            &llama_session_id,
            crate::types::MessageRole::User,
            "just a prompt",
        )
        .await;

        AcpServer::generate_and_emit_title(
            Arc::clone(&server.agent_server),
            server.notification_tx.clone(),
            llama_session_id,
            acp_session_id,
        )
        .await;

        let session = server
            .agent_server
            .session_manager()
            .get_session(&llama_session_id)
            .await
            .unwrap()
            .unwrap();
        assert!(
            session.title.is_none(),
            "no title should be generated before the first agent response"
        );
        assert!(
            notifications.try_recv().is_err(),
            "no notification should be emitted without a first exchange"
        );
    }

    /// `generate_and_emit_title` is a no-op when the session already has a
    /// title — the once-guard prevents a racing turn from overwriting it.
    #[tokio::test]
    #[serial]
    async fn generate_and_emit_title_skips_when_title_already_set() {
        let _state = StateDirGuard::new();
        let server = create_test_server().await;
        let mut notifications = server.notification_tx.subscribe();

        let llama_session = server.agent_server.create_session().await.unwrap();
        let llama_session_id = llama_session.id;
        let acp_session_id = AcpSessionId::new("01TITLEALREADYSET000000000");

        push_message(
            &server,
            &llama_session_id,
            crate::types::MessageRole::User,
            "Add dark mode to the settings page",
        )
        .await;
        push_message(
            &server,
            &llama_session_id,
            crate::types::MessageRole::Assistant,
            "Sure, here is how.",
        )
        .await;
        // A turn raced ahead and already set a title.
        server
            .agent_server
            .session_manager()
            .mutate_session(&llama_session_id, |session| {
                session.title = Some("Pre-existing title".to_string());
            })
            .await
            .unwrap();

        AcpServer::generate_and_emit_title(
            Arc::clone(&server.agent_server),
            server.notification_tx.clone(),
            llama_session_id,
            acp_session_id,
        )
        .await;

        let session = server
            .agent_server
            .session_manager()
            .get_session(&llama_session_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            session.title.as_deref(),
            Some("Pre-existing title"),
            "an existing title must not be overwritten"
        );
        assert!(
            notifications.try_recv().is_err(),
            "no notification should be emitted when a title already exists"
        );
    }
}
