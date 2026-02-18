//! HookableAgent - Proxy agent that fires hooks at ACP lifecycle points
//!
//! Matches Claude Code's hook event names and lifecycle model:
//! - `SessionStart` — fires on new_session() and load_session()
//! - `UserPromptSubmit` — fires before prompt()
//! - `PreToolUse` — fires on ToolCall notification
//! - `PostToolUse` — fires on ToolCallUpdate notification (success)
//! - `PostToolUseFailure` — fires on ToolCallUpdate with Failed status
//! - `Stop` — fires after prompt() returns
//! - `Notification` — fires on any SessionNotification
//!
//! Hook handlers return `HookDecision` values derived from their output
//! (command exit codes + JSON, prompt/agent evaluator responses).

use agent_client_protocol::{
    Agent, AuthenticateRequest, AuthenticateResponse, CancelNotification, ContentBlock,
    ExtNotification, ExtRequest, ExtResponse, InitializeRequest, InitializeResponse,
    LoadSessionRequest, LoadSessionResponse, NewSessionRequest, NewSessionResponse, PromptRequest,
    PromptResponse, SessionNotification, SessionUpdate, SetSessionModeRequest,
    SetSessionModeResponse, StopReason, TextContent, ToolCallStatus,
};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::broadcast;

// ---------------------------------------------------------------------------
// Hook command context (extra fields for AVP compatibility)
// ---------------------------------------------------------------------------

/// Extra context fields included in command hook JSON input.
///
/// These fields are required by AVP's `CommonInput` but not available
/// from ACP lifecycle events directly. Set via builder methods on
/// `HookableAgent` or passed through `build_registrations()`.
#[derive(Clone, Debug, Default)]
pub struct HookCommandContext {
    /// Path to conversation transcript file. Default: ""
    pub transcript_path: String,
    /// Permission mode string. Default: "bypassPermissions"
    pub permission_mode: String,
}

// ---------------------------------------------------------------------------
// Session source (type-safe replacement for stringly-typed field)
// ---------------------------------------------------------------------------

/// How a session was started — distinguishes new vs resumed sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SessionSource {
    /// New session created via `new_session()`.
    Startup,
    /// Existing session resumed via `load_session()`.
    Resume,
}

impl SessionSource {
    /// String representation matching Claude Code's JSON format.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Startup => "startup",
            Self::Resume => "resume",
        }
    }
}

// ---------------------------------------------------------------------------
// Hook event types (matching Claude Code naming)
// ---------------------------------------------------------------------------

/// Lifecycle events that hooks can respond to.
#[derive(Clone, Debug)]
pub enum HookEvent {
    /// Fires after new_session() or load_session().
    SessionStart {
        session_id: String,
        source: SessionSource,
        cwd: PathBuf,
    },
    /// Fires before prompt() delegates to inner agent.
    UserPromptSubmit {
        session_id: String,
        prompt: Vec<ContentBlock>,
        cwd: PathBuf,
    },
    /// Fires on ToolCall notification (before tool execution).
    PreToolUse {
        session_id: String,
        tool_name: String,
        tool_input: Option<serde_json::Value>,
        tool_use_id: Option<String>,
        cwd: PathBuf,
    },
    /// Fires on ToolCallUpdate notification (after successful tool execution).
    PostToolUse {
        session_id: String,
        tool_name: String,
        tool_input: Option<serde_json::Value>,
        tool_response: Option<serde_json::Value>,
        tool_use_id: Option<String>,
        cwd: PathBuf,
    },
    /// Fires on ToolCallUpdate when tool status is Failed.
    PostToolUseFailure {
        session_id: String,
        tool_name: String,
        tool_input: Option<serde_json::Value>,
        error: Option<serde_json::Value>,
        tool_use_id: Option<String>,
        cwd: PathBuf,
    },
    /// Fires after prompt() returns.
    Stop {
        session_id: String,
        stop_reason: StopReason,
        stop_hook_active: bool,
        cwd: PathBuf,
    },
    /// Fires on any SessionNotification.
    Notification {
        notification: Box<SessionNotification>,
        cwd: PathBuf,
    },
}

/// Which category of event a hook registration matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookEventKind {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    Notification,
}

impl HookEvent {
    /// The kind of this event.
    pub fn kind(&self) -> HookEventKind {
        match self {
            Self::SessionStart { .. } => HookEventKind::SessionStart,
            Self::UserPromptSubmit { .. } => HookEventKind::UserPromptSubmit,
            Self::PreToolUse { .. } => HookEventKind::PreToolUse,
            Self::PostToolUse { .. } => HookEventKind::PostToolUse,
            Self::PostToolUseFailure { .. } => HookEventKind::PostToolUseFailure,
            Self::Stop { .. } => HookEventKind::Stop,
            Self::Notification { .. } => HookEventKind::Notification,
        }
    }

