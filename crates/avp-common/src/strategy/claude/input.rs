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

    /// Tool execution result (may be named tool_result, tool_output, or tool_response).
    #[serde(default, alias = "tool_output", alias = "tool_response")]
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

/// Elicitation hook input - fired when MCP server requests user input.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The MCP server name requesting elicitation.
    #[serde(default)]
    pub mcp_server_name: Option<String>,

    /// The message to display to the user.
    #[serde(default)]
    pub message: Option<String>,

    /// The elicitation mode (e.g., "blocking").
    #[serde(default)]
    pub mode: Option<String>,

    /// The requested input schema.
    #[serde(default)]
    pub requested_schema: Option<serde_json::Value>,
}

/// ElicitationResult hook input - fired when user responds to MCP elicitation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElicitationResultInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// The MCP server name that requested elicitation.
    #[serde(default)]
    pub mcp_server_name: Option<String>,

    /// The user's action (e.g., "submit", "cancel").
    #[serde(default)]
    pub action: Option<String>,

    /// The user's response content.
    #[serde(default)]
    pub content: Option<serde_json::Value>,

    /// Identifier for the elicitation request.
    #[serde(default)]
    pub elicitation_id: Option<String>,
}

/// InstructionsLoaded hook input - fired when CLAUDE.md or rules files are loaded.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstructionsLoadedInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Path to the loaded file.
    #[serde(default)]
    pub file_path: Option<String>,

    /// Reason the file was loaded.
    #[serde(default)]
    pub load_reason: Option<String>,

    /// Glob patterns used for discovery.
    #[serde(default)]
    pub glob_patterns: Option<Vec<String>>,

    /// Type of memory/instruction file.
    #[serde(default)]
    pub memory_type: Option<String>,
}

/// ConfigChange hook input - fired when config files change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigChangeInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Source of the config change (e.g., "user_settings", "project_settings").
    #[serde(default)]
    pub source: Option<String>,
}

/// WorktreeCreate hook input - fired when a worktree is created.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeCreateInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Path to the worktree.
    #[serde(default)]
    pub worktree_path: Option<String>,

    /// Branch name for the worktree.
    #[serde(default)]
    pub branch_name: Option<String>,
}

/// WorktreeRemove hook input - fired when a worktree is removed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorktreeRemoveInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Path to the worktree being removed.
    #[serde(default)]
    pub worktree_path: Option<String>,
}

/// PostCompact hook input - fired after context compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PostCompactInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,
}

/// TeammateIdle hook input - fired when an agent teammate goes idle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeammateIdleInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Identifier of the idle teammate.
    #[serde(default)]
    pub teammate_id: Option<String>,
}

