//! Claude Code hook strategy implementation.

use async_trait::async_trait;
use tokio::sync::Mutex;

use crate::builtin::load_builtins;
use crate::chain::{Chain, HookInputType};
use crate::context::{AvpContext, Decision, HookEvent};
use crate::error::AvpError;
use crate::types::{
    HookOutput, HookType, NotificationInput, PermissionRequestInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
use crate::validator::{ExecutedValidator, ValidatorLoader, ValidatorRunner};

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
/// directories with proper precedence, and executed via ACP agent.
pub struct ClaudeCodeHookStrategy {
    /// Strategies for each hook type
    pre_tool_use: PreToolUseHandler,
    post_tool_use: PostToolUseHandler,
    /// Validator loader with all loaded validators
    validator_loader: ValidatorLoader,
    /// AVP context for logging, directory access, and agent creation
    context: AvpContext,
    /// Cached validator runner (lazily initialized)
    runner_cache: Mutex<Option<ValidatorRunner>>,
}

impl std::fmt::Debug for ClaudeCodeHookStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeCodeHookStrategy")
            .field("pre_tool_use", &self.pre_tool_use)
            .field("post_tool_use", &self.post_tool_use)
            .field("validator_loader", &self.validator_loader)
            .field("context", &self.context)
            .field("runner_cache", &"<cached>")
            .finish()
    }
}

impl ClaudeCodeHookStrategy {
    /// Create a new Claude Code hook strategy with the given context.
    ///
    /// This loads all validators:
    /// 1. Builtin validators (embedded in the binary)
    /// 2. User validators (~/.avp/validators)
    /// 3. Project validators (./.avp/validators)
    ///
    /// The validator runner is created lazily when validators are executed,
    /// using the agent from the AvpContext.
    pub fn new(context: AvpContext) -> Self {
        let mut validator_loader = ValidatorLoader::new();

        // Load builtins first (lowest precedence)
        load_builtins(&mut validator_loader);

        // Load user and project validators (higher precedence) using context
        if let Err(e) = validator_loader.load_from_context(&context) {
            tracing::warn!("Failed to load validators from directories: {}", e);
        }

        tracing::debug!("Loaded {} validators", validator_loader.len());

        Self {
            pre_tool_use: PreToolUseHandler,
            post_tool_use: PostToolUseHandler,
            validator_loader,
            context,
            runner_cache: Mutex::new(None),
        }
    }