    /// The string value that matchers test against.
    ///
    /// Returns `None` for events that don't support matchers
    /// (UserPromptSubmit, Stop) — these always fire.
    pub fn matcher_value(&self) -> Option<&str> {
        match self {
            Self::SessionStart { source, .. } => Some(source.as_str()),
            Self::UserPromptSubmit { .. } | Self::Stop { .. } => None,
            Self::PreToolUse { tool_name, .. }
            | Self::PostToolUse { tool_name, .. }
            | Self::PostToolUseFailure { tool_name, .. } => Some(tool_name.as_str()),
            Self::Notification { notification, .. } => {
                Some(notification_update_name(&notification.update))
            }
        }
    }

    /// Serialize this event as Claude-compatible JSON for command hook stdin.
    pub fn to_command_input(&self) -> serde_json::Value {
        self.to_command_input_full(&HookCommandContext::default())
    }

    /// Serialize this event with extra context fields for AVP compatibility.
    pub fn to_command_input_full(&self, ctx: &HookCommandContext) -> serde_json::Value {
        let mut obj = self.to_base_json();
        append_avp_context(&mut obj, ctx);
        obj
    }

    /// Build per-variant JSON without AVP context fields.
    fn to_base_json(&self) -> serde_json::Value {
        match self {
            Self::SessionStart {
                session_id,
                source,
                cwd,
            } => serde_json::json!({
                "session_id": session_id,
                "cwd": cwd.display().to_string(),
                "hook_event_name": "SessionStart",
                "source": source.as_str(),
            }),
            Self::UserPromptSubmit {
                session_id,
                prompt,
                cwd,
            } => serde_json::json!({
                "session_id": session_id,
                "cwd": cwd.display().to_string(),
                "hook_event_name": "UserPromptSubmit",
                "prompt": extract_prompt_text(prompt),
            }),
            Self::PreToolUse {
                session_id,
                tool_name,
                tool_input,
                tool_use_id,
                cwd,
            } => tool_event_json(
                "PreToolUse",
                session_id,
                tool_name,
                cwd,
                tool_input,
                tool_use_id,
                &None,
            ),
            Self::PostToolUse {
                session_id,
                tool_name,
                tool_input,
                tool_response,
                tool_use_id,
                cwd,
            } => tool_event_json(
                "PostToolUse",
                session_id,
                tool_name,
                cwd,
                tool_input,
                tool_use_id,
                tool_response,
            ),
            Self::PostToolUseFailure {
                session_id,
                tool_name,
                tool_input,
                error,
                tool_use_id,
                cwd,
            } => {
                let mut o = tool_event_json(
                    "PostToolUseFailure",
                    session_id,
                    tool_name,
                    cwd,
                    tool_input,
                    tool_use_id,
                    &None,
                );
                if let Some(err) = error {
                    o["error"] = err.clone();
                }
                o
            }
            Self::Stop {
                session_id,
                stop_reason,
                stop_hook_active,
                cwd,
            } => serde_json::json!({
                "session_id": session_id,
                "cwd": cwd.display().to_string(),
                "hook_event_name": "Stop",
                "stop_reason": format!("{:?}", stop_reason),
                "stop_hook_active": stop_hook_active,
            }),
            Self::Notification {
                notification, cwd, ..
            } => {
                let mut obj = serde_json::json!({
                    "session_id": notification.session_id.to_string(),
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "Notification",
                    "notification_type": notification_update_name(&notification.update),
                });
                if let Ok(update_value) = serde_json::to_value(&notification.update) {
                    obj["notification"] = update_value;
                }
                obj
            }
        }
    }
}

/// Build JSON for tool-related events (PreToolUse, PostToolUse, PostToolUseFailure).
fn tool_event_json(
    event_name: &str,
    session_id: &str,
    tool_name: &str,
    cwd: &Path,
    tool_input: &Option<serde_json::Value>,
    tool_use_id: &Option<String>,
    tool_response: &Option<serde_json::Value>,
) -> serde_json::Value {
    let mut o = serde_json::json!({
        "session_id": session_id,
        "cwd": cwd.display().to_string(),
        "hook_event_name": event_name,
        "tool_name": tool_name,
    });
    if let Some(input) = tool_input {
        o["tool_input"] = input.clone();
    }
    if let Some(id) = tool_use_id {
        o["tool_use_id"] = serde_json::Value::String(id.clone());
    }
    if let Some(response) = tool_response {
        o["tool_response"] = response.clone();
    }
    o
}

