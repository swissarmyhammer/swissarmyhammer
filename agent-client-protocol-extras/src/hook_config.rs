//! Hook configuration — Claude-compatible declarative hook registration
//!
//! Matches Claude Code's 3-level config nesting:
//! 1. Event name (PascalCase) → array of matcher groups
//! 2. Matcher group → optional regex matcher + array of handlers
//! 3. Handler → command, prompt, or agent
//!
//! JSON example (Claude Code format):
//! ```json
//! {
//!   "hooks": {
//!     "PreToolUse": [
//!       {
//!         "matcher": "Bash",
//!         "hooks": [
//!           { "type": "command", "command": "./check.sh" }
//!         ]
//!       }
//!     ]
//!   }
//! }
//! ```
//!
//! YAML example:
//! ```yaml
//! hooks:
//!   PreToolUse:
//!     - matcher: "Bash"
//!       hooks:
//!         - type: command
//!           command: "./check.sh"
//! ```

use agent_client_protocol::schema::{ContentBlock, SessionNotification, SessionUpdate, StopReason};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// ---------------------------------------------------------------------------
// Hook event data types
// ---------------------------------------------------------------------------
//
// These types describe the lifecycle events that hook handlers respond to and
// the registration metadata used to dispatch them. They are pure data — they
// do not depend on the ACP `Agent` Role or any wrapper. The `HookableAgent`
// wrapper in `crate::hookable_agent` (sibling task A2) consumes these types
// to fan out events at the right moments.

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
    /// Fires when MCP server requests user input.
    Elicitation {
        session_id: String,
        mcp_server_name: Option<String>,
        message: Option<String>,
        mode: String,
        requested_schema: serde_json::Value,
        cwd: PathBuf,
    },
    /// Fires when user responds to MCP elicitation.
    ElicitationResult {
        session_id: String,
        mcp_server_name: String,
        action: Option<String>,
        content: serde_json::Value,
        elicitation_id: String,
        cwd: PathBuf,
    },
    /// Fires when CLAUDE.md or rules files are loaded.
    InstructionsLoaded {
        file_path: Option<String>,
        load_reason: String,
        cwd: PathBuf,
    },
    /// Fires when config files change.
    ConfigChange {
        session_id: String,
        source: Option<String>,
        cwd: PathBuf,
    },
    /// Fires when a worktree is created.
    WorktreeCreate {
        worktree_path: Option<String>,
        branch_name: Option<String>,
        cwd: PathBuf,
    },
    /// Fires when a worktree is removed.
    WorktreeRemove { worktree_path: String, cwd: PathBuf },
    /// Fires after context compaction.
    PostCompact { session_id: String, cwd: PathBuf },
    /// Fires when an agent teammate goes idle.
    TeammateIdle {
        session_id: String,
        teammate_id: Option<String>,
        cwd: PathBuf,
    },
    /// Fires when a task is marked complete.
    TaskCompleted {
        session_id: String,
        task_id: Option<String>,
        task_title: Option<String>,
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
    PostCompact,
    TeammateIdle,
    TaskCompleted,
    Elicitation,
    ElicitationResult,
    InstructionsLoaded,
    ConfigChange,
    WorktreeCreate,
    WorktreeRemove,
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
            Self::Elicitation { .. } => HookEventKind::Elicitation,
            Self::ElicitationResult { .. } => HookEventKind::ElicitationResult,
            Self::InstructionsLoaded { .. } => HookEventKind::InstructionsLoaded,
            Self::ConfigChange { .. } => HookEventKind::ConfigChange,
            Self::WorktreeCreate { .. } => HookEventKind::WorktreeCreate,
            Self::WorktreeRemove { .. } => HookEventKind::WorktreeRemove,
            Self::PostCompact { .. } => HookEventKind::PostCompact,
            Self::TeammateIdle { .. } => HookEventKind::TeammateIdle,
            Self::TaskCompleted { .. } => HookEventKind::TaskCompleted,
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
            Self::Elicitation {
                mcp_server_name, ..
            } => mcp_server_name.as_deref(),
            Self::ElicitationResult {
                mcp_server_name, ..
            } => Some(mcp_server_name.as_str()),
            Self::InstructionsLoaded { file_path, .. } => file_path.as_deref(),
            Self::ConfigChange { source, .. } => source.as_deref(),
            Self::WorktreeCreate { .. }
            | Self::WorktreeRemove { .. }
            | Self::PostCompact { .. }
            | Self::TeammateIdle { .. }
            | Self::TaskCompleted { .. } => None,
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
            Self::Elicitation {
                session_id,
                mcp_server_name,
                message,
                mode,
                requested_schema,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "session_id": session_id,
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "Elicitation",
                    "mode": mode,
                    "requested_schema": requested_schema,
                });
                if let Some(name) = mcp_server_name {
                    obj["mcp_server_name"] = serde_json::Value::String(name.clone());
                }
                if let Some(msg) = message {
                    obj["message"] = serde_json::Value::String(msg.clone());
                }
                obj
            }
            Self::ElicitationResult {
                session_id,
                mcp_server_name,
                action,
                content,
                elicitation_id,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "session_id": session_id,
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "ElicitationResult",
                    "mcp_server_name": mcp_server_name,
                    "content": content,
                    "elicitation_id": elicitation_id,
                });
                if let Some(a) = action {
                    obj["action"] = serde_json::Value::String(a.clone());
                }
                obj
            }
            Self::InstructionsLoaded {
                file_path,
                load_reason,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "InstructionsLoaded",
                    "load_reason": load_reason,
                });
                if let Some(fp) = file_path {
                    obj["file_path"] = serde_json::Value::String(fp.clone());
                }
                obj
            }
            Self::ConfigChange {
                session_id,
                source,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "session_id": session_id,
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "ConfigChange",
                });
                if let Some(src) = source {
                    obj["source"] = serde_json::Value::String(src.clone());
                }
                obj
            }
            Self::WorktreeCreate {
                worktree_path,
                branch_name,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "WorktreeCreate",
                });
                if let Some(wp) = worktree_path {
                    obj["worktree_path"] = serde_json::Value::String(wp.clone());
                }
                if let Some(bn) = branch_name {
                    obj["branch_name"] = serde_json::Value::String(bn.clone());
                }
                obj
            }
            Self::WorktreeRemove { worktree_path, cwd } => serde_json::json!({
                "cwd": cwd.display().to_string(),
                "hook_event_name": "WorktreeRemove",
                "worktree_path": worktree_path,
            }),
            Self::PostCompact { session_id, cwd } => serde_json::json!({
                "session_id": session_id,
                "cwd": cwd.display().to_string(),
                "hook_event_name": "PostCompact",
            }),
            Self::TeammateIdle {
                session_id,
                teammate_id,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "session_id": session_id,
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "TeammateIdle",
                });
                if let Some(id) = teammate_id {
                    obj["teammate_id"] = serde_json::Value::String(id.clone());
                }
                obj
            }
            Self::TaskCompleted {
                session_id,
                task_id,
                task_title,
                cwd,
            } => {
                let mut obj = serde_json::json!({
                    "session_id": session_id,
                    "cwd": cwd.display().to_string(),
                    "hook_event_name": "TaskCompleted",
                });
                if let Some(id) = task_id {
                    obj["task_id"] = serde_json::Value::String(id.clone());
                }
                if let Some(title) = task_title {
                    obj["task_title"] = serde_json::Value::String(title.clone());
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
    o["tool_input"] = tool_input.clone().unwrap_or(serde_json::json!({}));
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

/// Append AVP common fields to JSON.
fn append_avp_context(obj: &mut serde_json::Value, ctx: &HookCommandContext) {
    obj["transcript_path"] = serde_json::Value::String(ctx.transcript_path.clone());
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
    pub fn matches(&self, event: &HookEvent) -> bool {
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
// Config types (3-level nesting matching Claude Code)
// ---------------------------------------------------------------------------

/// Top-level hook configuration, deserializable from JSON or YAML.
///
/// Matches Claude Code's format: event names are PascalCase keys in a map.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct HookConfig {
    /// Event name → array of matcher groups
    #[serde(default)]
    pub hooks: HashMap<HookEventKindConfig, Vec<MatcherGroup>>,
}

/// A matcher group: optional regex filter + array of hook handlers.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MatcherGroup {
    /// Optional regex pattern to filter when hooks fire.
    /// Omit or use "*" to match all occurrences.
    #[serde(default)]
    pub matcher: Option<String>,
    /// Hook handlers to run when matched.
    pub hooks: Vec<HookHandlerConfig>,
}

/// Event kind identifiers — PascalCase matching Claude Code.
///
/// Includes forward-compatible variants for Claude Code events that ACP
/// cannot fire. These are silently skipped during `build_registrations()`,
/// allowing the same config file to work with both Claude Code and ACP.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEventKindConfig {
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    Notification,
    // Forward-compatible: not fired by ACP, silently skipped
    PermissionRequest,
    SubagentStart,
    SubagentStop,
    PreCompact,
    Setup,
    SessionEnd,
    TeammateIdle,
    TaskCompleted,
    /// Forward-compatible: MCP elicitation request.
    Elicitation,
    /// Forward-compatible: MCP elicitation response.
    ElicitationResult,
    /// Forward-compatible: instructions/rules files loaded.
    InstructionsLoaded,
    /// Forward-compatible: config files changed.
    ConfigChange,
    /// Forward-compatible: worktree created.
    WorktreeCreate,
    /// Forward-compatible: worktree removed.
    WorktreeRemove,
    /// Forward-compatible: after context compaction.
    PostCompact,
}

/// Error returned when a config event kind has no ACP equivalent.
#[derive(Clone, Debug)]
pub struct UnsupportedEventKind;

impl std::fmt::Display for UnsupportedEventKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("event kind is not supported by ACP")
    }
}

