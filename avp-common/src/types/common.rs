//! Common types shared across all hook inputs and outputs.

use serde::{Deserialize, Serialize};

/// All possible hook event types supported by Claude Code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookType {
    /// Session begins or resumes.
    SessionStart,
    /// User submits a prompt.
    UserPromptSubmit,
    /// Before tool execution.
    PreToolUse,
    /// When permission dialog appears.
    PermissionRequest,
    /// After tool succeeds.
    PostToolUse,
    /// After tool fails.
    PostToolUseFailure,
    /// When spawning a subagent.
    SubagentStart,
    /// When subagent finishes.
    SubagentStop,
    /// Claude finishes responding.
    Stop,
    /// Before context compaction.
    PreCompact,
    /// During repository initialization.
    Setup,
    /// Session terminates.
    SessionEnd,
    /// Claude Code sends notifications.
    Notification,
}

impl std::fmt::Display for HookType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HookType::SessionStart => write!(f, "SessionStart"),
            HookType::UserPromptSubmit => write!(f, "UserPromptSubmit"),
            HookType::PreToolUse => write!(f, "PreToolUse"),
            HookType::PermissionRequest => write!(f, "PermissionRequest"),
            HookType::PostToolUse => write!(f, "PostToolUse"),
            HookType::PostToolUseFailure => write!(f, "PostToolUseFailure"),
            HookType::SubagentStart => write!(f, "SubagentStart"),
            HookType::SubagentStop => write!(f, "SubagentStop"),
            HookType::Stop => write!(f, "Stop"),
            HookType::PreCompact => write!(f, "PreCompact"),
            HookType::Setup => write!(f, "Setup"),
            HookType::SessionEnd => write!(f, "SessionEnd"),
            HookType::Notification => write!(f, "Notification"),
        }
    }
}

/// Common fields present in all hook inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonInput {
    /// Unique session identifier.
    pub session_id: String,

    /// Path to the transcript file.
    pub transcript_path: String,

    /// Current working directory.
    pub cwd: String,

    /// Permission mode (e.g., "default", "plan", "bypassPermissions").
    pub permission_mode: String,

    /// The hook event name.
    pub hook_event_name: HookType,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_type_serialization() {
        let hook_type = HookType::PreToolUse;
        let json = serde_json::to_string(&hook_type).unwrap();
        assert_eq!(json, "\"PreToolUse\"");
    }

    #[test]
    fn test_hook_type_deserialization() {
        let json = "\"SessionStart\"";
        let hook_type: HookType = serde_json::from_str(json).unwrap();
        assert_eq!(hook_type, HookType::SessionStart);
    }

    #[test]
    fn test_common_input_deserialization() {
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse"
        }"#;
        let input: CommonInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.session_id, "abc123");
        assert_eq!(input.hook_event_name, HookType::PreToolUse);
    }
}