/// Extract text from prompt content blocks.
fn extract_prompt_text(prompt: &[ContentBlock]) -> String {
    prompt
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text(t) => Some(t.text.clone()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Append AVP common fields to JSON if non-empty.
fn append_avp_context(obj: &mut serde_json::Value, ctx: &HookCommandContext) {
    if !ctx.transcript_path.is_empty() {
        obj["transcript_path"] = serde_json::Value::String(ctx.transcript_path.clone());
    }
    if !ctx.permission_mode.is_empty() {
        obj["permission_mode"] = serde_json::Value::String(ctx.permission_mode.clone());
    }
}

/// Map SessionUpdate variant to a string name for matcher/serialization.
fn notification_update_name(update: &SessionUpdate) -> &'static str {
    match update {
        SessionUpdate::AgentMessageChunk(_) => "agent_message",
        SessionUpdate::AgentThoughtChunk(_) => "agent_thought",
        SessionUpdate::ToolCall(_) => "tool_call",
        SessionUpdate::ToolCallUpdate(_) => "tool_call_update",
        SessionUpdate::Plan(_) => "plan",
        SessionUpdate::AvailableCommandsUpdate(_) => "available_commands",
        SessionUpdate::CurrentModeUpdate(_) => "current_mode",
        SessionUpdate::ConfigOptionUpdate(_) => "config_option",
        SessionUpdate::UserMessageChunk(_) => "user_message",
        _ => "unknown",
    }
}

// ---------------------------------------------------------------------------
// Hook decision
// ---------------------------------------------------------------------------

/// What a hook handler wants to happen after it runs.
///
/// Always derived from handler output at runtime (command JSON, prompt/agent
/// evaluator response), never configured statically.
#[derive(Clone, Debug, Default)]
pub enum HookDecision {
    /// Allow the operation to proceed unchanged.
    #[default]
    Allow,
    /// Block the operation (returned as ACP error).
    Block { reason: String },
    /// Allow but inject additional context (prepend text to prompt).
    AllowWithContext { context: String },
    /// Cancel the active prompt turn by calling inner.cancel().
    Cancel { reason: String },
    /// Signal that the agent should not have stopped.
    /// Response meta gets `hook_should_continue: true`.
    ShouldContinue { reason: String },
    /// Allow but modify tool input before execution (PreToolUse only).
    /// Note: In ACP, PreToolUse fires from notifications after tool initiation,
    /// so updatedInput cannot actually modify the call. Logged and treated as Allow.
    AllowWithUpdatedInput { updated_input: serde_json::Value },
}

// ---------------------------------------------------------------------------
// Hook handler trait
// ---------------------------------------------------------------------------

/// Async handler invoked when a matching hook event fires.
///
/// Uses `#[async_trait]` (Send) for tokio::spawn compatibility.
#[async_trait::async_trait]
pub trait HookHandler: Send + Sync {
    /// Inspect the event and return a decision.
    async fn handle(&self, event: &HookEvent) -> HookDecision;
}

// ---------------------------------------------------------------------------
// Hook registration
// ---------------------------------------------------------------------------

/// A registered hook: event filter + optional matcher + handler.
pub struct HookRegistration {
    pub events: Vec<HookEventKind>,
    pub matcher: Option<regex::Regex>,
    pub handler: Arc<dyn HookHandler>,
}

impl Clone for HookRegistration {
    fn clone(&self) -> Self {
        Self {
            events: self.events.clone(),
            matcher: self.matcher.clone(),
            handler: Arc::clone(&self.handler),
        }
    }
}

impl std::fmt::Debug for HookRegistration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HookRegistration")
            .field("events", &self.events)
            .field("matcher", &self.matcher)
            .field("handler", &"<dyn HookHandler>")
            .finish()
    }
}

impl HookRegistration {
    /// Create a new hook registration.
    pub fn new(
        events: Vec<HookEventKind>,
        matcher: Option<regex::Regex>,
        handler: Arc<dyn HookHandler>,
    ) -> Self {
        Self {
            events,
            matcher,
            handler,
        }
    }

    /// Which event kinds this hook fires on.
    pub fn events(&self) -> &[HookEventKind] {
        &self.events
    }

    /// Optional regex matcher pattern.
    pub fn matcher(&self) -> Option<&regex::Regex> {
        self.matcher.as_ref()
    }

    /// Does this registration match the given event?
    fn matches(&self, event: &HookEvent) -> bool {
        if !self.events.contains(&event.kind()) {
            return false;
        }
        match (&self.matcher, event.matcher_value()) {
            (None, _) | (_, None) => true,
            (Some(re), Some(val)) => re.is_match(val),
        }
    }
}

