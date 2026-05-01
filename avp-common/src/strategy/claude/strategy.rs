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
    ConfigChangeInput, ElicitationInput, ElicitationResultInput, InstructionsLoadedInput,
    NotificationInput, PermissionRequestInput, PostCompactInput, PostToolUseFailureInput,
    PostToolUseInput, PreCompactInput, PreToolUseInput, SessionEndInput, SessionStartInput,
    SetupInput, StopInput, SubagentStartInput, SubagentStopInput, TaskCompletedInput,
    TeammateIdleInput, UserPromptSubmitInput, WorktreeCreateInput, WorktreeRemoveInput,
};
use super::output::{
    HookOutput, HookSpecificOutput, PermissionBehavior, PermissionDecision,
    PermissionRequestDecision, PermissionRequestOutput, PreToolUseOutput,
};
use crate::validator::ValidatorLoader;

use crate::strategy::traits::AgentHookStrategy;

/// Dispatch a hook event to the appropriate chain.
///
/// Deserializes the input JSON to the typed input, picks the chain (from the
/// factory for validator-enabled hooks, or `Chain::success()` for pass-through
/// hooks), executes, and maps the error. Kept as a macro so `rustfmt` doesn't
/// expand 22 one-liner arms into 88 lines.
macro_rules! dispatch_hook {
    ($self:expr, $hook_type:expr, $input:expr) => {
        match $hook_type {
            HookType::SessionStart => run_chain!(
                SessionStartInput,
                $self.chain_factory.session_start_chain(),
                $input
            ),
            HookType::SessionEnd => run_chain!(
                SessionEndInput,
                $self.chain_factory.session_end_chain(),
                $input
            ),
            HookType::PreToolUse => run_chain!(
                PreToolUseInput,
                $self.chain_factory.pre_tool_use_chain(),
                $input
            ),
            HookType::PostToolUse => run_chain!(
                PostToolUseInput,
                $self.chain_factory.post_tool_use_chain(),
                $input
            ),
            HookType::Stop => run_chain!(StopInput, $self.chain_factory.stop_chain(), $input),
            HookType::Elicitation => run_chain!(
                ElicitationInput,
                $self.chain_factory.elicitation_chain(),
                $input
            ),
            HookType::ElicitationResult => run_chain!(
                ElicitationResultInput,
                $self.chain_factory.elicitation_result_chain(),
                $input
            ),
            HookType::ConfigChange => run_chain!(
                ConfigChangeInput,
                $self.chain_factory.config_change_chain(),
                $input
            ),
            HookType::WorktreeCreate => run_chain!(
                WorktreeCreateInput,
                $self.chain_factory.worktree_create_chain(),
                $input
            ),
            HookType::TeammateIdle => run_chain!(
                TeammateIdleInput,
                $self.chain_factory.teammate_idle_chain(),
                $input
            ),
            HookType::TaskCompleted => run_chain!(
                TaskCompletedInput,
                $self.chain_factory.task_completed_chain(),
                $input
            ),
            HookType::UserPromptSubmit => {
                run_chain!(UserPromptSubmitInput, Chain::success(), $input)
            }
            HookType::PermissionRequest => {
                run_chain!(PermissionRequestInput, Chain::success(), $input)
            }
            HookType::PostToolUseFailure => {
                run_chain!(PostToolUseFailureInput, Chain::success(), $input)
            }
            HookType::SubagentStart => run_chain!(SubagentStartInput, Chain::success(), $input),
            HookType::SubagentStop => run_chain!(SubagentStopInput, Chain::success(), $input),
            HookType::PreCompact => run_chain!(PreCompactInput, Chain::success(), $input),
            HookType::PostCompact => run_chain!(PostCompactInput, Chain::success(), $input),
            HookType::Setup => run_chain!(SetupInput, Chain::success(), $input),
            HookType::Notification => run_chain!(NotificationInput, Chain::success(), $input),
            HookType::InstructionsLoaded => {
                run_chain!(InstructionsLoadedInput, Chain::success(), $input)
            }
            HookType::WorktreeRemove => run_chain!(WorktreeRemoveInput, Chain::success(), $input),
        }
    };
}