impl std::error::Error for UnsupportedEventKind {}

impl TryFrom<HookEventKindConfig> for HookEventKind {
    type Error = UnsupportedEventKind;

    fn try_from(config: HookEventKindConfig) -> Result<Self, Self::Error> {
        match config {
            HookEventKindConfig::SessionStart => Ok(HookEventKind::SessionStart),
            HookEventKindConfig::UserPromptSubmit => Ok(HookEventKind::UserPromptSubmit),
            HookEventKindConfig::PreToolUse => Ok(HookEventKind::PreToolUse),
            HookEventKindConfig::PostToolUse => Ok(HookEventKind::PostToolUse),
            HookEventKindConfig::PostToolUseFailure => Ok(HookEventKind::PostToolUseFailure),
            HookEventKindConfig::Stop => Ok(HookEventKind::Stop),
            HookEventKindConfig::Notification => Ok(HookEventKind::Notification),
            HookEventKindConfig::PostCompact => Ok(HookEventKind::PostCompact),
            HookEventKindConfig::TeammateIdle => Ok(HookEventKind::TeammateIdle),
            HookEventKindConfig::TaskCompleted => Ok(HookEventKind::TaskCompleted),
            HookEventKindConfig::Elicitation => Ok(HookEventKind::Elicitation),
            HookEventKindConfig::ElicitationResult => Ok(HookEventKind::ElicitationResult),
            HookEventKindConfig::InstructionsLoaded => Ok(HookEventKind::InstructionsLoaded),
            HookEventKindConfig::ConfigChange => Ok(HookEventKind::ConfigChange),
            HookEventKindConfig::WorktreeCreate => Ok(HookEventKind::WorktreeCreate),
            HookEventKindConfig::WorktreeRemove => Ok(HookEventKind::WorktreeRemove),
            HookEventKindConfig::PermissionRequest
            | HookEventKindConfig::SubagentStart
            | HookEventKindConfig::SubagentStop
            | HookEventKindConfig::PreCompact
            | HookEventKindConfig::Setup
            | HookEventKindConfig::SessionEnd => Err(UnsupportedEventKind),
        }
    }
}

/// Handler configuration — only 3 types matching Claude Code.
///
/// - `command` — run a shell command, interpret exit code + JSON stdout
/// - `prompt` — send a prompt to an LLM for single-turn evaluation
/// - `agent` — spawn an agent with tool access for multi-turn evaluation
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum HookHandlerConfig {
    /// Run a shell command with JSON stdin/stdout protocol.
    Command {
        /// Shell command to execute.
        command: String,
        /// Timeout in seconds (default 600).
        #[serde(default = "default_command_timeout")]
        timeout: u64,
    },
    /// Send a prompt to an LLM for single-turn evaluation.
    Prompt {
        /// Prompt text. Use `$ARGUMENTS` as placeholder for hook input JSON.
        prompt: String,
        /// Optional model identifier.
        #[serde(default)]
        model: Option<String>,
        /// Timeout in seconds (default 30).
        #[serde(default = "default_prompt_timeout")]
        timeout: u64,
    },
    /// Spawn an agent with tool access for multi-turn evaluation.
    Agent {
        /// Prompt text. Use `$ARGUMENTS` as placeholder for hook input JSON.
        prompt: String,
        /// Optional model identifier.
        #[serde(default)]
        model: Option<String>,
        /// Timeout in seconds (default 60).
        #[serde(default = "default_agent_timeout")]
        timeout: u64,
    },
}