// ---------------------------------------------------------------------------
// HookableAgent
// ---------------------------------------------------------------------------

/// Proxy agent that wraps any `Agent` and fires hooks at lifecycle points.
///
/// Follows the same `Arc<dyn Agent>` wrapping pattern as `TracingAgent`.
/// Tracks session cwd from new_session/load_session for hook event context.
pub struct HookableAgent {
    inner: Arc<dyn Agent + Send + Sync>,
    hooks: Vec<HookRegistration>,
    /// Maps session_id -> cwd, populated from new_session/load_session.
    /// Arc-wrapped so `intercept_notifications` can share it.
    session_cwd: Arc<std::sync::Mutex<HashMap<String, PathBuf>>>,
    agent_type: Option<&'static str>,
    is_playback: bool,
    /// Whether we're currently inside a Stop hook (prevents recursion).
    in_stop_hook: std::sync::atomic::AtomicBool,
    command_context: HookCommandContext,
}

impl HookableAgent {
    /// Wrap an agent with no hooks. Add hooks with [`with_hook`](Self::with_hook).
    pub fn new(inner: Arc<dyn Agent + Send + Sync>) -> Self {
        Self {
            inner,
            hooks: Vec::new(),
            session_cwd: Arc::new(std::sync::Mutex::new(HashMap::new())),
            agent_type: None,
            is_playback: false,
            in_stop_hook: std::sync::atomic::AtomicBool::new(false),
            command_context: HookCommandContext {
                transcript_path: String::new(),
                permission_mode: "bypassPermissions".to_string(),
            },
        }
    }

    /// Wrap a fixture-aware agent. Preserves `agent_type()` and `is_playback()`.
    pub fn from_fixture_agent(inner: Arc<dyn crate::AgentWithFixture + Send + Sync>) -> Self {
        let agent_type = inner.agent_type();
        let is_playback = inner.is_playback();
        Self {
            inner,
            hooks: Vec::new(),
            session_cwd: Arc::new(std::sync::Mutex::new(HashMap::new())),
            agent_type: Some(agent_type),
            is_playback,
            in_stop_hook: std::sync::atomic::AtomicBool::new(false),
            command_context: HookCommandContext {
                transcript_path: String::new(),
                permission_mode: "bypassPermissions".to_string(),
            },
        }
    }

    /// Set the transcript path for hook JSON output (AVP compatibility).
    pub fn with_transcript_path(mut self, path: impl Into<String>) -> Self {
        self.command_context.transcript_path = path.into();
        self
    }

    /// Set the permission mode for hook JSON output (AVP compatibility).
    pub fn with_permission_mode(mut self, mode: impl Into<String>) -> Self {
        self.command_context.permission_mode = mode.into();
        self
    }

    /// Get the command context (for passing to build_registrations).
    pub fn command_context(&self) -> &HookCommandContext {
        &self.command_context
    }