/// TaskCompleted hook input - fired when a task is marked complete.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCompletedInput {
    /// Common input fields.
    #[serde(flatten)]
    pub common: CommonInput,

    /// Identifier of the completed task.
    #[serde(default)]
    pub task_id: Option<String>,

    /// Title of the completed task.
    #[serde(default)]
    pub task_title: Option<String>,
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
impl HookInputType for ElicitationInput {}
impl HookInputType for ElicitationResultInput {}
impl HookInputType for InstructionsLoadedInput {}
impl HookInputType for ConfigChangeInput {}
impl HookInputType for WorktreeCreateInput {}
impl HookInputType for WorktreeRemoveInput {}
impl HookInputType for PostCompactInput {}
impl HookInputType for TeammateIdleInput {}
impl HookInputType for TaskCompletedInput {}

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
    /// Elicitation hook input.
    Elicitation(ElicitationInput),
    /// ElicitationResult hook input.
    ElicitationResult(ElicitationResultInput),
    /// InstructionsLoaded hook input.
    InstructionsLoaded(InstructionsLoadedInput),
    /// ConfigChange hook input.
    ConfigChange(ConfigChangeInput),
    /// WorktreeCreate hook input.
    WorktreeCreate(WorktreeCreateInput),
    /// WorktreeRemove hook input.
    WorktreeRemove(WorktreeRemoveInput),
    /// PostCompact hook input.
    PostCompact(PostCompactInput),
    /// TeammateIdle hook input.
    TeammateIdle(TeammateIdleInput),
    /// TaskCompleted hook input.
    TaskCompleted(TaskCompletedInput),
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
            "Elicitation" => {
                let input: ElicitationInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::Elicitation(input))
            }
            "ElicitationResult" => {
                let input: ElicitationResultInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::ElicitationResult(input))
            }
            "InstructionsLoaded" => {
                let input: InstructionsLoadedInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::InstructionsLoaded(input))
            }
            "ConfigChange" => {
                let input: ConfigChangeInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::ConfigChange(input))
            }
            "WorktreeCreate" => {
                let input: WorktreeCreateInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::WorktreeCreate(input))
            }
            "WorktreeRemove" => {
                let input: WorktreeRemoveInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::WorktreeRemove(input))
            }
            "PostCompact" => {
                let input: PostCompactInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::PostCompact(input))
            }
            "TeammateIdle" => {
                let input: TeammateIdleInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::TeammateIdle(input))
            }
            "TaskCompleted" => {
                let input: TaskCompletedInput =
                    serde_json::from_value(value).map_err(D::Error::custom)?;
                Ok(HookInput::TaskCompleted(input))
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
                    "Elicitation",
                    "ElicitationResult",
                    "InstructionsLoaded",
                    "ConfigChange",
                    "WorktreeCreate",
                    "WorktreeRemove",
                    "PostCompact",
                    "TeammateIdle",
                    "TaskCompleted",
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
            HookInput::Elicitation(_) => HookType::Elicitation,
            HookInput::ElicitationResult(_) => HookType::ElicitationResult,
            HookInput::InstructionsLoaded(_) => HookType::InstructionsLoaded,
            HookInput::ConfigChange(_) => HookType::ConfigChange,
            HookInput::WorktreeCreate(_) => HookType::WorktreeCreate,
            HookInput::WorktreeRemove(_) => HookType::WorktreeRemove,
            HookInput::PostCompact(_) => HookType::PostCompact,
            HookInput::TeammateIdle(_) => HookType::TeammateIdle,
            HookInput::TaskCompleted(_) => HookType::TaskCompleted,
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
            HookInput::Elicitation(i) => &i.common,
            HookInput::ElicitationResult(i) => &i.common,
            HookInput::InstructionsLoaded(i) => &i.common,
            HookInput::ConfigChange(i) => &i.common,
            HookInput::WorktreeCreate(i) => &i.common,
            HookInput::WorktreeRemove(i) => &i.common,
            HookInput::PostCompact(i) => &i.common,
            HookInput::TeammateIdle(i) => &i.common,
            HookInput::TaskCompleted(i) => &i.common,
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
    fn test_elicitation_input_deserialization() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "Elicitation",
            "mcp_server_name": "sah",
            "message": "Pick one",
            "mode": "blocking",
            "requested_schema": {"type": "string"}
        }"#;
        let input: ElicitationInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.mcp_server_name.as_deref(), Some("sah"));
        assert_eq!(input.message.as_deref(), Some("Pick one"));
        assert_eq!(input.mode.as_deref(), Some("blocking"));
        assert_eq!(input.common.hook_event_name, HookType::Elicitation);
    }

    #[test]
    fn test_elicitation_result_input_deserialization() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/p",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "ElicitationResult",
            "mcp_server_name": "sah",
            "action": "submit",
            "content": {"answer": "yes"},
            "elicitation_id": "e-001"
        }"#;
        let input: ElicitationResultInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.mcp_server_name.as_deref(), Some("sah"));
        assert_eq!(input.action.as_deref(), Some("submit"));
        assert_eq!(input.elicitation_id.as_deref(), Some("e-001"));
    }

    #[test]
    fn test_new_hook_input_enum_dispatch() {
        let test_cases = vec![
            ("Elicitation", HookType::Elicitation),
            ("ElicitationResult", HookType::ElicitationResult),
            ("InstructionsLoaded", HookType::InstructionsLoaded),
            ("ConfigChange", HookType::ConfigChange),
            ("WorktreeCreate", HookType::WorktreeCreate),
            ("WorktreeRemove", HookType::WorktreeRemove),
            ("PostCompact", HookType::PostCompact),
            ("TeammateIdle", HookType::TeammateIdle),
            ("TaskCompleted", HookType::TaskCompleted),
        ];
        for (name, expected_type) in test_cases {
            let json = format!(
                r#"{{"session_id":"abc","transcript_path":"/p","cwd":"/c","permission_mode":"default","hook_event_name":"{}"}}"#,
                name
            );
            let input: HookInput = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to deserialize HookInput for {}: {}", name, e));
            assert_eq!(
                input.hook_type(),
                expected_type,
                "hook_type() mismatch for {}",
                name
            );
            assert_eq!(
                input.common().session_id.as_deref(),
                Some("abc"),
                "common() mismatch for {}",
                name
            );
        }
    }

    #[test]
    fn test_new_inputs_optional_fields_default() {
        // All event-specific fields are optional — deserialize with only common fields
        let names = [
            "Elicitation",
            "ElicitationResult",
            "InstructionsLoaded",
            "ConfigChange",
            "WorktreeCreate",
            "WorktreeRemove",
            "PostCompact",
            "TeammateIdle",
            "TaskCompleted",
        ];
        for name in &names {
            let json = format!(
                r#"{{"session_id":"s","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"{}"}}"#,
                name
            );
            let input: HookInput = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed with minimal fields for {}: {}", name, e));
            assert_eq!(input.hook_type().to_string(), *name);
        }
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

    /// Exhaustive test: every HookInput variant deserializes, returns correct
    /// hook_type(), and common() extracts session_id.
    #[test]
    fn test_hook_input_all_variants_deserialize_and_dispatch() {
        // Variants that need only common fields
        let simple_variants = vec![
            "SessionStart",
            "PreCompact",
            "Setup",
            "SessionEnd",
            "PostCompact",
        ];
        for name in &simple_variants {
            let json = format!(
                r#"{{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"{}"}}"#,
                name
            );
            let input: HookInput = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed to deserialize HookInput for {}: {}", name, e));
            assert_eq!(input.hook_type().to_string(), *name);
            assert_eq!(input.common().session_id.as_deref(), Some("s1"));
        }

        // Variants that need a prompt field
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"UserPromptSubmit","prompt":"hello"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::UserPromptSubmit);

        // Variants with tool_name + tool_input
        for name in &[
            "PreToolUse",
            "PostToolUse",
            "PostToolUseFailure",
            "PermissionRequest",
        ] {
            let json = format!(
                r#"{{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"{}","tool_name":"Bash","tool_input":{{"command":"ls"}}}}"#,
                name
            );
            let input: HookInput = serde_json::from_str(&json)
                .unwrap_or_else(|e| panic!("Failed for {}: {}", name, e));
            assert_eq!(input.hook_type().to_string(), *name);
        }

        // SubagentStart / SubagentStop
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"SubagentStart"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::SubagentStart);

        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"SubagentStop","stop_hook_active":false}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::SubagentStop);

        // Notification
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"Notification","message":"hi","notification_type":"info"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::Notification);

        // InstructionsLoaded with all optional fields
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"InstructionsLoaded","file_path":"/f","load_reason":"startup","glob_patterns":["*.md"],"memory_type":"project"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::InstructionsLoaded);

        // WorktreeCreate with all optional fields
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"WorktreeCreate","worktree_path":"/wt","branch_name":"feature"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::WorktreeCreate);

        // WorktreeRemove
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"WorktreeRemove","worktree_path":"/wt"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::WorktreeRemove);

        // TaskCompleted
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"TaskCompleted","task_id":"t1","task_title":"done"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::TaskCompleted);

        // TeammateIdle
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"TeammateIdle","teammate_id":"tm1"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::TeammateIdle);

        // ConfigChange
        let json = r#"{"session_id":"s1","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"ConfigChange","source":"user_settings"}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.hook_type(), HookType::ConfigChange);
    }

    #[test]
    fn test_hook_input_unknown_variant_error() {
        let json = r#"{"session_id":"s","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"UnknownHook"}"#;
        let result = serde_json::from_str::<HookInput>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("unknown variant"),
            "Error should mention unknown variant: {}",
            err
        );
    }

    #[test]
    fn test_hook_input_missing_hook_event_name_error() {
        let json =
            r#"{"session_id":"s","transcript_path":"/t","cwd":"/c","permission_mode":"default"}"#;
        let result = serde_json::from_str::<HookInput>(json);
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(
            err.contains("hook_event_name"),
            "Error should mention missing field: {}",
            err
        );
    }

    #[test]
    fn test_hook_input_serialize_round_trip() {
        // HookInput is Serialize - test that serialized output is valid JSON
        let json = r#"{"session_id":"s","transcript_path":"/t","cwd":"/c","permission_mode":"default","hook_event_name":"Stop","stop_hook_active":true}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        let serialized = serde_json::to_string(&input).unwrap();
        assert!(serialized.contains("stop_hook_active"));
    }

    #[test]
    fn test_session_start_input_optional_fields() {
        let json = r#"{
            "session_id": "abc",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SessionStart",
            "source": "startup",
            "model": "claude-sonnet"
        }"#;
        let input: SessionStartInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.source.as_deref(), Some("startup"));
        assert_eq!(input.model.as_deref(), Some("claude-sonnet"));
    }

    #[test]
    fn test_post_tool_use_input_aliases() {
        // tool_result can be named tool_output or tool_response
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {},
            "tool_output": "file contents"
        }"#;
        let input: PostToolUseInput = serde_json::from_str(json).unwrap();
        assert!(input.tool_result.is_some());

        let json2 = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PostToolUse",
            "tool_name": "Read",
            "tool_input": {},
            "tool_response": "file contents"
        }"#;
        let input2: PostToolUseInput = serde_json::from_str(json2).unwrap();
        assert!(input2.tool_result.is_some());
    }

    #[test]
    fn test_post_tool_use_failure_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PostToolUseFailure",
            "tool_name": "Write",
            "tool_input": {"file_path": "/x"},
            "error": {"message": "Permission denied"},
            "tool_use_id": "tu_456"
        }"#;
        let input: PostToolUseFailureInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Write");
        assert!(input.error.is_some());
        assert_eq!(input.tool_use_id.as_deref(), Some("tu_456"));
    }

    #[test]
    fn test_permission_request_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "PermissionRequest",
            "tool_name": "Bash",
            "tool_input": {"command": "rm -rf /"},
            "permission": "dangerous_command"
        }"#;
        let input: PermissionRequestInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.tool_name, "Bash");
        assert_eq!(input.permission.as_deref(), Some("dangerous_command"));
    }

    #[test]
    fn test_subagent_start_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SubagentStart",
            "subagent_type": "task_runner",
            "subagent_prompt": "run tests"
        }"#;
        let input: SubagentStartInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.subagent_type.as_deref(), Some("task_runner"));
        assert_eq!(input.subagent_prompt.as_deref(), Some("run tests"));
    }

    #[test]
    fn test_subagent_stop_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "SubagentStop",
            "stop_hook_active": true,
            "subagent_type": "task_runner"
        }"#;
        let input: SubagentStopInput = serde_json::from_str(json).unwrap();
        assert!(input.stop_hook_active);
        assert_eq!(input.subagent_type.as_deref(), Some("task_runner"));
    }

    #[test]
    fn test_stop_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Stop",
            "stop_hook_active": false
        }"#;
        let input: StopInput = serde_json::from_str(json).unwrap();
        assert!(!input.stop_hook_active);
    }

    #[test]
    fn test_notification_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "Notification",
            "message": "Build complete",
            "notification_type": "success"
        }"#;
        let input: NotificationInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.message.as_deref(), Some("Build complete"));
        assert_eq!(input.notification_type.as_deref(), Some("success"));
    }

    #[test]
    fn test_instructions_loaded_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "InstructionsLoaded",
            "file_path": "/project/CLAUDE.md",
            "load_reason": "project_init",
            "glob_patterns": ["*.md", ".claude/*"],
            "memory_type": "project"
        }"#;
        let input: InstructionsLoadedInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.file_path.as_deref(), Some("/project/CLAUDE.md"));
        assert_eq!(input.load_reason.as_deref(), Some("project_init"));
        assert_eq!(input.glob_patterns.as_ref().unwrap().len(), 2);
        assert_eq!(input.memory_type.as_deref(), Some("project"));
    }

    #[test]
    fn test_config_change_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "ConfigChange",
            "source": "project_settings"
        }"#;
        let input: ConfigChangeInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.source.as_deref(), Some("project_settings"));
    }

    #[test]
    fn test_worktree_create_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "WorktreeCreate",
            "worktree_path": "/tmp/wt-1",
            "branch_name": "feature-abc"
        }"#;
        let input: WorktreeCreateInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.worktree_path.as_deref(), Some("/tmp/wt-1"));
        assert_eq!(input.branch_name.as_deref(), Some("feature-abc"));
    }

    #[test]
    fn test_worktree_remove_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "WorktreeRemove",
            "worktree_path": "/tmp/wt-1"
        }"#;
        let input: WorktreeRemoveInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.worktree_path.as_deref(), Some("/tmp/wt-1"));
    }

    #[test]
    fn test_teammate_idle_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "TeammateIdle",
            "teammate_id": "agent-007"
        }"#;
        let input: TeammateIdleInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.teammate_id.as_deref(), Some("agent-007"));
    }

    #[test]
    fn test_task_completed_input() {
        let json = r#"{
            "session_id": "s",
            "transcript_path": "/t",
            "cwd": "/c",
            "permission_mode": "default",
            "hook_event_name": "TaskCompleted",
            "task_id": "task-123",
            "task_title": "Fix the bug"
        }"#;
        let input: TaskCompletedInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.task_id.as_deref(), Some("task-123"));
        assert_eq!(input.task_title.as_deref(), Some("Fix the bug"));
    }

    #[test]
    fn test_hook_input_common_returns_cwd() {
        let json = r#"{"session_id":"s","transcript_path":"/t","cwd":"/my/project","permission_mode":"plan","hook_event_name":"Stop","stop_hook_active":false}"#;
        let input: HookInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.common().cwd, "/my/project");
        assert_eq!(input.common().permission_mode, "plan");
    }
}
