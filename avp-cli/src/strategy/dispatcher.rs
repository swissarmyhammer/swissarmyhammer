//! Hook dispatcher for routing inputs to the correct strategy.

use crate::error::AvpError;
use crate::types::HookOutput;

use super::claude_code::ClaudeCodeHookStrategy;
use super::traits::AgentHookStrategy;

/// Dispatcher for routing hook inputs to the appropriate agent strategy.
///
/// The dispatcher maintains a list of agent strategies and routes
/// incoming hook requests to the first strategy that can handle them.
pub struct HookDispatcher {
    /// Registered agent strategies.
    strategies: Vec<Box<dyn AgentHookStrategy>>,
}

impl HookDispatcher {
    /// Create a new empty dispatcher.
    pub fn new() -> Self {
        Self {
            strategies: Vec::new(),
        }
    }

    /// Create a dispatcher with the Claude Code strategy registered.
    ///
    /// When adding support for a new agent platform, register it here:
    /// ```ignore
    /// dispatcher.register(NewAgentStrategy::new());
    /// ```
    pub fn with_claude_code() -> Self {
        let mut dispatcher = Self::new();
        dispatcher.register(ClaudeCodeHookStrategy::new());
        // Register additional agent strategies here as they are implemented
        dispatcher
    }

    /// Create a dispatcher with default strategies (alias for with_claude_code).
    pub fn with_defaults() -> Self {
        Self::with_claude_code()
    }

    /// Register an agent strategy.
    pub fn register<S: AgentHookStrategy + 'static>(&mut self, strategy: S) {
        self.strategies.push(Box::new(strategy));
    }

    /// Dispatch an input to the appropriate strategy.
    ///
    /// Returns the output and exit code from the first matching strategy.
    pub fn dispatch(&self, input: serde_json::Value) -> Result<(HookOutput, i32), AvpError> {
        // Find the first strategy that can handle this input
        for strategy in &self.strategies {
            if strategy.can_handle(&input) {
                return strategy.process(input);
            }
        }

        // No strategy found
        Err(AvpError::UnknownHookType(
            "No strategy found for input".to_string(),
        ))
    }

    /// Get the number of registered strategies.
    pub fn len(&self) -> usize {
        self.strategies.len()
    }

    /// Check if the dispatcher has no strategies.
    pub fn is_empty(&self) -> bool {
        self.strategies.is_empty()
    }
}

impl Default for HookDispatcher {
    fn default() -> Self {
        Self::with_claude_code()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dispatcher_with_claude_code() {
        let dispatcher = HookDispatcher::with_claude_code();
        assert_eq!(dispatcher.len(), 1);

        let input = serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        });

        let (output, exit_code) = dispatcher.dispatch(input).unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[test]
    fn test_dispatcher_no_strategy() {
        let dispatcher = HookDispatcher::new();

        let input = serde_json::json!({
            "some_field": "value"
        });

        let result = dispatcher.dispatch(input);
        assert!(matches!(result, Err(AvpError::UnknownHookType(_))));
    }

    #[test]
    fn test_dispatch_all_hook_types() {
        let dispatcher = HookDispatcher::with_defaults();

        // Build complete inputs for each hook type with all required fields
        let hook_inputs = [
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "SessionStart"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "UserPromptSubmit",
                "prompt": "test prompt"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PreToolUse",
                "tool_name": "Bash",
                "tool_input": {}
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PermissionRequest",
                "tool_name": "Bash",
                "tool_input": {}
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PostToolUse",
                "tool_name": "Bash",
                "tool_input": {},
                "tool_result": {}
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PostToolUseFailure",
                "tool_name": "Bash",
                "tool_input": {},
                "error": {}
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "SubagentStart"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "SubagentStop"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "Stop"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PreCompact"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "Setup"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "SessionEnd"
            }),
            serde_json::json!({
                "session_id": "test123",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "Notification"
            }),
        ];

        for input in hook_inputs {
            let hook_type = input
                .get("hook_event_name")
                .unwrap()
                .as_str()
                .unwrap()
                .to_string();
            let result = dispatcher.dispatch(input);
            assert!(result.is_ok(), "Failed for hook type: {}", hook_type);
        }
    }
}