fn default_command_timeout() -> u64 {
    600
}

fn default_prompt_timeout() -> u64 {
    30
}

fn default_agent_timeout() -> u64 {
    60
}

// ---------------------------------------------------------------------------
// Hook output types (Claude-compatible JSON parsing)
// ---------------------------------------------------------------------------

fn default_true() -> bool {
    true
}

/// Decision values for top-level and permission decisions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookDecisionValue {
    /// Allow the action.
    Allow,
    /// Block/deny the action.
    Block,
    /// Ask user for permission (permission decisions only).
    Ask,
}

/// Parsed JSON output from a command hook's stdout.
///
/// Field names use camelCase to match Claude Code's JSON format.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HookOutput {
    /// If false, stop Claude processing entirely. Takes precedence over other fields.
    #[serde(rename = "continue", default = "default_true")]
    pub should_continue: bool,
    /// Message shown to user when `should_continue` is false.
    pub stop_reason: Option<String>,
    /// If true, hide stdout from verbose output.
    #[serde(default)]
    pub suppress_output: bool,
    /// Warning message shown to the user.
    pub system_message: Option<String>,
    /// Top-level decision: "block" to prevent the action.
    pub decision: Option<HookDecisionValue>,
    /// Reason for the decision.
    pub reason: Option<String>,
    /// Event-specific output for richer control.
    pub hook_specific_output: Option<HookSpecificOutput>,
    /// Additional context string added to Claude's context.
    pub additional_context: Option<String>,
}

impl Default for HookOutput {
    fn default() -> Self {
        Self {
            should_continue: true,
            stop_reason: None,
            suppress_output: false,
            system_message: None,
            decision: None,
            reason: None,
            hook_specific_output: None,
            additional_context: None,
        }
    }
}