    /// Execute validators using the cached runner.
    ///
    /// The runner is created lazily on first access and reused for subsequent calls.
    /// This method handles the caching internally to avoid lifetime issues with Mutex guards.
    async fn execute_with_cached_runner(
        &self,
        matching: &[&crate::validator::Validator],
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Result<Vec<ExecutedValidator>, AvpError> {
        let mut guard = self.runner_cache.lock().await;

        // Create runner if not cached
        if guard.is_none() {
            tracing::debug!("Creating cached ValidatorRunner...");
            let (agent, notifications) = self.context.agent().await?;
            let runner = ValidatorRunner::new(agent, notifications)?;
            *guard = Some(runner);
            tracing::debug!("ValidatorRunner cached successfully");
        }

        // Execute with the cached runner
        let runner = guard.as_ref().unwrap();
        tracing::debug!(
            "Executing {} validators via cached ACP runner for hook {}",
            matching.len(),
            hook_type
        );
        Ok(runner.execute_validators(matching, hook_type, input).await)
    }

    /// Get a reference to the AVP context.
    pub fn context(&self) -> &AvpContext {
        &self.context
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
    /// Validators are executed via ACP agent obtained from the AvpContext.
    /// The ValidatorRunner is cached for performance (created once, reused).
    /// If the agent is unavailable (e.g., AVP_SKIP_AGENT is set), placeholder
    /// pass results are returned.
    ///
    /// Returns a list of executed validators with their results and metadata.
    pub async fn execute_validators(
        &self,
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Vec<ExecutedValidator> {
        let matching = self.matching_validators(hook_type, input);

        if matching.is_empty() {
            return Vec::new();
        }

        // Check if agent execution is disabled
        if std::env::var("AVP_SKIP_AGENT").is_ok() {
            tracing::debug!(
                "AVP_SKIP_AGENT set - returning placeholder results for {} validators",
                matching.len()
            );
            return self.placeholder_results(&matching, hook_type);
        }

        // Execute with cached runner
        match self.execute_with_cached_runner(&matching, hook_type, input).await {
            Ok(results) => results,
            Err(e) => {
                tracing::warn!("Failed to execute validators: {} - using placeholder results", e);
                self.placeholder_results(&matching, hook_type)
            }
        }
    }

    /// Generate placeholder pass results when agent is unavailable.
    fn placeholder_results(
        &self,
        matching: &[&crate::validator::Validator],
        hook_type: HookType,
    ) -> Vec<ExecutedValidator> {
        matching
            .iter()
            .map(|validator| {
                tracing::debug!(
                    "Would execute validator '{}' ({}) for hook {}",
                    validator.name(),
                    validator.source,
                    hook_type
                );

                ExecutedValidator {
                    name: validator.name().to_string(),
                    severity: validator.severity(),
                    result: crate::validator::ValidatorResult::pass(format!(
                        "Validator '{}' matched (runner unavailable)",
                        validator.name()
                    )),
                }
            })
            .collect()
    }

    /// Process a typed input through its chain.
    async fn process_typed<I: HookInputType>(
        &self,
        input: serde_json::Value,
        hook_type: HookType,
        handler: &impl TypedHookStrategy<I>,
    ) -> Result<(HookOutput, i32), AvpError> {
        // Execute matching validators
        let validator_results = self.execute_validators(hook_type, &input).await;

        // Check if any validator blocked (failed + error severity)
        if let Some(blocking) = validator_results.iter().find(|r| r.is_blocking()) {
            let reason = format!(
                "blocked by validator '{}': {}",
                blocking.name,
                blocking.message()
            );

            // Use the appropriate output type based on hook type
            // PostToolUse uses decision: "block" to flag the result to Claude
            // Other hooks use continue: false to stop execution
            let output = match hook_type {
                HookType::PostToolUse => HookOutput::post_tool_use_block(&reason),
                _ => HookOutput::blocking_error(&reason),
            };

            return Ok((output, 2)); // Exit code 2: blocking error
        }

        // Continue with normal processing
        let typed_input: I = serde_json::from_value(input).map_err(AvpError::Json)?;
        handler.process(typed_input).await
    }

    /// Default pass-through processing for hooks without custom logic.
    async fn process_passthrough<I: HookInputType>(
        &self,
        input: serde_json::Value,
        hook_type: HookType,
    ) -> Result<(HookOutput, i32), AvpError> {
        // Execute matching validators
        let validator_results = self.execute_validators(hook_type, &input).await;

        // Check if any validator blocked (failed + error severity)
        if let Some(blocking) = validator_results.iter().find(|r| r.is_blocking()) {
            let reason = format!(
                "blocked by validator '{}': {}",
                blocking.name,
                blocking.message()
            );

            // Use the appropriate output type based on hook type
            let output = match hook_type {
                HookType::PostToolUse => HookOutput::post_tool_use_block(&reason),
                _ => HookOutput::blocking_error(&reason),
            };

            return Ok((output, 2));
        }

        let typed_input: I = serde_json::from_value(input).map_err(AvpError::Json)?;

        let mut chain: Chain<I> = Chain::success();
        chain.execute(&typed_input).map_err(AvpError::Chain)
    }

    /// Get the validator loader for external access.
    pub fn validator_loader(&self) -> &ValidatorLoader {
        &self.validator_loader
    }

    /// Extract smart log details based on hook type.
    /// Only logs relevant info, not full payloads.
    fn extract_log_details(
        &self,
        hook_type: HookType,
        tool_name: &Option<String>,
        prompt_len: &Option<usize>,
        result: &Result<(HookOutput, i32), AvpError>,
    ) -> Option<String> {
        match hook_type {
            HookType::PreToolUse | HookType::PostToolUse | HookType::PostToolUseFailure => {
                // Log tool name
                tool_name.as_ref().map(|name| format!("tool={}", name))
            }
            HookType::UserPromptSubmit => {
                // Log prompt length, not content
                prompt_len.map(|len| format!("prompt_len={}", len))
            }
            _ => {
                // For block decisions, include stop reason
                if let Ok((output, _)) = result {
                    if !output.continue_execution {
                        return output
                            .stop_reason
                            .as_ref()
                            .map(|r| format!("reason=\"{}\"", r));
                    }
                }
                None
            }
        }
    }
}

#[async_trait(?Send)]
impl AgentHookStrategy for ClaudeCodeHookStrategy {
    fn name(&self) -> &'static str {
        "ClaudeCode"
    }

    fn can_handle(&self, input: &serde_json::Value) -> bool {
        // Claude Code hooks have hook_event_name field
        input.get("hook_event_name").is_some()
    }

    async fn process(&self, input: serde_json::Value) -> Result<(HookOutput, i32), AvpError> {
        let hook_type = self.extract_hook_type(&input)?;

        // Extract details for logging before moving input into processing functions
        let tool_name: Option<String> = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt_len: Option<usize> = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|p| p.len());

        let result = match hook_type {
            HookType::SessionStart => {
                self.process_passthrough::<SessionStartInput>(input, hook_type)
                    .await
            }
            HookType::UserPromptSubmit => {
                self.process_passthrough::<UserPromptSubmitInput>(input, hook_type)
                    .await
            }
            HookType::PreToolUse => {
                self.process_typed(input, hook_type, &self.pre_tool_use)
                    .await
            }
            HookType::PermissionRequest => {
                self.process_passthrough::<PermissionRequestInput>(input, hook_type)
                    .await
            }
            HookType::PostToolUse => {
                self.process_typed(input, hook_type, &self.post_tool_use)
                    .await
            }
            HookType::PostToolUseFailure => {
                self.process_passthrough::<PostToolUseFailureInput>(input, hook_type)
                    .await
            }
            HookType::SubagentStart => {
                self.process_passthrough::<SubagentStartInput>(input, hook_type)
                    .await
            }
            HookType::SubagentStop => {
                self.process_passthrough::<SubagentStopInput>(input, hook_type)
                    .await
            }
            HookType::Stop => {
                self.process_passthrough::<StopInput>(input, hook_type)
                    .await
            }
            HookType::PreCompact => {
                self.process_passthrough::<PreCompactInput>(input, hook_type)
                    .await
            }
            HookType::Setup => {
                self.process_passthrough::<SetupInput>(input, hook_type)
                    .await
            }
            HookType::SessionEnd => {
                self.process_passthrough::<SessionEndInput>(input, hook_type)
                    .await
            }
            HookType::Notification => {
                self.process_passthrough::<NotificationInput>(input, hook_type)
                    .await
            }
        };

        // Log the hook event with smart details based on hook type
        let hook_type_str = format!("{}", hook_type);
        let details = self.extract_log_details(hook_type, &tool_name, &prompt_len, &result);

        match &result {
            Ok((_, exit_code)) => {
                let decision = if *exit_code == 0 {
                    Decision::Allow
                } else {
                    Decision::Block
                };
                self.context.log_event(&HookEvent {
                    hook_type: &hook_type_str,
                    decision,
                    details,
                });
            }
            Err(e) => {
                let mut error_details = details.unwrap_or_default();
                if !error_details.is_empty() {
                    error_details.push(' ');
                }
                error_details.push_str(&format!("error={}", e));
                self.context.log_event(&HookEvent {
                    hook_type: &hook_type_str,
                    decision: Decision::Error,
                    details: Some(error_details),
                });
            }
        }

        result
    }
}

/// Handler for PreToolUse hooks.
#[derive(Debug, Default)]
pub struct PreToolUseHandler;

#[async_trait(?Send)]
impl TypedHookStrategy<PreToolUseInput> for PreToolUseHandler {
    async fn process(&self, input: PreToolUseInput) -> Result<(HookOutput, i32), AvpError> {
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

#[async_trait(?Send)]
impl TypedHookStrategy<PostToolUseInput> for PostToolUseHandler {
    async fn process(&self, input: PostToolUseInput) -> Result<(HookOutput, i32), AvpError> {
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
    use std::fs;
    use tempfile::TempDir;

    /// Helper to create a test strategy in a temporary git repo.
    fn create_test_strategy() -> (TempDir, ClaudeCodeHookStrategy) {
        let temp = TempDir::new().unwrap();
        fs::create_dir_all(temp.path().join(".git")).unwrap();

        // Disable agent execution in tests
        std::env::set_var("AVP_SKIP_AGENT", "1");

        // Change to temp directory to create context
        let original_dir = std::env::current_dir().unwrap();
        std::env::set_current_dir(temp.path()).unwrap();

        let context = AvpContext::init().unwrap();
        let strategy = ClaudeCodeHookStrategy::new(context);

        std::env::set_current_dir(&original_dir).unwrap();

        (temp, strategy)
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_claude_code_strategy_can_handle() {
        let (_temp, strategy) = create_test_strategy();

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

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_pre_tool_use_processing() {
        let (_temp, strategy) = create_test_strategy();

        let input = serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        });

        let (output, exit_code) = strategy.process(input).await.unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_session_start_processing() {
        let (_temp, strategy) = create_test_strategy();

        let input = serde_json::json!({
            "session_id": "test",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "SessionStart",
            "source": "startup"
        });

        let (output, exit_code) = strategy.process(input).await.unwrap();
        assert!(output.continue_execution);
        assert_eq!(exit_code, 0);
    }

    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_unknown_hook_type() {
        let (_temp, strategy) = create_test_strategy();

        let input = serde_json::json!({
            "hook_event_name": "UnknownHook"
        });

        let result = strategy.process(input).await;
        assert!(matches!(result, Err(AvpError::UnknownHookType(_))));
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_validators_loaded() {
        let (_temp, strategy) = create_test_strategy();

        // Should have at least the builtin validators
        assert!(strategy.validator_loader().len() >= 2);
        assert!(strategy.validator_loader().get("no-secrets").is_some());
        assert!(strategy.validator_loader().get("safe-commands").is_some());
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_matching_validators_pre_tool_use() {
        let (_temp, strategy) = create_test_strategy();

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
    #[serial_test::serial(cwd)]
    fn test_matching_validators_post_tool_use() {
        let (_temp, strategy) = create_test_strategy();

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
