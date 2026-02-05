//! Claude Code hook strategy implementation.

use std::sync::Arc;

use async_trait::async_trait;

use crate::builtin::load_builtins;
use crate::chain::{Chain, ChainFactory, ChainOutput};
use crate::context::{AvpContext, Decision, HookEvent};
use crate::error::AvpError;
use crate::types::HookType;

// Import Claude-specific types from sibling modules
use super::input::{
    NotificationInput, PermissionRequestInput, PostToolUseFailureInput, PostToolUseInput,
    PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput, SetupInput, StopInput,
    SubagentStartInput, SubagentStopInput, UserPromptSubmitInput,
};
use super::output::{
    HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PreToolUseOutput,
};
use crate::validator::ValidatorLoader;

use crate::strategy::traits::AgentHookStrategy;

/// Claude Code hook strategy with validator support.
///
/// This strategy handles all 13 hook types from Claude Code.
/// Processing is delegated to chains created by the ChainFactory,
/// which include:
/// - File tracking links (for detecting changed files)
/// - Validator executor links (for running validators)
///
/// The chain handles ALL processing - the strategy just routes
/// to the appropriate chain based on hook type.
pub struct ClaudeCodeHookStrategy {
    /// Chain factory for creating chains with proper links
    chain_factory: ChainFactory,
    /// Validator loader (kept for matching_validators helper)
    validator_loader: Arc<ValidatorLoader>,
    /// AVP context for logging
    context: Arc<AvpContext>,
}

impl std::fmt::Debug for ClaudeCodeHookStrategy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ClaudeCodeHookStrategy")
            .field("validator_loader", &self.validator_loader)
            .finish()
    }
}