    /// Register a hook handler for the given event kinds with an optional
    /// regex matcher pattern.
    pub fn with_hook(
        mut self,
        events: &[HookEventKind],
        matcher: Option<&str>,
        handler: impl HookHandler + 'static,
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

    /// Register a hook from a pre-built `HookRegistration`.
    pub fn with_registration(mut self, registration: HookRegistration) -> Self {
        self.hooks.push(registration);
        self
    }

    /// Get a reference to the inner agent.
    pub fn inner(&self) -> &Arc<dyn Agent + Send + Sync> {
        &self.inner
    }

    /// Intercept a notification broadcast channel and fire hooks on tool events.
    ///
    /// Returns:
    /// - A new receiver (forwarding all notifications)
    /// - A cancel channel (session ID sent when a hook returns `Cancel`)
    /// - A context channel (context strings sent when a hook returns `AllowWithContext`)
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

    /// Get the cwd for a session, falling back to "." if unknown.
    fn get_cwd(&self, session_id: &str) -> PathBuf {
        self.session_cwd
            .lock()
            .expect("session_cwd mutex not poisoned")
            .get(session_id)
            .cloned()
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Run all matching hooks for an event in parallel, returning collected decisions.
    async fn run_hooks(&self, event: &HookEvent) -> Vec<HookDecision> {
        let futures: Vec<_> = self
            .hooks
            .iter()
            .filter(|h| h.matches(event))
            .map(|h| h.handler.handle(event))
            .collect();
        futures::future::join_all(futures).await
    }

    /// Check decisions for a blocking or cancelling response.
    fn check_blockable(decisions: &[HookDecision]) -> agent_client_protocol::Result<()> {
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
            HookDecision::Allow => None,
            HookDecision::AllowWithContext { .. } => None,
            HookDecision::Cancel { .. } => None,
            HookDecision::ShouldContinue { .. } => None,
            HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    fn find_cancel(decisions: &[HookDecision]) -> Option<&str> {
        decisions.iter().find_map(|d| match d {
            HookDecision::Cancel { reason } => Some(reason.as_str()),
            HookDecision::Allow => None,
            HookDecision::Block { .. } => None,
            HookDecision::AllowWithContext { .. } => None,
            HookDecision::ShouldContinue { .. } => None,
            HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    fn find_should_continue(decisions: &[HookDecision]) -> Option<&str> {
        decisions.iter().find_map(|d| match d {
            HookDecision::ShouldContinue { reason } => Some(reason.as_str()),
            HookDecision::Allow => None,
            HookDecision::Block { .. } => None,
            HookDecision::AllowWithContext { .. } => None,
            HookDecision::Cancel { .. } => None,
            HookDecision::AllowWithUpdatedInput { .. } => None,
        })
    }

    /// Prepend AllowWithContext strings to a prompt request.
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

    /// Annotate a response with ShouldContinue meta.
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

    /// Run `prompt()` while monitoring a cancel channel from `intercept_notifications`.
    pub async fn prompt_with_cancel(
        &self,
        request: PromptRequest,
        cancel_rx: &mut tokio::sync::mpsc::UnboundedReceiver<String>,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let session_id = request.session_id.to_string();

        tokio::select! {
            result = <Self as Agent>::prompt(self, request) => result,
            Some(_) = cancel_rx.recv() => {
                let cancel = CancelNotification::new(session_id);
                let _ = self.inner.cancel(cancel).await;
                Err(agent_client_protocol::Error::new(
                    agent_client_protocol::ErrorCode::InvalidRequest.into(),
                    "Cancelled by notification hook",
                ))
            }
        }
    }

    /// Track session cwd and fire SessionStart hook.
    async fn fire_session_start(&self, session_id: String, source: SessionSource, cwd: PathBuf) {
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
}

#[async_trait::async_trait(?Send)]
impl Agent for HookableAgent {
    async fn initialize(
        &self,
        request: InitializeRequest,
    ) -> agent_client_protocol::Result<InitializeResponse> {
        self.inner.initialize(request).await
    }

    async fn authenticate(
        &self,
        request: AuthenticateRequest,
    ) -> agent_client_protocol::Result<AuthenticateResponse> {
        self.inner.authenticate(request).await
    }

    async fn new_session(
        &self,
        request: NewSessionRequest,
    ) -> agent_client_protocol::Result<NewSessionResponse> {
        let cwd = request.cwd.clone();
        let response = self.inner.new_session(request).await?;
        let session_id = response.session_id.to_string();
        self.fire_session_start(session_id, SessionSource::Startup, cwd)
            .await;
        Ok(response)
    }

    async fn prompt(
        &self,
        request: PromptRequest,
    ) -> agent_client_protocol::Result<PromptResponse> {
        let session_id = request.session_id.to_string();
        let cwd = self.get_cwd(&session_id);

        // UserPromptSubmit hooks
        let event = HookEvent::UserPromptSubmit {
            session_id: session_id.clone(),
            prompt: request.prompt.clone(),
            cwd: cwd.clone(),
        };
        let decisions = self.run_hooks(&event).await;
        Self::check_blockable(&decisions)?;
        let request = Self::inject_context(&decisions, request);

        // Delegate to inner agent
        let response = self.inner.prompt(request).await?;

        // Stop hooks
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
            return Ok(Self::annotate_should_continue(response, reason));
        }
        Ok(response)
    }

    async fn cancel(&self, request: CancelNotification) -> agent_client_protocol::Result<()> {
        self.inner.cancel(request).await
    }

    async fn load_session(
        &self,
        request: LoadSessionRequest,
    ) -> agent_client_protocol::Result<LoadSessionResponse> {
        let session_id = request.session_id.to_string();
        let cwd = request.cwd.clone();
        let response = self.inner.load_session(request).await?;
        self.fire_session_start(session_id, SessionSource::Resume, cwd)
            .await;
        Ok(response)
    }

    async fn set_session_mode(
        &self,
        request: SetSessionModeRequest,
    ) -> agent_client_protocol::Result<SetSessionModeResponse> {
        self.inner.set_session_mode(request).await
    }

    async fn ext_method(&self, request: ExtRequest) -> agent_client_protocol::Result<ExtResponse> {
        self.inner.ext_method(request).await
    }

    async fn ext_notification(
        &self,
        notification: ExtNotification,
    ) -> agent_client_protocol::Result<()> {
        self.inner.ext_notification(notification).await
    }
}

// ---------------------------------------------------------------------------
// AgentWithFixture integration
// ---------------------------------------------------------------------------

impl crate::AgentWithFixture for HookableAgent {
    fn agent_type(&self) -> &'static str {
        self.agent_type
            .expect("HookableAgent: agent_type() requires from_fixture_agent() constructor")
    }

    fn is_playback(&self) -> bool {
        self.is_playback
    }
}

// ---------------------------------------------------------------------------
// Notification stream hooking (internal helpers)
// ---------------------------------------------------------------------------

/// Main loop for the notification interception task.
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

/// Run matching hooks for each event and handle decisions.
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

/// Convert a SessionNotification into hook events.
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

#[cfg(test)]
mod tests {
    use super::*;
    use agent_client_protocol::{ContentBlock, SessionId, TextContent};
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

    // -- Mock agent --

    struct MockAgent {
        prompt_called: Arc<AtomicBool>,
    }

    impl MockAgent {
        fn new() -> (Self, Arc<AtomicBool>) {
            let called = Arc::new(AtomicBool::new(false));
            (
                Self {
                    prompt_called: called.clone(),
                },
                called,
            )
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Agent for MockAgent {
        async fn initialize(
            &self,
            _request: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(InitializeResponse::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
        }

        async fn authenticate(
            &self,
            _request: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }

        async fn new_session(
            &self,
            _request: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session"))
        }

        async fn prompt(
            &self,
            _request: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            self.prompt_called.store(true, Ordering::SeqCst);
            Ok(PromptResponse::new(StopReason::EndTurn))
        }

        async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
            Ok(())
        }

        async fn load_session(
            &self,
            _request: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }

        async fn set_session_mode(
            &self,
            _request: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }

        async fn ext_method(
            &self,
            _request: ExtRequest,
        ) -> agent_client_protocol::Result<ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }

        async fn ext_notification(
            &self,
            _notification: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    fn make_prompt_request() -> PromptRequest {
        PromptRequest::new(
            SessionId::from("test-session"),
            vec![ContentBlock::Text(TextContent::new("hello"))],
        )
    }

    // -- Tests --

    #[tokio::test]
    async fn test_passthrough_delegates_prompt() {
        let (mock, called) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock));

        let response = agent.prompt(make_prompt_request()).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
        assert_eq!(response.stop_reason, StopReason::EndTurn);
    }

    #[tokio::test]
    async fn test_user_prompt_submit_block_prevents_delegation() {
        let (mock, called) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            BlockHook {
                reason: "not allowed".into(),
            },
        );

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("not allowed"));
    }

    #[tokio::test]
    async fn test_allow_with_context_modifies_prompt() {
        let captured = Arc::new(std::sync::Mutex::new(Vec::<ContentBlock>::new()));
        let captured_clone = captured.clone();

        struct CapturingAgent {
            captured: Arc<std::sync::Mutex<Vec<ContentBlock>>>,
        }

        #[async_trait::async_trait(?Send)]
        impl Agent for CapturingAgent {
            async fn initialize(
                &self,
                _r: InitializeRequest,
            ) -> agent_client_protocol::Result<InitializeResponse> {
                Ok(InitializeResponse::new(
                    agent_client_protocol::ProtocolVersion::LATEST,
                ))
            }
            async fn authenticate(
                &self,
                _r: AuthenticateRequest,
            ) -> agent_client_protocol::Result<AuthenticateResponse> {
                Ok(AuthenticateResponse::new())
            }
            async fn new_session(
                &self,
                _r: NewSessionRequest,
            ) -> agent_client_protocol::Result<NewSessionResponse> {
                Ok(NewSessionResponse::new("test-session"))
            }
            async fn prompt(
                &self,
                request: PromptRequest,
            ) -> agent_client_protocol::Result<PromptResponse> {
                *self.captured.lock().unwrap() = request.prompt.clone();
                Ok(PromptResponse::new(StopReason::EndTurn))
            }
            async fn cancel(&self, _r: CancelNotification) -> agent_client_protocol::Result<()> {
                Ok(())
            }
            async fn load_session(
                &self,
                _r: LoadSessionRequest,
            ) -> agent_client_protocol::Result<LoadSessionResponse> {
                Ok(LoadSessionResponse::new())
            }
            async fn set_session_mode(
                &self,
                _r: SetSessionModeRequest,
            ) -> agent_client_protocol::Result<SetSessionModeResponse> {
                Ok(SetSessionModeResponse::new())
            }
            async fn ext_method(
                &self,
                _r: ExtRequest,
            ) -> agent_client_protocol::Result<ExtResponse> {
                Err(agent_client_protocol::Error::method_not_found())
            }
            async fn ext_notification(
                &self,
                _n: ExtNotification,
            ) -> agent_client_protocol::Result<()> {
                Ok(())
            }
        }

        let agent = HookableAgent::new(Arc::new(CapturingAgent {
            captured: captured_clone,
        }))
        .with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            ContextHook {
                context: "extra context".into(),
            },
        );

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
        let blocks = captured.lock().unwrap();
        // Context should be prepended as the first content block
        assert!(blocks.len() >= 2);
        match &blocks[0] {
            ContentBlock::Text(t) => assert_eq!(t.text, "extra context"),
            other => panic!("Expected Text block, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_stop_should_continue_sets_meta() {
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::Stop],
            None,
            ShouldContinueHook {
                reason: "tests not passing".into(),
            },
        );

        let response = agent.prompt(make_prompt_request()).await.unwrap();
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
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::UserPromptSubmit],
            Some("nonexistent-pattern"),
            RecordingHook {
                called: called.clone(),
            },
        );

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_matcher_skipped_for_stop() {
        let called = Arc::new(AtomicBool::new(false));
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::Stop],
            Some("nonexistent-pattern"),
            RecordingHook {
                called: called.clone(),
            },
        );

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
        assert!(called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_session_start_fires_with_startup_source() {
        let source = Arc::new(std::sync::Mutex::new(None));
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::SessionStart],
            None,
            SourceRecordingHook {
                source: source.clone(),
            },
        );

        let _ = agent
            .new_session(NewSessionRequest::new("/tmp"))
            .await
            .unwrap();
        assert_eq!(*source.lock().unwrap(), Some(SessionSource::Startup));
    }