/// Deserialize JSON to a typed input, execute a chain, and map the error.
macro_rules! run_chain {
    ($type:ty, $chain:expr, $input:expr) => {{
        let typed: $type = serde_json::from_value($input).map_err(AvpError::Json)?;
        $chain.execute(&typed).await.map_err(AvpError::Chain)
    }};
}

/// Exit code that tells Claude Code to treat output as stderr (non-JSON block).
const EXIT_CODE_STDERR: i32 = 2;

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

    /// Route a hook event to its chain. Deserializes input and executes the chain.
    async fn route_to_chain(
        &self,
        hook_type: HookType,
        input: serde_json::Value,
    ) -> Result<(ChainOutput, i32), AvpError> {
        // Each arm: deserialize → pick chain → execute → map error.
        // rustfmt expands these; the logic per arm is one line via the macro.
        dispatch_hook!(self, hook_type, input)
    }

    /// Log the result of a chain execution.
    fn log_chain_result(
        &self,
        hook_type: HookType,
        tool_name: &Option<String>,
        prompt_len: &Option<usize>,
        chain_result: &Result<(ChainOutput, i32), AvpError>,
    ) {
        let hook_type_str = format!("{}", hook_type);
        match chain_result {
            Ok((chain_output, _)) => {
                let details =
                    self.extract_log_details(hook_type, tool_name, prompt_len, chain_output);
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
    }

    /// Transform a chain output to a Claude-specific hook output.
    fn transform_to_claude_output(
        chain_output: ChainOutput,
        hook_type: HookType,
    ) -> (HookOutput, i32) {
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
        Self::transform_block_output(chain_output, hook_type)
    }

    /// Build the block output for a validator that blocked the chain.
    fn transform_block_output(chain_output: ChainOutput, hook_type: HookType) -> (HookOutput, i32) {
        let reason = Self::extract_block_reason(&chain_output);
        let base = HookOutput {
            continue_execution: true,
            system_message: chain_output.system_message,
            suppress_output: chain_output.suppress_output,
            ..Default::default()
        };

        match hook_type {
            HookType::PreToolUse => Self::block_pre_tool_use(base, reason),
            HookType::PermissionRequest => Self::block_permission_request(base, reason),
            HookType::PostToolUse | HookType::PostToolUseFailure => {
                Self::block_with_decision(base, reason)
            }
            HookType::Stop | HookType::SubagentStop => Self::block_stop(base, reason),
            HookType::UserPromptSubmit => Self::block_with_decision(base, reason),
            _ => (
                HookOutput {
                    continue_execution: false,
                    stop_reason: Some(reason),
                    ..base
                },
                EXIT_CODE_STDERR, // non-zero tells Claude Code to treat output as stderr
            ),
        }
    }

    /// Extract a human-readable block reason from chain output.
    fn extract_block_reason(chain_output: &ChainOutput) -> String {
        let (name, message) = match &chain_output.validator_block {
            Some(block) => (block.validator_name.as_str(), block.message.as_str()),
            None => (
                "unknown",
                chain_output
                    .stop_reason
                    .as_deref()
                    .unwrap_or("Unknown reason"),
            ),
        };
        format!("blocked by validator '{}': {}", name, message)
    }

    fn block_pre_tool_use(base: HookOutput, reason: String) -> (HookOutput, i32) {
        (
            HookOutput {
                hook_specific_output: Some(HookSpecificOutput::PreToolUse(PreToolUseOutput {
                    permission_decision: Some(PermissionDecision::Deny),
                    permission_decision_reason: Some(reason),
                    ..Default::default()
                })),
                ..base
            },
            0,
        )
    }

    fn block_permission_request(base: HookOutput, reason: String) -> (HookOutput, i32) {
        (
            HookOutput {
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
                ..base
            },
            0,
        )
    }

    fn block_with_decision(base: HookOutput, reason: String) -> (HookOutput, i32) {
        (
            HookOutput {
                decision: Some("block".to_string()),
                reason: Some(reason),
                ..base
            },
            0,
        )
    }

    /// Clean up turn diffs, state, and pre-content after an allowed Stop.
    ///
    /// Only cleans when the hook is Stop AND the chain result is allowed
    /// (no validator blocked). When Stop is blocked, diffs must survive
    /// for the next Stop iteration.
    fn maybe_cleanup_turn_state(
        &self,
        hook_type: HookType,
        session_id: &str,
        chain_output: &ChainOutput,
    ) {
        if hook_type != HookType::Stop {
            return;
        }
        if chain_output.validator_block.is_some() || !chain_output.continue_execution {
            return;
        }

        self.record_allowed_stop_baseline(session_id);
        self.clear_allowed_stop_sidecars(session_id);
        tracing::debug!(session_id, "Cleaned turn state after allowed Stop");
    }

    /// Snapshot the currently-changed paths into `last_stop_shas` for
    /// `session_id`.
    ///
    /// `record_stop_baseline` replaces `last_stop_shas`, empties
    /// `pending`/`changed`, and saves in one shot — no separate `clear()`
    /// call (which would delete the YAML and wipe the baseline). The
    /// baseline lets the next Stop validator run skip files that haven't
    /// been touched since this allowed Stop, avoiding the cumulative-diff
    /// re-validation loop.
    fn record_allowed_stop_baseline(&self, session_id: &str) {
        let turn_state = self.context.turn_state();
        let changed_paths = match turn_state.load(session_id) {
            Ok(state) => state.changed,
            Err(e) => {
                tracing::warn!(
                    "Failed to load turn state for baseline on allowed Stop: {}",
                    e
                );
                Vec::new()
            }
        };
        if let Err(e) = turn_state.record_stop_baseline(session_id, &changed_paths) {
            tracing::warn!("Failed to record allowed-Stop SHA baseline: {}", e);
        }
    }

    /// Clear sidecar diff and pre-content directories for `session_id`.
    ///
    /// The baseline write in [`record_allowed_stop_baseline`] is the
    /// canonical "Stop is over" signal; the sidecars are derived data
    /// that must not survive into the next turn.
    fn clear_allowed_stop_sidecars(&self, session_id: &str) {
        let turn_state = self.context.turn_state();
        if let Err(e) = turn_state.clear_diffs(session_id) {
            tracing::warn!("Failed to clear diffs on allowed Stop: {}", e);
        }
        if let Err(e) = turn_state.clear_pre_content(session_id) {
            tracing::warn!("Failed to clear pre-content on allowed Stop: {}", e);
        }
    }

    fn block_stop(base: HookOutput, reason: String) -> (HookOutput, i32) {
        (
            HookOutput {
                stop_reason: Some(reason.clone()),
                decision: Some("block".to_string()),
                reason: Some(reason),
                ..base
            },
            0,
        )
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
        let tool_name = input
            .get("tool_name")
            .and_then(|v| v.as_str())
            .map(String::from);
        let prompt_len = input.get("prompt").and_then(|v| v.as_str()).map(str::len);
        let session_id = input
            .get("session_id")
            .and_then(|v| v.as_str())
            .map(String::from);

        let chain_result = self.route_to_chain(hook_type, input).await;
        self.log_chain_result(hook_type, &tool_name, &prompt_len, &chain_result);

        // Clean up turn diffs/state after an allowed Stop
        if let (Ok((ref chain_output, _)), Some(ref sid)) = (&chain_result, &session_id) {
            self.maybe_cleanup_turn_state(hook_type, sid, chain_output);
        }

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
        // (security-rules, code-quality, test-integrity)
        assert!(
            strategy.validator_loader().ruleset_count() >= 3,
            "Should have at least 3 builtin RuleSets, got {}",
            strategy.validator_loader().ruleset_count()
        );
        assert!(
            strategy
                .validator_loader()
                .get_ruleset("security-rules")
                .is_some(),
            "Should have security-rules"
        );
    }

    /// Test that all 6 new blockable hook types route to a validator chain
    /// (i.e., they are processed without panicking and return success when no
    /// validators block them).
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_new_blockable_hooks_process_via_validator_chain() {
        let (_temp, strategy) = create_test_strategy();

        let blockable_hooks = vec![
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "Elicitation",
                "mcp_server_name": "test-server",
                "message": "Choose an option"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "ElicitationResult",
                "mcp_server_name": "test-server",
                "action": "submit"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "ConfigChange",
                "source": "user_settings"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "WorktreeCreate",
                "worktree_path": "/tmp/worktree",
                "branch_name": "feature/test"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "TeammateIdle",
                "teammate_id": "agent-1"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "TaskCompleted",
                "task_id": "task-1",
                "task_title": "Implement feature"
            }),
        ];

        for input in blockable_hooks {
            let hook_name = input["hook_event_name"].as_str().unwrap().to_string();
            let (output, exit_code) = strategy
                .process(input)
                .await
                .unwrap_or_else(|e| panic!("Hook {} failed: {}", hook_name, e));
            assert!(
                output.continue_execution,
                "Hook {} should allow by default (no blocking validators)",
                hook_name
            );
            assert_eq!(
                exit_code, 0,
                "Hook {} should return exit 0 by default",
                hook_name
            );
        }
    }

    /// Test that observe-only hook types process correctly via Chain::success().
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_observe_only_hooks_always_succeed() {
        let (_temp, strategy) = create_test_strategy();

        let observe_only_hooks = vec![
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "InstructionsLoaded",
                "file_path": "/project/CLAUDE.md"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "WorktreeRemove",
                "worktree_path": "/tmp/old-worktree"
            }),
            serde_json::json!({
                "session_id": "test",
                "transcript_path": "/path",
                "cwd": "/home",
                "permission_mode": "default",
                "hook_event_name": "PostCompact"
            }),
        ];

        for input in observe_only_hooks {
            let hook_name = input["hook_event_name"].as_str().unwrap().to_string();
            let (output, exit_code) = strategy
                .process(input)
                .await
                .unwrap_or_else(|e| panic!("Hook {} failed: {}", hook_name, e));
            assert!(
                output.continue_execution,
                "Observe-only hook {} should always succeed",
                hook_name
            );
            assert_eq!(
                exit_code, 0,
                "Observe-only hook {} should return exit 0",
                hook_name
            );
        }
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
        assert!(
            names.contains(&"security-rules"),
            "security-rules should match PostToolUse + Write + source files"
        );
    }

    /// After an allowed Stop (no validator blocks), turn diffs, state, and
    /// pre-content should be cleaned for the session.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_allowed_stop_cleans_turn_state() {
        let (_temp, strategy) = create_test_strategy();
        let session_id = "stop-clean-test";
        let turn_state = strategy.context.turn_state();

        // Seed some state so we can verify cleanup
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/tmp/foo.rs"));
        turn_state.save(session_id, &state).unwrap();
        turn_state
            .write_diff(session_id, std::path::Path::new("/tmp/foo.rs"), "some diff")
            .unwrap();
        turn_state
            .write_pre_content(
                session_id,
                "tool-1",
                std::path::Path::new("/tmp/foo.rs"),
                Some(b"old content"),
            )
            .unwrap();

        // Verify state exists before Stop
        assert!(turn_state.load(session_id).unwrap().has_changes());
        assert!(!turn_state.load_all_diffs(session_id).is_empty());

        // Process a Stop hook (no blocking validators → allowed)
        let input = serde_json::json!({
            "session_id": session_id,
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "Stop",
            "stop_hook_active": true,
        });
        let (output, _exit_code) = strategy.process(input).await.unwrap();

        // Stop was allowed (no blocking validators)
        assert!(output.continue_execution);

        // After allowed Stop, turn state should be cleaned
        let loaded = turn_state.load(session_id).unwrap();
        assert!(
            !loaded.has_changes(),
            "Turn state should be cleared after allowed Stop"
        );
        assert!(
            turn_state.load_all_diffs(session_id).is_empty(),
            "Diffs should be cleared after allowed Stop"
        );
    }

    /// Non-Stop hooks (e.g. PostToolUse) must NOT clean turn state.
    /// This proves the cleanup is conditional on the Stop hook type.
    #[tokio::test]
    #[serial_test::serial(cwd)]
    async fn test_non_stop_hook_preserves_turn_state() {
        let (_temp, strategy) = create_test_strategy();
        let session_id = "non-stop-preserve";
        let turn_state = strategy.context.turn_state();

        // Seed state
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/tmp/bar.rs"));
        turn_state.save(session_id, &state).unwrap();
        turn_state
            .write_diff(session_id, std::path::Path::new("/tmp/bar.rs"), "bar diff")
            .unwrap();

        // Process a non-Stop hook (PostToolUse)
        let input = serde_json::json!({
            "session_id": session_id,
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PostToolUse",
            "tool_name": "Write",
            "tool_input": {"file_path": "/tmp/bar.rs"},
            "tool_use_id": "tool-1"
        });
        let (_output, _exit_code) = strategy.process(input).await.unwrap();

        // After non-Stop hook, state must survive
        let loaded = turn_state.load(session_id).unwrap();
        assert!(
            loaded.has_changes(),
            "Turn state should survive after non-Stop hooks"
        );
        assert!(
            !turn_state.load_all_diffs(session_id).is_empty(),
            "Diffs should survive after non-Stop hooks"
        );
    }

    /// The cleanup helper should be a no-op when the chain output indicates a block.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_cleanup_skipped_when_blocked() {
        let (_temp, strategy) = create_test_strategy();
        let session_id = "cleanup-blocked";
        let turn_state = strategy.context.turn_state();

        // Seed state
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/tmp/baz.rs"));
        turn_state.save(session_id, &state).unwrap();
        turn_state
            .write_diff(session_id, std::path::Path::new("/tmp/baz.rs"), "baz diff")
            .unwrap();

        // Simulate a blocked chain output
        let blocked_output = ChainOutput {
            continue_execution: true,
            validator_block: Some(crate::chain::ValidatorBlockInfo {
                validator_name: "test-blocker".to_string(),
                message: "blocked for test".to_string(),
                hook_type: HookType::Stop,
            }),
            ..Default::default()
        };

        // Call the cleanup helper directly -- should NOT clean when blocked
        strategy.maybe_cleanup_turn_state(HookType::Stop, session_id, &blocked_output);

        // State should survive
        let loaded = turn_state.load(session_id).unwrap();
        assert!(
            loaded.has_changes(),
            "Turn state should survive when Stop is blocked"
        );
        assert!(
            !turn_state.load_all_diffs(session_id).is_empty(),
            "Diffs should survive when Stop is blocked"
        );
    }

    // ========================================================================
    // Block-render contract for Stop hooks (kanban 01KQ7M20F27D0Z67H9XX0XQ4QZ).
    //
    // When the chain produces a Stop block (validator_block set,
    // continue_execution=false), the strategy must render it as the JSON
    // shape claude-code parses on stdout:
    //   {"continue": true, "decision": "block", "reason": "...", "stopReason": "..."}
    // with strategy exit code 0 — Stop blocks are surfaced via stdout JSON,
    // not exit-2 stderr, see `block_stop`.
    //
    // This test calls `transform_to_claude_output` directly with a hand-built
    // `ChainOutput` so the assertion exercises the rendering in isolation —
    // no `PlaybackAgent`, no fixture timing. The chain-level half (chain
    // produces validator_block + exit code 2) lives in
    // `avp-common/tests/validator_block_e2e_integration.rs`.
    // ========================================================================

    /// A blocked Stop chain output renders as claude-code's stop-block JSON
    /// (`continue: true`, `decision: "block"`, `reason` and `stopReason` set
    /// to the same string) with exit code 0.
    #[test]
    fn block_stop_renders_claude_parseable_json() {
        use crate::chain::ValidatorBlockInfo;

        let chain_output = ChainOutput {
            continue_execution: false,
            stop_reason: Some(
                "Found magic number 8675309 on line 12 of src/replay-fixture-target.rs".to_string(),
            ),
            validator_block: Some(ValidatorBlockInfo {
                validator_name: "code-quality:no-magic-numbers".to_string(),
                message: "Found magic number 8675309 on line 12 of src/replay-fixture-target.rs"
                    .to_string(),
                hook_type: HookType::Stop,
            }),
            ..Default::default()
        };

        let (hook_output, exit_code) =
            ClaudeCodeHookStrategy::transform_to_claude_output(chain_output, HookType::Stop);

        // Stop blocks are surfaced via stdout JSON, not exit-2 stderr.
        assert_eq!(
            exit_code, 0,
            "Stop block strategy exit code is 0 — block is surfaced via stdout JSON",
        );

        // continue=true (claude cannot stop the assistant), decision=block,
        // reason and stopReason both carry the formatted message.
        assert!(
            hook_output.continue_execution,
            "Stop block must keep continue=true (claude cannot stop the assistant)"
        );
        assert_eq!(
            hook_output.decision.as_deref(),
            Some("block"),
            "Stop block must set decision=\"block\" so claude-code surfaces the failure"
        );

        let reason = hook_output
            .reason
            .as_deref()
            .expect("Stop block reason must be set");
        assert!(
            reason.contains("8675309"),
            "reason must contain the validator's finding, got: {}",
            reason
        );
        assert!(
            reason.contains("code-quality:no-magic-numbers"),
            "reason must include the qualified rule name so the user knows \
             which rule blocked, got: {}",
            reason
        );

        let stop_reason = hook_output
            .stop_reason
            .as_deref()
            .expect("Stop block stopReason must be set");
        assert_eq!(
            stop_reason, reason,
            "block_stop() sets stopReason and reason to the same value"
        );

        // Round-trip: serialize to JSON the way the CLI does. Claude-code
        // parses exactly that string from stdout, so the keys must use
        // camelCase and the values must round-trip without dropping the
        // validator info.
        let json = serde_json::to_value(&hook_output).expect("serialize HookOutput");
        assert_eq!(
            json.get("continue").and_then(|v| v.as_bool()),
            Some(true),
            "JSON `continue` must be true"
        );
        assert_eq!(
            json.get("decision").and_then(|v| v.as_str()),
            Some("block"),
            "JSON `decision` must be \"block\""
        );
        let json_reason = json
            .get("reason")
            .and_then(|v| v.as_str())
            .expect("JSON must have `reason` string");
        assert!(
            json_reason.contains("8675309") && json_reason.contains("no-magic-numbers"),
            "JSON `reason` must contain both the finding and the rule name, got: {}",
            json_reason
        );
        let json_stop_reason = json
            .get("stopReason")
            .and_then(|v| v.as_str())
            .expect("JSON must have `stopReason` string");
        assert_eq!(
            json_stop_reason, json_reason,
            "JSON `stopReason` mirrors `reason`"
        );
    }

    /// On an allowed Stop with two changed files, both files' SHA-256s
    /// must be written into `last_stop_shas` before the rest of the state
    /// is cleared. This is the baseline that the next Stop's validator
    /// run will diff against.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_allowed_stop_records_baseline_for_changed_files() {
        let (temp, strategy) = create_test_strategy();
        let session_id = "baseline-records";
        let turn_state = strategy.context.turn_state();

        // Two real files in the temp dir
        let path_a = temp.path().join("alpha.rs");
        let path_b = temp.path().join("beta.rs");
        std::fs::write(&path_a, b"alpha contents").unwrap();
        std::fs::write(&path_b, b"beta contents").unwrap();

        // Seed turn state: both files in `changed`
        let mut state = crate::turn::TurnState::new();
        state.changed.push(path_a.clone());
        state.changed.push(path_b.clone());
        turn_state.save(session_id, &state).unwrap();

        // Simulate allowed Stop
        let allowed_output = ChainOutput {
            continue_execution: true,
            validator_block: None,
            ..Default::default()
        };

        strategy.maybe_cleanup_turn_state(HookType::Stop, session_id, &allowed_output);

        // Baseline must be present and contain both files
        let loaded = turn_state.load(session_id).unwrap();
        assert_eq!(
            loaded.last_stop_shas.len(),
            2,
            "allowed Stop must record one SHA per changed file"
        );
        assert!(loaded.last_stop_shas.contains_key(&path_a));
        assert!(loaded.last_stop_shas.contains_key(&path_b));
        // pending/changed must be empty (record_stop_baseline empties them)
        assert!(loaded.changed.is_empty());
        assert!(loaded.pending.is_empty());
    }

    /// On a blocked Stop the cleanup early-returns, so `last_stop_shas`
    /// must remain untouched (no baseline recorded mid-iteration).
    #[test]
    #[serial_test::serial(cwd)]
    fn test_blocked_stop_records_no_baseline() {
        let (temp, strategy) = create_test_strategy();
        let session_id = "baseline-blocked";
        let turn_state = strategy.context.turn_state();

        let path_a = temp.path().join("changed.rs");
        std::fs::write(&path_a, b"x").unwrap();

        let mut state = crate::turn::TurnState::new();
        state.changed.push(path_a.clone());
        turn_state.save(session_id, &state).unwrap();

        let blocked_output = ChainOutput {
            continue_execution: true,
            validator_block: Some(crate::chain::ValidatorBlockInfo {
                validator_name: "blocker".to_string(),
                message: "blocked".to_string(),
                hook_type: HookType::Stop,
            }),
            ..Default::default()
        };

        strategy.maybe_cleanup_turn_state(HookType::Stop, session_id, &blocked_output);

        let loaded = turn_state.load(session_id).unwrap();
        assert!(
            loaded.last_stop_shas.is_empty(),
            "blocked Stop must not write a baseline"
        );
        // changed list survives so the next Stop iteration sees the same files.
        assert_eq!(loaded.changed.len(), 1);
    }

    /// The cleanup helper should clean state when the chain output is allowed.
    #[test]
    #[serial_test::serial(cwd)]
    fn test_cleanup_runs_when_allowed() {
        let (_temp, strategy) = create_test_strategy();
        let session_id = "cleanup-allowed";
        let turn_state = strategy.context.turn_state();

        // Seed state
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/tmp/qux.rs"));
        turn_state.save(session_id, &state).unwrap();
        turn_state
            .write_diff(session_id, std::path::Path::new("/tmp/qux.rs"), "qux diff")
            .unwrap();

        // Simulate an allowed chain output
        let allowed_output = ChainOutput {
            continue_execution: true,
            validator_block: None,
            ..Default::default()
        };

        // Call the cleanup helper directly -- should clean when allowed
        strategy.maybe_cleanup_turn_state(HookType::Stop, session_id, &allowed_output);

        // State should be cleaned
        let loaded = turn_state.load(session_id).unwrap();
        assert!(
            !loaded.has_changes(),
            "Turn state should be cleared after allowed Stop cleanup"
        );
        assert!(
            turn_state.load_all_diffs(session_id).is_empty(),
            "Diffs should be cleared after allowed Stop cleanup"
        );
    }
}
