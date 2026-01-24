//! Claude Code hook strategy implementation.

use crate::builtin::load_builtins;
use crate::chain::{Chain, HookInputType};
use crate::error::AvpError;
use crate::types::{
    HookOutput, HookType, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
use crate::validator::{ValidatorLoader, ValidatorResult};

use super::traits::{AgentHookStrategy, TypedHookStrategy};

/// Claude Code hook strategy with validator support.
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
/// processed through a chain of responsibility. Validators are loaded
/// from builtin, user (~/.avp/validators), and project (./.avp/validators)
/// directories with proper precedence.
#[derive(Debug)]
pub struct ClaudeCodeHookStrategy {
    /// Strategies for each hook type
    pre_tool_use: PreToolUseHandler,
    post_tool_use: PostToolUseHandler,
    /// Validator loader with all loaded validators
    validator_loader: ValidatorLoader,
}

impl Default for ClaudeCodeHookStrategy {
    fn default() -> Self {
        Self::new()
    }
}

impl ClaudeCodeHookStrategy {
    /// Create a new Claude Code hook strategy with default handlers.
    ///
    /// This loads all validators:
    /// 1. Builtin validators (embedded in the binary)
    /// 2. User validators (~/.avp/validators)
    /// 3. Project validators (./.avp/validators)
    pub fn new() -> Self {
        let mut validator_loader = ValidatorLoader::new();

        // Load builtins first (lowest precedence)
        load_builtins(&mut validator_loader);

        // Load user and project validators (higher precedence)
        if let Err(e) = validator_loader.load_all() {
            tracing::warn!("Failed to load validators from directories: {}", e);
        }

        tracing::debug!(
            "Loaded {} validators",
            validator_loader.len()
        );

        Self {
            pre_tool_use: PreToolUseHandler,
            post_tool_use: PostToolUseHandler,
            validator_loader,
        }
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

    /// Get matching validators for the current hook event.
    pub fn matching_validators(
        &self,
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Vec<&crate::validator::Validator> {
        let ctx = crate::validator::MatchContext::from_json(hook_type, input);
        self.validator_loader.matching(&ctx)
    }

    /// Execute matching validators for a hook event.
    ///
    /// Currently, this returns placeholder results. In the future, this will
    /// execute validators via ACP agent calls.
    ///
    /// Returns a list of validator results.
    pub fn execute_validators(
        &self,
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Vec<ValidatorResult> {
        let matching = self.matching_validators(hook_type, input);

        let mut results = Vec::new();

        for validator in matching {
            tracing::debug!(
                "Would execute validator '{}' ({}) for hook {}",
                validator.name(),
                validator.source,
                hook_type
            );

            // TODO: Execute via ACP agent
            // For now, return a placeholder pass result
            results.push(ValidatorResult::pass(
                validator.name(),
                format!("Validator '{}' matched (execution pending)", validator.name()),
            ));
        }

        results
    }

    /// Process a typed input through its chain.
    fn process_typed<I: HookInputType>(
        &self,
        input: serde_json::Value,
        hook_type: HookType,
        handler: &impl TypedHookStrategy<I>,
    ) -> Result<(HookOutput, i32), AvpError> {
        // Execute matching validators
        let validator_results = self.execute_validators(hook_type, &input);

        // Check if any validator blocked
        let blocked = validator_results.iter().any(|r| {
            !r.passed() && r.severity() == Some(crate::validator::Severity::Error)
        });

        if blocked {
            // Find the blocking result for the message
            let blocking = validator_results
                .iter()
                .find(|r| !r.passed() && r.severity() == Some(crate::validator::Severity::Error))
                .unwrap();

            return Ok((
                HookOutput::blocking_error(format!("blocked by validator: {}", blocking.message())),
                2, // Blocking exit code
            ));
        }

        // Continue with normal processing
        let typed_input: I = serde_json::from_value(input).map_err(AvpError::Json)?;
        handler.process(typed_input)
    }

    /// Default pass-through processing for hooks without custom logic.
    fn process_passthrough<I: HookInputType>(
        &self,
        input: serde_json::Value,
        hook_type: HookType,
    ) -> Result<(HookOutput, i32), AvpError> {
        // Execute matching validators
        let validator_results = self.execute_validators(hook_type, &input);

        // Check if any validator blocked
        let blocked = validator_results.iter().any(|r| {
            !r.passed() && r.severity() == Some(crate::validator::Severity::Error)
        });

        if blocked {
            let blocking = validator_results
                .iter()
                .find(|r| !r.passed() && r.severity() == Some(crate::validator::Severity::Error))
                .unwrap();

            return Ok((
                HookOutput::blocking_error(format!("blocked by validator: {}", blocking.message())),
                2,
            ));
        }

        let typed_input: I = serde_json::from_value(input).map_err(AvpError::Json)?;

        let mut chain: Chain<I> = Chain::success();
        chain.execute(&typed_input).map_err(AvpError::Chain)
    }

    /// Get the validator loader for external access.
    pub fn validator_loader(&self) -> &ValidatorLoader {
        &self.validator_loader
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
            HookType::SessionStart => self.process_passthrough::<SessionStartInput>(input, hook_type),
            HookType::UserPromptSubmit => {
                self.process_passthrough::<UserPromptSubmitInput>(input, hook_type)
            }
            HookType::PreToolUse => self.process_typed(input, hook_type, &self.pre_tool_use),
            HookType::PermissionRequest => {
                self.process_passthrough::<PermissionRequestInput>(input, hook_type)
            }
            HookType::PostToolUse => self.process_typed(input, hook_type, &self.post_tool_use),
            HookType::PostToolUseFailure => {
                self.process_passthrough::<PostToolUseFailureInput>(input, hook_type)
            }
            HookType::SubagentStart => {
                self.process_passthrough::<SubagentStartInput>(input, hook_type)
            }
            HookType::SubagentStop => self.process_passthrough::<SubagentStopInput>(input, hook_type),
            HookType::Stop => self.process_passthrough::<StopInput>(input, hook_type),
            HookType::PreCompact => self.process_passthrough::<PreCompactInput>(input, hook_type),
            HookType::Setup => self.process_passthrough::<SetupInput>(input, hook_type),
            HookType::SessionEnd => self.process_passthrough::<SessionEndInput>(input, hook_type),
            HookType::Notification => {
                self.process_passthrough::<NotificationInput>(input, hook_type)
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

    #[test]
    fn test_validators_loaded() {
        let strategy = ClaudeCodeHookStrategy::new();

        // Should have at least the builtin validators
        assert!(strategy.validator_loader().len() >= 2);
        assert!(strategy.validator_loader().get("no-secrets").is_some());
        assert!(strategy.validator_loader().get("safe-commands").is_some());
    }

    #[test]
    fn test_matching_validators_pre_tool_use() {
        let strategy = ClaudeCodeHookStrategy::new();

        let input = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        });

        let matching = strategy.matching_validators(HookType::PreToolUse, &input);

        // safe-commands should match PreToolUse + Bash
        let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
        assert!(names.contains(&"safe-commands"));
    }

    #[test]
    fn test_matching_validators_post_tool_use() {
        let strategy = ClaudeCodeHookStrategy::new();

        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "test.ts"}
        });

        let matching = strategy.matching_validators(HookType::PostToolUse, &input);

        // no-secrets should match PostToolUse + Write + *.ts
        let names: Vec<_> = matching.iter().map(|v| v.name()).collect();
        assert!(names.contains(&"no-secrets"));
    }

}
