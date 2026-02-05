//! Chain link for executing validators.
//!
//! This link executes validators matching the hook type and blocks the chain
//! if any validator fails with error severity.
//!
//! The link produces agent-agnostic output containing validator block info.
//! Agent strategies (e.g., ClaudeCodeHookStrategy) transform this into
//! their platform-specific format.

use std::sync::Arc;

use async_trait::async_trait;

use crate::chain::output::LinkOutput;
use crate::chain::{ChainContext, ChainLink, ChainResult, VALIDATOR_BLOCK_EXIT_CODE};
use crate::context::AvpContext;
use crate::turn::TurnStateManager;
use crate::types::HookType;
use crate::validator::{MatchContext, ValidatorLoader};

/// Chain link that executes validators for a given hook type.
///
/// The link:
/// 1. Finds validators matching the hook type and tool name
/// 2. Executes them via the AvpContext
/// 3. Blocks the chain if any validator fails with error severity
///
/// For Stop hooks, it automatically loads changed files from turn state
/// and passes them to the validators.
pub struct ValidatorExecutorLink<I> {
    /// AVP context for validator execution
    context: Arc<AvpContext>,
    /// Validator loader
    loader: Arc<ValidatorLoader>,
    /// Turn state manager (for loading changed files)
    turn_state: Arc<TurnStateManager>,
    /// Phantom data for input type
    _phantom: std::marker::PhantomData<I>,
}

impl<I> ValidatorExecutorLink<I> {
    /// Create a new validator executor link.
    pub fn new(
        context: Arc<AvpContext>,
        loader: Arc<ValidatorLoader>,
        turn_state: Arc<TurnStateManager>,
    ) -> Self {
        Self {
            context,
            loader,
            turn_state,
            _phantom: std::marker::PhantomData,
        }
    }
}

impl<I: ValidatorMatchInfo> ValidatorExecutorLink<I> {
    /// Load changed files for Stop hooks from turn state.
    fn load_changed_files_for_stop(&self, input: &I) -> Option<Vec<String>> {
        if input.hook_type() != HookType::Stop {
            return None;
        }
        match self.turn_state.load(input.session_id()) {
            Ok(state) if !state.changed.is_empty() => Some(
                state
                    .changed
                    .iter()
                    .map(|p| p.display().to_string())
                    .collect(),
            ),
            _ => None,
        }
    }

    /// Handle RuleSet results, returning appropriate ChainResult.
    ///
    /// This produces agent-agnostic output with validator block info.
    /// Agent strategies transform this into their platform-specific format.
    fn handle_ruleset_results(
        &self,
        results: &[crate::validator::ExecutedRuleSet],
        hook_type: HookType,
        ctx: &mut ChainContext,
    ) -> ChainResult {
        // Find the first blocking failure across all RuleSets
        for ruleset_result in results {
            if let Some(blocking) = ruleset_result.blocking_failures().first() {
                let full_name = format!("{}:{}", ruleset_result.ruleset_name, blocking.rule_name);
                tracing::info!(
                    "ValidatorExecutorLink: Rule '{}' blocked chain for {:?}",
                    full_name,
                    hook_type
                );

                // Set exit code for hooks that use stderr-only format
                let uses_stderr_only = matches!(
                    hook_type,
                    HookType::SessionStart
                        | HookType::SessionEnd
                        | HookType::Notification
                        | HookType::SubagentStart
                        | HookType::PreCompact
                        | HookType::Setup
                );
                if uses_stderr_only {
                    ctx.set_exit_code(VALIDATOR_BLOCK_EXIT_CODE);
                }

                // Return agent-agnostic validator block info
                return ChainResult::stop(LinkOutput::from_validator_block(
                    &full_name,
                    blocking.message(),
                    hook_type,
                ));
            }
        }

        ChainResult::continue_empty()
    }
}

/// Build a MatchContext from input implementing ValidatorMatchInfo.
fn build_match_context<I: ValidatorMatchInfo>(input: &I) -> MatchContext {
    let mut ctx = MatchContext::new(input.hook_type());
    if let Some(tool) = input.tool_name() {
        ctx = ctx.with_tool(tool);
    }
    if let Some(file) = input.file_path() {
        ctx = ctx.with_file(file);
    }
    ctx
}

/// Trait for extracting match context info from input types.
pub trait ValidatorMatchInfo {
    /// Get the hook type for this input.
    fn hook_type(&self) -> HookType;

    /// Get the optional tool name for this input.
    fn tool_name(&self) -> Option<&str>;

    /// Get the optional file path for this input (from tool_input).
    fn file_path(&self) -> Option<&str>;

    /// Get the session ID for this input.
    fn session_id(&self) -> &str;
}

/// Macro to implement ValidatorMatchInfo for input types.
/// Reduces boilerplate for the 13 input types that need this trait.
macro_rules! impl_validator_match_info {
    // For types with tool_name and tool_input fields
    ($type:ty, $hook:ident, with_tool) => {
        impl ValidatorMatchInfo for $type {
            fn hook_type(&self) -> HookType {
                HookType::$hook
            }
            fn tool_name(&self) -> Option<&str> {
                Some(&self.tool_name)
            }
            fn file_path(&self) -> Option<&str> {
                self.tool_input.get("file_path").and_then(|v| v.as_str())
            }
            fn session_id(&self) -> &str {
                &self.common.session_id
            }
        }
    };
    // For types without tool fields
    ($type:ty, $hook:ident) => {
        impl ValidatorMatchInfo for $type {
            fn hook_type(&self) -> HookType {
                HookType::$hook
            }
            fn tool_name(&self) -> Option<&str> {
                None
            }
            fn file_path(&self) -> Option<&str> {
                None
            }
            fn session_id(&self) -> &str {
                &self.common.session_id
            }
        }
    };
}

