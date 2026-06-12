//! HookableAgent - middleware that fires hooks at ACP lifecycle points.
//!
//! In ACP 0.10 `HookableAgent` was an `Arc<dyn Agent>` wrapper that
//! implemented the `Agent` trait. ACP 0.11 replaces the trait with a
//! Role/Builder/handler model, so the wrapper is reshaped on the same
//! [`ConnectTo<Client>`] middleware shape used by [`TracingAgent`]:
//!
//! ```text
//!     Client  <----[real channel]---->  HookableAgent  <----[duplex channel]---->  inner Agent
//!                                       (fires hooks at lifecycle points)
//! ```
//!
//! The hook event surface ([`HookEvent`], [`HookEventKind`], [`HookDecision`],
//! [`HookHandler`], [`HookRegistration`], [`HookCommandContext`],
//! [`SessionSource`]) is unchanged from 0.10 — it lives in
//! [`crate::hook_config`] and is re-exported from the crate root.
//!
//! Hook firing logic is exposed as standalone async helper methods on
//! [`HookableAgent`]:
//!
//! - [`HookableAgent::run_user_prompt_submit`] — pre-prompt hook fan-out;
//!   returns either a possibly-modified `PromptRequest` (with prepended
//!   context) or an [`agent_client_protocol::Error`] from a `Block` /
//!   `Cancel` decision.
//! - [`HookableAgent::run_stop`] — post-prompt hook fan-out; returns the
//!   response, possibly annotated with `hook_should_continue` meta.
//! - [`HookableAgent::track_session_start`] — records session cwd and fires
//!   `SessionStart` hooks (called after `new_session` / `load_session`).
//! - [`HookableAgent::intercept_notifications`] — taps a session
//!   notification broadcast channel, fires `PreToolUse` / `PostToolUse` /
//!   `PostToolUseFailure` / `Notification` hooks, and surfaces `Cancel` /
//!   `AllowWithContext` decisions back through mpsc channels.
//! - [`HookableAgent::fire_event`] — fan out an arbitrary [`HookEvent`] to
//!   matching registrations. Used by callers (CLI, MCP proxy, …) to fire
//!   events that don't correspond to ACP lifecycle methods, such as
//!   `TeammateIdle`, `TaskCompleted`, `PostCompact`, `ConfigChange`.
//!
//! These helpers are composable: an outer driver (a real `ConnectTo<Client>`
//! middleware, or a test) calls them at the appropriate seams in a prompt
//! turn. The [`ConnectTo<Client>`] impl on [`HookableAgent`] is a thin
//! TracingAgent-style passthrough; richer JSON-RPC interception layered on
//! top of these helpers will land in follow-up tasks.