impl ClaudeCodeHookStrategy {
    /// Create a new Claude Code hook strategy with the given context.
    ///
    /// This loads all validators and creates a ChainFactory that will
    /// handle all processing through chains with proper links.
    pub fn new(context: AvpContext) -> Self {
        let mut validator_loader = ValidatorLoader::new();

        // Load builtins first (lowest precedence)
        load_builtins(&mut validator_loader);

        // Load user and project validators (higher precedence)
        if let Err(e) = validator_loader.load_from_context(&context) {
            tracing::warn!("Failed to load validators from directories: {}", e);
        }

        tracing::debug!("Loaded {} validators", validator_loader.len());

        let context = Arc::new(context);
        let validator_loader = Arc::new(validator_loader);
        // Use the turn state manager from the context
        let turn_state = context.turn_state();

        let chain_factory =
            ChainFactory::new(context.clone(), validator_loader.clone(), turn_state);

        Self {
            chain_factory,
            validator_loader,
            context,
        }
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
    ///
    /// This is a helper for tests and introspection - actual validation
    /// is done by the chain's ValidatorExecutorLink.
    pub fn matching_validators(
        &self,
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Vec<&crate::validator::Validator> {
        let ctx = crate::validator::MatchContext::from_json(hook_type, input);
        self.validator_loader.matching(&ctx)
    }

    /// Find RuleSets matching a hook event.
    ///
    /// Returns all RuleSets that match the given hook type and input context.
    pub fn matching_rulesets(
        &self,
        hook_type: HookType,
        input: &serde_json::Value,
    ) -> Vec<&crate::validator::RuleSet> {
        let ctx = crate::validator::MatchContext::from_json(hook_type, input);
        self.validator_loader.matching_rulesets(&ctx)
    }

    /// Get the validator loader for external access.
    pub fn validator_loader(&self) -> &ValidatorLoader {
        &self.validator_loader
    }

    /// Extract smart log details based on hook type.
    fn extract_log_details(
        &self,
        hook_type: HookType,
        tool_name: &Option<String>,
        prompt_len: &Option<usize>,
        chain_output: &ChainOutput,
    ) -> Option<String> {
        match hook_type {
            HookType::PreToolUse | HookType::PostToolUse | HookType::PostToolUseFailure => {
                tool_name.as_ref().map(|name| format!("tool={}", name))
            }
            HookType::UserPromptSubmit => prompt_len.map(|len| format!("prompt_len={}", len)),
            _ => {
                if !chain_output.continue_execution {
                    return chain_output
                        .stop_reason
                        .as_ref()
                        .map(|r| format!("reason=\"{}\"", r));
                }
                None
            }
        }
    }

    /// Transform agent-agnostic ChainOutput to Claude Code-specific HookOutput.
    ///
    /// This is where all Claude-specific formatting happens, based on the hook type:
    /// - PreToolUse: hookSpecificOutput.permissionDecision: "deny"
    /// - PermissionRequest: hookSpecificOutput.decision.behavior: "deny"
    /// - PostToolUse/PostToolUseFailure: decision: "block", reason
    /// - Stop/SubagentStop: decision: "block", reason, continue: true
    /// - UserPromptSubmit: decision: "block", reason
    /// - Other hooks: continue: false, stopReason
    fn transform_to_claude_output(
        chain_output: ChainOutput,
        hook_type: HookType,
    ) -> (HookOutput, i32) {
        // If no validator blocked, return success
        if chain_output.validator_block.is_none() && chain_output.continue_execution {
            return (
                HookOutput {
                    continue_execution: true,
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                },
                0,
            );
        }

        // Get validator block info
        let (validator_name, message) = if let Some(ref block) = chain_output.validator_block {
            (block.validator_name.clone(), block.message.clone())
        } else {
            (
                "unknown".to_string(),
                chain_output
                    .stop_reason
                    .clone()
                    .unwrap_or_else(|| "Unknown reason".to_string()),
            )
        };

        let reason = format!("blocked by validator '{}': {}", validator_name, message);

        // Transform based on hook type per Claude Code docs
        match hook_type {
            // PreToolUse: use hookSpecificOutput.permissionDecision: "deny"
            HookType::PreToolUse => {
                let output = HookOutput {
                    continue_execution: true, // Exit 0 so JSON is parsed
                    hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                        permission_decision: Some(PermissionDecision::Deny),
                        permission_decision_reason: Some(reason),
                        ..Default::default()
                    })),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 0)
            }

            // PermissionRequest: use hookSpecificOutput.decision.behavior: "deny"
            HookType::PermissionRequest => {
                let output = HookOutput {
                    continue_execution: true, // Exit 0 so JSON is parsed
                    hook_specific_output: Some(HookSpecificOutput::PermissionRequest(
                        PermissionRequestOutput {
                            decision: Some(PermissionRequestDecision {
                                behavior: PermissionBehavior::Deny,
                                message: Some(reason),
                                updated_input: None,
                                interrupt: false,
                            }),
                        },
                    )),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 0)
            }

            // PostToolUse/PostToolUseFailure: decision: "block" + reason
            HookType::PostToolUse | HookType::PostToolUseFailure => {
                let output = HookOutput {
                    continue_execution: true, // Tool already ran
                    decision: Some("block".to_string()),
                    reason: Some(reason),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 0)
            }

            // Stop/SubagentStop: decision: "block" + reason, continue: true
            HookType::Stop | HookType::SubagentStop => {
                let output = HookOutput {
                    continue_execution: true, // Claude MUST continue, can't stop
                    stop_reason: Some(reason.clone()),
                    decision: Some("block".to_string()),
                    reason: Some(reason),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 0)
            }

            // UserPromptSubmit: decision: "block" + reason
            HookType::UserPromptSubmit => {
                let output = HookOutput {
                    continue_execution: true, // Exit 0 so JSON is parsed
                    decision: Some("block".to_string()),
                    reason: Some(reason),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 0)
            }

            // Other hooks: use stderr format (exit code 2)
            _ => {
                let output = HookOutput {
                    continue_execution: false,
                    stop_reason: Some(reason),
                    system_message: chain_output.system_message,
                    suppress_output: chain_output.suppress_output,
                    ..Default::default()
                };
                (output, 2)
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
        input.get("hook_event_name").is_some()
    }

    async fn process(&self, input: serde_json::Value) -> Result<(HookOutput, i32), AvpError> {
        let hook_type = self.extract_hook_type(&input)?;

        // Extract details for logging
        let tool_name: Option<String> = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let prompt_len: Option<usize> = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .map(|p| p.len());

        // Route to appropriate chain - the chain handles everything:
        // file tracking, validator execution, etc.
        // Chain returns ChainOutput (agent-agnostic), which we transform to HookOutput (Claude-specific)
        let chain_result = match hook_type {
            HookType::SessionStart => {
                let typed: SessionStartInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                self.chain_factory
                    .session_start_chain()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::SessionEnd => {
                let typed: SessionEndInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                self.chain_factory
                    .session_end_chain()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::PreToolUse => {
                let typed: PreToolUseInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                self.chain_factory
                    .pre_tool_use_chain()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::PostToolUse => {
                let typed: PostToolUseInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                self.chain_factory
                    .post_tool_use_chain()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::Stop => {
                let typed: StopInput = serde_json::from_value(input).map_err(AvpError::Json)?;
                self.chain_factory
                    .stop_chain()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            // Pass-through hooks - no file tracking or validators, just allow
            HookType::UserPromptSubmit => {
                let typed: UserPromptSubmitInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::PermissionRequest => {
                let typed: PermissionRequestInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::PostToolUseFailure => {
                let typed: PostToolUseFailureInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::SubagentStart => {
                let typed: SubagentStartInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::SubagentStop => {
                let typed: SubagentStopInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::PreCompact => {
                let typed: PreCompactInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::Setup => {
                let typed: SetupInput = serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
            HookType::Notification => {
                let typed: NotificationInput =
                    serde_json::from_value(input).map_err(AvpError::Json)?;
                Chain::success()
                    .execute(&typed)
                    .await
                    .map_err(AvpError::Chain)
            }
        };

        // Log the hook event based on chain result
        let hook_type_str = format!("{}", hook_type);
        match &chain_result {
            Ok((chain_output, _)) => {
                let details =
                    self.extract_log_details(hook_type, &tool_name, &prompt_len, chain_output);
                let decision =
                    if chain_output.continue_execution && chain_output.validator_block.is_none() {
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
                self.context.log_event(&HookEvent {
                    hook_type: &hook_type_str,
                    decision: Decision::Error,
                    details: Some(format!("error={}", e)),
                });
            }
        }

        // Transform ChainOutput to Claude-specific HookOutput
        chain_result
            .map(|(chain_output, _)| Self::transform_to_claude_output(chain_output, hook_type))
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
    fn test_rulesets_loaded() {
        let (_temp, strategy) = create_test_strategy();

        // Should have at least the builtin RuleSets
        assert!(strategy.validator_loader().ruleset_count() >= 5, "Should have at least 5 builtin RuleSets");
        assert!(strategy.validator_loader().get_ruleset("security-rules").is_some(), "Should have security-rules");
        assert!(strategy.validator_loader().get_ruleset("command-safety").is_some(), "Should have command-safety");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_matching_rulesets_pre_tool_use() {
        let (_temp, strategy) = create_test_strategy();

        let input = serde_json::json!({
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls"}
        });

        let ctx = crate::validator::MatchContext::from_json(HookType::PreToolUse, &input);
        let matching = strategy.validator_loader().matching_rulesets(&ctx);

        // command-safety should match PreToolUse + Bash
        let names: Vec<_> = matching.iter().map(|rs| rs.name()).collect();
        assert!(names.contains(&"command-safety"), "command-safety should match PreToolUse + Bash");
    }

    #[test]
    #[serial_test::serial(cwd)]
    fn test_matching_rulesets_post_tool_use() {
        let (_temp, strategy) = create_test_strategy();

        let input = serde_json::json!({
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "test.ts"}
        });

        let ctx = crate::validator::MatchContext::from_json(HookType::PostToolUse, &input);
        let matching = strategy.validator_loader().matching_rulesets(&ctx);

        // security-rules should match PostToolUse + Write + *.ts
        let names: Vec<_> = matching.iter().map(|rs| rs.name()).collect();
        assert!(names.contains(&"security-rules"), "security-rules should match PostToolUse + Write + source files");
    }
}
