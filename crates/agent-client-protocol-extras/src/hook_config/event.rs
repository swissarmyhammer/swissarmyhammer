//! Hook event data types.
//!
//! These types describe the lifecycle events that hook handlers respond to and
//! the registration metadata used to dispatch them. They are pure data — they
//! do not depend on the ACP `Agent` Role or any wrapper. The `HookableAgent`
//! wrapper in `crate::hookable_agent` (sibling task A2) consumes these types
//! to fan out events at the right moments.
//!
//! Each event also knows the Claude-compatible JSON it sends to a command hook
//! on stdin. The `json_*` helpers below build that JSON, one helper for each
//! event, all on top of one shared base builder.

use agent_client_protocol::schema::{ContentBlock, SessionNotification, SessionUpdate, StopReason};
use std::path::{Path, PathBuf};

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
        if self == Self::Startup {
            "startup"
        } else {
            "resume"
        }
    }
}

/// Lifecycle events that hooks can respond to.
#[derive(Clone, Debug)]
pub enum HookEvent {
    /// Fires after new_session() or load_session().
    SessionStart {
        /// The ACP session that started. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// Tells if the session is new or resumed. The hook JSON sends it as
        /// `source`, and a matcher tests this value.
        source: SessionSource,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires before prompt() delegates to inner agent.
    UserPromptSubmit {
        /// The ACP session that received the prompt. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The content blocks of the user prompt. Only the text blocks go into
        /// the hook JSON, joined by a new line, as `prompt`. An empty list
        /// gives an empty `prompt` string.
        prompt: Vec<ContentBlock>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires on ToolCall notification (before tool execution).
    PreToolUse {
        /// The ACP session that starts the tool call. The hook JSON sends it
        /// as `session_id`.
        session_id: String,
        /// The name of the tool. The hook JSON sends it as `tool_name`, and a
        /// matcher tests this value.
        tool_name: String,
        /// The arguments of the tool call. `None` means the notification gave
        /// no arguments, and the hook JSON then sends an empty object as
        /// `tool_input`.
        tool_input: Option<serde_json::Value>,
        /// The identifier of this tool call. `None` means the notification
        /// gave no identifier, and the hook JSON then has no `tool_use_id`
        /// key.
        tool_use_id: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires on ToolCallUpdate notification (after successful tool execution).
    PostToolUse {
        /// The ACP session that ran the tool call. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The name of the tool. The hook JSON sends it as `tool_name`, and a
        /// matcher tests this value.
        tool_name: String,
        /// The arguments of the tool call. `None` means the notification gave
        /// no arguments, and the hook JSON then sends an empty object as
        /// `tool_input`.
        tool_input: Option<serde_json::Value>,
        /// The result the tool gave. `None` means the notification gave no
        /// result, and the hook JSON then has no `tool_response` key.
        tool_response: Option<serde_json::Value>,
        /// The identifier of this tool call. `None` means the notification
        /// gave no identifier, and the hook JSON then has no `tool_use_id`
        /// key.
        tool_use_id: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires on ToolCallUpdate when tool status is Failed.
    PostToolUseFailure {
        /// The ACP session whose tool call failed. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The name of the tool that failed. The hook JSON sends it as
        /// `tool_name`, and a matcher tests this value.
        tool_name: String,
        /// The arguments of the tool call. `None` means the notification gave
        /// no arguments, and the hook JSON then sends an empty object as
        /// `tool_input`.
        tool_input: Option<serde_json::Value>,
        /// The error the tool gave. `None` means the notification gave no
        /// error, and the hook JSON then has no `error` key.
        error: Option<serde_json::Value>,
        /// The identifier of this tool call. `None` means the notification
        /// gave no identifier, and the hook JSON then has no `tool_use_id`
        /// key.
        tool_use_id: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires after prompt() returns.
    Stop {
        /// The ACP session whose turn stopped. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// Why the turn stopped. The hook JSON sends the debug text of this
        /// value as `stop_reason`.
        stop_reason: StopReason,
        /// Tells if a stop hook is already active for this turn. The hook JSON
        /// sends it as `stop_hook_active`. A hook reads it to prevent an
        /// endless loop.
        stop_hook_active: bool,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires on any SessionNotification.
    Notification {
        /// The full ACP session notification. The hook JSON sends the name of
        /// its update as `notification_type` and the whole notification as
        /// `notification`. The session id also comes from this value, and a
        /// matcher tests the update name.
        notification: Box<SessionNotification>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when MCP server requests user input.
    Elicitation {
        /// The ACP session that received the request. The hook JSON sends it
        /// as `session_id`.
        session_id: String,
        /// The MCP server that asks for the input. `None` means the request
        /// gave no name; the hook JSON then has no `mcp_server_name` key, and
        /// no matcher can match the event.
        mcp_server_name: Option<String>,
        /// The message shown to the user. `None` means the request gave no
        /// message, and the hook JSON then has no `message` key.
        message: Option<String>,
        /// How the agent asks the user. The hook JSON sends it as `mode`.
        mode: String,
        /// The JSON schema of the data the server asks for. The hook JSON
        /// sends it as `requested_schema`.
        requested_schema: serde_json::Value,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when user responds to MCP elicitation.
    ElicitationResult {
        /// The ACP session that received the answer. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The MCP server that asked for the input. The hook JSON sends it as
        /// `mcp_server_name`, and a matcher tests this value.
        mcp_server_name: String,
        /// What the user did, for example accept or cancel. `None` means the
        /// answer gave no action, and the hook JSON then has no `action` key.
        action: Option<String>,
        /// The data the user gave. The hook JSON sends it as `content`.
        content: serde_json::Value,
        /// The identifier of the request this answer belongs to. The hook JSON
        /// sends it as `elicitation_id`.
        elicitation_id: String,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when CLAUDE.md or rules files are loaded.
    InstructionsLoaded {
        /// The file that was loaded, for example `CLAUDE.md`. `None` means the
        /// instructions have no file; the hook JSON then has no `file_path`
        /// key, and no matcher can match the event.
        file_path: Option<String>,
        /// Why the instructions were loaded. The hook JSON sends it as
        /// `load_reason`.
        load_reason: String,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`. This event has no session, so the hook JSON has no
        /// `session_id` key.
        cwd: PathBuf,
    },
    /// Fires when config files change.
    ConfigChange {
        /// The ACP session that saw the change. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// Which config file changed. `None` means the change has no known
        /// source; the hook JSON then has no `source` key, and no matcher can
        /// match the event.
        source: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when a worktree is created.
    WorktreeCreate {
        /// The path of the new worktree. `None` means the event gave no path,
        /// and the hook JSON then has no `worktree_path` key.
        worktree_path: Option<String>,
        /// The branch the new worktree uses. `None` means the event gave no
        /// branch, and the hook JSON then has no `branch_name` key.
        branch_name: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`. This event has no session, so the hook JSON has no
        /// `session_id` key.
        cwd: PathBuf,
    },
    /// Fires when a worktree is removed.
    WorktreeRemove {
        /// The path of the worktree that was removed. The hook JSON sends it
        /// as `worktree_path`.
        worktree_path: String,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`. This event has no session, so the hook JSON has no
        /// `session_id` key.
        cwd: PathBuf,
    },
    /// Fires after context compaction.
    PostCompact {
        /// The ACP session whose context was compacted. The hook JSON sends it
        /// as `session_id`.
        session_id: String,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when an agent teammate goes idle.
    TeammateIdle {
        /// The ACP session the teammate belongs to. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The teammate that went idle. `None` means the event gave no
        /// identifier, and the hook JSON then has no `teammate_id` key.
        teammate_id: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
    /// Fires when a task is marked complete.
    TaskCompleted {
        /// The ACP session that completed the task. The hook JSON sends it as
        /// `session_id`.
        session_id: String,
        /// The task that is complete. `None` means the event gave no
        /// identifier, and the hook JSON then has no `task_id` key.
        task_id: Option<String>,
        /// The title of the task. `None` means the event gave no title, and
        /// the hook JSON then has no `task_title` key.
        task_title: Option<String>,
        /// The directory the session works in. The hook JSON sends it as
        /// `cwd`.
        cwd: PathBuf,
    },
}

/// Which category of event a hook registration matches.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum HookEventKind {
    /// Session started event kind.
    SessionStart,
    /// User prompt submission event kind.
    UserPromptSubmit,
    /// Pre-tool use event kind.
    PreToolUse,
    /// Post-tool use event kind.
    PostToolUse,
    /// Post-tool use failure event kind.
    PostToolUseFailure,
    /// Stop event kind.
    Stop,
    /// Notification event kind.
    Notification,
    /// Post-compaction event kind.
    PostCompact,
    /// Teammate idle event kind.
    TeammateIdle,
    /// Task completed event kind.
    TaskCompleted,
    /// Elicitation event kind.
    Elicitation,
    /// Elicitation result event kind.
    ElicitationResult,
    /// Instructions loaded event kind.
    InstructionsLoaded,
    /// Config change event kind.
    ConfigChange,
    /// Worktree creation event kind.
    WorktreeCreate,
    /// Worktree removal event kind.
    WorktreeRemove,
}

// `HookEvent::kind()` is a mechanical 1:1 mapping from each `HookEvent`
// variant to the `HookEventKind` variant of the same name. Generating it from
// a single list of shared identifiers means adding a variant to one enum
// without adding it here fails to compile (the match becomes non-exhaustive)
// instead of silently drifting out of sync.
macro_rules! hook_event_kind_map {
    ($($variant:ident),+ $(,)?) => {
        impl HookEvent {
            /// The kind of this event.
            pub fn kind(&self) -> HookEventKind {
                match self {
                    $(Self::$variant { .. } => HookEventKind::$variant,)+
                }
            }
        }
    };
}

hook_event_kind_map!(
    SessionStart,
    UserPromptSubmit,
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    Stop,
    Notification,
    Elicitation,
    ElicitationResult,
    InstructionsLoaded,
    ConfigChange,
    WorktreeCreate,
    WorktreeRemove,
    PostCompact,
    TeammateIdle,
    TaskCompleted,
);

impl HookEvent {
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
    ///
    /// A simple dispatcher: each arm extracts its variant's fields and hands
    /// them to a `json_<variant>` helper that owns that variant's
    /// JSON-building logic (including any conditional optional fields).
    fn to_base_json(&self) -> serde_json::Value {
        match self {
            Self::SessionStart {
                session_id,
                source,
                cwd,
            } => json_session_start(session_id, *source, cwd),
            Self::UserPromptSubmit {
                session_id,
                prompt,
                cwd,
            } => json_user_prompt_submit(session_id, prompt, cwd),
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
            } => json_post_tool_use_failure(
                session_id,
                tool_name,
                tool_input,
                error,
                tool_use_id,
                cwd,
            ),
            Self::Stop {
                session_id,
                stop_reason,
                stop_hook_active,
                cwd,
            } => json_stop(session_id, stop_reason, *stop_hook_active, cwd),
            Self::Notification { notification, cwd } => json_notification(notification, cwd),
            Self::Elicitation {
                session_id,
                mcp_server_name,
                message,
                mode,
                requested_schema,
                cwd,
            } => json_elicitation(
                session_id,
                mcp_server_name,
                message,
                mode,
                requested_schema,
                cwd,
            ),
            Self::ElicitationResult {
                session_id,
                mcp_server_name,
                action,
                content,
                elicitation_id,
                cwd,
            } => json_elicitation_result(
                session_id,
                mcp_server_name,
                action,
                content,
                elicitation_id,
                cwd,
            ),
            Self::InstructionsLoaded {
                file_path,
                load_reason,
                cwd,
            } => json_instructions_loaded(file_path, load_reason, cwd),
            Self::ConfigChange {
                session_id,
                source,
                cwd,
            } => json_config_change(session_id, source, cwd),
            Self::WorktreeCreate {
                worktree_path,
                branch_name,
                cwd,
            } => json_worktree_create(worktree_path, branch_name, cwd),
            Self::WorktreeRemove { worktree_path, cwd } => json_worktree_remove(worktree_path, cwd),
            Self::PostCompact { session_id, cwd } => json_post_compact(session_id, cwd),
            Self::TeammateIdle {
                session_id,
                teammate_id,
                cwd,
            } => json_teammate_idle(session_id, teammate_id, cwd),
            Self::TaskCompleted {
                session_id,
                task_id,
                task_title,
                cwd,
            } => json_task_completed(session_id, task_id, task_title, cwd),
        }
    }
}

/// Build the base JSON object shared by every hook event: `hook_event_name`
/// and `cwd`, plus `session_id` when the event carries one.
///
/// Every `json_*` helper in this file builds its event-specific JSON on top
/// of this single base builder, so the base fields are constructed in
/// exactly one place no matter how many hook event shapes exist.
fn base_event_json(session_id: Option<&str>, event_name: &str, cwd: &Path) -> serde_json::Value {
    let mut obj = serde_json::json!({
        "cwd": cwd.display().to_string(),
        "hook_event_name": event_name,
    });
    if let Some(session_id) = session_id {
        obj["session_id"] = serde_json::Value::String(session_id.to_string());
    }
    obj
}

/// Build JSON for a `SessionStart` event.
fn json_session_start(session_id: &str, source: SessionSource, cwd: &Path) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), "SessionStart", cwd);
    obj["source"] = serde_json::Value::String(source.as_str().to_string());
    obj
}

/// Build JSON for a `UserPromptSubmit` event.
fn json_user_prompt_submit(
    session_id: &str,
    prompt: &[ContentBlock],
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), "UserPromptSubmit", cwd);
    obj["prompt"] = serde_json::Value::String(extract_prompt_text(prompt));
    obj
}

/// Build JSON for a `PostToolUseFailure` event.
fn json_post_tool_use_failure(
    session_id: &str,
    tool_name: &str,
    tool_input: &Option<serde_json::Value>,
    error: &Option<serde_json::Value>,
    tool_use_id: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
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

/// Build JSON for a `Stop` event.
fn json_stop(
    session_id: &str,
    stop_reason: &StopReason,
    stop_hook_active: bool,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), "Stop", cwd);
    obj["stop_reason"] = serde_json::Value::String(format!("{:?}", stop_reason));
    obj["stop_hook_active"] = serde_json::Value::Bool(stop_hook_active);
    obj
}

/// Build JSON for a `Notification` event.
fn json_notification(notification: &SessionNotification, cwd: &Path) -> serde_json::Value {
    let session_id = notification.session_id.to_string();
    let mut obj = base_event_json(Some(&session_id), "Notification", cwd);
    obj["notification_type"] =
        serde_json::Value::String(notification_update_name(&notification.update).to_string());
    if let Ok(update_value) = serde_json::to_value(&notification.update) {
        obj["notification"] = update_value;
    }
    obj
}

/// Build JSON for an `Elicitation` event.
fn json_elicitation(
    session_id: &str,
    mcp_server_name: &Option<String>,
    message: &Option<String>,
    mode: &str,
    requested_schema: &serde_json::Value,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), "Elicitation", cwd);
    obj["mode"] = serde_json::Value::String(mode.to_string());
    obj["requested_schema"] = requested_schema.clone();
    if let Some(name) = mcp_server_name {
        obj["mcp_server_name"] = serde_json::Value::String(name.clone());
    }
    if let Some(msg) = message {
        obj["message"] = serde_json::Value::String(msg.clone());
    }
    obj
}

/// Build JSON for an `ElicitationResult` event.
fn json_elicitation_result(
    session_id: &str,
    mcp_server_name: &str,
    action: &Option<String>,
    content: &serde_json::Value,
    elicitation_id: &str,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), "ElicitationResult", cwd);
    obj["mcp_server_name"] = serde_json::Value::String(mcp_server_name.to_string());
    obj["content"] = content.clone();
    obj["elicitation_id"] = serde_json::Value::String(elicitation_id.to_string());
    if let Some(a) = action {
        obj["action"] = serde_json::Value::String(a.clone());
    }
    obj
}

/// Build JSON for an `InstructionsLoaded` event.
fn json_instructions_loaded(
    file_path: &Option<String>,
    load_reason: &str,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(None, "InstructionsLoaded", cwd);
    obj["load_reason"] = serde_json::Value::String(load_reason.to_string());
    if let Some(fp) = file_path {
        obj["file_path"] = serde_json::Value::String(fp.clone());
    }
    obj
}

/// Build a session-scoped event JSON object on top of [`base_event_json`],
/// with one optional string field conditionally added.
///
/// Shared by hook events whose JSON shape is the base fields plus a single
/// optional named field — e.g. `ConfigChange`'s `source` and
/// `TeammateIdle`'s `teammate_id`.
fn build_session_event_with_optional_string(
    session_id: &str,
    event_name: &str,
    optional_field_name: &str,
    optional_value: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(Some(session_id), event_name, cwd);
    if let Some(value) = optional_value {
        obj[optional_field_name] = serde_json::Value::String(value.clone());
    }
    obj
}

/// Build a base event JSON object on top of [`base_event_json`], with two
/// optional string fields conditionally added.
///
/// Shared by hook events whose JSON shape is the base fields plus two
/// optional named fields — e.g. `WorktreeCreate`'s `worktree_path`/
/// `branch_name` and `TaskCompleted`'s `task_id`/`task_title`. Follows the
/// same pattern as [`build_session_event_with_optional_string`], generalized
/// to two fields and an optional (rather than required) `session_id`.
fn build_event_with_two_optional_string_fields(
    session_id: Option<&str>,
    event_name: &str,
    field1_name: &str,
    field1_value: &Option<String>,
    field2_name: &str,
    field2_value: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
    let mut obj = base_event_json(session_id, event_name, cwd);
    if let Some(value) = field1_value {
        obj[field1_name] = serde_json::Value::String(value.clone());
    }
    if let Some(value) = field2_value {
        obj[field2_name] = serde_json::Value::String(value.clone());
    }
    obj
}

/// Build JSON for a `ConfigChange` event.
fn json_config_change(session_id: &str, source: &Option<String>, cwd: &Path) -> serde_json::Value {
    build_session_event_with_optional_string(session_id, "ConfigChange", "source", source, cwd)
}

/// Build JSON for a `WorktreeCreate` event.
fn json_worktree_create(
    worktree_path: &Option<String>,
    branch_name: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
    build_event_with_two_optional_string_fields(
        None,
        "WorktreeCreate",
        "worktree_path",
        worktree_path,
        "branch_name",
        branch_name,
        cwd,
    )
}

/// Build JSON for a `WorktreeRemove` event.
fn json_worktree_remove(worktree_path: &str, cwd: &Path) -> serde_json::Value {
    let mut obj = base_event_json(None, "WorktreeRemove", cwd);
    obj["worktree_path"] = serde_json::Value::String(worktree_path.to_string());
    obj
}

/// Build JSON for a `PostCompact` event.
fn json_post_compact(session_id: &str, cwd: &Path) -> serde_json::Value {
    base_event_json(Some(session_id), "PostCompact", cwd)
}

/// Build JSON for a `TeammateIdle` event.
fn json_teammate_idle(
    session_id: &str,
    teammate_id: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
    build_session_event_with_optional_string(
        session_id,
        "TeammateIdle",
        "teammate_id",
        teammate_id,
        cwd,
    )
}

/// Build JSON for a `TaskCompleted` event.
fn json_task_completed(
    session_id: &str,
    task_id: &Option<String>,
    task_title: &Option<String>,
    cwd: &Path,
) -> serde_json::Value {
    build_event_with_two_optional_string_fields(
        Some(session_id),
        "TaskCompleted",
        "task_id",
        task_id,
        "task_title",
        task_title,
        cwd,
    )
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
    let mut o = base_event_json(Some(session_id), event_name, cwd);
    o["tool_name"] = serde_json::Value::String(tool_name.to_string());
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
///
/// `SessionUpdate` is `#[serde(tag = "sessionUpdate", rename_all = "snake_case")]`,
/// so each variant already carries a stable wire-format name. Looking that
/// name up in a small table — instead of a match with one arm per variant —
/// means this function does not need its own arm for every `SessionUpdate`
/// variant, and automatically returns `"unknown"` for variants added to the
/// (`#[non_exhaustive]`) upstream enum after this table was written.
fn notification_update_name(update: &SessionUpdate) -> &'static str {
    /// `(wire-format "sessionUpdate" tag, matcher/notification name)` pairs.
    const NAMES: &[(&str, &str)] = &[
        ("agent_message_chunk", "agent_message"),
        ("agent_thought_chunk", "agent_thought"),
        ("tool_call", "tool_call"),
        ("tool_call_update", "tool_call_update"),
        ("plan", "plan"),
        ("available_commands_update", "available_commands"),
        ("current_mode_update", "current_mode"),
        ("config_option_update", "config_option"),
        ("user_message_chunk", "user_message"),
    ];

    let wire_value = serde_json::to_value(update).ok();
    let wire_tag = wire_value
        .as_ref()
        .and_then(|value| value.get("sessionUpdate"))
        .and_then(serde_json::Value::as_str);

    NAMES
        .iter()
        .find_map(|(tag, name)| (Some(*tag) == wire_tag).then_some(*name))
        .unwrap_or("unknown")
}