    #[tokio::test]
    async fn test_session_start_fires_with_resume_source() {
        let source = Arc::new(std::sync::Mutex::new(None));
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::SessionStart],
            None,
            SourceRecordingHook {
                source: source.clone(),
            },
        );

        let _ = agent
            .load_session(LoadSessionRequest::new("some-session", "/tmp"))
            .await
            .unwrap();
        assert_eq!(*source.lock().unwrap(), Some(SessionSource::Resume));
    }

    #[tokio::test]
    async fn test_multiple_hooks_all_fire() {
        let called1 = Arc::new(AtomicBool::new(false));
        let called2 = Arc::new(AtomicBool::new(false));
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock))
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

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
        assert!(called1.load(Ordering::SeqCst));
        assert!(called2.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_block_takes_priority_over_context() {
        let (mock, called) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock))
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

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_cwd_tracked_from_new_session() {
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock));

        let _ = agent
            .new_session(NewSessionRequest::new("/my/project"))
            .await
            .unwrap();

        let cwd = agent.get_cwd("test-session");
        assert_eq!(cwd, PathBuf::from("/my/project"));
    }

    // -- Cancel flow tests --

    struct SlowMockAgent {
        cancel_called: Arc<AtomicBool>,
        cancel_notify: Arc<tokio::sync::Notify>,
    }

    impl SlowMockAgent {
        fn new() -> (Self, Arc<AtomicBool>) {
            let cancelled = Arc::new(AtomicBool::new(false));
            (
                Self {
                    cancel_called: cancelled.clone(),
                    cancel_notify: Arc::new(tokio::sync::Notify::new()),
                },
                cancelled,
            )
        }
    }

    #[async_trait::async_trait(?Send)]
    impl Agent for SlowMockAgent {
        async fn initialize(
            &self,
            _request: InitializeRequest,
        ) -> agent_client_protocol::Result<InitializeResponse> {
            Ok(InitializeResponse::new(
                agent_client_protocol::ProtocolVersion::LATEST,
            ))
        }
        async fn authenticate(
            &self,
            _request: AuthenticateRequest,
        ) -> agent_client_protocol::Result<AuthenticateResponse> {
            Ok(AuthenticateResponse::new())
        }
        async fn new_session(
            &self,
            _request: NewSessionRequest,
        ) -> agent_client_protocol::Result<NewSessionResponse> {
            Ok(NewSessionResponse::new("test-session"))
        }
        async fn prompt(
            &self,
            _request: PromptRequest,
        ) -> agent_client_protocol::Result<PromptResponse> {
            self.cancel_notify.notified().await;
            Ok(PromptResponse::new(StopReason::EndTurn))
        }
        async fn cancel(&self, _request: CancelNotification) -> agent_client_protocol::Result<()> {
            self.cancel_called.store(true, Ordering::SeqCst);
            self.cancel_notify.notify_one();
            Ok(())
        }
        async fn load_session(
            &self,
            _request: LoadSessionRequest,
        ) -> agent_client_protocol::Result<LoadSessionResponse> {
            Ok(LoadSessionResponse::new())
        }
        async fn set_session_mode(
            &self,
            _request: SetSessionModeRequest,
        ) -> agent_client_protocol::Result<SetSessionModeResponse> {
            Ok(SetSessionModeResponse::new())
        }
        async fn ext_method(
            &self,
            _request: ExtRequest,
        ) -> agent_client_protocol::Result<ExtResponse> {
            Err(agent_client_protocol::Error::method_not_found())
        }
        async fn ext_notification(
            &self,
            _notification: ExtNotification,
        ) -> agent_client_protocol::Result<()> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn test_prompt_with_cancel_aborts_on_signal() {
        let (mock, cancelled) = SlowMockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock));

        let (cancel_tx, mut cancel_rx) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            let _ = cancel_tx.send("test-session".to_string());
        });

        let result = agent
            .prompt_with_cancel(make_prompt_request(), &mut cancel_rx)
            .await;

        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .message
            .contains("Cancelled by notification hook"));
        assert!(cancelled.load(Ordering::SeqCst));
    }

    #[tokio::test]
    async fn test_prompt_with_cancel_completes_normally() {
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock));

        let (_cancel_tx, mut cancel_rx) = tokio::sync::mpsc::unbounded_channel::<String>();

        let result = agent
            .prompt_with_cancel(make_prompt_request(), &mut cancel_rx)
            .await;

        assert!(result.is_ok());
        assert_eq!(result.unwrap().stop_reason, StopReason::EndTurn);
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

        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::Stop],
            None,
            StopActiveRecorder {
                recorded: recorded_clone,
            },
        );

        let _ = agent.prompt(make_prompt_request()).await.unwrap();
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

    /// A hook returning AllowWithUpdatedInput should not prevent prompt execution
    /// (updatedInput is a PreToolUse concern, not a blocking signal).
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

        let (mock, called) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            UpdatedInputHook,
        );

        let result = agent.prompt(make_prompt_request()).await;
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

        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::Stop],
            None,
            ContextOnStopHook,
        );

        let response = agent.prompt(make_prompt_request()).await.unwrap();
        // AllowWithContext on Stop should NOT add hook_should_continue
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
        // Hook handlers receiving Notification events should be able to
        // inspect the actual message content, not just the type string.
        use agent_client_protocol::ContentChunk;

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
        // The serialized update should contain the original message text
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
        use agent_client_protocol::ContentChunk;

        // Create an AgentMessageChunk notification
        let content = ContentChunk::new(ContentBlock::Text(TextContent::new("hi")));
        let notification = SessionNotification::new(
            SessionId::from("sess-1"),
            SessionUpdate::AgentMessageChunk(content),
        );
        let event = HookEvent::Notification {
            notification: Box::new(notification),
            cwd: PathBuf::from("/tmp"),
        };

        // Matcher for "agent_message" should match
        let matching_reg = HookRegistration::new(
            vec![HookEventKind::Notification],
            Some(regex::Regex::new("agent_message").unwrap()),
            Arc::new(RecordingHook {
                called: Arc::new(AtomicBool::new(false)),
            }),
        );
        assert!(matching_reg.matches(&event));

        // Matcher for "tool_call" should NOT match
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
        // When a notification hook returns AllowWithContext, the context
        // should be sent through the context channel so it can be injected
        // into the agent's next prompt.
        let (mock, _) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::PostToolUse],
            None,
            ContextHook {
                context: "lint warning: unused variable".into(),
            },
        );

        let (notify_tx, notify_rx) = broadcast::channel(16);
        let (_, _cancel_rx, mut context_rx) = agent.intercept_notifications(notify_rx);

        // Send a ToolCall then ToolCallUpdate to trigger PostToolUse
        use agent_client_protocol::{ToolCall, ToolCallUpdate, ToolCallUpdateFields};
        let tool_call = ToolCall::new("call-1", "Bash");
        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCall(tool_call),
        ));
        // Small delay for the spawn to process
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        let update = ToolCallUpdate::new(
            "call-1",
            ToolCallUpdateFields::new().status(agent_client_protocol::ToolCallStatus::Completed),
        );
        let _ = notify_tx.send(SessionNotification::new(
            SessionId::from("s1"),
            SessionUpdate::ToolCallUpdate(update),
        ));
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;

        // Context should arrive through the channel
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

        let (mock, called) = MockAgent::new();
        let agent = HookableAgent::new(Arc::new(mock)).with_hook(
            &[HookEventKind::UserPromptSubmit],
            None,
            CancelHook,
        );

        let result = agent.prompt(make_prompt_request()).await;
        assert!(result.is_err());
        assert!(!called.load(Ordering::SeqCst));
        assert!(result.unwrap_err().message.contains("Cancelled"));
    }
}
