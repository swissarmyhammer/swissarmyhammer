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
    /// MCP server requests user input.
    Elicitation,
    /// User responds to MCP elicitation.
    ElicitationResult,
    /// CLAUDE.md or rules files loaded.
    InstructionsLoaded,
    /// Config files change.
    ConfigChange,
    /// Worktree created.
    WorktreeCreate,
    /// Worktree removed.
    WorktreeRemove,
    /// After context compaction.
    PostCompact,
    /// Agent teammate goes idle.
    TeammateIdle,
    /// Task marked complete.
    TaskCompleted,
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
            HookType::Elicitation => write!(f, "Elicitation"),
            HookType::ElicitationResult => write!(f, "ElicitationResult"),
            HookType::InstructionsLoaded => write!(f, "InstructionsLoaded"),
            HookType::ConfigChange => write!(f, "ConfigChange"),
            HookType::WorktreeCreate => write!(f, "WorktreeCreate"),
            HookType::WorktreeRemove => write!(f, "WorktreeRemove"),
            HookType::PostCompact => write!(f, "PostCompact"),
            HookType::TeammateIdle => write!(f, "TeammateIdle"),
            HookType::TaskCompleted => write!(f, "TaskCompleted"),
        }
    }
}

/// Default permission mode when not provided.
fn default_permission_mode() -> String {
    "bypassPermissions".to_string()
}

/// Common fields present in all hook inputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommonInput {
    /// Unique session identifier. `None` for session-less events
    /// (e.g., InstructionsLoaded, WorktreeCreate, WorktreeRemove).
    pub session_id: Option<String>,

    /// Path to the transcript file. `None` for session-less events.
    pub transcript_path: Option<String>,

    /// Current working directory.
    pub cwd: String,

    /// Permission mode (e.g., "default", "plan", "bypassPermissions").
    /// Defaults to "default" if not provided by Claude Code.
    #[serde(default = "default_permission_mode")]
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
        assert_eq!(input.session_id.as_deref(), Some("abc123"));
        assert_eq!(input.hook_event_name, HookType::PreToolUse);
    }

    #[test]
    fn test_new_hook_type_serde_round_trip() {
        let new_variants = vec![
            (HookType::Elicitation, "\"Elicitation\""),
            (HookType::ElicitationResult, "\"ElicitationResult\""),
            (HookType::InstructionsLoaded, "\"InstructionsLoaded\""),
            (HookType::ConfigChange, "\"ConfigChange\""),
            (HookType::WorktreeCreate, "\"WorktreeCreate\""),
            (HookType::WorktreeRemove, "\"WorktreeRemove\""),
            (HookType::PostCompact, "\"PostCompact\""),
            (HookType::TeammateIdle, "\"TeammateIdle\""),
            (HookType::TaskCompleted, "\"TaskCompleted\""),
        ];
        for (variant, expected_json) in &new_variants {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(&json, expected_json, "serialize {:?}", variant);
            let deserialized: HookType = serde_json::from_str(expected_json).unwrap();
            assert_eq!(&deserialized, variant, "deserialize {}", expected_json);
        }
    }

    #[test]
    fn test_common_input_with_new_hook_types() {
        let new_names = [
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
        for name in &new_names {
            let json = format!(
                r#"{{"session_id":"abc","transcript_path":"/p","cwd":"/c","permission_mode":"default","hook_event_name":"{}"}}"#,
                name
            );
            let input: CommonInput =
                serde_json::from_str(&json).unwrap_or_else(|e| panic!("Failed to deserialize CommonInput with hook_event_name={}: {}", name, e));
            assert_eq!(input.hook_event_name.to_string(), *name);
        }
    }

    #[test]
    fn test_common_input_session_less_event() {
        // Session-less events (e.g. InstructionsLoaded) omit session_id and transcript_path entirely.
        let json = r#"{
            "cwd": "/home/user/project",
            "permission_mode": "default",
            "hook_event_name": "InstructionsLoaded"
        }"#;
        let input: CommonInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.session_id, None);
        assert_eq!(input.transcript_path, None);
        assert_eq!(input.hook_event_name, HookType::InstructionsLoaded);
    }

    #[test]
    fn test_common_input_without_permission_mode() {
        // Claude Code doesn't always send permission_mode for SessionStart
        let json = r#"{
            "session_id": "abc123",
            "transcript_path": "/path/to/transcript.jsonl",
            "cwd": "/home/user/project",
            "hook_event_name": "SessionStart"
        }"#;
        let input: CommonInput = serde_json::from_str(json).unwrap();
        assert_eq!(input.session_id.as_deref(), Some("abc123"));
        assert_eq!(input.permission_mode, "bypassPermissions");
        assert_eq!(input.hook_event_name, HookType::SessionStart);
    }
}