use crate::hook_config::{
    HookCommandContext, HookDecision, HookEvent, HookEventKind, HookRegistration, SessionSource,
};
use agent_client_protocol::schema::{
    ContentBlock, PromptRequest, PromptResponse, SessionNotification, TextContent,
};
use agent_client_protocol::{Channel, Client, ConnectTo, Result as AcpResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

/// `_meta` key under which an ACP `ToolCall` / `ToolCallUpdate` carries the
/// **bare** llama-agent tool name (e.g. `fs_read`, `shell`,
/// `mcp__<server>__<tool>`).
///
/// The human-readable `ToolCall.title` may be decorated as
/// `"<name>: <description>"` for display, so hook matchers must never test
/// against it. Producers (see `llama-agent`'s `tool_call_to_acp`) write the
/// un-decorated name here; [`notification_to_events`] reads it so
/// `PreToolUse` / `PostToolUse` / `PostToolUseFailure` events carry the bare
/// name into matcher evaluation.
pub const TOOL_NAME_META_KEY: &str = "tool_name";

/// The outcome of firing `PreToolUse` hooks at the tool-dispatch seam.
///
/// Returned by [`HookableAgent::run_pre_tool_use`]; the dispatch driver
/// matches on it to decide whether to run the tool, skip it, or stop the turn.
#[derive(Clone, Debug)]
pub enum PreToolUseOutcome {
    /// Run the tool. `updated_input`, when `Some`, replaces the tool arguments
    /// before execution (`updatedInput`); `context`, when `Some`, is injected
    /// alongside the tool result (`additionalContext`).
    Proceed {
        updated_input: Option<serde_json::Value>,
        context: Option<String>,
    },
    /// Do not run the tool — a hook denied it (`Block` / permissionDecision
    /// `deny`). The `reason` is fed back to the model as the tool result.
    Deny { reason: String },
    /// Stop the agentic turn without running the tool (`continue:false` /
    /// `Cancel`).
    StopTurn { reason: String },
}

// ---------------------------------------------------------------------------
// HookableAgent middleware
// ---------------------------------------------------------------------------

/// Middleware that fires hooks at ACP lifecycle points.
///
/// `HookableAgent` is generic over its inner component `A: ConnectTo<Client>`,
/// so it composes with any agent built via `Agent.builder()` or any other
/// component that exposes the `ConnectTo<Client>` interface.
///
/// Hook registrations are added via [`with_hook`](Self::with_hook) or
/// [`with_registration`](Self::with_registration). The inner component is
/// driven through a duplex channel exactly like [`crate::TracingAgent`];
/// callers fire hooks at the right seams using the helper methods on this
/// type.
pub struct HookableAgent<A> {
    inner: A,
    hooks: Vec<HookRegistration>,
    /// Maps session_id -> cwd, populated by [`Self::track_session_start`].
    /// Arc-wrapped so [`Self::intercept_notifications`] can share it with
    /// the spawned listener task.
    session_cwd: Arc<std::sync::Mutex<HashMap<String, PathBuf>>>,
    /// Whether we're currently inside a Stop hook. Used to set the
    /// `stop_hook_active` flag on `HookEvent::Stop` so handlers can detect
    /// recursion.
    in_stop_hook: std::sync::atomic::AtomicBool,
    command_context: HookCommandContext,
}

impl<A> HookableAgent<A> {
    /// Wrap an inner ACP component with no hooks.
    ///
    /// Add hooks with [`with_hook`](Self::with_hook) or
    /// [`with_registration`](Self::with_registration).
    pub fn new(inner: A) -> Self {
        Self {
            inner,
            hooks: Vec::new(),
            session_cwd: Arc::new(std::sync::Mutex::new(HashMap::new())),
            in_stop_hook: std::sync::atomic::AtomicBool::new(false),
            command_context: HookCommandContext {
                transcript_path: String::new(),
                permission_mode: "bypassPermissions".to_string(),
            },
        }
    }

    /// Set the transcript path included in command-hook JSON input.
    ///
    /// AVP's `CommonInput` requires a `transcript_path` field; ACP itself
    /// has no transcript so this is configured by the caller.
    pub fn with_transcript_path(mut self, path: impl Into<String>) -> Self {
        self.command_context.transcript_path = path.into();
        self
    }

    /// Set the permission-mode string included in command-hook JSON input.
    ///
    /// Empty string is treated as "no permission mode set" — the field is
    /// omitted from the JSON.
    pub fn with_permission_mode(mut self, mode: impl Into<String>) -> Self {
        self.command_context.permission_mode = mode.into();
        self
    }

    /// Borrow the configured command context.
    pub fn command_context(&self) -> &HookCommandContext {
        &self.command_context
    }

    /// Register a hook handler for the given event kinds with an optional
    /// matcher pattern.
    ///
    /// The matcher follows Claude Code's rules (see [`crate::hook_config::Matcher`]):
    /// `None`, `Some("")`, or `Some("*")` match everything; a plain identifier
    /// (or `|`-separated identifiers) is a full-string match; anything else is a
    /// JavaScript-style regex evaluated like `RegExp(pattern).test(value)`. The
    /// regex is intentionally unanchored, so the pattern author controls anchoring
    /// via `^`/`$`.
    ///
    /// # Panics
    /// If `matcher` is `Some(pat)` where `pat` is treated as a regex and is not
    /// a valid regular expression.
    pub fn with_hook(
        mut self,
        events: &[HookEventKind],
        matcher: Option<&str>,
        handler: impl crate::hook_config::HookHandler + 'static,
    ) -> Self {
        let matcher = crate::hook_config::Matcher::try_parse(matcher.unwrap_or(""))
            .unwrap_or_else(|e| panic!("invalid hook matcher regex: {e}"));
        self.hooks.push(HookRegistration::new(
            events.to_vec(),
            matcher,
            Arc::new(handler),
        ));
        self
    }

    /// Register a hook from a pre-built [`HookRegistration`].
    pub fn with_registration(mut self, registration: HookRegistration) -> Self {
        self.hooks.push(registration);
        self
    }

    /// Borrow the inner component.
    pub fn inner(&self) -> &A {
        &self.inner
    }

    /// Consume the wrapper and return the inner component.
    pub fn into_inner(self) -> A {
        self.inner
    }

    /// Get the cwd for a session, falling back to "." if unknown.
    ///
    /// Populated by [`Self::track_session_start`].
    pub fn get_cwd(&self, session_id: &str) -> PathBuf {
        self.session_cwd
            .lock()
            .expect("session_cwd mutex not poisoned")
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Record a new or resumed session's cwd and fire `SessionStart` hooks.
    ///
    /// Should be called after the inner component returns a `NewSessionResponse`
    /// or `LoadSessionResponse`.
    pub async fn track_session_start(
        &self,
        session_id: String,
        source: SessionSource,
        cwd: PathBuf,
    ) {
        self.session_cwd
            .lock()
            .expect("session_cwd mutex not poisoned")
            .insert(session_id.clone(), cwd.clone());
        let event = HookEvent::SessionStart {
            session_id,
            source,
            cwd,
        };
        let _ = self.run_hooks(&event).await;
    }

    /// Fire `UserPromptSubmit` hooks before forwarding a prompt to the
    /// inner agent.
    ///
    /// Returns the (possibly-modified) `PromptRequest` to forward, or an
    /// `agent_client_protocol::Error` if any hook returned `Block` or
    /// `Cancel`.
    ///
    /// `AllowWithContext` decisions cause the hook context strings to be
    /// prepended as a `TextContent` block at the front of `request.prompt`.
    pub async fn run_user_prompt_submit(&self, request: PromptRequest) -> AcpResult<PromptRequest> {
        let session_id = request.session_id.to_string();
        let cwd = self.get_cwd(&session_id);
        let event = HookEvent::UserPromptSubmit {
            session_id,
            prompt: request.prompt.clone(),
            cwd,
        };
        let decisions = self.run_hooks(&event).await;
        Self::check_blockable(&decisions)?;
        Ok(Self::inject_context(&decisions, request))
    }

    /// Fire `Stop` hooks after the inner agent returns a `PromptResponse`.
    ///
    /// Returns the (possibly-annotated) response. If any hook returned
    /// `ShouldContinue`, the response's `meta` is annotated with
    /// `hook_should_continue: true` and `hook_reason: <reason>`.
    pub async fn run_stop(&self, session_id: String, response: PromptResponse) -> PromptResponse {
        let cwd = self.get_cwd(&session_id);
        let stop_hook_active = self.in_stop_hook.load(std::sync::atomic::Ordering::SeqCst);
        self.in_stop_hook
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let event = HookEvent::Stop {
            session_id,
            stop_reason: response.stop_reason,
            stop_hook_active,
            cwd,
        };
        let decisions = self.run_hooks(&event).await;
        self.in_stop_hook
            .store(false, std::sync::atomic::Ordering::SeqCst);

        if let Some(reason) = Self::find_should_continue(&decisions) {
            return Self::annotate_should_continue(response, reason);
        }
        response
    }

    /// Fire `PreToolUse` hooks synchronously at the tool-dispatch seam, before
    /// the tool runs.
    ///
    /// Unlike the notification path ([`Self::intercept_notifications`]), this
    /// fires *before* the tool is executed, so its decisions can genuinely gate
    /// and rewrite the call — Claude Code's true-blocking semantics. The caller
    /// drives the returned [`PreToolUseOutcome`]:
    ///
    /// - [`PreToolUseOutcome::Deny`] → skip execution, feed the reason back to
    ///   the model as the tool result.
    /// - [`PreToolUseOutcome::StopTurn`] → stop the agentic turn (`continue:false`).
    /// - [`PreToolUseOutcome::Proceed`] → run the tool, applying any
    ///   `updated_input` to the arguments and injecting any `context` alongside
    ///   the result.
    ///
    /// Decision priority mirrors the other seams: the first `Block`/deny wins
    /// (over context/updatedInput), then `Cancel` (`continue:false`), then the
    /// first `updatedInput`, then the first `additionalContext`.
    ///
    /// The `cwd` carried into the event is resolved from the session id via
    /// [`Self::get_cwd`].
    pub async fn run_pre_tool_use(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: Option<serde_json::Value>,
        tool_use_id: Option<&str>,
    ) -> PreToolUseOutcome {
        let cwd = self.get_cwd(session_id);
        let event = HookEvent::PreToolUse {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_input,
            tool_use_id: tool_use_id.map(str::to_string),
            cwd,
        };
        let decisions = self.run_hooks(&event).await;

        if let Some(reason) = Self::find_block(&decisions) {
            return PreToolUseOutcome::Deny {
                reason: reason.to_string(),
            };
        }
        if let Some(reason) = Self::find_cancel(&decisions) {
            return PreToolUseOutcome::StopTurn {
                reason: reason.to_string(),
            };
        }
        let updated_input = decisions.iter().find_map(|d| match d {
            HookDecision::AllowWithUpdatedInput { updated_input } => Some(updated_input.clone()),
            _ => None,
        });
        let context = Self::collect_context(&decisions);
        PreToolUseOutcome::Proceed {
            updated_input,
            context,
        }
    }

    /// Fire `PostToolUse` hooks synchronously after a successful tool call.
    ///
    /// Returns any joined `additionalContext` to feed back to the model
    /// alongside the tool result. These hooks cannot block — the action has
    /// already happened — so `Block`/`Cancel`/`updatedInput` decisions are not
    /// honored here.
    pub async fn run_post_tool_use(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: Option<serde_json::Value>,
        tool_response: Option<serde_json::Value>,
        tool_use_id: Option<&str>,
    ) -> Option<String> {
        let cwd = self.get_cwd(session_id);
        let event = HookEvent::PostToolUse {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_input,
            tool_response,
            tool_use_id: tool_use_id.map(str::to_string),
            cwd,
        };
        let decisions = self.run_hooks(&event).await;
        Self::collect_context(&decisions)
    }

    /// Fire `PostToolUseFailure` hooks synchronously after a failed tool call.
    ///
    /// Returns any joined `additionalContext` (Claude's exit-2 stderr feedback)
    /// to feed back to the model. Like [`Self::run_post_tool_use`], these hooks
    /// cannot block.
    pub async fn run_post_tool_use_failure(
        &self,
        session_id: &str,
        tool_name: &str,
        tool_input: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
        tool_use_id: Option<&str>,
    ) -> Option<String> {
        let cwd = self.get_cwd(session_id);
        let event = HookEvent::PostToolUseFailure {
            session_id: session_id.to_string(),
            tool_name: tool_name.to_string(),
            tool_input,
            error,
            tool_use_id: tool_use_id.map(str::to_string),
            cwd,
        };
        let decisions = self.run_hooks(&event).await;
        Self::collect_context(&decisions)
    }

    /// Join every `AllowWithContext` string from a decision set with newlines,
    /// or `None` when no decision carried context.
    fn collect_context(decisions: &[HookDecision]) -> Option<String> {
        let contexts: Vec<&str> = decisions
            .iter()
            .filter_map(|d| match d {
                HookDecision::AllowWithContext { context } => Some(context.as_str()),
                _ => None,
            })
            .collect();
        if contexts.is_empty() {
            None
        } else {
            Some(contexts.join("\n"))
        }
    }

    /// Fire an arbitrary hook event and return all decisions.
    ///
    /// This is the public entry point for callers (CLI, MCP proxy, etc.)
    /// to fire hook events that don't correspond to an ACP lifecycle
    /// method — e.g. [`HookEvent::TeammateIdle`],
    /// [`HookEvent::TaskCompleted`], [`HookEvent::PostCompact`],
    /// [`HookEvent::ConfigChange`].
    pub async fn fire_event(&self, event: &HookEvent) -> Vec<HookDecision> {
        self.run_hooks(event).await
    }

    /// Intercept a `SessionNotification` broadcast channel and fire hooks
    /// on tool-call events.
    ///
    /// Returns:
    /// - A new `broadcast::Receiver` that downstream consumers should use
    ///   in place of `receiver`.
    /// - An mpsc receiver that emits the session id of any active session
    ///   whose hooks return [`HookDecision::Cancel`] — callers should
    ///   `select!` against this and call the inner agent's cancel method.
    /// - An mpsc receiver of context strings from
    ///   [`HookDecision::AllowWithContext`] decisions; intended to be
    ///   prepended to the next `UserPromptSubmit` event for the same
    ///   session.
    pub fn intercept_notifications(
        &self,
        receiver: broadcast::Receiver<SessionNotification>,
    ) -> (
        broadcast::Receiver<SessionNotification>,
        tokio::sync::mpsc::UnboundedReceiver<String>,
        tokio::sync::mpsc::UnboundedReceiver<String>,
    ) {
        let hooks = self.hooks.clone();
        let session_cwd = Arc::clone(&self.session_cwd);
        let (tx, rx) = broadcast::channel(256);
        let (cancel_tx, cancel_rx) = tokio::sync::mpsc::unbounded_channel();
        let (context_tx, context_rx) = tokio::sync::mpsc::unbounded_channel();
        let mut recv = receiver;

        tokio::spawn(async move {
            run_notification_loop(
                &mut recv,
                &hooks,
                &session_cwd,
                &tx,
                &cancel_tx,
                &context_tx,
            )
            .await;
        });

        (rx, cancel_rx, context_rx)
    }

    /// Run all matching hooks for an event in parallel and collect their
    /// decisions.
    async fn run_hooks(&self, event: &HookEvent) -> Vec<HookDecision> {
        let futures: Vec<_> = self
            .hooks
            .iter()
            .filter(|h| h.matches(event))
            .map(|h| h.handler.handle(event))
            .collect();
        futures::future::join_all(futures).await
    }

    /// Check decisions for a blocking or cancelling response. Returns an
    /// `Err` for the first `Block` or `Cancel` encountered.
    fn check_blockable(decisions: &[HookDecision]) -> AcpResult<()> {
        if let Some(reason) = Self::find_block(decisions) {
            return Err(agent_client_protocol::Error::new(
                agent_client_protocol::ErrorCode::InvalidRequest.into(),
                format!("Blocked by hook: {reason}"),
            ));
        }
        if let Some(reason) = Self::find_cancel(decisions) {
            return Err(agent_client_protocol::Error::new(
                agent_client_protocol::ErrorCode::InvalidRequest.into(),
                format!("Cancelled by hook: {reason}"),
            ));
        }
        Ok(())
    }

    fn find_block(decisions: &[HookDecision]) -> Option<&str> {
        decisions.iter().find_map(|d| match d {
            HookDecision::Block { reason } => Some(reason.as_str()),
            HookDecision::Allow
            | HookDecision::AllowWithContext { .. }
            | HookDecision::Cancel { .. }
            | HookDecision::ShouldContinue { .. }
            | HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    fn find_cancel(decisions: &[HookDecision]) -> Option<&str> {
        decisions.iter().find_map(|d| match d {
            HookDecision::Cancel { reason } => Some(reason.as_str()),
            HookDecision::Allow
            | HookDecision::Block { .. }
            | HookDecision::AllowWithContext { .. }
            | HookDecision::ShouldContinue { .. }
            | HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    fn find_should_continue(decisions: &[HookDecision]) -> Option<&str> {
        decisions.iter().find_map(|d| match d {
            HookDecision::ShouldContinue { reason } => Some(reason.as_str()),
            HookDecision::Allow
            | HookDecision::Block { .. }
            | HookDecision::AllowWithContext { .. }
            | HookDecision::Cancel { .. }
            | HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    /// Prepend any `AllowWithContext` strings as a single `TextContent`
    /// block at the front of the prompt.
    fn inject_context(decisions: &[HookDecision], request: PromptRequest) -> PromptRequest {
        let contexts: Vec<&str> = decisions
            .iter()
            .filter_map(|d| match d {
                HookDecision::AllowWithContext { context } => Some(context.as_str()),
                HookDecision::Allow
                | HookDecision::Block { .. }
                | HookDecision::Cancel { .. }
                | HookDecision::ShouldContinue { .. }
                | HookDecision::AllowWithUpdatedInput { .. } => None,
            })
            .collect();
        if contexts.is_empty() {
            return request;
        }
        let mut modified = request;
        modified
            .prompt
            .insert(0, ContentBlock::Text(TextContent::new(contexts.join("\n"))));
        modified
    }

    /// Annotate a `PromptResponse` with `hook_should_continue` meta.
    fn annotate_should_continue(mut response: PromptResponse, reason: &str) -> PromptResponse {
        let meta = response.meta.get_or_insert_with(Default::default);
        meta.insert(
            "hook_should_continue".to_string(),
            serde_json::Value::Bool(true),
        );
        meta.insert(
            "hook_reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );
        response
    }
}

/// Convenience: build a [`HookableAgent`] from a parsed [`crate::HookConfig`]
/// and an inner ACP component.
///
/// Translates each matcher group + handler in the config into a runtime
/// [`HookRegistration`] (via [`crate::HookConfig::build_registrations`]) and
/// attaches it to a fresh [`HookableAgent`] wrapping `inner`.
///
/// # Errors
/// Returns [`crate::HookConfigError`] if the config contains an empty hook
/// list, an invalid matcher regex, or a `prompt`/`agent` handler without a
/// corresponding `evaluator`.
pub fn hookable_agent_from_config<A>(
    inner: A,
    config: &crate::HookConfig,
    evaluator: Option<Arc<dyn crate::HookEvaluator>>,
) -> Result<HookableAgent<A>, crate::HookConfigError> {
    hookable_agent_from_config_with_context(inner, config, evaluator, HookCommandContext::default())
}

/// Like [`hookable_agent_from_config`] but folds an explicit
/// [`HookCommandContext`] into both the built registrations and the wrapper.
///
/// The `command_context` (`transcript_path`, `permission_mode`) is captured
/// into every command/prompt/agent handler — so their JSON stdin matches
/// Claude Code's input shape — *and* recorded on the wrapper so
/// [`HookableAgent::command_context`] reflects it. Building handlers with the
/// same context the wrapper reports keeps the two from drifting; the builder
/// methods ([`HookableAgent::with_transcript_path`] /
/// [`HookableAgent::with_permission_mode`]) only retag the wrapper and do not
/// rebuild already-built handlers, so a context known up front must be threaded
/// here.
///
/// # Errors
/// Returns [`crate::HookConfigError`] if the config contains an empty hook
/// list, an invalid matcher regex, or a `prompt`/`agent` handler without a
/// corresponding `evaluator`.
pub fn hookable_agent_from_config_with_context<A>(
    inner: A,
    config: &crate::HookConfig,
    evaluator: Option<Arc<dyn crate::HookEvaluator>>,
    command_context: HookCommandContext,
) -> Result<HookableAgent<A>, crate::HookConfigError> {
    let registrations = config.build_registrations_with_context(evaluator, &command_context)?;
    let mut agent = HookableAgent::new(inner);
    for reg in registrations {
        agent = agent.with_registration(reg);
    }
    agent.command_context = command_context;
    Ok(agent)
}

impl<A> ConnectTo<Client> for HookableAgent<A>
where
    A: ConnectTo<Client> + Send + 'static,
{
    /// Wire the client transport to the inner agent through a transparent
    /// duplex tee.
    ///
    /// At this layer (task A2) the wrapper forwards messages unchanged;
    /// callers fire hooks via the helper methods on [`HookableAgent`]
    /// (`run_user_prompt_submit`, `run_stop`, `track_session_start`,
    /// `intercept_notifications`, `fire_event`). Richer per-message
    /// JSON-RPC interception will be layered on top in follow-up tasks.
    async fn connect_to(
        self,
        client: impl ConnectTo<<Client as agent_client_protocol::Role>::Counterpart>,
    ) -> AcpResult<()> {
        let (to_inner, inner_side) = Channel::duplex();

        let inner_future = self.inner.connect_to(inner_side);
        let (client_channel, client_future) = client.into_channel_and_future();

        let copy_client_to_inner = Channel {
            rx: client_channel.rx,
            tx: to_inner.tx,
        }
        .copy();
        let copy_inner_to_client = Channel {
            rx: to_inner.rx,
            tx: client_channel.tx,
        }
        .copy();

        match futures::try_join!(
            inner_future,
            client_future,
            copy_client_to_inner,
            copy_inner_to_client,
        ) {
            Ok(((), (), (), ())) => Ok(()),
            Err(err) => Err(err),
        }
    }
}

// ---------------------------------------------------------------------------
// Notification stream hooking (internal helpers)
// ---------------------------------------------------------------------------

/// Main loop for the notification interception task spawned by
/// [`HookableAgent::intercept_notifications`].
async fn run_notification_loop(
    recv: &mut broadcast::Receiver<SessionNotification>,
    hooks: &[HookRegistration],
    session_cwd: &std::sync::Mutex<HashMap<String, PathBuf>>,
    tx: &broadcast::Sender<SessionNotification>,
    cancel_tx: &tokio::sync::mpsc::UnboundedSender<String>,
    context_tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    loop {
        match recv.recv().await {
            Ok(notification) => {
                let cwd = session_cwd
                    .lock()
                    .expect("session_cwd mutex not poisoned")
                    .get(&notification.session_id.to_string())
                    .cloned()
                    .unwrap_or_else(|| PathBuf::from("."));
                let events = notification_to_events(&notification, &cwd);
                dispatch_notification_hooks(hooks, &events, &notification, cancel_tx, context_tx)
                    .await;
                let _ = tx.send(notification);
            }
            Err(broadcast::error::RecvError::Lagged(n)) => {
                tracing::warn!("intercept_notifications: lagged by {n}");
            }
            Err(broadcast::error::RecvError::Closed) => {
                tracing::debug!("intercept_notifications: channel closed");
                break;
            }
        }
    }
}

/// Run matching hooks for each event and surface side-effects through the
/// cancel and context channels.
///
/// In the notification pipeline, only `Cancel` and `AllowWithContext`
/// decisions have a meaningful effect:
///
/// - `Cancel` → `cancel_tx.send(session_id)` so the prompt-driving task
///   can call the inner agent's cancel method.
/// - `AllowWithContext` → `context_tx.send(context)` so the prompt-driving
///   task can prepend the context to its next prompt.
/// - `Block` / `ShouldContinue` are not meaningful here (the tool call
///   has already been initiated by the inner agent) and are no-ops.
/// - `AllowWithUpdatedInput` is logged at warn level and treated as
///   `Allow`, since updated tool input cannot be applied retroactively.
async fn dispatch_notification_hooks(
    hooks: &[HookRegistration],
    events: &[HookEvent],
    notification: &SessionNotification,
    cancel_tx: &tokio::sync::mpsc::UnboundedSender<String>,
    context_tx: &tokio::sync::mpsc::UnboundedSender<String>,
) {
    for event in events {
        let futures: Vec<_> = hooks
            .iter()
            .filter(|h| h.matches(event))
            .map(|h| h.handler.handle(event))
            .collect();
        for decision in futures::future::join_all(futures).await {
            match &decision {
                HookDecision::Cancel { reason } => {
                    tracing::warn!(
                        "Hook cancelled prompt for session {}: {}",
                        notification.session_id,
                        reason
                    );
                    let _ = cancel_tx.send(notification.session_id.to_string());
                }
                HookDecision::AllowWithContext { context } => {
                    let _ = context_tx.send(context.clone());
                }
                HookDecision::AllowWithUpdatedInput { .. } => {
                    tracing::warn!(
                        "AllowWithUpdatedInput in notification pipeline \
                         (tool already initiated in ACP), treating as Allow"
                    );
                }
                HookDecision::Allow
                | HookDecision::Block { .. }
                | HookDecision::ShouldContinue { .. } => {}
            }
        }
    }
}

/// Convert a [`SessionNotification`] into the hook events it should fire.
///
/// Produces a single `Notification` event for the client-UI hook family.
///
/// `PreToolUse` / `PostToolUse` / `PostToolUseFailure` are deliberately **not**
/// derived from notifications: those fire synchronously at the real
/// tool-dispatch seam (see [`HookableAgent::run_pre_tool_use`] and friends),
/// where their decisions can genuinely gate, rewrite, and feed back the call —
/// true blocking, unlike the after-the-fact notification stream. Firing them
/// here too would double-fire every tool hook, so this path stays
/// `Notification`-only while notifications are still broadcast for the UI.
fn notification_to_events(notification: &SessionNotification, cwd: &Path) -> Vec<HookEvent> {
    vec![HookEvent::Notification {
        notification: Box::new(notification.clone()),
        cwd: cwd.to_path_buf(),
    }]
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_config::HookHandler;
    use agent_client_protocol::schema::{
        ContentChunk, SessionId, SessionUpdate, StopReason, ToolCall, ToolCallStatus,
        ToolCallUpdate, ToolCallUpdateFields,
    };
    use std::sync::atomic::{AtomicBool, Ordering};

    // -- Test hook handlers --

    struct BlockHook {
        reason: String,
    }

    #[async_trait::async_trait]
    impl HookHandler for BlockHook {
        async fn handle(&self, _event: &HookEvent) -> HookDecision {
            HookDecision::Block {
                reason: self.reason.clone(),
            }
        }
    }

    struct ContextHook {
        context: String,
    }

    #[async_trait::async_trait]
    impl HookHandler for ContextHook {
        async fn handle(&self, _event: &HookEvent) -> HookDecision {
            HookDecision::AllowWithContext {
                context: self.context.clone(),
            }
        }
    }

    struct ShouldContinueHook {
        reason: String,
    }

    #[async_trait::async_trait]
    impl HookHandler for ShouldContinueHook {
        async fn handle(&self, _event: &HookEvent) -> HookDecision {
            HookDecision::ShouldContinue {
                reason: self.reason.clone(),
            }
        }
    }

    struct RecordingHook {
        called: Arc<AtomicBool>,
    }

    #[async_trait::async_trait]
    impl HookHandler for RecordingHook {
        async fn handle(&self, _event: &HookEvent) -> HookDecision {
            self.called.store(true, Ordering::SeqCst);
            HookDecision::Allow
        }
    }

    /// Records which session source was seen.
    struct SourceRecordingHook {
        source: Arc<std::sync::Mutex<Option<SessionSource>>>,
    }

    #[async_trait::async_trait]
    impl HookHandler for SourceRecordingHook {
        async fn handle(&self, event: &HookEvent) -> HookDecision {
            if let HookEvent::SessionStart { source, .. } = event {
                *self.source.lock().unwrap() = Some(*source);
            }
            HookDecision::Allow
        }
    }

    /// Test inner — has no behaviour; the new ACP 0.11 `ConnectTo<Client>`
    /// passthrough is exercised by the tracing-style integration tests in
    /// other suites. These unit tests focus on the hook-firing helpers.
    struct DummyInner;

    fn make_prompt_request() -> PromptRequest {
        PromptRequest::new(
            SessionId::from("test-session"),
            vec![ContentBlock::Text(TextContent::new("hello"))],
        )
    }

    /// Run a full `prompt` turn through the helper methods, mirroring what
    /// a real `ConnectTo<Client>` middleware would do at the JSON-RPC seam.
    ///
    /// Returns the response from the supplied inner closure, transformed
    /// by the pre- and post-prompt hooks. Used by tests that previously
    /// drove the (now-removed) `Agent::prompt` impl directly.
    async fn run_prompt_turn<A, F, Fut>(
        agent: &HookableAgent<A>,
        request: PromptRequest,
        inner: F,
    ) -> AcpResult<PromptResponse>
    where
        F: FnOnce(PromptRequest) -> Fut,
        Fut: std::future::Future<Output = AcpResult<PromptResponse>>,
    {
        let session_id = request.session_id.to_string();
        let request = agent.run_user_prompt_submit(request).await?;
        let response = inner(request).await?;
        Ok(agent.run_stop(session_id, response).await)
    }

    // -- Hook firing tests --

    #[tokio::test]
    async fn test_passthrough_delegates_prompt() {
        let agent = HookableAgent::new(DummyInner);
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let response = run_prompt_turn(&agent, make_prompt_request(), |_req| {
            let called = called_clone.clone();
            async move {
                called.store(true, Ordering::SeqCst);
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await
        .unwrap();
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(response.stop_reason, StopReason::EndTurn);
    }

    #[tokio::test]
    async fn test_user_prompt_submit_block_prevents_delegation() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            BlockHook {
                reason: "not allowed".into(),
            },
        );
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let result = run_prompt_turn(&agent, make_prompt_request(), |_req| {
            let called = called_clone.clone();
            async move {
                called.store(true, Ordering::SeqCst);
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("not allowed"));
    }

    #[tokio::test]
    async fn test_allow_with_context_modifies_prompt() {
        let captured = Arc::new(std::sync::Mutex::new(Vec::<ContentBlock>::new()));
        let captured_clone = captured.clone();

        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            ContextHook {
                context: "extra context".into(),
            },
        );

        let _ = run_prompt_turn(&agent, make_prompt_request(), |req| {
            let captured = captured_clone.clone();
            async move {
                *captured.lock().unwrap() = req.prompt.clone();
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await
        .unwrap();

        let blocks = captured.lock().unwrap();
        assert!(blocks.len() >= 2);
        match &blocks[0] {
            ContentBlock::Text(t) => assert_eq!(t.text, "extra context"),
            other => panic!("Expected Text block, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_stop_should_continue_sets_meta() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::Stop],
            None,
            ShouldContinueHook {
                reason: "tests not passing".into(),
            },
        );

        let response = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        let meta = response.meta.as_ref().unwrap();
        assert_eq!(
            meta.get("hook_should_continue"),
            Some(&serde_json::Value::Bool(true))
        );
        assert_eq!(
            meta.get("hook_reason"),
            Some(&serde_json::Value::String("tests not passing".into()))
        );
    }

    #[tokio::test]
    async fn test_matcher_filters_pre_tool_use() {
        let called = Arc::new(AtomicBool::new(false));
        let hook = RecordingHook {
            called: called.clone(),
        };

        let event = HookEvent::PreToolUse {
            session_id: "s1".into(),
            tool_name: "Bash".into(),
            tool_input: None,
            tool_use_id: None,
            cwd: PathBuf::from("/tmp"),
        };

        let reg = HookRegistration::new(
            vec![HookEventKind::PreToolUse],
            crate::hook_config::Matcher::try_parse("Edit").unwrap(),
            Arc::new(hook),
        );

        assert!(!reg.matches(&event));
        assert!(!called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_matcher_skipped_for_user_prompt_submit() {
        let called = Arc::new(AtomicBool::new(false));
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::UserPromptSubmit],
            Some("nonexistent-pattern"),
            RecordingHook {
                called: called.clone(),
            },
        );

        let _ = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_matcher_skipped_for_stop() {
        let called = Arc::new(AtomicBool::new(false));
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::Stop],
            Some("nonexistent-pattern"),
            RecordingHook {
                called: called.clone(),
            },
        );

        let _ = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_session_start_fires_with_startup_source() {
        let source = Arc::new(std::sync::Mutex::new(None));
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::SessionStart],
            None,
            SourceRecordingHook {
                source: source.clone(),
            },
        );

        agent
            .track_session_start(
                "test-session".to_string(),
                SessionSource::Startup,
                PathBuf::from("/tmp"),
            )
            .await;
        assert_eq!(*source.lock().unwrap(), Some(SessionSource::Startup));
    }

    #[tokio::test]
    async fn test_session_start_fires_with_resume_source() {
        let source = Arc::new(std::sync::Mutex::new(None));
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::SessionStart],
            None,
            SourceRecordingHook {
                source: source.clone(),
            },
        );

        agent
            .track_session_start(
                "some-session".to_string(),
                SessionSource::Resume,
                PathBuf::from("/tmp"),
            )
            .await;
        assert_eq!(*source.lock().unwrap(), Some(SessionSource::Resume));
    }

    #[tokio::test]
    async fn test_multiple_hooks_all_fire() {
        let called1 = Arc::new(AtomicBool::new(false));
        let called2 = Arc::new(AtomicBool::new(false));
        let agent = HookableAgent::new(DummyInner)
            .with_hook(
                &[HookEventKind::UserPromptSubmit],
                None,
                RecordingHook {
                    called: called1.clone(),
                },
            )
            .with_hook(
                &[HookEventKind::UserPromptSubmit],
                None,
                RecordingHook {
                    called: called2.clone(),
                },
            );

        let _ = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert!(called1.load(Ordering::SeqCst));
        assert!(called2.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_block_takes_priority_over_context() {
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();
        let agent = HookableAgent::new(DummyInner)
            .with_hook(
                &[HookEventKind::UserPromptSubmit],
                None,
                ContextHook {
                    context: "extra".into(),
                },
            )
            .with_hook(
                &[HookEventKind::UserPromptSubmit],
                None,
                BlockHook {
                    reason: "blocked".into(),
                },
            );

        let result = run_prompt_turn(&agent, make_prompt_request(), |_req| {
            let called = called_clone.clone();
            async move {
                called.store(true, Ordering::SeqCst);
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_cwd_tracked_from_track_session_start() {
        let agent = HookableAgent::new(DummyInner);

        agent
            .track_session_start(
                "test-session".to_string(),
                SessionSource::Startup,
                PathBuf::from("/my/project"),
            )
            .await;

        let cwd = agent.get_cwd("test-session");
        assert_eq!(cwd, PathBuf::from("/my/project"));
    }

    #[tokio::test]
    async fn test_pre_tool_use_event_provides_complete_context_for_hooks() {
        let event = HookEvent::PreToolUse {
            session_id: "sess-1".into(),
            tool_name: "Bash".into(),
            tool_input: Some(serde_json::json!({"command": "npm test"})),
            tool_use_id: Some("toolu_123".into()),
            cwd: PathBuf::from("/project"),
        };

        let json = event.to_command_input();
        assert_eq!(json["hook_event_name"], "PreToolUse");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_input"]["command"], "npm test");
        assert_eq!(json["session_id"], "sess-1");
        assert_eq!(json["cwd"], "/project");
        assert_eq!(json["tool_use_id"], "toolu_123");
    }

    // -- AVP compatibility tests --

    #[tokio::test]
    async fn test_avp_context_fields_are_included_when_configured() {
        let event = HookEvent::SessionStart {
            session_id: "sess-1".into(),
            source: SessionSource::Startup,
            cwd: PathBuf::from("/project"),
        };

        let ctx = HookCommandContext {
            transcript_path: "/tmp/transcript.jsonl".into(),
            permission_mode: "default".into(),
        };

        let json = event.to_command_input_full(&ctx);
        assert_eq!(json["transcript_path"], "/tmp/transcript.jsonl");
        assert_eq!(json["permission_mode"], "default");
        assert_eq!(json["hook_event_name"], "SessionStart");
    }

    #[tokio::test]
    async fn test_hooks_can_correlate_post_tool_use_with_originating_call() {
        let event = HookEvent::PostToolUse {
            session_id: "s1".into(),
            tool_name: "Write".into(),
            tool_input: None,
            tool_response: Some(serde_json::json!({"success": true})),
            tool_use_id: Some("toolu_456".into()),
            cwd: PathBuf::from("/tmp"),
        };

        let json = event.to_command_input();
        assert_eq!(json["tool_use_id"], "toolu_456");
        assert_eq!(json["hook_event_name"], "PostToolUse");
    }

    #[tokio::test]
    async fn test_stop_hooks_know_they_are_not_recursive_on_first_invocation() {
        let recorded_active = Arc::new(std::sync::Mutex::new(None));
        let recorded_clone = recorded_active.clone();

        struct StopActiveRecorder {
            recorded: Arc<std::sync::Mutex<Option<bool>>>,
        }

        #[async_trait::async_trait]
        impl HookHandler for StopActiveRecorder {
            async fn handle(&self, event: &HookEvent) -> HookDecision {
                if let HookEvent::Stop {
                    stop_hook_active, ..
                } = event
                {
                    *self.recorded.lock().unwrap() = Some(*stop_hook_active);
                }
                HookDecision::Allow
            }
        }

        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::Stop],
            None,
            StopActiveRecorder {
                recorded: recorded_clone,
            },
        );

        let _ = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert_eq!(*recorded_active.lock().unwrap(), Some(false));
    }

    #[tokio::test]
    async fn test_stop_event_exposes_recursion_guard_to_hooks() {
        let event = HookEvent::Stop {
            session_id: "s1".into(),
            stop_reason: StopReason::EndTurn,
            stop_hook_active: false,
            cwd: PathBuf::from("/tmp"),
        };

        let json = event.to_command_input();
        assert_eq!(json["stop_hook_active"], false);
        assert_eq!(json["hook_event_name"], "Stop");
    }

    #[tokio::test]
    async fn test_failed_tool_calls_provide_error_details_to_hooks() {
        let event = HookEvent::PostToolUseFailure {
            session_id: "s1".into(),
            tool_name: "Bash".into(),
            tool_input: Some(serde_json::json!({"command": "false"})),
            error: Some(serde_json::json!("exit code 1")),
            tool_use_id: Some("toolu_789".into()),
            cwd: PathBuf::from("/tmp"),
        };

        let json = event.to_command_input();
        assert_eq!(json["hook_event_name"], "PostToolUseFailure");
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["error"], "exit code 1");
        assert_eq!(json["tool_use_id"], "toolu_789");
    }

    #[tokio::test]
    async fn test_post_tool_use_failure_matcher_matches_tool_name() {
        let event = HookEvent::PostToolUseFailure {
            session_id: "s1".into(),
            tool_name: "Bash".into(),
            tool_input: None,
            error: None,
            tool_use_id: None,
            cwd: PathBuf::from("/tmp"),
        };

        assert_eq!(event.kind(), HookEventKind::PostToolUseFailure);
        assert_eq!(event.matcher_value(), Some("Bash"));
    }

    // -- Decision routing intent tests --

    /// A hook returning AllowWithUpdatedInput should not prevent prompt
    /// execution (updatedInput is a PreToolUse concern, not a blocking
    /// signal).
    #[tokio::test]
    async fn test_updated_input_decision_does_not_block_prompt() {
        struct UpdatedInputHook;

        #[async_trait::async_trait]
        impl HookHandler for UpdatedInputHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::AllowWithUpdatedInput {
                    updated_input: serde_json::json!({"modified": true}),
                }
            }
        }

        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            UpdatedInputHook,
        );
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let result = run_prompt_turn(&agent, make_prompt_request(), |_req| {
            let called = called_clone.clone();
            async move {
                called.store(true, Ordering::SeqCst);
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await;
        assert!(result.is_ok());
        assert!(called.load(Ordering::SeqCst));
    }

    /// Only ShouldContinue decisions annotate the response — other decision
    /// types like AllowWithContext or AllowWithUpdatedInput do not add
    /// hook_should_continue metadata after the stop phase.
    #[tokio::test]
    async fn test_only_should_continue_annotates_stop_response() {
        struct ContextOnStopHook;

        #[async_trait::async_trait]
        impl HookHandler for ContextOnStopHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::AllowWithContext {
                    context: "extra info".into(),
                }
            }
        }

        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::Stop],
            None,
            ContextOnStopHook,
        );

        let response = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert!(
            response.meta.is_none()
                || !response
                    .meta
                    .as_ref()
                    .unwrap()
                    .contains_key("hook_should_continue")
        );
    }

    // -- Notification JSON enrichment tests --

    #[test]
    fn test_notification_event_exposes_agent_message_text_to_hooks() {
        let content = ContentChunk::new(ContentBlock::Text(TextContent::new("hello world")));
        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(content),
        );
        let event = HookEvent::Notification {
            notification: Box::new(notification),
            cwd: PathBuf::from("/project"),
        };

        let json = event.to_command_input();
        let notification_data = &json["notification"];
        let serialized = serde_json::to_string(notification_data).unwrap();
        assert!(
            serialized.contains("hello world"),
            "Expected notification data to contain the message text, got: {}",
            serialized
        );
    }

    #[test]
    fn test_notification_matcher_filters_by_update_type() {
        let content = ContentChunk::new(ContentBlock::Text(TextContent::new("hi")));
        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(content),
        );
        let event = HookEvent::Notification {
            notification: Box::new(notification),
            cwd: PathBuf::from("/tmp"),
        };

        let matching_reg = HookRegistration::new(
            vec![HookEventKind::Notification],
            crate::hook_config::Matcher::try_parse("agent_message").unwrap(),
            Arc::new(RecordingHook {
                called: Arc::new(AtomicBool::new(false)),
            }),
        );
        assert!(matching_reg.matches(&event));

        let non_matching_reg = HookRegistration::new(
            vec![HookEventKind::Notification],
            crate::hook_config::Matcher::try_parse("^tool_call$").unwrap(),
            Arc::new(RecordingHook {
                called: Arc::new(AtomicBool::new(false)),
            }),
        );
        assert!(!non_matching_reg.matches(&event));
    }

    #[tokio::test]
    async fn test_notification_hook_context_forwarded_via_channel() {
        // A `Notification`-family hook returning AllowWithContext still forwards
        // its context through the channel. (Tool-call hooks no longer fire here;
        // they fire at the dispatch seam — so this exercises the surviving
        // Notification path.)
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::Notification],
            None,
            ContextHook {
                context: "lint warning: unused variable".into(),
            },
        );

        let (notify_tx, notify_rx) = broadcast::channel(16);
        let (_, _cancel_rx, mut context_rx) = agent.intercept_notifications(notify_rx);

        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(ToolCall::new("call-1", "Bash")),
        ));
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let ctx = context_rx.try_recv();
        assert!(
            ctx.is_ok(),
            "Expected context from notification hook, got nothing"
        );
        assert_eq!(ctx.unwrap(), "lint warning: unused variable");
    }

    /// A Cancel decision on UserPromptSubmit prevents the inner agent from running.
    #[tokio::test]
    async fn test_cancel_decision_prevents_prompt_execution() {
        struct CancelHook;

        #[async_trait::async_trait]
        impl HookHandler for CancelHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::Cancel {
                    reason: "user cancelled".into(),
                }
            }
        }

        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            CancelHook,
        );
        let called = Arc::new(AtomicBool::new(false));
        let called_clone = called.clone();

        let result = run_prompt_turn(&agent, make_prompt_request(), |_req| {
            let called = called_clone.clone();
            async move {
                called.store(true, Ordering::SeqCst);
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
        })
        .await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("Cancelled"));
    }

    #[tokio::test]
    async fn test_fire_event_runs_matching_hooks() {
        let fired = Arc::new(std::sync::atomic::AtomicU32::new(0));

        struct CountHook(Arc<std::sync::atomic::AtomicU32>);

        #[async_trait::async_trait]
        impl HookHandler for CountHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                self.0.fetch_add(1, Ordering::SeqCst);
                HookDecision::Allow
            }
        }

        let agent = HookableAgent::new(DummyInner)
            .with_hook(
                &[HookEventKind::TeammateIdle],
                None,
                CountHook(fired.clone()),
            )
            .with_hook(
                &[HookEventKind::TaskCompleted],
                None,
                CountHook(fired.clone()),
            );

        let event = HookEvent::TeammateIdle {
            session_id: "s1".into(),
            teammate_id: None,
            cwd: PathBuf::from("/tmp"),
        };
        let decisions = agent.fire_event(&event).await;
        assert_eq!(decisions.len(), 1);
        assert_eq!(fired.load(Ordering::SeqCst), 1);

        let event = HookEvent::TaskCompleted {
            session_id: "s1".into(),
            task_id: None,
            task_title: None,
            cwd: PathBuf::from("/tmp"),
        };
        let decisions = agent.fire_event(&event).await;
        assert_eq!(decisions.len(), 1);
        assert_eq!(fired.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn test_fire_event_returns_block_decision() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PostCompact],
            None,
            BlockHook {
                reason: "blocked by test".into(),
            },
        );

        let event = HookEvent::PostCompact {
            session_id: "s1".into(),
            cwd: PathBuf::from("/tmp"),
        };
        let decisions = agent.fire_event(&event).await;
        assert_eq!(decisions.len(), 1);
        assert!(matches!(decisions[0], HookDecision::Block { .. }));
    }

    // -- HookableAgent builder tests --

    #[tokio::test]
    async fn test_hookable_agent_with_transcript_path() {
        let agent = HookableAgent::new(DummyInner).with_transcript_path("/tmp/transcript.jsonl");
        assert_eq!(
            agent.command_context().transcript_path,
            "/tmp/transcript.jsonl"
        );
    }

    #[tokio::test]
    async fn test_hookable_agent_with_permission_mode() {
        let agent = HookableAgent::new(DummyInner).with_permission_mode("default");
        assert_eq!(agent.command_context().permission_mode, "default");
    }

    #[tokio::test]
    async fn test_hookable_agent_with_registration() {
        let called = Arc::new(AtomicBool::new(false));
        let reg = HookRegistration::new(
            vec![HookEventKind::UserPromptSubmit],
            crate::hook_config::Matcher::All,
            Arc::new(RecordingHook {
                called: called.clone(),
            }),
        );
        let agent = HookableAgent::new(DummyInner).with_registration(reg);

        let _ = run_prompt_turn(&agent, make_prompt_request(), |_req| async {
            Ok(PromptResponse::new(StopReason::EndTurn))
        })
        .await
        .unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_hookable_agent_inner_returns_inner_agent() {
        let agent = HookableAgent::new(DummyInner);
        let _: &DummyInner = agent.inner();
    }

    #[tokio::test]
    async fn test_hookable_agent_into_inner_returns_wrapped_value() {
        struct WrappedInner(u32);
        let agent = HookableAgent::new(WrappedInner(42));
        let inner = agent.into_inner();
        assert_eq!(inner.0, 42);
    }

    #[tokio::test]
    async fn test_hookable_agent_get_cwd_unknown_session_fallback() {
        let agent = HookableAgent::new(DummyInner);

        let cwd = agent.get_cwd("nonexistent-session");
        assert_eq!(cwd, PathBuf::from("."));
    }

    // -- notification_to_events tests --
    //
    // The notification path is now `Notification`-only: tool-call notifications
    // no longer derive `PreToolUse`/`PostToolUse`/`PostToolUseFailure` events.
    // Those fire synchronously at the dispatch seam (`run_pre_tool_use` etc.),
    // so deriving them here would double-fire every tool hook.

    #[test]
    fn test_notification_to_events_tool_call_emits_only_notification() {
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(ToolCall::new("c1", "Bash")),
        );
        let events = notification_to_events(&notification, &PathBuf::from("/tmp"));

        // A ToolCall notification fires the UI `Notification` hook only — NOT a
        // PreToolUse (that path moved to the dispatch seam to avoid double-fire).
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HookEvent::Notification { .. }));
    }

    #[test]
    fn test_notification_to_events_tool_call_update_emits_only_notification() {
        let update = ToolCallUpdate::new(
            "c1",
            ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
        );
        let notification =
            SessionNotification::new(SessionId::from("s1"), SessionUpdate::ToolCallUpdate(update));

        let events = notification_to_events(&notification, &PathBuf::from("/tmp"));

        // A ToolCallUpdate fires the UI `Notification` hook only — NOT a
        // PostToolUse/PostToolUseFailure (those moved to the dispatch seam).
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HookEvent::Notification { .. }));
    }

    #[test]
    fn test_notification_to_events_non_tool_notification() {
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hi"),
            ))),
        );
        let events = notification_to_events(&notification, &PathBuf::from("/tmp"));

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HookEvent::Notification { .. }));
    }

    /// A `PreToolUse` hook registered on the wrapper does NOT fire from the
    /// notification stream — the notification path stays `Notification`-only so
    /// the dispatch-seam firing is the sole source. This is the no-double-firing
    /// guard at the notification boundary.
    #[tokio::test]
    async fn test_pre_tool_use_hook_does_not_fire_from_notifications() {
        let called = Arc::new(AtomicBool::new(false));
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            None,
            RecordingHook {
                called: called.clone(),
            },
        );

        let (notify_tx, notify_rx) = broadcast::channel(16);
        let (_rx, _cancel_rx, _context_rx) = agent.intercept_notifications(notify_rx);

        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(ToolCall::new("c1", "Bash")),
        ));
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        assert!(
            !called.load(Ordering::SeqCst),
            "a PreToolUse hook must not fire from the notification stream — \
             tool hooks fire only at the dispatch seam"
        );
    }

    // -- dispatch_notification_hooks tests --

    #[tokio::test]
    async fn test_dispatch_notification_hooks_allow_with_updated_input() {
        struct UpdatedInputHook;

        #[async_trait::async_trait]
        impl HookHandler for UpdatedInputHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::AllowWithUpdatedInput {
                    updated_input: serde_json::json!({"modified": true}),
                }
            }
        }

        let hooks = vec![HookRegistration::new(
            vec![HookEventKind::Notification],
            crate::hook_config::Matcher::All,
            Arc::new(UpdatedInputHook),
        )];

        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hi"),
            ))),
        );

        let events = vec![HookEvent::Notification {
            notification: Box::new(notification.clone()),
            cwd: PathBuf::from("/tmp"),
        }];

        let (cancel_tx, _cancel_rx) = tokio::sync::mpsc::unbounded_channel();
        let (context_tx, _context_rx) = tokio::sync::mpsc::unbounded_channel();

        dispatch_notification_hooks(&hooks, &events, &notification, &cancel_tx, &context_tx).await;
    }

    #[tokio::test]
    async fn test_dispatch_notification_hooks_block_decision_is_noop() {
        let hooks = vec![HookRegistration::new(
            vec![HookEventKind::Notification],
            crate::hook_config::Matcher::All,
            Arc::new(BlockHook {
                reason: "blocked".into(),
            }),
        )];

        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hi"),
            ))),
        );

        let events = vec![HookEvent::Notification {
            notification: Box::new(notification.clone()),
            cwd: PathBuf::from("/tmp"),
        }];

        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::unbounded_channel();
        let (context_tx, _context_rx) = tokio::sync::mpsc::unbounded_channel();

        dispatch_notification_hooks(&hooks, &events, &notification, &cancel_tx, &context_tx).await;

        assert!(cancel_rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn test_dispatch_notification_hooks_should_continue_is_noop() {
        let hooks = vec![HookRegistration::new(
            vec![HookEventKind::Notification],
            crate::hook_config::Matcher::All,
            Arc::new(ShouldContinueHook {
                reason: "keep going".into(),
            }),
        )];

        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hi"),
            ))),
        );

        let events = vec![HookEvent::Notification {
            notification: Box::new(notification.clone()),
            cwd: PathBuf::from("/tmp"),
        }];

        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::unbounded_channel();
        let (context_tx, mut context_rx) = tokio::sync::mpsc::unbounded_channel();

        dispatch_notification_hooks(&hooks, &events, &notification, &cancel_tx, &context_tx).await;

        assert!(cancel_rx.try_recv().is_err());
        assert!(context_rx.try_recv().is_err());
    }

    // -- Spot-check: HookRegistration / HookDecision sanity (deeper coverage
    //    lives in `hook_config` tests).

    // -- PreToolUse / PostToolUse synchronous-seam helper tests --

    /// A `deny` (Block) decision yields a `Deny` outcome carrying the reason,
    /// so the dispatch seam can skip execution and feed the reason back.
    #[tokio::test]
    async fn test_run_pre_tool_use_block_yields_deny() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            Some("Bash"),
            BlockHook {
                reason: "no shell".into(),
            },
        );
        let outcome = agent
            .run_pre_tool_use(
                "s1",
                "Bash",
                Some(serde_json::json!({"command": "ls"})),
                Some("t1"),
            )
            .await;
        match outcome {
            PreToolUseOutcome::Deny { reason } => assert_eq!(reason, "no shell"),
            other => panic!("expected Deny, got {other:?}"),
        }
    }

    /// A non-matching matcher leaves the call to proceed unchanged.
    #[tokio::test]
    async fn test_run_pre_tool_use_non_matching_proceeds() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            Some("Edit"),
            BlockHook {
                reason: "blocked".into(),
            },
        );
        let outcome = agent.run_pre_tool_use("s1", "Bash", None, None).await;
        assert!(matches!(outcome, PreToolUseOutcome::Proceed { .. }));
    }

    /// `updatedInput` is surfaced so the seam can rewrite the tool arguments
    /// before dispatch.
    #[tokio::test]
    async fn test_run_pre_tool_use_updated_input_rewrites_args() {
        struct UpdatedInputHook;
        #[async_trait::async_trait]
        impl HookHandler for UpdatedInputHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::AllowWithUpdatedInput {
                    updated_input: serde_json::json!({"command": "safe"}),
                }
            }
        }
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            None,
            UpdatedInputHook,
        );
        let outcome = agent
            .run_pre_tool_use(
                "s1",
                "Bash",
                Some(serde_json::json!({"command": "rm"})),
                None,
            )
            .await;
        match outcome {
            PreToolUseOutcome::Proceed {
                updated_input,
                context,
            } => {
                assert_eq!(updated_input, Some(serde_json::json!({"command": "safe"})));
                assert!(context.is_none());
            }
            other => panic!("expected Proceed, got {other:?}"),
        }
    }

    /// `additionalContext` is surfaced so the seam can inject it alongside the
    /// tool result.
    #[tokio::test]
    async fn test_run_pre_tool_use_context_is_surfaced() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            None,
            ContextHook {
                context: "be careful".into(),
            },
        );
        let outcome = agent.run_pre_tool_use("s1", "Bash", None, None).await;
        match outcome {
            PreToolUseOutcome::Proceed {
                context,
                updated_input,
            } => {
                assert_eq!(context.as_deref(), Some("be careful"));
                assert!(updated_input.is_none());
            }
            other => panic!("expected Proceed, got {other:?}"),
        }
    }

    /// `Block` wins over a context decision from another hook on the same call.
    #[tokio::test]
    async fn test_run_pre_tool_use_block_wins_over_context() {
        let agent = HookableAgent::new(DummyInner)
            .with_hook(
                &[HookEventKind::PreToolUse],
                None,
                ContextHook {
                    context: "fyi".into(),
                },
            )
            .with_hook(
                &[HookEventKind::PreToolUse],
                None,
                BlockHook {
                    reason: "denied".into(),
                },
            );
        let outcome = agent.run_pre_tool_use("s1", "Bash", None, None).await;
        assert!(matches!(outcome, PreToolUseOutcome::Deny { .. }));
    }

    /// A `Cancel` (continue:false) decision yields a `StopTurn` outcome.
    #[tokio::test]
    async fn test_run_pre_tool_use_cancel_yields_stop_turn() {
        struct CancelHook;
        #[async_trait::async_trait]
        impl HookHandler for CancelHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                HookDecision::Cancel {
                    reason: "halt".into(),
                }
            }
        }
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            None,
            CancelHook,
        );
        let outcome = agent.run_pre_tool_use("s1", "Bash", None, None).await;
        match outcome {
            PreToolUseOutcome::StopTurn { reason } => assert_eq!(reason, "halt"),
            other => panic!("expected StopTurn, got {other:?}"),
        }
    }

    /// `PostToolUse` surfaces additionalContext to feed back after success.
    #[tokio::test]
    async fn test_run_post_tool_use_surfaces_context() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PostToolUse],
            None,
            ContextHook {
                context: "lint clean".into(),
            },
        );
        let ctx = agent
            .run_post_tool_use("s1", "Bash", None, Some(serde_json::json!("ok")), None)
            .await;
        assert_eq!(ctx.as_deref(), Some("lint clean"));
    }

    /// `PostToolUseFailure` surfaces additionalContext to feed back after error.
    #[tokio::test]
    async fn test_run_post_tool_use_failure_surfaces_context() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PostToolUseFailure],
            None,
            ContextHook {
                context: "see stderr".into(),
            },
        );
        let ctx = agent
            .run_post_tool_use_failure("s1", "Bash", None, Some(serde_json::json!("exit 2")), None)
            .await;
        assert_eq!(ctx.as_deref(), Some("see stderr"));
    }

    /// The synchronous PreToolUse helper fires a matching hook exactly once.
    #[tokio::test]
    async fn test_run_pre_tool_use_fires_once() {
        let count = Arc::new(std::sync::atomic::AtomicU32::new(0));
        struct CountHook(Arc<std::sync::atomic::AtomicU32>);
        #[async_trait::async_trait]
        impl HookHandler for CountHook {
            async fn handle(&self, _event: &HookEvent) -> HookDecision {
                self.0.fetch_add(1, Ordering::SeqCst);
                HookDecision::Allow
            }
        }
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PreToolUse],
            None,
            CountHook(count.clone()),
        );
        let _ = agent.run_pre_tool_use("s1", "Bash", None, None).await;
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn test_hook_decision_default_is_allow() {
        let decision: HookDecision = HookDecision::default();
        assert!(matches!(decision, HookDecision::Allow));
    }

    #[test]
    fn test_session_source_as_str() {
        assert_eq!(SessionSource::Startup.as_str(), "startup");
        assert_eq!(SessionSource::Resume.as_str(), "resume");
    }
}
