//! Claude Code hook input types.
//!
//! These types represent the JSON input format that Claude Code passes to hooks.
//! They are specific to Claude Code's hook system.

use serde::{Deserialize, Serialize};

use crate::chain::HookInputType;
use crate::types::{CommonInput, HookType};

/// SessionStart hook input - fired when session begins or resumes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStartInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Source of the session start (e.g., "startup").
    #[serde(default)]
    pub source: Option<String>,

    /// The model being used.
    #[serde(default)]
    pub model: Option<String>,
}

/// UserPromptSubmit hook input - fired when user submits a prompt.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPromptSubmitInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The user's prompt text.
    pub prompt: String,
}

/// PreToolUse hook input - fired before tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreToolUseInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The tool being invoked.
    pub tool_name: String,

    /// Tool input parameters.
    pub tool_input: serde_json::Value,

    /// Unique identifier for this tool use.
    #[serde(default)]
    pub tool_use_id: Option<String>,
}

/// PermissionRequest hook input - fired when permission dialog appears.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequestInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The tool requesting permission.
    pub tool_name: String,

    /// Tool input parameters.
    pub tool_input: serde_json::Value,

    /// The permission being requested.
    #[serde(default)]
    pub permission: Option<String>,
}

/// PostToolUse hook input - fired after tool succeeds.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The tool that was invoked.
    pub tool_name: String,

    /// Tool input parameters.
    pub tool_input: serde_json::Value,

    /// Tool execution result (may be named tool_result or tool_output).
    #[serde(default, alias = "tool_output")]
    pub tool_result: Option<serde_json::Value>,

    /// Unique identifier for this tool use.
    #[serde(default)]
    pub tool_use_id: Option<String>,
}

/// PostToolUseFailure hook input - fired after tool fails.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostToolUseFailureInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The tool that failed.
    pub tool_name: String,

    /// Tool input parameters.
    pub tool_input: serde_json::Value,

    /// Error information from the failure.
    #[serde(default)]
    pub error: Option<serde_json::Value>,

    /// Unique identifier for this tool use.
    #[serde(default)]
    pub tool_use_id: Option<String>,
}

/// SubagentStart hook input - fired when spawning a subagent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStartInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The subagent type being spawned.
    #[serde(default)]
    pub subagent_type: Option<String>,

    /// The prompt given to the subagent.
    #[serde(default)]
    pub subagent_prompt: Option<String>,
}

/// SubagentStop hook input - fired when subagent finishes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubagentStopInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Whether the stop hook is active.
    #[serde(default)]
    pub stop_hook_active: bool,

    /// The subagent type that finished.
    #[serde(default)]
    pub subagent_type: Option<String>,
}

/// Stop hook input - fired when Claude finishes responding.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StopInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Whether the stop hook is active.
    #[serde(default)]
    pub stop_hook_active: bool,
}

/// PreCompact hook input - fired before context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PreCompactInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,
}

/// Setup hook input - fired during repository initialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetupInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,
}

/// SessionEnd hook input - fired when session terminates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionEndInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,
}

/// Notification hook input - fired when Claude Code sends notifications.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The notification message.
    #[serde(default)]
    pub message: Option<String>,

    /// The notification type.
    #[serde(default)]
    pub notification_type: Option<String>,
}

// Implement HookInputType for all input types
impl HookInputType for SessionStartInput {}
impl HookInputType for UserPromptSubmitInput {}
impl HookInputType for PreToolUseInput {}
impl HookInputType for PermissionRequestInput {}
impl HookInputType for PostToolUseInput {}
impl HookInputType for PostToolUseFailureInput {}
impl HookInputType for SubagentStartInput {}
impl HookInputType for SubagentStopInput {}
impl HookInputType for StopInput {}
impl HookInputType for PreCompactInput {}
impl HookInputType for SetupInput {}
impl HookInputType for SessionEndInput {}
impl HookInputType for NotificationInput {}

/// Enum wrapper for all possible hook inputs, enabling type-safe dispatch.
#[derive(Debug, Clone, Serialize)]
pub enum HookInput {
    /// SessionStart hook input.
    SessionStart(SessionStartInput),
    /// UserPromptSubmit hook input.
    UserPromptSubmit(UserPromptSubmitInput),
    /// PreToolUse hook input.
    PreToolUse(PreToolUseInput),
    /// PermissionRequest hook input.
    PermissionRequest(PermissionRequestInput),
    /// PostToolUse hook input.
    PostToolUse(PostToolUseInput),
    /// PostToolUseFailure hook input.
    PostToolUseFailure(PostToolUseFailureInput),
    /// SubagentStart hook input.
    SubagentStart(SubagentStartInput),
    /// SubagentStop hook input.
    SubagentStop(SubagentStopInput),
    /// Stop hook input.
    Stop(StopInput),
    /// PreCompact hook input.
    PreCompact(PreCompactInput),
    /// Setup hook input.
    Setup(SetupInput),
    /// SessionEnd hook input.
    SessionEnd(SessionEndInput),
    /// Notification hook input.
    Notification(NotificationInput),
}