/// Event-specific output fields inside `hookSpecificOutput`.
///
/// Tagged by `hookEventName` to enforce per-event field sets, matching
/// AVP's `#[serde(tag = "hookEventName")]` convention.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "hookEventName")]
pub enum HookSpecificOutput {
    PreToolUse {
        #[serde(rename = "permissionDecision")]
        permission_decision: Option<String>,
        #[serde(rename = "permissionDecisionReason")]
        permission_decision_reason: Option<String>,
        #[serde(rename = "updatedInput")]
        updated_input: Option<serde_json::Value>,
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    PostToolUse {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    PostToolUseFailure {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    UserPromptSubmit {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    Stop {
        reason: Option<String>,
    },
    SessionStart {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
    Notification {
        #[serde(rename = "additionalContext")]
        additional_context: Option<String>,
    },
}

/// Parsed JSON response from a prompt/agent hook evaluator.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PromptHookResponse {
    /// true to allow, false to block/prevent stopping.
    pub ok: bool,
    /// Reason for blocking (required when ok is false).
    pub reason: Option<String>,
}

// ---------------------------------------------------------------------------
// HookEvaluator trait (for prompt/agent hooks)
// ---------------------------------------------------------------------------

/// Evaluator for prompt-based and agent-based hooks.
///
/// Callers implement this with their own LLM client.
/// For "prompt" hooks: single-turn evaluation (is_agent=false).
/// For "agent" hooks: multi-turn evaluation with tool access (is_agent=true).
#[async_trait::async_trait]
pub trait HookEvaluator: Send + Sync {
    /// Evaluate a prompt and return a JSON response string.
    ///
    /// Expected response format: `{ "ok": true }` or `{ "ok": false, "reason": "..." }`
    async fn evaluate(&self, prompt: &str, is_agent: bool) -> Result<String, String>;
}

// ---------------------------------------------------------------------------
// Config errors
// ---------------------------------------------------------------------------

/// Error building hook registrations from config.
#[derive(Debug, thiserror::Error)]
pub enum HookConfigError {
    #[error("Invalid regex pattern in hook matcher: {0}")]
    InvalidRegex(#[from] regex::Error),
    #[error("Hook entry has empty hooks list")]
    EmptyHooks,
    #[error("Prompt or agent hook requires a HookEvaluator, but none was provided")]
    MissingEvaluator,
}

// ---------------------------------------------------------------------------
// Built-in handlers
// ---------------------------------------------------------------------------

/// Command handler: runs shell command with JSON stdin/stdout protocol.
///
/// Exit codes (following Claude Code):
/// - 0 → parse stdout as HookOutput JSON, interpret based on event
/// - 2 → Block (stderr becomes reason)
/// - Other → Allow (warning logged)
struct CommandHandler {
    command: String,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for CommandHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let stdin_json = event.to_command_input().to_string();
        match run_command(&self.command, &stdin_json, self.timeout).await {
            Ok(output) => interpret_exit_code(&output, &self.command, event.kind()),
            Err(CommandRunError::SpawnFailed(e)) => {
                tracing::error!(command = %self.command, error = %e, "Hook command failed to execute");
                HookDecision::Allow
            }
            Err(CommandRunError::TimedOut) => {
                tracing::error!(command = %self.command, "Hook command timed out");
                HookDecision::Block {
                    reason: format!("Command '{}' timed out", self.command),
                }
            }
        }
    }
}

enum CommandRunError {
    SpawnFailed(std::io::Error),
    TimedOut,
}

/// Execute a hook command string via shell.
///
/// # Trust model
///
/// Hook commands come from admin-controlled configuration files (`.claude/settings.json`,
/// project `CLAUDE.md`, etc.) — the same trust model as Claude Code's hook system.
/// Shell execution via `sh -c` is intentional: hooks need pipes, redirects, and
/// multi-command chains. The config file itself is the trust boundary, not this function.
async fn run_command(
    command: &str,
    stdin_json: &str,
    timeout: std::time::Duration,
) -> Result<std::process::Output, CommandRunError> {
    use tokio::process::Command;

    let result = tokio::time::timeout(timeout, async {
        let mut child = Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            let _ = stdin.write_all(stdin_json.as_bytes()).await;
            drop(stdin);
        }

        child.wait_with_output().await
    })
    .await;

    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(CommandRunError::SpawnFailed(e)),
        Err(_) => Err(CommandRunError::TimedOut),
    }
}

/// Interpret a command's exit code into a HookDecision.
fn interpret_exit_code(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let code = output.status.code().unwrap_or(-1);
    match code {
        0 => interpret_exit_0_stdout(output, command, event_kind),
        2 => interpret_exit_2_stderr(output, command, event_kind),
        other => {
            tracing::warn!(
                command = %command,
                exit_code = other,
                "Hook command exited with unexpected code, allowing"
            );
            HookDecision::Allow
        }
    }
}

fn interpret_exit_0_stdout(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stdout = stdout.trim();
    if stdout.is_empty() {
        return HookDecision::Allow;
    }
    match serde_json::from_str::<HookOutput>(stdout) {
        Ok(hook_output) => interpret_output(&hook_output, event_kind),
        Err(e) => {
            tracing::warn!(
                command = %command,
                error = %e,
                stdout = %stdout,
                "Failed to parse hook command JSON output, treating as Allow"
            );
            HookDecision::Allow
        }
    }
}

fn interpret_exit_2_stderr(
    output: &std::process::Output,
    command: &str,
    event_kind: HookEventKind,
) -> HookDecision {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let reason = if stderr.trim().is_empty() {
        format!("Command '{}' exited with code 2", command)
    } else {
        stderr.trim().to_string()
    };
    if is_blockable(event_kind) {
        HookDecision::Block { reason }
    } else if event_kind == HookEventKind::Stop {
        HookDecision::ShouldContinue { reason }
    } else if feeds_stderr_to_agent(event_kind) {
        HookDecision::AllowWithContext { context: reason }
    } else {
        tracing::warn!(
            command = %command,
            "Exit 2 on non-blockable event {:?}, treating as Allow",
            event_kind,
        );
        HookDecision::Allow
    }
}

/// Whether an event kind supports blocking via exit-2.
///
/// Only PreToolUse and UserPromptSubmit can block because the action
/// hasn't happened yet. All other events (PostToolUse, PostToolUseFailure,
/// Notification, SessionStart) cannot block.
fn is_blockable(kind: HookEventKind) -> bool {
    matches!(
        kind,
        HookEventKind::PreToolUse | HookEventKind::UserPromptSubmit
    )
}

/// Whether exit-2 stderr should be fed back as agent context.
///
/// PostToolUse and PostToolUseFailure can't block (action already happened)
/// but Claude Code feeds the stderr back to the agent as context.
fn feeds_stderr_to_agent(kind: HookEventKind) -> bool {
    matches!(
        kind,
        HookEventKind::PostToolUse | HookEventKind::PostToolUseFailure
    )
}

/// Prompt handler: calls HookEvaluator for single-turn LLM evaluation.
struct PromptHandler {
    prompt_template: String,
    evaluator: Arc<dyn HookEvaluator>,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for PromptHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let arguments_json = event.to_command_input().to_string();
        let prompt = self.prompt_template.replace("$ARGUMENTS", &arguments_json);

        let result = tokio::time::timeout(self.timeout, async {
            self.evaluator.evaluate(&prompt, false).await
        })
        .await;

        match result {
            Ok(Ok(response_json)) => {
                match serde_json::from_str::<PromptHookResponse>(&response_json) {
                    Ok(response) => interpret_prompt_response(&response, event.kind()),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to parse prompt hook response, treating as Allow"
                        );
                        HookDecision::Allow
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Prompt hook evaluator failed");
                HookDecision::Allow
            }
            Err(_) => {
                tracing::error!("Prompt hook timed out");
                HookDecision::Block {
                    reason: "Prompt hook timed out".to_string(),
                }
            }
        }
    }
}

/// Agent handler: calls HookEvaluator for multi-turn evaluation with tool access.
struct AgentHandler {
    prompt_template: String,
    evaluator: Arc<dyn HookEvaluator>,
    timeout: std::time::Duration,
}

#[async_trait::async_trait]
impl HookHandler for AgentHandler {
    async fn handle(&self, event: &HookEvent) -> HookDecision {
        let arguments_json = event.to_command_input().to_string();
        let prompt = self.prompt_template.replace("$ARGUMENTS", &arguments_json);

        let result = tokio::time::timeout(self.timeout, async {
            self.evaluator.evaluate(&prompt, true).await
        })
        .await;

        match result {
            Ok(Ok(response_json)) => {
                match serde_json::from_str::<PromptHookResponse>(&response_json) {
                    Ok(response) => interpret_prompt_response(&response, event.kind()),
                    Err(e) => {
                        tracing::warn!(
                            error = %e,
                            "Failed to parse agent hook response, treating as Allow"
                        );
                        HookDecision::Allow
                    }
                }
            }
            Ok(Err(e)) => {
                tracing::error!(error = %e, "Agent hook evaluator failed");
                HookDecision::Allow
            }
            Err(_) => {
                tracing::error!("Agent hook timed out");
                HookDecision::Block {
                    reason: "Agent hook timed out".to_string(),
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Output interpretation
// ---------------------------------------------------------------------------

/// Interpret command hook JSON output based on event type.
///
/// Maps HookOutput fields to HookDecision following Claude Code semantics.
fn interpret_output(output: &HookOutput, event_kind: HookEventKind) -> HookDecision {
    // `continue: false` takes precedence over everything
    if !output.should_continue {
        return HookDecision::Cancel {
            reason: output
                .stop_reason
                .clone()
                .unwrap_or_else(|| "Hook requested stop".to_string()),
        };
    }

    // Check hookSpecificOutput
    if let Some(specific) = &output.hook_specific_output {
        if let Some(decision) = interpret_specific_output(specific) {
            return decision;
        }
    }

    // Top-level decision: "block"
    if let Some(decision) = &output.decision {
        if decision == &HookDecisionValue::Block {
            let reason = output
                .reason
                .clone()
                .unwrap_or_else(|| "Blocked by hook".to_string());
            // For Stop event, "block" means "don't stop" → ShouldContinue
            if event_kind == HookEventKind::Stop {
                return HookDecision::ShouldContinue { reason };
            }
            return HookDecision::Block { reason };
        }
    }

    // Additional context (top-level or in hookSpecificOutput)
    let context = output
        .additional_context
        .clone()
        .or_else(|| extract_specific_context(&output.hook_specific_output));

    if let Some(ctx) = context {
        return HookDecision::AllowWithContext { context: ctx };
    }

    HookDecision::Allow
}

/// Interpret hookSpecificOutput for PreToolUse events.
///
/// Returns `Some(decision)` if the specific output determines the outcome,
/// `None` to fall through to top-level fields.
fn interpret_specific_output(specific: &HookSpecificOutput) -> Option<HookDecision> {
    match specific {
        HookSpecificOutput::PreToolUse {
            permission_decision,
            permission_decision_reason,
            updated_input,
            additional_context,
        } => interpret_pre_tool_use_specific(
            permission_decision.as_deref(),
            permission_decision_reason,
            updated_input,
            additional_context,
        ),
        HookSpecificOutput::PostToolUse { additional_context }
        | HookSpecificOutput::PostToolUseFailure { additional_context }
        | HookSpecificOutput::UserPromptSubmit { additional_context }
        | HookSpecificOutput::SessionStart { additional_context }
        | HookSpecificOutput::Notification { additional_context } => additional_context
            .as_ref()
            .map(|ctx| HookDecision::AllowWithContext {
                context: ctx.clone(),
            }),
        HookSpecificOutput::Stop { reason } => reason
            .as_ref()
            .map(|r| HookDecision::ShouldContinue { reason: r.clone() }),
    }
}

/// Interpret PreToolUse-specific fields into a decision.
fn interpret_pre_tool_use_specific(
    permission_decision: Option<&str>,
    permission_decision_reason: &Option<String>,
    updated_input: &Option<serde_json::Value>,
    additional_context: &Option<String>,
) -> Option<HookDecision> {
    if let Some(decision) = permission_decision {
        match decision {
            "deny" | "block" => {
                return Some(HookDecision::Block {
                    reason: permission_decision_reason
                        .clone()
                        .unwrap_or_else(|| "Denied by hook".to_string()),
                });
            }
            "allow" => {
                if let Some(ctx) = additional_context {
                    return Some(HookDecision::AllowWithContext {
                        context: ctx.clone(),
                    });
                }
                return Some(HookDecision::Allow);
            }
            _ => {} // "ask" or unknown — fall through
        }
    }
    if let Some(input) = updated_input {
        return Some(HookDecision::AllowWithUpdatedInput {
            updated_input: input.clone(),
        });
    }
    if let Some(ctx) = additional_context {
        return Some(HookDecision::AllowWithContext {
            context: ctx.clone(),
        });
    }
    None
}

/// Extract additionalContext from a HookSpecificOutput if present.
fn extract_specific_context(specific: &Option<HookSpecificOutput>) -> Option<String> {
    match specific.as_ref()? {
        HookSpecificOutput::PreToolUse {
            additional_context, ..
        }
        | HookSpecificOutput::PostToolUse { additional_context }
        | HookSpecificOutput::PostToolUseFailure { additional_context }
        | HookSpecificOutput::UserPromptSubmit { additional_context }
        | HookSpecificOutput::SessionStart { additional_context }
        | HookSpecificOutput::Notification { additional_context } => additional_context.clone(),
        HookSpecificOutput::Stop { .. } => None,
    }
}

/// Interpret prompt/agent evaluator response based on event type.
fn interpret_prompt_response(
    response: &PromptHookResponse,
    event_kind: HookEventKind,
) -> HookDecision {
    if response.ok {
        HookDecision::Allow
    } else {
        let reason = response
            .reason
            .clone()
            .unwrap_or_else(|| "Blocked by prompt hook".to_string());
        if is_blockable(event_kind) {
            HookDecision::Block { reason }
        } else if event_kind == HookEventKind::Stop {
            HookDecision::ShouldContinue { reason }
        } else if feeds_stderr_to_agent(event_kind) {
            HookDecision::AllowWithContext { context: reason }
        } else {
            HookDecision::Allow
        }
    }
}

// ---------------------------------------------------------------------------
// Factory: config → registrations
// ---------------------------------------------------------------------------

/// Build a handler from config, requiring an evaluator for prompt/agent types.
fn build_handler(
    config: &HookHandlerConfig,
    evaluator: &Option<Arc<dyn HookEvaluator>>,
) -> Result<Arc<dyn HookHandler>, HookConfigError> {
    match config {
        HookHandlerConfig::Command { command, timeout } => Ok(Arc::new(CommandHandler {
            command: command.clone(),
            timeout: std::time::Duration::from_secs(*timeout),
        })),
        HookHandlerConfig::Prompt {
            prompt, timeout, ..
        } => {
            let eval = evaluator
                .as_ref()
                .ok_or(HookConfigError::MissingEvaluator)?
                .clone();
            Ok(Arc::new(PromptHandler {
                prompt_template: prompt.clone(),
                evaluator: eval,
                timeout: std::time::Duration::from_secs(*timeout),
            }))
        }
        HookHandlerConfig::Agent {
            prompt, timeout, ..
        } => {
            let eval = evaluator
                .as_ref()
                .ok_or(HookConfigError::MissingEvaluator)?
                .clone();
            Ok(Arc::new(AgentHandler {
                prompt_template: prompt.clone(),
                evaluator: eval,
                timeout: std::time::Duration::from_secs(*timeout),
            }))
        }
    }
}

impl HookConfig {
    /// Build runtime [`HookRegistration`]s from this config.
    ///
    /// Each matcher group + handler combination becomes one `HookRegistration`.
    /// Prompt/agent handlers require an evaluator.
    pub fn build_registrations(
        &self,
        evaluator: Option<Arc<dyn HookEvaluator>>,
    ) -> Result<Vec<HookRegistration>, HookConfigError> {
        let mut registrations = Vec::new();

        for (event_kind_config, matcher_groups) in &self.hooks {
            let event_kind: HookEventKind = match event_kind_config.clone().try_into() {
                Ok(kind) => kind,
                Err(_) => continue, // Skip forward-compatible event kinds
            };

            for group in matcher_groups {
                if group.hooks.is_empty() {
                    return Err(HookConfigError::EmptyHooks);
                }

                let matcher = group
                    .matcher
                    .as_deref()
                    .filter(|m| !m.is_empty() && *m != "*")
                    .map(regex::Regex::new)
                    .transpose()?;

                for handler_config in &group.hooks {
                    let handler = build_handler(handler_config, &evaluator)?;
                    registrations.push(HookRegistration::new(
                        vec![event_kind],
                        matcher.clone(),
                        handler,
                    ));
                }
            }
        }

        Ok(registrations)
    }
}

// `hookable_agent_from_config` lives in `crate::hookable_agent` from ACP 0.11
// onward, because the inner-agent argument type changed shape with the new
// SDK (no more `Arc<dyn Agent>`). It is re-exported through `lib.rs`.

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};

    // -- Mock evaluator (for prompt/agent handler config tests) --

    struct MockEvaluator {
        response: String,
        is_agent_called: Arc<AtomicBool>,
    }

    impl MockEvaluator {
        fn allowing() -> Self {
            Self {
                response: r#"{"ok": true}"#.to_string(),
                is_agent_called: Arc::new(AtomicBool::new(false)),
            }
        }
    }

    #[async_trait::async_trait]
    impl HookEvaluator for MockEvaluator {
        async fn evaluate(&self, _prompt: &str, is_agent: bool) -> Result<String, String> {
            if is_agent {
                self.is_agent_called.store(true, Ordering::SeqCst);
            }
            Ok(self.response.clone())
        }
    }

    // =====================================================================
    // JSON deserialization tests (3-level nesting)
    // =====================================================================

    #[test]
    fn test_json_command_hook() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "Bash",
                        "hooks": [
                            { "type": "command", "command": "./check.sh" }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 1);
        let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(groups[0].hooks.len(), 1);
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Command { command, .. } if command == "./check.sh"
        ));
    }

