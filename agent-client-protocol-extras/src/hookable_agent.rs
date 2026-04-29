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
    ContentBlock, PromptRequest, PromptResponse, SessionNotification, SessionUpdate, TextContent,
    ToolCallStatus,
};
use agent_client_protocol::{Channel, Client, ConnectTo, Result as AcpResult};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

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
    /// regex matcher pattern.
    ///
    /// # Panics
    /// If `matcher` is `Some(pat)` and `pat` is not a valid regular
    /// expression.
    pub fn with_hook(
        mut self,
        events: &[HookEventKind],
        matcher: Option<&str>,
        handler: impl crate::hook_config::HookHandler + 'static,
    ) -> Self {
        let matcher = matcher.map(|pat| {
            regex::Regex::new(pat).unwrap_or_else(|e| panic!("invalid hook matcher regex: {e}"))
        });
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
    pub async fn run_user_prompt_submit(
        &self,
        request: PromptRequest,
    ) -> AcpResult<PromptRequest> {
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
    pub async fn run_stop(
        &self,
        session_id: String,
        response: PromptResponse,
    ) -> PromptResponse {
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
            let mut tool_names = HashMap::new();
            run_notification_loop(
                &mut recv,
                &hooks,
                &session_cwd,
                &tx,
                &cancel_tx,
                &context_tx,
                &mut tool_names,
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
    tool_names: &mut HashMap<String, (String, Option<serde_json::Value>)>,
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
                let events = notification_to_events(&notification, &cwd, tool_names);
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
/// Always produces a `Notification` event; for tool-call related updates,
/// also produces a `PreToolUse`, `PostToolUse`, or `PostToolUseFailure`
/// event ahead of it.
///
/// Tracks tool-call ids in `tool_names` so subsequent `ToolCallUpdate`s
/// can be correlated back to the originating `ToolCall`'s name and input.
fn notification_to_events(
    notification: &SessionNotification,
    cwd: &Path,
    tool_names: &mut HashMap<String, (String, Option<serde_json::Value>)>,
) -> Vec<HookEvent> {
    let session_id = notification.session_id.to_string();
    let mut events = vec![];

    match &notification.update {
        SessionUpdate::ToolCall(tool_call) => {
            let id = tool_call.tool_call_id.0.to_string();
            tool_names.insert(
                id.clone(),
                (tool_call.title.clone(), tool_call.raw_input.clone()),
            );
            events.push(HookEvent::PreToolUse {
                session_id: session_id.clone(),
                tool_name: tool_call.title.clone(),
                tool_input: tool_call.raw_input.clone(),
                tool_use_id: Some(id),
                cwd: cwd.to_path_buf(),
            });
        }
        SessionUpdate::ToolCallUpdate(update) => {
            let id = update.tool_call_id.0.to_string();
            let (name, input) = tool_names.get(&id).cloned().unwrap_or((id.clone(), None));
            if update.fields.status == Some(ToolCallStatus::Failed) {
                events.push(HookEvent::PostToolUseFailure {
                    session_id: session_id.clone(),
                    tool_name: name,
                    tool_input: input,
                    error: update.fields.raw_output.clone(),
                    tool_use_id: Some(id),
                    cwd: cwd.to_path_buf(),
                });
            } else {
                events.push(HookEvent::PostToolUse {
                    session_id: session_id.clone(),
                    tool_name: name,
                    tool_input: input,
                    tool_response: update.fields.raw_output.clone(),
                    tool_use_id: Some(id),
                    cwd: cwd.to_path_buf(),
                });
            }
        }
        _ => {}
    }

    events.push(HookEvent::Notification {
        notification: Box::new(notification.clone()),
        cwd: cwd.to_path_buf(),
    });

    events
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hook_config::HookHandler;
    use agent_client_protocol::schema::{
        ContentChunk, SessionId, StopReason, ToolCall, ToolCallUpdate, ToolCallUpdateFields,
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
            Some(regex::Regex::new("Edit").unwrap()),
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
            Some(regex::Regex::new("agent_message").unwrap()),
            Arc::new(RecordingHook {
                called: Arc::new(AtomicBool::new(false)),
            }),
        );
        assert!(matching_reg.matches(&event));

        let non_matching_reg = HookRegistration::new(
            vec![HookEventKind::Notification],
            Some(regex::Regex::new("^tool_call$").unwrap()),
            Arc::new(RecordingHook {
                called: Arc::new(AtomicBool::new(false)),
            }),
        );
        assert!(!non_matching_reg.matches(&event));
    }

    #[tokio::test]
    async fn test_notification_hook_context_forwarded_via_channel() {
        let agent = HookableAgent::new(DummyInner).with_hook(
            &[HookEventKind::PostToolUse],
            None,
            ContextHook {
                context: "lint warning: unused variable".into(),
            },
        );

        let (notify_tx, notify_rx) = broadcast::channel(16);
        let (_, _cancel_rx, mut context_rx) = agent.intercept_notifications(notify_rx);

        let tool_call = ToolCall::new("call-1", "Bash");
        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(tool_call),
        ));
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let update = ToolCallUpdate::new(
            "call-1",
            ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
        );
        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCallUpdate(update),
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

        let agent =
            HookableAgent::new(DummyInner).with_hook(&[HookEventKind::UserPromptSubmit], None, CancelHook);
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
        let agent =
            HookableAgent::new(DummyInner).with_transcript_path("/tmp/transcript.jsonl");
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
            None,
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

    #[test]
    fn test_notification_to_events_tool_call() {
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(ToolCall::new("c1", "Bash")),
        );
        let mut tool_names = HashMap::new();
        let events = notification_to_events(&notification, &PathBuf::from("/tmp"), &mut tool_names);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], HookEvent::PreToolUse { .. }));
        assert!(matches!(events[1], HookEvent::Notification { .. }));
        assert!(tool_names.contains_key("c1"));
    }

    #[test]
    fn test_notification_to_events_tool_call_update_success() {
        let mut tool_names = HashMap::new();
        tool_names.insert(
            "c1".to_string(),
            ("Bash".to_string(), Some(serde_json::json!({"cmd": "ls"}))),
        );

        let update = ToolCallUpdate::new(
            "c1",
            ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
        );
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCallUpdate(update),
        );

        let events = notification_to_events(&notification, &PathBuf::from("/tmp"), &mut tool_names);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], HookEvent::PostToolUse { .. }));
    }

    #[test]
    fn test_notification_to_events_tool_call_update_failure() {
        let mut tool_names = HashMap::new();
        tool_names.insert("c1".to_string(), ("Bash".to_string(), None));

        let update = ToolCallUpdate::new(
            "c1",
            ToolCallUpdateFields::new().status(ToolCallStatus::Failed),
        );
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCallUpdate(update),
        );

        let events = notification_to_events(&notification, &PathBuf::from("/tmp"), &mut tool_names);

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], HookEvent::PostToolUseFailure { .. }));
    }

    #[test]
    fn test_notification_to_events_unknown_tool_call_id() {
        let mut tool_names = HashMap::new();

        let update = ToolCallUpdate::new(
            "unknown-id",
            ToolCallUpdateFields::new().status(ToolCallStatus::Completed),
        );
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCallUpdate(update),
        );

        let events = notification_to_events(&notification, &PathBuf::from("/tmp"), &mut tool_names);

        assert_eq!(events.len(), 2);
        match &events[0] {
            HookEvent::PostToolUse { tool_name, .. } => {
                assert_eq!(tool_name, "unknown-id");
            }
            other => panic!("Expected PostToolUse, got {:?}", other.kind()),
        }
    }

    #[test]
    fn test_notification_to_events_non_tool_notification() {
        let notification = SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::AgentMessageChunk(ContentChunk::new(ContentBlock::Text(
                TextContent::new("hi"),
            ))),
        );
        let mut tool_names = HashMap::new();
        let events = notification_to_events(&notification, &PathBuf::from("/tmp"), &mut tool_names);

        assert_eq!(events.len(), 1);
        assert!(matches!(events[0], HookEvent::Notification { .. }));
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
            None,
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
            None,
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
            None,
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