impl<'de> Deserialize<'de> for HookInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de::Error;

        // First deserialize to a generic Value to peek at hook_event_name
        let value = serde_json::Value::deserialize(deserializer)?;

        // Extract the hook_event_name
        let hook_name = value
            .get("hook_event_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| D::Error::missing_field("hook_event_name"))?;

        // Parse to the appropriate variant
        match hook_name {
            "SessionStart" => {
                let input: SessionStartInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::SessionStart(input))
            }
            "UserPromptSubmit" => {
                let input: UserPromptSubmitInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::UserPromptSubmit(input))
            }
            "PreToolUse" => {
                let input: PreToolUseInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PreToolUse(input))
            }
            "PermissionRequest" => {
                let input: PermissionRequestInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PermissionRequest(input))
            }
            "PostToolUse" => {
                let input: PostToolUseInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PostToolUse(input))
            }
            "PostToolUseFailure" => {
                let input: PostToolUseFailureInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PostToolUseFailure(input))
            }
            "SubagentStart" => {
                let input: SubagentStartInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::SubagentStart(input))
            }
            "SubagentStop" => {
                let input: SubagentStopInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::SubagentStop(input))
            }
            "Stop" => {
                let input: StopInput = serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::Stop(input))
            }
            "PreCompact" => {
                let input: PreCompactInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PreCompact(input))
            }
            "Setup" => {
                let input: SetupInput = serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::Setup(input))
            }
            "SessionEnd" => {
                let input: SessionEndInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::SessionEnd(input))
            }
            "Notification" => {
                let input: NotificationInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::Notification(input))
            }
            other => Err(D::Error::unknown_variant(
                other,
                &[
                    "SessionStart",
                    "UserPromptSubmit",
                    "PreToolUse",
                    "PermissionRequest",
                    "PostToolUse",
                    "PostToolUseFailure",
                    "SubagentStart",
                    "SubagentStop",
                    "Stop",
                    "PreCompact",
                    "Setup",
                    "SessionEnd",
                    "Notification",
                ],
            )),
        }
    }
}

impl HookInput {
    /// Get the hook type for this input.
    pub fn hook_type(&self) -> HookType {
        match self {
            HookInput::SessionStart(_) => HookType::SessionStart,
            HookInput::UserPromptSubmit(_) => HookType::UserPromptSubmit,
            HookInput::PreToolUse(_) => HookType::PreToolUse,
            HookInput::PermissionRequest(_) => HookType::PermissionRequest,
            HookInput::PostToolUse(_) => HookType::PostToolUse,
            HookInput::PostToolUseFailure(_) => HookType::PostToolUseFailure,
            HookInput::SubagentStart(_) => HookType::SubagentStart,
            HookInput::SubagentStop(_) => HookType::SubagentStop,
            HookInput::Stop(_) => HookType::Stop,
            HookInput::PreCompact(_) => HookType::PreCompact,
            HookInput::Setup(_) => HookType::Setup,
            HookInput::SessionEnd(_) => HookType::SessionEnd,
            HookInput::Notification(_) => HookType::Notification,
        }
    }

    /// Get the common input fields.
    pub fn common(&self) -> &CommonInput {
        match self {
            HookInput::SessionStart(i) => &i.common,
            HookInput::UserPromptSubmit(i) => &i.common,
            HookInput::PreToolUse(i) => &i.common,
            HookInput::PermissionRequest(i) => &i.common,
            HookInput::PostToolUse(i) => &i.common,
            HookInput::PostToolUseFailure(i) => &i.common,
            HookInput::SubagentStart(i) => &i.common,
            HookInput::SubagentStop(i) => &i.common,
            HookInput::Stop(i) => &i.common,
            HookInput::PreCompact(i) => &i.common,
            HookInput::Setup(i) => &i.common,
            HookInput::SessionEnd(i) => &i.common,
            HookInput::Notification(i) => &i.common,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pre_tool_use_input_deserialization() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls -la"},
            "tool_use_id": "toolu_123"
        }"#;
        let input: PreToolUseInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Bash");
        assert_eq!(input.common.hook_event_name, HookType::PreToolUse);
    }

    #[test]
    fn test_user_prompt_submit_input() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "UserPromptSubmit",
            "prompt": "Write a function"
        }"#;
        let input: UserPromptSubmitInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.prompt, "Write a function");
    }

    #[test]
    fn test_hook_input_enum() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "Stop",
            "stop_hook_active": true
        }"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::Stop);
    }
}
