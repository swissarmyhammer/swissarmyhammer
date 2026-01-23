//! Claude Code hook strategy implementation.

use crate::chain::{Chain, HookInputType};
use crate::error::AvpError;
use crate::types::{
    HookOutput, HookType, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};

use super::traits::{AgentHookStrategy, TypedHookStrategy};

/// Claude Code hook strategy.
///
/// This strategy handles all 13 hook types from Claude Code:
/// - SessionStart, SessionEnd
/// - UserPromptSubmit
/// - PreToolUse, PostToolUse, PostToolUseFailure
/// - PermissionRequest
/// - SubagentStart, SubagentStop
/// - Stop
/// - PreCompact
/// - Setup
/// - Notification
///
/// Each hook type has its own typed Input that is parsed from JSON and
/// processed through a chain of responsibility.
#[derive(Debug, Default)]
pub struct ClaudeCodeHookStrategy {
    /// Strategies for each hook type
    pre_tool_use: PreToolUseHandler,
    post_tool_use: PostToolUseHandler,
    // Add more handlers as needed for custom behavior
}

impl ClaudeCodeHookStrategy {
    /// Create a new Claude Code hook strategy with default handlers.
    pub fn new() -> Self {
        Self::default()
    }

    /// Extract the hook type from the input JSON.
    fn extract_hook_type(&self, input: &serde_json::Value) -> Result<HookType, AvpError> {
        let hook_name = input
            .get("hook_event_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| AvpError::MissingField("hook_event_name".to_string()))?;

        serde_json::from_value(serde_json::Value::String(hook_name.to_string()))
            .map_err(|_| AvpError::UnknownHookType(hook_name.to_string()))
    }

    /// Process a typed input through its chain.
    fn process_typed<I: HookInputType>(
        &self,
        input: serde_json::Value,
        handler: &impl TypedHookStrategy<I>,
    ) -> Result<(HookOutput, i32), AvpError> {
        let typed_input: I =
            serde_json::from_value(input).map_err(AvpError::Json)?;
        handler.process(typed_input)
    }

    /// Default pass-through processing for hooks without custom logic.
    fn process_passthrough<I: HookInputType>(
        &self,
        input: serde_json::Value,
    ) -> Result<(HookOutput, i32), AvpError> {
        let typed_input: I =
            serde_json::from_value(input).map_err(AvpError::Json)?;

        let mut chain: Chain<I> = Chain::success();
        chain.execute(&typed_input).map_err(AvpError::Chain)
    }
}

impl AgentHookStrategy for ClaudeCodeHookStrategy {
    fn name(&self) -> &'static str {
        "ClaudeCode"
    }

    fn can_handle(&self, input: &serde_json::Value) -> bool {
        // Claude Code hooks have hook_event_name field
        input.get("hook_event_name").is_some()
    }

    fn process(&self, input: serde_json::Value) -> Result<(HookOutput, i32), AvpError> {
        let hook_type = self.extract_hook_type(&input)?;

        match hook_type {
            HookType::SessionStart => {
                self.process_passthrough::<SessionStartInput>(input)
            }
            HookType::UserPromptSubmit => {
                self.process_passthrough::<UserPromptSubmitInput>(input)
            }
            HookType::PreToolUse => {
                self.process_typed(input, &self.pre_tool_use)
            }
            HookType::PermissionRequest => {
                self.process_passthrough::<PermissionRequestInput>(input)
            }
            HookType::PostToolUse => {
                self.process_typed(input, &self.post_tool_use)
            }
            HookType::PostToolUseFailure => {
                self.process_passthrough::<PostToolUseFailureInput>(input)
            }
            HookType::SubagentStart => {
                self.process_passthrough::<SubagentStartInput>(input)
            }
            HookType::SubagentStop => {
                self.process_passthrough::<SubagentStopInput>(input)
            }
            HookType::Stop => {
                self.process_passthrough::<StopInput>(input)
            }
            HookType::PreCompact => {
                self.process_passthrough::<PreCompactInput>(input)
            }
            HookType::Setup => {
                self.process_passthrough::<SetupInput>(input)
            }
            HookType::SessionEnd => {
                self.process_passthrough::<SessionEndInput>(input)
            }
            HookType::Notification => {
                self.process_passthrough::<NotificationInput>(input)
            }
        }
    }
}

/// Handler for PreToolUse hooks.
#[derive(Debug, Default)]
pub struct PreToolUseHandler;

impl TypedHookStrategy<PreToolUseInput> for PreToolUseHandler {
    fn process(&self, input: PreToolUseInput) -> Result<(HookOutput, i32), AvpError> {
        let mut chain: Chain<PreToolUseInput> = Chain::success();
        chain.execute(&input).map_err(AvpError::Chain)
    }

    fn name(&self) -> &'static str {
        "PreToolUseHandler"
    }
}

/// Handler for PostToolUse hooks.
#[derive(Debug, Default)]
pub struct PostToolUseHandler;

impl TypedHookStrategy<PostToolUseInput> for PostToolUseHandler {
    fn process(&self, input: PostToolUseInput) -> Result<(HookOutput, i32), AvpError> {
        let mut chain: Chain<PostToolUseInput> = Chain::success();
        chain.execute(&input).map_err(AvpError::Chain)
    }

    fn name(&self) -> &'static str {
        "PostToolUseHandler"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_claude_code_strategy_can_handle() {
        let strategy = ClaudeCodeHookStrategy::new();

        let valid_input = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "session_id": "test"
        });
        assert!(strategy.can_handle(&valid_input));

        let invalid_input = serde_json::json!({
            "some_other_field": "value"
        });
        assert!(!strategy.can_handle(&invalid_input));
    }

    #[test]
    fn test_pre_tool_use_processing() {
        let strategy = ClaudeCodeHookStrategy::new();

        let input = serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        });

        let (output, exit_code) = strategy.process(input).unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_session_start_processing() {
        let strategy = ClaudeCodeHookStrategy::new();

        let input = serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "SessionStart",
            "source": "startup"
        });

        let (output, exit_code) = strategy.process(input).unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_unknown_hook_type() {
        let strategy = ClaudeCodeHookStrategy::new();

        let input = serde_json::json!({
            "hook_event_name": "UnknownHook"
        });

        let result = strategy.process(input);
        assert!(matches!(result, Err(AvpError::UnknownHookType(_))));
    }
}