    #[test]
    fn test_json_prompt_hook() {
        let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "prompt", "prompt": "Check if all tasks are complete: $ARGUMENTS" }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(groups[0].matcher.is_none());
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Prompt { prompt, .. }
                if prompt.contains("$ARGUMENTS")
        ));
    }

    #[test]
    fn test_json_agent_hook() {
        let json = r#"{
            "hooks": {
                "Stop": [
                    {
                        "hooks": [
                            { "type": "agent", "prompt": "Verify tests pass: $ARGUMENTS", "timeout": 120 }
                        ]
                    }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Agent { prompt, timeout, .. }
                if prompt.contains("Verify tests") && *timeout == 120
        ));
    }

    #[test]
    fn test_json_multiple_events_with_matchers() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "./bash-check.sh" } ] },
                    { "matcher": "Edit|Write", "hooks": [ { "type": "command", "command": "./lint.sh" } ] }
                ],
                "Stop": [
                    { "hooks": [ { "type": "prompt", "prompt": "All done? $ARGUMENTS" } ] }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 2);
        let pre_tool = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(pre_tool.len(), 2);
        assert_eq!(pre_tool[0].matcher.as_deref(), Some("Bash"));
        assert_eq!(pre_tool[1].matcher.as_deref(), Some("Edit|Write"));
    }

    #[test]
    fn test_json_empty_config() {
        let config: HookConfig = serde_json::from_str("{}").unwrap();
        assert!(config.hooks.is_empty());
    }

    #[test]
    fn test_json_default_timeouts() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "hooks": [
                        { "type": "command", "command": "true" },
                        { "type": "prompt", "prompt": "check" },
                        { "type": "agent", "prompt": "verify" }
                    ]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let hooks = &config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap()[0].hooks;
        assert!(matches!(&hooks[0], HookHandlerConfig::Command { timeout, .. } if *timeout == 600));
        assert!(matches!(&hooks[1], HookHandlerConfig::Prompt { timeout, .. } if *timeout == 30));
        assert!(matches!(&hooks[2], HookHandlerConfig::Agent { timeout, .. } if *timeout == 60));
    }

    // =====================================================================
    // YAML deserialization tests
    // =====================================================================

    #[test]
    fn test_yaml_command_hook() {
        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
"#;
        let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::PreToolUse).unwrap();
        assert_eq!(groups[0].matcher.as_deref(), Some("Bash"));
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Command { command, .. } if command == "./check.sh"
        ));
    }

    #[test]
    fn test_yaml_prompt_hook() {
        let yaml = r#"
hooks:
  Stop:
    - hooks:
        - type: prompt
          prompt: "Check completion: $ARGUMENTS"
"#;
        let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
        let groups = config.hooks.get(&HookEventKindConfig::Stop).unwrap();
        assert!(matches!(
            &groups[0].hooks[0],
            HookHandlerConfig::Prompt { prompt, .. } if prompt.contains("$ARGUMENTS")
        ));
    }

    #[test]
    fn test_yaml_multiple_events() {
        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Verify completion"
  SessionStart:
    - matcher: "startup"
      hooks:
        - type: command
          command: "./init.sh"
"#;
        let config: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();
        assert_eq!(config.hooks.len(), 3);
    }

    #[test]
    fn test_yaml_empty_config() {
        let config: HookConfig = serde_yaml_ng::from_str("{}").unwrap();
        assert!(config.hooks.is_empty());
    }

    // =====================================================================
    // JSON ↔ YAML equivalence
    // =====================================================================

    #[test]
    fn test_json_yaml_equivalence() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "./check.sh" } ] }
                ],
                "Stop": [
                    { "hooks": [ { "type": "prompt", "prompt": "Done?" } ] }
                ]
            }
        }"#;

        let yaml = r#"