// Types with tool_name and tool_input
impl_validator_match_info!(crate::types::PreToolUseInput, PreToolUse, with_tool);
impl_validator_match_info!(crate::types::PostToolUseInput, PostToolUse, with_tool);
impl_validator_match_info!(
    crate::types::PostToolUseFailureInput,
    PostToolUseFailure,
    with_tool
);
impl_validator_match_info!(
    crate::types::PermissionRequestInput,
    PermissionRequest,
    with_tool
);

// Types without tool fields
impl_validator_match_info!(crate::types::StopInput, Stop);
impl_validator_match_info!(crate::types::SessionStartInput, SessionStart);
impl_validator_match_info!(crate::types::SessionEndInput, SessionEnd);
impl_validator_match_info!(crate::types::UserPromptSubmitInput, UserPromptSubmit);
impl_validator_match_info!(crate::types::NotificationInput, Notification);
impl_validator_match_info!(crate::types::SubagentStartInput, SubagentStart);
impl_validator_match_info!(crate::types::SubagentStopInput, SubagentStop);
impl_validator_match_info!(crate::types::PreCompactInput, PreCompact);
impl_validator_match_info!(crate::types::SetupInput, Setup);

#[async_trait(?Send)]
impl<I> ChainLink<I> for ValidatorExecutorLink<I>
where
    I: crate::chain::link::HookInputType + ValidatorMatchInfo + serde::Serialize,
{
    async fn process(&self, input: &I, ctx: &mut ChainContext) -> ChainResult {
        let hook_type = input.hook_type();
        let match_ctx = build_match_context(input);

        // Use new RuleSet architecture
        let rulesets = self.loader.matching_rulesets(&match_ctx);

        if rulesets.is_empty() {
            tracing::trace!("ValidatorExecutorLink: No RuleSets for {:?}", hook_type);
            return ChainResult::continue_empty();
        }

        tracing::debug!(
            "ValidatorExecutorLink: Executing {} RuleSets for {:?}",
            rulesets.len(),
            hook_type
        );

        let changed_files = self.load_changed_files_for_stop(input);
        let input_json = match serde_json::to_value(input) {
            Ok(json) => json,
            Err(e) => {
                tracing::warn!("ValidatorExecutorLink: Failed to serialize input: {}", e);
                return ChainResult::continue_empty();
            }
        };

        let results = self
            .context
            .execute_rulesets(
                &rulesets,
                hook_type,
                &input_json,
                changed_files.as_deref(),
            )
            .await;

        self.handle_ruleset_results(&results, hook_type, ctx)
    }

    fn name(&self) -> &'static str {
        "ValidatorExecutor"
    }

    fn can_short_circuit(&self) -> bool {
        true
    }
}

/// Load changed files for a session from turn state.
///
/// This utility function loads the list of files that changed during
/// the current turn and converts their paths to strings for use in
/// validator prompts (particularly Stop hook validators).
///
/// # Arguments
///
/// * `turn_state` - The turn state manager tracking file changes
/// * `session_id` - The session ID to load changed files for
///
/// # Returns
///
/// A vector of file path strings. Returns an empty vector if the session
/// has no recorded changes or if loading fails.
pub fn load_changed_files_as_strings(
    turn_state: &TurnStateManager,
    session_id: &str,
) -> Vec<String> {
    match turn_state.load(session_id) {
        Ok(state) => state
            .changed
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
        Err(e) => {
            tracing::warn!(
                "Failed to load changed files for session {}: {}",
                session_id,
                e
            );
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validator_match_info_pre_tool_use() {
        let input: crate::types::PreToolUseInput = serde_json::from_value(serde_json::json!({
            "session_id": "test-session",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "PreToolUse",
            "tool_name": "Bash",
            "tool_input": {"command": "ls", "file_path": "/some/file.txt"}
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::PreToolUse);
        assert_eq!(input.tool_name(), Some("Bash"));
        assert_eq!(input.file_path(), Some("/some/file.txt"));
        assert_eq!(input.session_id(), "test-session");
    }

    #[test]
    fn test_validator_match_info_stop() {
        let input: crate::types::StopInput = serde_json::from_value(serde_json::json!({
            "session_id": "test-session",
            "transcript_path": "/path",
            "cwd": "/home",
            "permission_mode": "default",
            "hook_event_name": "Stop",
            "stop_hook_active": true
        }))
        .unwrap();

        assert_eq!(input.hook_type(), HookType::Stop);
        assert_eq!(input.tool_name(), None);
        assert_eq!(input.file_path(), None);
        assert_eq!(input.session_id(), "test-session");
    }

    #[test]
    fn test_load_changed_files_as_strings() {
        use tempfile::TempDir;

        let temp = TempDir::new().unwrap();
        let turn_state = TurnStateManager::new(temp.path());

        // Clear any existing state first
        turn_state.clear("test-session").ok();

        // Initially empty (no state file exists)
        let files = load_changed_files_as_strings(&turn_state, "test-session");
        assert!(files.is_empty(), "Should be empty initially");

        // Add some changed files
        let mut state = crate::turn::TurnState::new();
        state.changed.push(std::path::PathBuf::from("/src/main.rs"));
        state.changed.push(std::path::PathBuf::from("/src/lib.rs"));
        turn_state.save("test-session", &state).unwrap();

        // Should return file paths as strings
        let files = load_changed_files_as_strings(&turn_state, "test-session");
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"/src/main.rs".to_string()));
        assert!(files.contains(&"/src/lib.rs".to_string()));

        // Clear and verify empty again
        turn_state.clear("test-session").unwrap();
        let files = load_changed_files_as_strings(&turn_state, "test-session");
        assert!(files.is_empty(), "Should be empty after clear");
    }
}