hooks:
  PreToolUse:
    - matcher: "Bash"
      hooks:
        - type: command
          command: "./check.sh"
  Stop:
    - hooks:
        - type: prompt
          prompt: "Done?"
"#;

        let from_json: HookConfig = serde_json::from_str(json).unwrap();
        let from_yaml: HookConfig = serde_yaml_ng::from_str(yaml).unwrap();

        assert_eq!(from_json.hooks.len(), from_yaml.hooks.len());
        assert!(from_json
            .hooks
            .contains_key(&HookEventKindConfig::PreToolUse));
        assert!(from_yaml
            .hooks
            .contains_key(&HookEventKindConfig::PreToolUse));
        assert!(from_json.hooks.contains_key(&HookEventKindConfig::Stop));
        assert!(from_yaml.hooks.contains_key(&HookEventKindConfig::Stop));
    }

    // =====================================================================
    // Build registration tests
    // =====================================================================

    #[test]
    fn test_build_registrations_command() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    { "matcher": "Bash", "hooks": [ { "type": "command", "command": "true" } ] }
                ]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].events, vec![HookEventKind::PreToolUse]);
        assert!(regs[0].matcher.is_some());
    }

    #[test]
    fn test_build_registrations_invalid_regex() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "[invalid",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let result = config.build_registrations(None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HookConfigError::InvalidRegex(_)
        ));
    }

    #[test]
    fn test_build_registrations_missing_evaluator() {
        let json = r#"{
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "prompt", "prompt": "check" }] }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let result = config.build_registrations(None);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            HookConfigError::MissingEvaluator
        ));
    }

    #[test]
    fn test_build_registrations_prompt_with_evaluator() {
        let json = r#"{
            "hooks": {
                "Stop": [{ "hooks": [{ "type": "prompt", "prompt": "check" }] }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let evaluator: Arc<dyn HookEvaluator> = Arc::new(MockEvaluator::allowing());
        let regs = config.build_registrations(Some(evaluator)).unwrap();
        assert_eq!(regs.len(), 1);
        assert_eq!(regs[0].events, vec![HookEventKind::Stop]);
    }

    #[test]
    fn test_build_registrations_wildcard_matcher_treated_as_none() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [{
                    "matcher": "*",
                    "hooks": [{ "type": "command", "command": "true" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        let regs = config.build_registrations(None).unwrap();
        assert!(regs[0].matcher.is_none());
    }

    // =====================================================================
    // HookOutput interpretation tests
    // =====================================================================

    #[test]
    fn test_interpret_output_continue_false() {
        let output = HookOutput {
            should_continue: false,
            stop_reason: Some("Build failed".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Cancel { reason } if reason == "Build failed"
        ));
    }

    #[test]
    fn test_interpret_output_pre_tool_use_deny() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some("Not allowed".into()),
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Not allowed"
        ));
    }

    #[test]
    fn test_interpret_output_pre_tool_use_allow() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("allow".into()),
                permission_decision_reason: None,
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    #[test]
    fn test_interpret_output_stop_block_is_should_continue() {
        let output = HookOutput {
            decision: Some(HookDecisionValue::Block),
            reason: Some("Tests not passing".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::Stop);
        assert!(matches!(
            decision,
            HookDecision::ShouldContinue { reason } if reason == "Tests not passing"
        ));
    }

    #[test]
    fn test_interpret_output_user_prompt_block() {
        let output = HookOutput {
            decision: Some(HookDecisionValue::Block),
            reason: Some("Prompt rejected".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Prompt rejected"
        ));
    }

    #[test]
    fn test_interpret_output_additional_context() {
        let output = HookOutput {
            additional_context: Some("Extra info".into()),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::SessionStart);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Extra info"
        ));
    }

    #[test]
    fn test_interpret_output_empty_is_allow() {
        let output = HookOutput::default();
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    // =====================================================================
    // Prompt/agent response interpretation
    // =====================================================================

    #[test]
    fn test_prompt_response_ok_true() {
        let response = PromptHookResponse {
            ok: true,
            reason: None,
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Allow));
    }

    #[test]
    fn test_prompt_response_ok_false_blocks() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Forbidden".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PreToolUse);
        assert!(matches!(
            decision,
            HookDecision::Block { reason } if reason == "Forbidden"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_stop_is_should_continue() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Tests not complete".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::Stop);
        assert!(matches!(
            decision,
            HookDecision::ShouldContinue { reason } if reason == "Tests not complete"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_post_tool_feeds_context() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Lint warning detected".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PostToolUse);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Lint warning detected"
        ));
    }

    #[test]
    fn test_prompt_response_ok_false_post_tool_failure_feeds_context() {
        let response = PromptHookResponse {
            ok: false,
            reason: Some("Failure noted".into()),
        };
        let decision = interpret_prompt_response(&response, HookEventKind::PostToolUseFailure);
        assert!(matches!(
            decision,
            HookDecision::AllowWithContext { context } if context == "Failure noted"
        ));
    }

    // =====================================================================
    // Event-aware exit-2 tests (interpret_exit_2_stderr is in this crate)
    // =====================================================================

    #[test]
    fn test_exit_2_on_silent_events_allows() {
        let silent = vec![HookEventKind::Notification, HookEventKind::SessionStart];
        for kind in &silent {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("echo 'should not block' >&2; exit 2")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .unwrap();
            let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
            assert!(
                matches!(decision, HookDecision::Allow),
                "Expected Allow for silent {:?}, got {:?}",
                kind,
                decision
            );
        }
    }

    #[test]
    fn test_exit_2_on_blockable_event_blocks() {
        let blockable = vec![HookEventKind::PreToolUse, HookEventKind::UserPromptSubmit];
        for kind in &blockable {
            let output = std::process::Command::new("sh")
                .arg("-c")
                .arg("echo 'blocked' >&2; exit 2")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .output()
                .unwrap();
            let decision = interpret_exit_2_stderr(&output, "test-cmd", *kind);
            assert!(
                matches!(decision, HookDecision::Block { .. }),
                "Expected Block for blockable {:?}, got {:?}",
                kind,
                decision
            );
        }
    }

    // =====================================================================
    // Forward-compatible event kinds
    // =====================================================================

    #[test]
    fn test_unsupported_event_kinds_return_error() {
        let unsupported_kinds = vec![
            HookEventKindConfig::PermissionRequest,
            HookEventKindConfig::SubagentStart,
            HookEventKindConfig::SubagentStop,
            HookEventKindConfig::PreCompact,
            HookEventKindConfig::Setup,
            HookEventKindConfig::SessionEnd,
        ];
        for kind in &unsupported_kinds {
            let result: Result<HookEventKind, _> = kind.clone().try_into();
            assert!(result.is_err(), "Expected {:?} to be unsupported", kind);
        }
    }

    #[test]
    fn test_supported_event_kinds_succeed() {
        let supported_kinds = vec![
            (
                HookEventKindConfig::SessionStart,
                HookEventKind::SessionStart,
            ),
            (
                HookEventKindConfig::UserPromptSubmit,
                HookEventKind::UserPromptSubmit,
            ),
            (HookEventKindConfig::PreToolUse, HookEventKind::PreToolUse),
            (HookEventKindConfig::PostToolUse, HookEventKind::PostToolUse),
            (
                HookEventKindConfig::PostToolUseFailure,
                HookEventKind::PostToolUseFailure,
            ),
            (HookEventKindConfig::Stop, HookEventKind::Stop),
            (
                HookEventKindConfig::Notification,
                HookEventKind::Notification,
            ),
            (HookEventKindConfig::PostCompact, HookEventKind::PostCompact),
            (
                HookEventKindConfig::TeammateIdle,
                HookEventKind::TeammateIdle,
            ),
            (
                HookEventKindConfig::TaskCompleted,
                HookEventKind::TaskCompleted,
            ),
            (HookEventKindConfig::Elicitation, HookEventKind::Elicitation),
            (
                HookEventKindConfig::ElicitationResult,
                HookEventKind::ElicitationResult,
            ),
            (
                HookEventKindConfig::InstructionsLoaded,
                HookEventKind::InstructionsLoaded,
            ),
            (
                HookEventKindConfig::ConfigChange,
                HookEventKind::ConfigChange,
            ),
            (
                HookEventKindConfig::WorktreeCreate,
                HookEventKind::WorktreeCreate,
            ),
            (
                HookEventKindConfig::WorktreeRemove,
                HookEventKind::WorktreeRemove,
            ),
        ];
        for (config_kind, expected_kind) in &supported_kinds {
            let result: Result<HookEventKind, _> = config_kind.clone().try_into();
            assert_eq!(
                result.unwrap(),
                *expected_kind,
                "Expected {:?} to convert successfully",
                config_kind
            );
        }
    }

    #[test]
    fn test_unsupported_event_kind_display() {
        let err = UnsupportedEventKind;
        assert_eq!(err.to_string(), "event kind is not supported by ACP");
    }

    #[test]
    fn test_unsupported_event_kind_is_error() {
        let err = UnsupportedEventKind;
        let _: &dyn std::error::Error = &err;
    }

    #[test]
    fn test_post_tool_use_failure_deserialization() {
        let json = r#"{
            "hooks": {
                "PostToolUseFailure": [{
                    "hooks": [{ "type": "command", "command": "echo 'Tool failed'" }]
                }]
            }
        }"#;
        let config: HookConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.hooks.len(), 1);
        assert!(config
            .hooks
            .contains_key(&HookEventKindConfig::PostToolUseFailure));
    }

    // =====================================================================
    // HookDecisionValue enum
    // =====================================================================

    #[test]
    fn test_hook_decision_value_serialization() {
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Allow).unwrap(),
            "\"allow\""
        );
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Block).unwrap(),
            "\"block\""
        );
        assert_eq!(
            serde_json::to_string(&HookDecisionValue::Ask).unwrap(),
            "\"ask\""
        );
    }

    #[test]
    fn test_hook_decision_value_deserialization() {
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"allow\"").unwrap(),
            HookDecisionValue::Allow
        );
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"block\"").unwrap(),
            HookDecisionValue::Block
        );
        assert_eq!(
            serde_json::from_str::<HookDecisionValue>("\"ask\"").unwrap(),
            HookDecisionValue::Ask
        );
    }

    #[test]
    fn test_hook_output_with_decision_value() {
        let json = r#"{ "continue": true, "decision": "block", "reason": "Blocked by hook" }"#;
        let output: HookOutput = serde_json::from_str(json).unwrap();
        assert_eq!(output.decision, Some(HookDecisionValue::Block));
        assert_eq!(output.reason, Some("Blocked by hook".to_string()));
    }

    #[test]
    fn test_pre_tool_use_deny_decision_parses_with_reason() {
        let json = r#"{
            "hookEventName": "PreToolUse",
            "permissionDecision": "deny",
            "permissionDecisionReason": "Too risky"
        }"#;
        let output: HookSpecificOutput = serde_json::from_str(json).unwrap();
        match output {
            HookSpecificOutput::PreToolUse {
                permission_decision,
                permission_decision_reason,
                ..
            } => {
                assert_eq!(permission_decision, Some("deny".to_string()));
                assert_eq!(permission_decision_reason, Some("Too risky".to_string()));
            }
            other => panic!("Expected PreToolUse, got {:?}", other),
        }
    }

    #[test]
    fn test_interpret_output_with_enum_block_decision() {
        let output = HookOutput {
            should_continue: true,
            stop_reason: None,
            suppress_output: false,
            system_message: None,
            decision: Some(HookDecisionValue::Block),
            reason: Some("Blocked".to_string()),
            hook_specific_output: None,
            additional_context: None,
        };
        let decision = interpret_output(&output, HookEventKind::UserPromptSubmit);
        assert!(matches!(decision, HookDecision::Block { .. }));
    }

    #[test]
    fn test_interpret_output_with_enum_permission_decision() {
        let output = HookOutput {
            hook_specific_output: Some(HookSpecificOutput::PreToolUse {
                permission_decision: Some("deny".into()),
                permission_decision_reason: Some("Denied".into()),
                updated_input: None,
                additional_context: None,
            }),
            ..Default::default()
        };
        let decision = interpret_output(&output, HookEventKind::PreToolUse);
        assert!(matches!(decision, HookDecision::Block { .. }));
    }

    // -- New hook event config variants --

    #[test]
    fn test_new_config_variants_serde_round_trip() {
        let variants = vec![
            HookEventKindConfig::Elicitation,
            HookEventKindConfig::ElicitationResult,
            HookEventKindConfig::InstructionsLoaded,
            HookEventKindConfig::ConfigChange,
            HookEventKindConfig::WorktreeCreate,
            HookEventKindConfig::WorktreeRemove,
            HookEventKindConfig::PostCompact,
        ];
        for variant in &variants {
            let json = serde_json::to_string(variant).unwrap();
            let deserialized: HookEventKindConfig = serde_json::from_str(&json).unwrap();
            assert_eq!(
                std::mem::discriminant(&deserialized),
                std::mem::discriminant(variant),
                "round-trip failed for {:?}",
                variant
            );
        }
    }

    #[test]
    fn test_new_config_variants_in_hook_config() {
        let names = [
            "Elicitation",
            "ElicitationResult",
            "InstructionsLoaded",
            "ConfigChange",
            "WorktreeCreate",
            "WorktreeRemove",
            "PostCompact",
        ];
        for name in &names {
            let json = format!(
                r#"{{"hooks":{{"{}":[{{"hooks":[{{"type":"command","command":"./check.sh"}}]}}]}}}}"#,
                name
            );
            let config: HookConfig = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to parse HookConfig with {}: {}", name, e));
            assert_eq!(config.hooks.len(), 1, "Expected 1 entry for {}", name);
        }
    }

    #[test]
    fn test_try_from_new_active_variants() {
        let result: Result<HookEventKind, _> = HookEventKindConfig::PostCompact.try_into();
        assert!(matches!(result.unwrap(), HookEventKind::PostCompact));

        let result: Result<HookEventKind, _> = HookEventKindConfig::TeammateIdle.try_into();
        assert!(matches!(result.unwrap(), HookEventKind::TeammateIdle));

        let result: Result<HookEventKind, _> = HookEventKindConfig::TaskCompleted.try_into();
        assert!(matches!(result.unwrap(), HookEventKind::TaskCompleted));
    }

    #[test]
    fn test_try_from_new_event_kinds_succeed() {
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::Elicitation),
            Ok(HookEventKind::Elicitation)
        ));
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::ElicitationResult),
            Ok(HookEventKind::ElicitationResult)
        ));
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::InstructionsLoaded),
            Ok(HookEventKind::InstructionsLoaded)
        ));
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::ConfigChange),
            Ok(HookEventKind::ConfigChange)
        ));
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::WorktreeCreate),
            Ok(HookEventKind::WorktreeCreate)
        ));
        assert!(matches!(
            HookEventKind::try_from(HookEventKindConfig::WorktreeRemove),
            Ok(HookEventKind::WorktreeRemove)
        ));
    }
}
